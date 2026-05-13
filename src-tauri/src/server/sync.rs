use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use blake3::Hasher;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use tokio::io::AsyncReadExt;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use walkdir::WalkDir;

use crate::entity::{local_file_index, local_files};
use crate::server::share::{canonicalize_existing, relative_path_for, ROOT_RELATIVE_PATH};

pub struct SyncRuntime {
    shutdown_tx: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl SyncRuntime {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        for handle in self.tasks {
            let _ = handle.await;
        }
    }
}

#[derive(Clone)]
struct LocalShareRoot {
    id: String,
    root: PathBuf,
}

pub async fn start(db: DatabaseConnection) -> SyncRuntime {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut tasks = Vec::new();

    let scan_db = db.clone();
    let mut scan_shutdown = shutdown_rx.clone();
    tasks.push(tokio::spawn(async move {
        let _ = scan_local_shares_once(&scan_db).await;
        loop {
            tokio::select! {
                _ = scan_shutdown.changed() => {
                    if *scan_shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    let _ = scan_local_shares_once(&scan_db).await;
                }
            }
        }
    }));

    let watcher_db = db.clone();
    let mut watcher_shutdown = shutdown_rx.clone();
    tasks.push(tokio::spawn(async move {
        let _ = run_watcher_loop(watcher_db, &mut watcher_shutdown).await;
    }));

    let hash_db = db.clone();
    let mut hash_shutdown = shutdown_rx;
    tasks.push(tokio::spawn(async move {
        let _ = run_hash_worker(hash_db, &mut hash_shutdown).await;
    }));

    SyncRuntime { shutdown_tx, tasks }
}

async fn run_watcher_loop(
    db: DatabaseConnection,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    let roots = list_local_share_roots(&db).await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_)
                        | EventKind::Any
                        | EventKind::Other
                ) {
                    for p in event.paths {
                        let _ = tx.send(p);
                    }
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("watcher init failed: {e}"))?;

    for root in &roots {
        let _ = watcher.watch(&root.root, RecursiveMode::Recursive);
    }

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            maybe_path = rx.recv() => {
                if let Some(path) = maybe_path {
                    let _ = upsert_path_for_matching_shares(&db, &roots, path).await;
                }
            }
        }
    }

    Ok(())
}

async fn run_hash_worker(
    db: DatabaseConnection,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
                let job = local_file_index::Entity::find()
                    .filter(local_file_index::Column::Dirty.eq(1))
                    .filter(local_file_index::Column::IsDir.eq(0))
                    .filter(local_file_index::Column::ExistsFlag.eq(1))
                    .order_by_asc(local_file_index::Column::LastSeenAt)
                    .one(&db)
                    .await
                    .map_err(|e| format!("query local file index failed: {e}"))?;

                let Some(job) = job else {
                    continue;
                };

                let path = PathBuf::from(&job.absolute_path);
                let hash = compute_file_hash(&path).await.ok();
                let now = now_ts();

                if let Some(model) = local_file_index::Entity::find_by_id((
                    job.local_file_id,
                    job.relative_path,
                ))
                .one(&db)
                .await
                .map_err(|e| format!("query local file index failed: {e}"))? {
                    let mut am: local_file_index::ActiveModel = model.into();
                    am.dirty = Set(0);
                    am.hash = Set(hash);
                    am.last_hashed_at = Set(Some(now));
                    let _ = am.update(&db).await;
                }
            }
        }
    }

    Ok(())
}

pub async fn scan_local_shares_once(db: &DatabaseConnection) -> Result<(), String> {
    let roots = list_local_share_roots(db).await?;
    for share in roots {
        if share.root.is_file() {
            let _ = upsert_share_path(db, &share, share.root.clone()).await;
            continue;
        }
        for entry in WalkDir::new(&share.root)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            let _ = upsert_share_path(db, &share, entry.path().to_path_buf()).await;
        }
    }
    Ok(())
}

