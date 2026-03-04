#![allow(dead_code)]

use log::{debug, warn};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, ModelTrait, Set};

use crate::db::repository::clipboard_record;
use crate::db::DbState;
use crate::entity::clipboard_record::{ActiveModel, Entity, Model};
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
    active.update(&db.conn).await.map_err(|e| {
        debug!("toggle_share update failed: id={id}, error={e}");
        AppError::from(e)
    })?;

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
                    warn!("delete_item remove cache file failed: id={id}, path={}, error={e}", path.display());
                }
            }
        }
    }

    record.delete(&db.conn).await.map_err(|e| {
        debug!("delete_item delete record failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    Ok(())
}
