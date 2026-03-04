use log::debug;
use sea_orm::*;

use crate::app::config::AppConfig;
use crate::entity::clipboard_record;
use crate::error::AppError;
use crate::models::clipboard::ClipboardType;

pub async fn cleanup_old_items(
    db: &DatabaseConnection,
    config: &AppConfig,
) -> Result<(), AppError> {
    if let Some(days) = config.cleanup_after_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_timestamp = cutoff.timestamp();

        clipboard_record::Entity::delete_many()
            .filter(clipboard_record::Column::CreatedAt.lt(cutoff_timestamp))
            .filter(clipboard_record::Column::IsFavorite.eq(0))
            .exec(db)
            .await
            .map_err(|e| {
                debug!("cleanup_old_items delete by days failed: days={days}, cutoff={cutoff_timestamp}, error={e}");
                AppError::from(e)
            })?;
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

            let ids: Vec<i32> = to_delete.into_iter().map(|item| item.id).collect();
            clipboard_record::Entity::delete_many()
                .filter(clipboard_record::Column::Id.is_in(ids))
                .exec(db)
                .await
                .map_err(|e| {
                    debug!("cleanup_old_items delete overflow items failed: max={max}, error={e}");
                    AppError::from(e)
                })?;
        }
    }

    Ok(())
}

async fn handle_invalid_item(
    db: &DatabaseConnection,
    item: clipboard_record::Model,
    auto_cleanup: bool,
) -> Result<(), DbErr> {
    let id = item.id;
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
            if paths.iter().any(|p| !std::path::Path::new(p).exists()) {
                let auto_cleanup = item.is_favorite != 1 && config.auto_cleanup_invalid_clipboard_data;
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
            let path = std::path::Path::new(&path_str);
            if !path.exists() {
                let auto_cleanup = item.is_favorite != 1 && config.auto_cleanup_invalid_clipboard_data;
                handle_invalid_item(db, item, auto_cleanup).await?;
            }
        }
    }

    Ok(())
}