async fn upsert_path_for_matching_shares(
    db: &DatabaseConnection,
    roots: &[LocalShareRoot],
    path: PathBuf,
) -> Result<(), String> {
    let target = match canonicalize_existing(&path) {
        Ok(target) => target,
        Err(_) => {
            mark_deleted_by_path(db, &path).await?;
            return Ok(());
        }
    };
    for share in roots {
        if target == share.root || target.starts_with(&share.root) {
            let _ = upsert_share_path(db, &share, target.clone()).await;
        }
    }
    Ok(())
}

async fn upsert_share_path(
    db: &DatabaseConnection,
    share: &LocalShareRoot,
    path: PathBuf,
) -> Result<(), String> {
    let canonical = match canonicalize_existing(&path) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let metadata = match tokio::fs::metadata(&canonical).await {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let size = if metadata.is_file() {
        metadata.len() as i64
    } else {
        0
    };
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let now = now_ts();
    let relative_path = relative_path_for(&share.root, &canonical);
    let absolute_path = canonical.to_string_lossy().to_string();

    let existed = local_file_index::Entity::find_by_id((share.id.clone(), relative_path.clone()))
        .one(db)
        .await
        .map_err(|e| format!("query local_file_index failed: {e}"))?;

    if let Some(old) = existed {
        let changed = old.size != size || old.mtime != mtime || old.exists_flag != 1;
        let old_dirty = old.dirty;
        let mut am: local_file_index::ActiveModel = old.into();
        am.absolute_path = Set(absolute_path);
        am.size = Set(size);
        am.mtime = Set(mtime);
        am.is_dir = Set(if metadata.is_dir() { 1 } else { 0 });
        am.exists_flag = Set(1);
        am.last_seen_at = Set(now);
        am.dirty = Set(if changed && metadata.is_file() {
            1
        } else {
            old_dirty
        });
        let _ = am
            .update(db)
            .await
            .map_err(|e| format!("update local file index failed: {e}"))?;
    } else {
        let am = local_file_index::ActiveModel {
            local_file_id: Set(share.id.clone()),
            relative_path: Set(if relative_path.is_empty() {
                ROOT_RELATIVE_PATH.to_string()
            } else {
                relative_path
            }),
            absolute_path: Set(absolute_path),
            size: Set(size),
            mtime: Set(mtime),
            is_dir: Set(if metadata.is_dir() { 1 } else { 0 }),
            hash: Set(None),
            dirty: Set(if metadata.is_file() { 1 } else { 0 }),
            exists_flag: Set(1),
            last_seen_at: Set(now),
            last_hashed_at: Set(None),
        };
        let _ = am
            .insert(db)
            .await
            .map_err(|e| format!("insert local file index failed: {e}"))?;
    }
    Ok(())
}

async fn mark_deleted_by_path(db: &DatabaseConnection, path: &Path) -> Result<(), String> {
    let absolute = path.to_string_lossy().to_string();
    let rows = local_file_index::Entity::find()
        .filter(local_file_index::Column::AbsolutePath.eq(absolute))
        .all(db)
        .await
        .map_err(|e| format!("query local_file_index failed: {e}"))?;
    for row in rows {
        let mut am: local_file_index::ActiveModel = row.into();
        am.exists_flag = Set(0);
        am.dirty = Set(0);
        let _ = am.update(db).await;
    }
    Ok(())
}

async fn list_local_share_roots(db: &DatabaseConnection) -> Result<Vec<LocalShareRoot>, String> {
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .all(db)
        .await
        .map_err(|e| format!("query local files failed: {e}"))?;

    let mut roots = Vec::new();
    for row in rows {
        let parsed = PathBuf::from(&row.path);
        let Ok(root) = canonicalize_existing(&parsed) else {
            continue;
        };
        roots.push(LocalShareRoot { id: row.id, root });
    }
    Ok(roots)
}

pub async fn compute_file_hash(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("open file failed: {e}"))?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0_u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| format!("read file failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}
