use sea_orm::*;
use crate::app::config::AppConfig;
use crate::entity::clipboard_record;
use crate::error::AppError;
use crate::models::clipboard::ClipboardType;

/// 清理过期和超出数量的条目
pub async fn cleanup_old_items(
    db: &DatabaseConnection,
    config: &AppConfig,
) -> Result<(), AppError> {
    // 1. 基于天数清理（跳过收藏条目）
    if let Some(days) = config.cleanup_after_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_timestamp = cutoff.timestamp();

        clipboard_record::Entity::delete_many()
            .filter(clipboard_record::Column::CreatedAt.lt(cutoff_timestamp))
            .filter(clipboard_record::Column::IsFavorite.eq(0)) // 只删除非收藏
            .exec(db)
            .await?;
    }

    // 2. 基于最大条目数清理（跳过收藏条目）
    if let Some(max) = config.max_items {
        // 统计非收藏的记录总数
        let total_non_favorite = clipboard_record::Entity::find()
            .filter(clipboard_record::Column::IsFavorite.eq(0))
            .count(db)
            .await? as usize;

        if total_non_favorite > max {
            // 找出需要删除的最旧的非收藏记录的 ID
            let to_delete = clipboard_record::Entity::find()
                .filter(clipboard_record::Column::IsFavorite.eq(0))
                .order_by_asc(clipboard_record::Column::CreatedAt)
                .limit((total_non_favorite - max) as u64)
                .all(db)
                .await?;

            let ids: Vec<i32> = to_delete.into_iter().map(|item| item.id).collect();
            clipboard_record::Entity::delete_many()
                .filter(clipboard_record::Column::Id.is_in(ids))
                .exec(db)
                .await?;
        }
    }

    Ok(())
}

async fn handle_invalid_item(
    db: &DatabaseConnection,
    item: clipboard_record::Model,
    auto_cleanup: bool,
) -> Result<(), DbErr> {
    if auto_cleanup {
        item.delete(db).await?;
    } else {
        let mut active: clipboard_record::ActiveModel = item.into();
        active.is_valid = Set(0);
        active.update(db).await?;
    }
    Ok(())
}

pub async fn cleanup_invalid_items(
    db: &DatabaseConnection,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_types = [ClipboardType::File as i32, ClipboardType::Folder as i32];
    let image_type = ClipboardType::Image as i32;

    // 1. 处理文件/文件夹类型
    let file_items = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Type.is_in(file_types))
        .all(db)
        .await?;

    for item in file_items {
        if let Some(data) = item.data.clone() {
            let paths: Vec<String> = serde_json::from_slice(&data)?;
            if paths.iter().any(|p| !std::path::Path::new(p).exists()) {
                if item.is_favorite == 1 {
                    // 收藏条目只标记无效，不删除
                    handle_invalid_item(db, item, false).await?;
                } else {
                    // 非收藏条目根据配置处理
                    handle_invalid_item(db, item, config.auto_cleanup_invalid_clipboard_data).await?;
                }
            }
        }
    }

    // 2. 处理图片类型
    let image_items = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Type.eq(image_type))
        .all(db)
        .await?;

    for item in image_items {
        if let Some(data) = item.data.clone() {
            let path_str = String::from_utf8(data)?;
            let path = std::path::Path::new(&path_str);
            if !path.exists() {
                if item.is_favorite == 1 {
                    handle_invalid_item(db, item, false).await?;
                } else {
                    handle_invalid_item(db, item, config.auto_cleanup_invalid_clipboard_data).await?;
                }
            }
        }
    }

    Ok(())
}