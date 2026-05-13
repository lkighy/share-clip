use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::db::service::clipboard::invalidate_shared_clipboards_by_fs_change;
use crate::db::DbState;
use crate::entity::local_files;
use crate::error::AppError;
use crate::utils::format::normalize_file_uri;

pub const SOURCE_DIRECT: i32 = 0;
pub const SOURCE_CLIPBOARD: i32 = 1;
pub const SOURCE_DIRECT_AND_CLIPBOARD: i32 = 2;

pub const SHARE_MODE_MANUAL: i32 = 0;
pub const SHARE_MODE_TEMP: i32 = 1;

pub fn parse_source_clipboard_ids(raw: Option<&str>) -> Vec<i32> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<i32>>(raw).unwrap_or_default()
}

pub fn has_direct_source(source_type: i32) -> bool {
    source_type == SOURCE_DIRECT || source_type == SOURCE_DIRECT_AND_CLIPBOARD
}

pub fn has_clipboard_source(source_type: i32) -> bool {
    source_type == SOURCE_CLIPBOARD || source_type == SOURCE_DIRECT_AND_CLIPBOARD
}

pub fn source_type_after_adding_clipboard(source_type: i32) -> i32 {
    if has_direct_source(source_type) {
        SOURCE_DIRECT_AND_CLIPBOARD
    } else {
        SOURCE_CLIPBOARD
    }
}

pub fn source_type_after_adding_direct(source_type: i32) -> i32 {
    if has_clipboard_source(source_type) {
        SOURCE_DIRECT_AND_CLIPBOARD
    } else {
        SOURCE_DIRECT
    }
}

pub fn source_type_after_removing_clipboard(source_type: i32) -> Option<i32> {
    if has_direct_source(source_type) {
        Some(SOURCE_DIRECT)
    } else {
        None
    }
}

pub fn source_type_after_removing_direct(source_type: i32) -> Option<i32> {
    if has_clipboard_source(source_type) {
        Some(SOURCE_CLIPBOARD)
    } else {
        None
    }
}

pub async fn cleanup_orphaned_clipboard_local_files(
    db: &DatabaseConnection,
) -> Result<(), AppError> {
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .filter(
            local_files::Column::SourceType.is_in([SOURCE_CLIPBOARD, SOURCE_DIRECT_AND_CLIPBOARD]),
        )
        .all(db)
        .await
        .map_err(AppError::from)?;

    for row in rows {
        let ids = parse_source_clipboard_ids(row.source_clipboard_id.as_deref());
        if !ids.is_empty() {
            continue;
        }

        let mut am: local_files::ActiveModel = row.clone().into();
        if let Some(next_source_type) = source_type_after_removing_clipboard(row.source_type) {
            am.source_type = Set(next_source_type);
            am.share_mode = Set(SHARE_MODE_MANUAL);
            am.source_clipboard_id = Set(None);
        } else {
            am.is_valid = Set(0);
            am.source_clipboard_id = Set(None);
        }
        am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
        am.update(db).await.map_err(AppError::from)?;
    }

    Ok(())
}

pub async fn cleanup_missing_local_files(db: &DatabaseConnection) -> Result<(), AppError> {
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .all(db)
        .await
        .map_err(AppError::from)?;

    let now = chrono::Utc::now().timestamp();
    for row in rows {
        if std::path::Path::new(normalize_file_uri(&row.path)).exists() {
            continue;
        }

        invalidate_shared_clipboards_by_fs_change(
            &DbState { conn: db.clone() },
            std::path::Path::new(normalize_file_uri(&row.path)),
        )
        .await?;

        let mut am: local_files::ActiveModel = row.clone().into();
        am.is_valid = Set(0);
        am.source_clipboard_id = Set(None);
        am.updated_at = Set(Some(now));
        am.update(db).await.map_err(AppError::from)?;
    }

    Ok(())
}

pub async fn expire_temp_local_files(db: &DatabaseConnection) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .filter(local_files::Column::ShareMode.eq(SHARE_MODE_TEMP))
        .filter(local_files::Column::ExpiresAt.is_not_null())
        .filter(local_files::Column::ExpiresAt.lte(now))
        .all(db)
        .await
        .map_err(AppError::from)?;

    for row in rows {
        let mut am: local_files::ActiveModel = row.clone().into();
        if let Some(next_source_type) = source_type_after_removing_clipboard(row.source_type) {
            am.source_type = Set(next_source_type);
            am.share_mode = Set(SHARE_MODE_MANUAL);
            am.source_clipboard_id = Set(None);
        } else {
            am.is_valid = Set(0);
            am.source_clipboard_id = Set(None);
        }
        am.expires_at = Set(None);
        am.updated_at = Set(Some(now));
        am.update(db).await.map_err(AppError::from)?;
    }

    Ok(())
}
