use std::path::PathBuf;
use tauri::Manager;
use crate::app::config::AppConfig;
use crate::db;
use crate::db::DbState;
use crate::entity::clipboard_record;
use crate::error::{ApiError, AppError};
use crate::models::clipboard::{ClipboardResponse, ClipboardType};
use crate::platform::automation::{Automation, InjectContent};


// 查询列表
#[tauri::command]
pub async fn clipboard_record_list(app: tauri::AppHandle, page: u64, page_size: u64) -> Result<Vec<ClipboardResponse>, ApiError> {
    let db = app.state::<DbState>();

    let records = db::service::clipboard::list_records(&db, page, page_size).await.map_err(AppError::from)?;
    Ok(records)
}

/// 查询剪切板数据的接口
#[tauri::command]
pub async fn paste(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfig>();
    let record = db::service::clipboard::get_and_validate_clipboard_record(&db, id, config.auto_cleanup_invalid_clipboard_data).await.map_err(AppError::from)?;

    let record = if let Some(record) = record {
        record
    } else {
        return Err(AppError::NotFound.into())
    };

    let mut auto = Automation::new();

    match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(AppError::from)?;
            auto.inject(
                InjectContent::Text(data)
            ).map_err(AppError::from)?;
        }
        t if t == ClipboardType::Html as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(AppError::from)?;
            auto.inject(
                InjectContent::Html(data)
            ).map_err(AppError::from)?;
        }
        t if t == ClipboardType::Rtf as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(AppError::from)?;
            auto.inject(
                InjectContent::Rtf(data)
            ).map_err(AppError::from)?;
        }
        t if t == ClipboardType::Image as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(AppError::from)?;
            auto.inject(
                InjectContent::Files(vec![PathBuf::from(data)])
            ).map_err(AppError::from)?;
        }
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(AppError::from)?;
            let files: Vec<String> = serde_json::from_str(&data).map_err(AppError::from)?;

            auto.inject(
                InjectContent::Files(files.into_iter().map(PathBuf::from).collect())
            ).map_err(AppError::from)?;
        }
        _ => {}
    }

    Ok(())
}

// TODO: 复制
#[tauri::command]
pub async fn copy_item(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {

    Ok(())
}

// TODO: 收藏
#[tauri::command]
pub async fn toggle_favorite(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {

    Ok(())
}

// TODO: 分享和取消分享
#[tauri::command]
pub async fn toggle_share(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {

    Ok(())
}

// TODO: 删除
#[tauri::command]
pub async fn delete_item(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {

    Ok(())
}
