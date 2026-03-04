use std::path::PathBuf;
use html2text::from_read;
use log::{error, info};
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

    let records = db::service::clipboard::list_records(&db, page, page_size)
        .await
        .map_err(|e| {
            error!("clipboard_record_list failed: page={page}, page_size={page_size}, error={e}");
            AppError::from(e)
        })?;
    Ok(records)
}

/// 查询剪切板数据的接口
#[tauri::command]
pub async fn paste_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfig>();
    let record = db::service::clipboard::get_and_validate_clipboard_record(&db, id, config.auto_cleanup_invalid_clipboard_data)
        .await
        .map_err(|e| {
            error!("paste_clipboard_record query failed: id={id}, error={e}");
            AppError::from(e)
        })?;

    let record = if let Some(record) = record {
        record
    } else {
        error!("paste_clipboard_record not found: id={id}");
        return Err(AppError::NotFound.into())
    };

    let mut auto = Automation::new();

    match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=text, error={e}");
                AppError::from(e)
            })?;
            auto.inject(
                InjectContent::Text(data)
            ).map_err(|e| {
                error!("paste_clipboard_record inject text failed: id={id}, error={e}");
                AppError::from(e)
            })?;
        }
        t if t == ClipboardType::Html as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=html, error={e}");
                AppError::from(e)
            })?;
            auto.inject(
                InjectContent::Html(data)
            ).map_err(|e| {
                error!("paste_clipboard_record inject html failed: id={id}, error={e}");
                AppError::from(e)
            })?;
        }
        t if t == ClipboardType::Rtf as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=rtf, error={e}");
                AppError::from(e)
            })?;
            auto.inject(
                InjectContent::Rtf(data)
            ).map_err(|e| {
                error!("paste_clipboard_record inject rtf failed: id={id}, error={e}");
                AppError::from(e)
            })?;
        }
        t if t == ClipboardType::Image as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=image, error={e}");
                AppError::from(e)
            })?;
            auto.inject(
                InjectContent::Files(vec![PathBuf::from(data)])
            ).map_err(|e| {
                error!("paste_clipboard_record inject image-path failed: id={id}, error={e}");
                AppError::from(e)
            })?;
        }
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=file/folder, error={e}");
                AppError::from(e)
            })?;
            let files: Vec<String> = serde_json::from_str(&data).map_err(|e| {
                error!("paste_clipboard_record json decode failed: id={id}, error={e}");
                AppError::from(e)
            })?;

            auto.inject(
                InjectContent::Files(files.into_iter().map(PathBuf::from).collect())
            ).map_err(|e| {
                error!("paste_clipboard_record inject files failed: id={id}, error={e}");
                AppError::from(e)
            })?;
        }
        _ => {}
    }

    Ok(())
}

/// 复制
#[tauri::command]
pub async fn copy_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfig>();
    let record = db::service::clipboard::get_and_validate_clipboard_record(&db, id, config.auto_cleanup_invalid_clipboard_data)
        .await
        .map_err(|e| {
            error!("copy_clipboard_record query failed: id={id}, error={e}");
            AppError::from(e)
        })?;

    let record: clipboard_record::Model = if let Some(record) = record {
        record
    } else {
        error!("copy_clipboard_record not found: id={id}");
        return Err(AppError::NotFound.into())
    };

    match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("copy_clipboard_record utf8 decode failed: id={id}, type=text, error={e}");
                AppError::from(e)
            })?;
            tauri_plugin_clipboard_x::write_text(data)
                .await
                .map_err(|e| {
                    error!("copy_clipboard_record write text failed: id={id}, error={e}");
                    AppError::InvalidInput(e.to_string())
                })?;
        }
        t if t == ClipboardType::Html as i32 => {
            let html = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("copy_clipboard_record utf8 decode failed: id={id}, type=html, error={e}");
                AppError::from(e)
            })?;
            let text = from_read(html.as_bytes(), usize::MAX);
            tauri_plugin_clipboard_x::write_html(text, html)
                .await
                .map_err(|e| {
                    error!("copy_clipboard_record write html failed: id={id}, error={e}");
                    AppError::InvalidInput(e.to_string())
                })?;
        }
        t if t == ClipboardType::Rtf as i32 => {
            let rtf = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("copy_clipboard_record utf8 decode failed: id={id}, type=rtf, error={e}");
                AppError::from(e)
            })?;
            let text = record.preview.unwrap_or_default();
            tauri_plugin_clipboard_x::write_rtf(text, rtf)
                .await
                .map_err(|e| {
                    error!("copy_clipboard_record write rtf failed: id={id}, error={e}");
                    AppError::InvalidInput(e.to_string())
                })?;
        }
        t if t == ClipboardType::Image as i32 => {
            let path = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("copy_clipboard_record utf8 decode failed: id={id}, type=image, error={e}");
                AppError::from(e)
            })?;
            tauri_plugin_clipboard_x::write_image(path)
                .await
                .map_err(|e| {
                    error!("copy_clipboard_record write image failed: id={id}, error={e}");
                    AppError::InvalidInput(e.to_string())
                })?;
        }
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("copy_clipboard_record utf8 decode failed: id={id}, type=file/folder, error={e}");
                AppError::from(e)
            })?;
            let files: Vec<String> = serde_json::from_str(&data).map_err(|e| {
                error!("copy_clipboard_record json decode failed: id={id}, error={e}");
                AppError::from(e)
            })?;
            tauri_plugin_clipboard_x::write_files(files)
                .await
                .map_err(|e| {
                    error!("copy_clipboard_record write files failed: id={id}, error={e}");
                    AppError::InvalidInput(e.to_string())
                })?;
        }
        _ => {}
    }

    Ok(())
}

/// 收藏
#[tauri::command]
pub async fn toggle_favorite(app: tauri::AppHandle, id: i32) -> Result<bool, ApiError> {
    let db = app.state::<DbState>();

    let data = db::service::clipboard::toggle_favorite(&db, id).await.map_err(|e| {
        error!("toggle_favorite failed: id={id}, error={e}");
        AppError::from(e)
    })?;
    Ok(data)
}

/// 分享和取消分享
#[tauri::command]
pub async fn toggle_share(app: tauri::AppHandle, id: i32) -> Result<bool, ApiError> {
    let db = app.state::<DbState>();

    let data = db::service::clipboard::toggle_share(&db, id).await.map_err(|e| {
        error!("toggle_share failed: id={id}, error={e}");
        AppError::from(e)
    })?;
    Ok(data)
}

/// 删除
#[tauri::command]
pub async fn delete_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfig>();

    match db::service::clipboard::delete_item(&db, id, &config.cache_dir).await {
        Ok(()) => Ok(()),
        Err(AppError::NotFound) => {
            // 业务正常情况，记录 info 或 debug
            info!("delete_clipboard_record: item {} not found, maybe already deleted", id);
            Err(ApiError::from(AppError::NotFound))
        }
        Err(e) => {
            // 真正的错误，记录 error
            error!("delete_clipboard_record failed: id={id}, error={e}");
            Err(ApiError::from(e))
        }
    }
}
