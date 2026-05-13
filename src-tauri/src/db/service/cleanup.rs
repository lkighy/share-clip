use log::debug;
use sea_orm::*;

use crate::app::config::AppConfig;
use crate::db::service::clipboard::{
    invalidate_shared_clipboards_by_fs_change, sync_local_files_on_clipboard_unshared_or_invalid,
};
use crate::db::service::local_files::{
    cleanup_missing_local_files, cleanup_orphaned_clipboard_local_files, expire_temp_local_files,
};
use crate::db::DbState;
use crate::entity::clipboard_record;
use crate::error::AppError;
use crate::models::clipboard::ClipboardType;
use crate::utils::format::normalize_file_uri;

pub async fn cleanup_old_items(
    db: &DatabaseConnection,
    config: &AppConfig,
) -> Result<(), AppError> {
    expire_temp_local_files(db).await?;
    cleanup_orphaned_clipboard_local_files(db).await?;
    cleanup_missing_local_files(db).await?;

    let db_state = DbState { conn: db.clone() };

    if let Some(days) = config.cleanup_after_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_timestamp = cutoff.timestamp();

        let to_delete = clipboard_record::Entity::find()
            .filter(clipboard_record::Column::CreatedAt.lt(cutoff_timestamp))
            .filter(clipboard_record::Column::IsFavorite.eq(0))
            .all(db)
            .await
            .map_err(|e| {
                debug!("cleanup_old_items query by days failed: days={days}, cutoff={cutoff_timestamp}, error={e}");
                AppError::from(e)
            })?;

        for item in &to_delete {
            sync_local_files_on_clipboard_unshared_or_invalid(&db_state, item).await?;
        }

        let ids: Vec<i32> = to_delete.into_iter().map(|item| item.id).collect();
        if !ids.is_empty() {
            clipboard_record::Entity::delete_many()
                .filter(clipboard_record::Column::Id.is_in(ids))
                .exec(db)
                .await
                .map_err(|e| {
                    debug!("cleanup_old_items delete by days failed: days={days}, cutoff={cutoff_timestamp}, error={e}");
                    AppError::from(e)
                })?;
        }
    }

    if let Some(max) = config.max_items {
        let total_non_favorite = clipboard_record::Entity::find()
            .filter(clipboard_record::Column::IsFavorite.eq(0))
            .count(db)
            .await
            .map_err(|e| {
                debug!("cleanup_old_items count failed: max={max}, error={e}");
                AppError::from(e)
            })? as usize;

        if total_non_favorite > max {
            let to_delete = clipboard_record::Entity::find()
                .filter(clipboard_record::Column::IsFavorite.eq(0))
                .order_by_asc(clipboard_record::Column::CreatedAt)
                .limit((total_non_favorite - max) as u64)
                .all(db)
                .await
                .map_err(|e| {
                    debug!(
                        "cleanup_old_items query overflow items failed: total_non_favorite={total_non_favorite}, max={max}, error={e}"
                    );
                    AppError::from(e)
                })?;

            for item in &to_delete {
                sync_local_files_on_clipboard_unshared_or_invalid(&db_state, item).await?;
            }

            let ids: Vec<i32> = to_delete.into_iter().map(|item| item.id).collect();
            if !ids.is_empty() {
                clipboard_record::Entity::delete_many()
                    .filter(clipboard_record::Column::Id.is_in(ids))
                    .exec(db)
                    .await
                    .map_err(|e| {
                        debug!(
                            "cleanup_old_items delete overflow items failed: max={max}, error={e}"
                        );
                        AppError::from(e)
                    })?;
            }
        }
    }

    cleanup_orphaned_clipboard_local_files(db).await?;

    Ok(())
}

async fn handle_invalid_item(
    db: &DatabaseConnection,
    item: clipboard_record::Model,
    auto_cleanup: bool,
) -> Result<(), DbErr> {
    let id = item.id;
    if let Err(e) =
        sync_local_files_on_clipboard_unshared_or_invalid(&DbState { conn: db.clone() }, &item)
            .await
    {
        debug!("handle_invalid_item sync local files failed: id={id}, error={e}");
    }
    if auto_cleanup {
        item.delete(db).await.map_err(|e| {
            debug!("handle_invalid_item delete failed: id={id}, auto_cleanup={auto_cleanup}, error={e}");
            e
        })?;
    } else {
        let mut active: clipboard_record::ActiveModel = item.into();
        active.is_valid = Set(0);
        active.update(db).await.map_err(|e| {
            debug!("handle_invalid_item mark invalid failed: id={id}, error={e}");
            e
        })?;
    }
    Ok(())
}

pub async fn cleanup_invalid_items(
    db: &DatabaseConnection,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_types = [ClipboardType::File as i32, ClipboardType::Folder as i32];
    let image_type = ClipboardType::Image as i32;
    let db_state = DbState { conn: db.clone() };

    let file_items = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Type.is_in(file_types))
        .all(db)
        .await
        .map_err(|e| {
            debug!("cleanup_invalid_items query file/folder items failed: error={e}");
            e
        })?;

    for item in file_items {
        if let Some(data) = item.data.clone() {
            let item_id = item.id;
            let paths: Vec<String> = serde_json::from_slice(&data).map_err(|e| {
                debug!("cleanup_invalid_items parse file list failed: id={item_id}, error={e}");
                e
            })?;
            let missing_path = paths
                .iter()
                .find(|p| !std::path::Path::new(normalize_file_uri(p)).exists());
            if let Some(missing_path) = missing_path {
                let auto_cleanup =
                    item.is_favorite != 1 && config.auto_cleanup_invalid_clipboard_data;
                if let Err(e) = invalidate_shared_clipboards_by_fs_change(
                    &db_state,
                    std::path::Path::new(normalize_file_uri(missing_path)),
                )
                .await
                {
                    debug!(
                        "cleanup_invalid_items invalidate shared clipboards failed: id={item_id}, error={e}"
                    );
                }
                handle_invalid_item(db, item, auto_cleanup).await?;
            }
        }
    }

    let image_items = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Type.eq(image_type))
        .all(db)
        .await
        .map_err(|e| {
            debug!("cleanup_invalid_items query image items failed: error={e}");
            e
        })?;

    for item in image_items {
        if let Some(data) = item.data.clone() {
            let item_id = item.id;
            let path_str = String::from_utf8(data).map_err(|e| {
                debug!("cleanup_invalid_items parse image path failed: id={item_id}, error={e}");
                e
            })?;
            let path = std::path::Path::new(normalize_file_uri(&path_str));
            if !path.exists() {
                let auto_cleanup =
                    item.is_favorite != 1 && config.auto_cleanup_invalid_clipboard_data;
                if let Err(e) = invalidate_shared_clipboards_by_fs_change(&db_state, path).await {
                    debug!(
                        "cleanup_invalid_items invalidate shared clipboards failed: id={item_id}, error={e}"
                    );
                }
                handle_invalid_item(db, item, auto_cleanup).await?;
            }
        }
    }

    expire_temp_local_files(db).await?;
    cleanup_orphaned_clipboard_local_files(db).await?;
    cleanup_missing_local_files(db).await?;

    Ok(())
}
