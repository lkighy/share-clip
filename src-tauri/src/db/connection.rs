use log::info;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};
use tauri::{AppHandle, Manager};

pub struct DbState {
    pub conn: DatabaseConnection,
}

const DB_FILE_NAME: &str = "share_clip.db";

pub async fn init_db(app: &AppHandle) -> Result<DatabaseConnection, DbErr> {
    let db_path = database_path(app).map_err(|err| DbErr::Custom(err.to_string()))?;
    migrate_legacy_database(&db_path).map_err(|err| DbErr::Custom(err.to_string()))?;

    let mut options = ConnectOptions::new(sqlite_url(&db_path));
    options
        .max_connections(10)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8));

    let db = Database::connect(options).await?;

    Migrator::up(&db, None).await?;

    Ok(db)
}

fn database_path(app: &AppHandle) -> io::Result<PathBuf> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| io::Error::other(err.to_string()))?;
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join(DB_FILE_NAME))
}

fn migrate_legacy_database(target_path: &Path) -> io::Result<()> {
    if target_path.exists() {
        return Ok(());
    }

    let Some(legacy_path) = legacy_database_candidates()
        .into_iter()
        .find(|path| path.exists() && !is_same_path(path, target_path))
    else {
        return Ok(());
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    move_file(&legacy_path, target_path)?;
    move_legacy_sidecar_file(&legacy_path, target_path, "wal")?;
    move_legacy_sidecar_file(&legacy_path, target_path, "shm")?;
    info!(
        "migrated sqlite database from {} to {}",
        legacy_path.display(),
        target_path.display()
    );

    Ok(())
}

fn legacy_database_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(DB_FILE_NAME)];
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join(DB_FILE_NAME));
        }
    }
    candidates
}

fn move_legacy_sidecar_file(
    legacy_path: &Path,
    target_path: &Path,
    suffix: &str,
) -> io::Result<()> {
    let legacy_sidecar = legacy_path.with_file_name(format!("{DB_FILE_NAME}-{suffix}"));
    if !legacy_sidecar.exists() {
        return Ok(());
    }

    let target_sidecar = target_path.with_file_name(format!("{DB_FILE_NAME}-{suffix}"));
    move_file(&legacy_sidecar, &target_sidecar)
}

fn move_file(source: &Path, target: &Path) -> io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            fs::copy(source, target)?;
            fs::remove_file(source).map_err(|remove_error| {
                io::Error::new(
                    remove_error.kind(),
                    format!(
                        "copied {} but failed to remove it: {remove_error}; original rename error: {rename_error}",
                        source.display()
                    ),
                )
            })
        }
    }
}

fn is_same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn sqlite_url(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{}?mode=rwc", value)
}
