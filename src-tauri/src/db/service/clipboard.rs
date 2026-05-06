#![allow(dead_code)]

use log::{debug, warn};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ModelTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::repository::clipboard_record;
use crate::db::DbState;
use crate::entity::clipboard_record::{ActiveModel, Entity, Model};
use crate::entity::local_files;
use crate::error::AppError;
use crate::models::clipboard::{ClipboardResponse, ClipboardType};

pub async fn list_records(
    db: &DbState,
    page: u64,
    page_size: u64,
) -> Result<Vec<ClipboardResponse>, DbErr> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;

    clipboard_record::list_latest(&db.conn, page_size, offset)
        .await
        .map_err(|e| {
            debug!("list_records failed: page={page}, page_size={page_size}, offset={offset}, error={e}");
            e
        })
}

pub async fn get_and_validate_clipboard_record(
    db: &DbState,
    id: i32,
    auto_cleanup: bool,
) -> Result<Option<Model>, AppError> {
    let record = match Entity::find_by_id(id).one(&db.conn).await.map_err(|e| {
        debug!("get_and_validate_clipboard_record query failed: id={id}, error={e}");
        AppError::from(e)
    })? {
        Some(r) => r,
        None => return Ok(None),
    };

    match record.r#type {
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let paths: Vec<String> = match &record.data {
                Some(bytes) => serde_json::from_slice(bytes).map_err(|e| {
                    debug!("get_and_validate_clipboard_record parse file list failed: id={id}, error={e}");
                    AppError::Json(e)
                })?,
                None => return handle_invalid_entry(db, record, auto_cleanup).await,
            };

            let any_missing = paths.iter().any(|p| !std::path::Path::new(p).exists());
            if any_missing {
                return handle_invalid_entry(db, record, auto_cleanup).await;
            }
        }
        t if t == ClipboardType::Image as i32 => {
            let path_str = match &record.data {
                Some(bytes) => String::from_utf8(bytes.clone()).map_err(|e| {
                    debug!("get_and_validate_clipboard_record parse image path failed: id={id}, error={e}");
                    AppError::from(e)
                })?,
                None => return handle_invalid_entry(db, record, auto_cleanup).await,
            };
            let path = std::path::Path::new(&path_str);
            if !path.exists() {
                return handle_invalid_entry(db, record, auto_cleanup).await;
            }
        }
        _ => {}
    }

    Ok(Some(record))
}

async fn handle_invalid_entry(
    db: &DbState,
    record: Model,
    auto_cleanup: bool,
) -> Result<Option<Model>, AppError> {
    let id = record.id;
    sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
    if auto_cleanup {
        record.delete(&db.conn).await.map_err(|e| {
            debug!("handle_invalid_entry delete failed: id={id}, auto_cleanup={auto_cleanup}, error={e}");
            AppError::from(e)
        })?;
    } else {
        let mut active: ActiveModel = record.into();
        active.is_valid = Set(0);
        active.update(&db.conn).await.map_err(|e| {
            debug!("handle_invalid_entry mark invalid failed: id={id}, error={e}");
            AppError::from(e)
        })?;
    }
    Ok(None)
}

pub async fn toggle_favorite(db: &DbState, id: i32) -> Result<bool, AppError> {
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await
        .map_err(|e| {
            debug!("toggle_favorite query failed: id={id}, error={e}");
            AppError::from(e)
        })?
        .ok_or(AppError::NotFound)?;

    let new_favorite = if record.is_favorite == 1 { 0 } else { 1 };

    let mut active: ActiveModel = record.into();
    active.is_favorite = Set(new_favorite);
    active.update(&db.conn).await.map_err(|e| {
        debug!("toggle_favorite update failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    Ok(new_favorite == 1)
}

pub async fn toggle_share(db: &DbState, id: i32) -> Result<bool, AppError> {
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await
        .map_err(|e| {
            debug!("toggle_share query failed: id={id}, error={e}");
            AppError::from(e)
        })?
        .ok_or(AppError::NotFound)?;

    let new_share = if record.is_shared == 1 { 0 } else { 1 };

    let mut active: ActiveModel = record.into();
    active.is_shared = Set(new_share);
    let updated = active.update(&db.conn).await.map_err(|e| {
        debug!("toggle_share update failed: id={id}, error={e}");
        AppError::from(e)
    })?;
    if new_share == 1 {
        upsert_local_files_for_shared_clipboard(db, &updated).await?;
    } else {
        sync_local_files_on_clipboard_unshared_or_invalid(db, &updated).await?;
    }
    if let Err(e) = crate::server::sync::refresh_sync_roots_from_clipboard(&db.conn).await {
        debug!("toggle_share refresh sync roots failed: id={id}, error={e}");
    }

    Ok(new_share == 1)
}

pub async fn delete_item(db: &DbState, id: i32, cache_dir: &str) -> Result<(), AppError> {
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await
        .map_err(|e| {
            debug!("delete_item query failed: id={id}, error={e}");
            AppError::from(e)
        })?
        .ok_or_else(|| {
            debug!("delete_item not found: id={id}");
            AppError::NotFound
        })?;

    if record.r#type == ClipboardType::Image as i32 {
        if let Some(data) = record.data.clone() {
            let path_str = String::from_utf8(data).unwrap_or_default();
            let path = std::path::Path::new(&path_str);
            if path.exists() && path.starts_with(cache_dir) {
                if let Err(e) = std::fs::remove_file(path) {
                    warn!(
                        "delete_item remove cache file failed: id={id}, path={}, error={e}",
                        path.display()
                    );
                }
            }
        }
    }

    sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
    record.delete(&db.conn).await.map_err(|e| {
        debug!("delete_item delete record failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    Ok(())
}

fn extract_paths_from_clipboard_record(record: &Model) -> Result<Vec<String>, AppError> {
    if record.r#type == ClipboardType::File as i32 || record.r#type == ClipboardType::Folder as i32 {
        let bytes = record
            .data
            .clone()
            .ok_or_else(|| AppError::InvalidInput("clipboard file data is empty".to_string()))?;
        let paths: Vec<String> = serde_json::from_slice(&bytes).map_err(AppError::from)?;
        return Ok(paths);
    }
    if record.r#type == ClipboardType::Image as i32 {
        let bytes = record
            .data
            .clone()
            .ok_or_else(|| AppError::InvalidInput("clipboard image data is empty".to_string()))?;
        let path = String::from_utf8(bytes).map_err(AppError::from)?;
        return Ok(vec![path]);
    }
    Ok(Vec::new())
}

fn local_file_type_from_clipboard_type(clipboard_type: i32, path: &std::path::Path) -> i32 {
    if clipboard_type == ClipboardType::Folder as i32 || path.is_dir() {
        return 1;
    }
    if clipboard_type == ClipboardType::Image as i32 {
        return 2;
    }
    0
}

fn parse_source_clipboard_ids(raw: Option<&str>) -> Vec<i32> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<i32>>(raw).unwrap_or_default()
}

async fn upsert_local_files_for_shared_clipboard(db: &DbState, record: &Model) -> Result<(), AppError> {
    let paths = extract_paths_from_clipboard_record(record)?;
    for raw_path in paths {
        let path = std::path::PathBuf::from(raw_path);
        if !path.exists() {
            continue;
        }
        let metadata = std::fs::metadata(&path).ok();
        let size = metadata
            .as_ref()
            .and_then(|m| if m.is_file() { Some(m.len() as i64) } else { None });
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let path_text = canonical.to_string_lossy().to_string();
        let existed = local_files::Entity::find()
            .filter(local_files::Column::Path.eq(path_text.clone()))
            .one(&db.conn)
            .await
            .map_err(AppError::from)?;

        if let Some(existing) = existed {
            let mut am: local_files::ActiveModel = existing.clone().into();
            am.is_valid = Set(1);
            am.size = Set(size);
            if existing.source_type != 0 {
                let mut ids = parse_source_clipboard_ids(existing.source_clipboard_id.as_deref());
                if !ids.contains(&record.id) {
                    ids.push(record.id);
                }
                am.source_clipboard_id = Set(Some(
                    serde_json::to_string(&ids).map_err(AppError::from)?,
                ));
                am.source_type = Set(1);
            }
            am.update(&db.conn).await.map_err(AppError::from)?;
        } else {
            let now = chrono::Utc::now().timestamp();
            let new_item = local_files::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                path: Set(path_text),
                r#type: Set(local_file_type_from_clipboard_type(record.r#type, &canonical)),
                created_at: Set(now),
                access_count: Set(0),
                is_valid: Set(1),
                size: Set(size),
                source_clipboard_id: Set(Some(
                    serde_json::to_string(&vec![record.id]).map_err(AppError::from)?,
                )),
                source_type: Set(1),
                is_favorite: Set(0),
            };
            new_item.insert(&db.conn).await.map_err(AppError::from)?;
        }
    }
    Ok(())
}

pub async fn sync_local_files_on_clipboard_unshared_or_invalid(
    db: &DbState,
    record: &Model,
) -> Result<(), AppError> {
    let paths = extract_paths_from_clipboard_record(record)?;
    for raw_path in paths {
        let path = std::path::PathBuf::from(raw_path);
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let path_text = canonical.to_string_lossy().to_string();

        let existed = local_files::Entity::find()
            .filter(local_files::Column::Path.eq(path_text))
            .one(&db.conn)
            .await
            .map_err(AppError::from)?;

        let Some(existing) = existed else {
            continue;
        };
        if existing.source_type == 0 {
            continue;
        }

        let mut ids = parse_source_clipboard_ids(existing.source_clipboard_id.as_deref());
        ids.retain(|x| *x != record.id);

        let mut am: local_files::ActiveModel = existing.into();
        if ids.is_empty() {
            am.source_clipboard_id = Set(None);
            am.is_valid = Set(0);
        } else {
            am.source_clipboard_id = Set(Some(
                serde_json::to_string(&ids).map_err(AppError::from)?,
            ));
            am.is_valid = Set(1);
        }
        am.update(&db.conn).await.map_err(AppError::from)?;
    }
    Ok(())
}

pub async fn invalidate_shared_clipboards_by_fs_change(db: &DbState, changed_path: &std::path::Path) -> Result<(), AppError> {
    let records = Entity::find()
        .filter(crate::entity::clipboard_record::Column::IsShared.eq(1))
        .filter(crate::entity::clipboard_record::Column::IsValid.eq(1))
        .filter(
            crate::entity::clipboard_record::Column::Type
                .is_in([ClipboardType::File as i32, ClipboardType::Folder as i32, ClipboardType::Image as i32]),
        )
        .all(&db.conn)
        .await
        .map_err(AppError::from)?;

    for record in records {
        let paths = extract_paths_from_clipboard_record(&record)?;
        let mut should_check = false;
        for raw in &paths {
            let p = std::path::PathBuf::from(raw);
            let canonical = std::fs::canonicalize(&p).unwrap_or(p);
            if canonical == changed_path || changed_path.starts_with(&canonical) || canonical.starts_with(changed_path) {
                should_check = true;
                break;
            }
        }
        if !should_check {
            continue;
        }

        let invalid = if record.r#type == ClipboardType::Folder as i32 {
            // 文件夹模式：忽略文件夹内部变动，只在文件夹本身不存在时失效
            paths.iter().any(|raw| !std::path::Path::new(raw).exists())
        } else {
            // 文件/图片/多文件：任何一个缺失即整条剪贴板失效
            paths.iter().any(|raw| !std::path::Path::new(raw).exists())
        };

        if invalid {
            let mut am: ActiveModel = record.clone().into();
            am.is_valid = Set(0);
            am.update(&db.conn).await.map_err(AppError::from)?;
            sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
        }
    }

    Ok(())
}
