use crate::app::config::AppConfigStore;
use crate::db;
use crate::db::service::clipboard_formats::{
    ClipboardFormats, FORMAT_HTML, FORMAT_RTF, FORMAT_TEXT,
};
use crate::db::DbState;
use crate::entity::clipboard_record;
use crate::error::{ApiError, AppError};
use crate::models::clipboard::{ClipboardResponse, ClipboardType};
use crate::platform::automation::{Automation, InjectContent};
use crate::utils::text::{html_to_plain_text, rtf_to_plain_text};
use base64::Engine;
use log::{error, info};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

// 查询列表
#[tauri::command]
pub async fn clipboard_record_list(
    app: tauri::AppHandle,
    page: u64,
    page_size: u64,
) -> Result<Vec<ClipboardResponse>, ApiError> {
    let db = app.state::<DbState>();

    let records = db::service::clipboard::list_records(&db, page, page_size)
        .await
        .map_err(|e| {
            error!("clipboard_record_list failed: page={page}, page_size={page_size}, error={e}");
            AppError::from(e)
        })?;
    Ok(records)
}

#[derive(serde::Deserialize)]
pub struct RemoteClipboardContentPayload {
    pub r#type: i32,
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image_base64: Option<String>,
    pub files: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFormatPayload {
    pub id: i32,
    pub format: String,
    pub as_text: bool,
}

#[tauri::command]
pub async fn paste_remote_clipboard_content(
    payload: RemoteClipboardContentPayload,
) -> Result<(), ApiError> {
    let content = inject_content_from_remote_payload(payload)?;
    let mut auto = Automation::new();
    crate::services::clipboard_watcher::suppress_next_clipboard_changes(1, Duration::from_secs(2));
    if let Err(e) = auto.inject(content) {
        crate::services::clipboard_watcher::clear_suppressed_clipboard_changes();
        return Err(AppError::from(e).into());
    }
    Ok(())
}

#[tauri::command]
pub async fn copy_remote_clipboard_content(
    payload: RemoteClipboardContentPayload,
) -> Result<(), ApiError> {
    let content = inject_content_from_remote_payload(payload)?;
    copy_inject_content_to_clipboard(content).await
}

#[tauri::command]
pub async fn paste_clipboard_record_as(
    app: tauri::AppHandle,
    payload: ClipboardFormatPayload,
) -> Result<(), ApiError> {
    let content =
        clipboard_record_format_content(&app, payload.id, &payload.format, payload.as_text).await?;
    let mut auto = Automation::new();
    crate::services::clipboard_watcher::suppress_next_clipboard_changes(1, Duration::from_secs(2));
    if let Err(e) = auto.inject(content) {
        crate::services::clipboard_watcher::clear_suppressed_clipboard_changes();
        return Err(AppError::from(e).into());
    }

    let db = app.state::<DbState>();
    db::service::clipboard::mark_record_accessed(&db, payload.id)
        .await
        .map_err(|e| AppError::from(e))?;

    #[cfg(target_os = "windows")]
    crate::platform::non_activating::windows::hide_window();

    Ok(())
}

#[tauri::command]
pub async fn copy_clipboard_record_as(
    app: tauri::AppHandle,
    payload: ClipboardFormatPayload,
) -> Result<(), ApiError> {
    let content =
        clipboard_record_format_content(&app, payload.id, &payload.format, payload.as_text).await?;
    copy_inject_content_to_clipboard(content).await
}

fn inject_content_from_remote_payload(
    payload: RemoteClipboardContentPayload,
) -> Result<InjectContent, ApiError> {
    if payload.html.is_some() || payload.rtf.is_some() {
        return Ok(InjectContent::RichText(ClipboardFormats {
            text: payload.text,
            html: payload.html,
            rtf: payload.rtf,
        }));
    }
    if payload.r#type == ClipboardType::Text as i32 {
        return Ok(InjectContent::Text(payload.text.unwrap_or_default()));
    }
    if payload.r#type == ClipboardType::Html as i32 {
        return Ok(InjectContent::Html(payload.html.unwrap_or_default()));
    }
    if payload.r#type == ClipboardType::Rtf as i32 {
        return Ok(InjectContent::Rtf(payload.rtf.unwrap_or_default()));
    }
    if payload.r#type == ClipboardType::Image as i32 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.image_base64.unwrap_or_default())
            .map_err(|e| AppError::InvalidInput(format!("无效图片数据: {e}")))?;
        return Ok(InjectContent::Image(bytes));
    }
    if payload.r#type == ClipboardType::File as i32
        || payload.r#type == ClipboardType::Folder as i32
    {
        return Ok(InjectContent::Files(
            payload
                .files
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        ));
    }
    Err(AppError::InvalidInput("不支持的远程剪贴板类型".to_string()).into())
}

async fn clipboard_record_format_content(
    app: &tauri::AppHandle,
    id: i32,
    format: &str,
    as_text: bool,
) -> Result<InjectContent, ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfigStore>().get();
    let record = db::service::clipboard::get_and_validate_clipboard_record(
        &db,
        id,
        config.auto_cleanup_invalid_clipboard_data,
    )
    .await
    .map_err(|e| AppError::from(e))?
    .ok_or(AppError::NotFound)?;

    let formats = db::service::clipboard_formats::load_formats(&db.conn, &record)
        .await
        .map_err(|e| AppError::from(e))?;

    match format {
        FORMAT_TEXT => {
            let text = formats.primary_text();
            Ok(InjectContent::Text(text))
        }
        FORMAT_HTML => {
            let html = formats
                .html
                .ok_or_else(|| AppError::InvalidInput("该条目没有 HTML 格式".to_string()))?;
            if as_text {
                Ok(InjectContent::Text(html))
            } else {
                Ok(InjectContent::Html(html))
            }
        }
        FORMAT_RTF => {
            let rtf = formats
                .rtf
                .ok_or_else(|| AppError::InvalidInput("该条目没有 RTF 格式".to_string()))?;
            if as_text {
                Ok(InjectContent::Text(rtf))
            } else {
                Ok(InjectContent::Rtf(rtf))
            }
        }
        _ => Err(AppError::InvalidInput(format!("不支持的剪贴板格式: {format}")).into()),
    }
}

async fn copy_inject_content_to_clipboard(content: InjectContent) -> Result<(), ApiError> {
    match content {
        InjectContent::Text(text) => tauri_plugin_clipboard_x::write_text(text)
            .await
            .map_err(|e| AppError::InvalidInput(e.to_string()).into()),
        InjectContent::Html(html) => {
            let text = html_to_plain_text(&html);
            tauri_plugin_clipboard_x::write_html(text, html)
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()).into())
        }
        InjectContent::Rtf(rtf) => {
            let text = rtf_to_plain_text(&rtf);
            tauri_plugin_clipboard_x::write_rtf(text, rtf)
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()).into())
        }
        InjectContent::RichText(formats) => write_rich_text_to_clipboard(formats)
            .await
            .map_err(|e| AppError::InvalidInput(e).into()),
        InjectContent::Files(files) => {
            let files = files
                .into_iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            tauri_plugin_clipboard_x::write_files(files)
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()).into())
        }
        InjectContent::Image(bytes) => {
            let cache_path = std::env::temp_dir().join(format!(
                "share-clip-remote-{}.png",
                chrono::Utc::now().timestamp_millis()
            ));
            std::fs::write(&cache_path, bytes).map_err(AppError::from)?;
            tauri_plugin_clipboard_x::write_image(cache_path.to_string_lossy().to_string())
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()).into())
        }
    }
}

async fn write_rich_text_to_clipboard(formats: ClipboardFormats) -> Result<(), String> {
    let text = formats.primary_text();

    #[cfg(target_os = "windows")]
    if formats.html.is_some() && formats.rtf.is_some() {
        use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};

        let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
        let mut contents = vec![ClipboardContent::Text(text)];
        if let Some(html) = formats.html {
            contents.push(ClipboardContent::Html(html));
        }
        if let Some(rtf) = formats.rtf {
            contents.push(ClipboardContent::Rtf(rtf));
        }
        ctx.set(contents).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if let Some(html) = formats.html {
        tauri_plugin_clipboard_x::write_html(text, html)
            .await
            .map_err(|e| e.to_string())?;
    } else if let Some(rtf) = formats.rtf {
        tauri_plugin_clipboard_x::write_rtf(text, rtf)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tauri_plugin_clipboard_x::write_text(text)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 查询剪切板数据的接口
#[tauri::command]
pub async fn paste_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfigStore>().get();
    let record = db::service::clipboard::get_and_validate_clipboard_record(
        &db,
        id,
        config.auto_cleanup_invalid_clipboard_data,
    )
    .await
    .map_err(|e| {
        error!("paste_clipboard_record query failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    let record = if let Some(record) = record {
        record
    } else {
        error!("paste_clipboard_record not found: id={id}");
        return Err(AppError::NotFound.into());
    };

    let formats = db::service::clipboard_formats::load_formats(&db.conn, &record)
        .await
        .map_err(|e| {
            error!("paste_clipboard_record load formats failed: id={id}, error={e}");
            AppError::from(e)
        })?;

    let content = match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            if !formats.is_empty() {
                Some(InjectContent::RichText(formats))
            } else {
                let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "paste_clipboard_record utf8 decode failed: id={id}, type=text, error={e}"
                    );
                    AppError::from(e)
                })?;
                Some(InjectContent::Text(data))
            }
        }
        t if t == ClipboardType::Html as i32 => {
            if !formats.is_empty() {
                Some(InjectContent::RichText(formats))
            } else {
                let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "paste_clipboard_record utf8 decode failed: id={id}, type=html, error={e}"
                    );
                    AppError::from(e)
                })?;
                Some(InjectContent::Html(data))
            }
        }
        t if t == ClipboardType::Rtf as i32 => {
            if !formats.is_empty() {
                Some(InjectContent::RichText(formats))
            } else {
                let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "paste_clipboard_record utf8 decode failed: id={id}, type=rtf, error={e}"
                    );
                    AppError::from(e)
                })?;
                Some(InjectContent::Rtf(data))
            }
        }
        t if t == ClipboardType::Image as i32 => {
            let path = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                error!("paste_clipboard_record utf8 decode failed: id={id}, type=image, error={e}");
                AppError::from(e)
            })?;
            let image_data = std::fs::read(&path).map_err(|e| {
                error!(
                    "paste_clipboard_record read image cache failed: id={id}, path={path}, error={e}"
                );
                AppError::from(e)
            })?;
            Some(InjectContent::Image(image_data))
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

            Some(InjectContent::Files(
                files.into_iter().map(PathBuf::from).collect(),
            ))
        }
        _ => None,
    };

    if let Some(content) = content {
        let mut auto = Automation::new();
        crate::services::clipboard_watcher::suppress_next_clipboard_changes(
            1,
            Duration::from_secs(2),
        );
        if let Err(e) = auto.inject(content) {
            crate::services::clipboard_watcher::clear_suppressed_clipboard_changes();
            error!("paste_clipboard_record inject failed: id={id}, error={e}");
            return Err(AppError::from(e).into());
        }
    }

    db::service::clipboard::mark_record_accessed(&db, id)
        .await
        .map_err(|e| {
            error!("paste_clipboard_record mark accessed failed: id={id}, error={e}");
            AppError::from(e)
        })?;
    crate::app::events::emit_clipboard_changed(
        &app,
        vec![id.to_string()],
        "clipboard_record_accessed",
    );

    #[cfg(target_os = "windows")]
    crate::platform::non_activating::windows::hide_window();

    Ok(())
}

/// 复制
#[tauri::command]
pub async fn copy_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfigStore>().get();
    let record = db::service::clipboard::get_and_validate_clipboard_record(
        &db,
        id,
        config.auto_cleanup_invalid_clipboard_data,
    )
    .await
    .map_err(|e| {
        error!("copy_clipboard_record query failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    let record: clipboard_record::Model = if let Some(record) = record {
        record
    } else {
        error!("copy_clipboard_record not found: id={id}");
        return Err(AppError::NotFound.into());
    };

    let formats = db::service::clipboard_formats::load_formats(&db.conn, &record)
        .await
        .map_err(|e| {
            error!("copy_clipboard_record load formats failed: id={id}, error={e}");
            AppError::from(e)
        })?;

    match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            if !formats.is_empty() {
                write_rich_text_to_clipboard(formats).await.map_err(|e| {
                    error!("copy_clipboard_record write rich text failed: id={id}, error={e}");
                    AppError::InvalidInput(e)
                })?;
            } else {
                let data = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "copy_clipboard_record utf8 decode failed: id={id}, type=text, error={e}"
                    );
                    AppError::from(e)
                })?;
                tauri_plugin_clipboard_x::write_text(data)
                    .await
                    .map_err(|e| {
                        error!("copy_clipboard_record write text failed: id={id}, error={e}");
                        AppError::InvalidInput(e.to_string())
                    })?;
            }
        }
        t if t == ClipboardType::Html as i32 => {
            if !formats.is_empty() {
                write_rich_text_to_clipboard(formats).await.map_err(|e| {
                    error!("copy_clipboard_record write rich text failed: id={id}, error={e}");
                    AppError::InvalidInput(e)
                })?;
            } else {
                let html = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "copy_clipboard_record utf8 decode failed: id={id}, type=html, error={e}"
                    );
                    AppError::from(e)
                })?;
                let text = html_to_plain_text(&html);
                tauri_plugin_clipboard_x::write_html(text, html)
                    .await
                    .map_err(|e| {
                        error!("copy_clipboard_record write html failed: id={id}, error={e}");
                        AppError::InvalidInput(e.to_string())
                    })?;
            }
        }
        t if t == ClipboardType::Rtf as i32 => {
            if !formats.is_empty() {
                write_rich_text_to_clipboard(formats).await.map_err(|e| {
                    error!("copy_clipboard_record write rich text failed: id={id}, error={e}");
                    AppError::InvalidInput(e)
                })?;
            } else {
                let rtf = String::from_utf8(record.data.unwrap_or_default()).map_err(|e| {
                    error!(
                        "copy_clipboard_record utf8 decode failed: id={id}, type=rtf, error={e}"
                    );
                    AppError::from(e)
                })?;
                let text = rtf_to_plain_text(&rtf);
                tauri_plugin_clipboard_x::write_rtf(text, rtf)
                    .await
                    .map_err(|e| {
                        error!("copy_clipboard_record write rtf failed: id={id}, error={e}");
                        AppError::InvalidInput(e.to_string())
                    })?;
            }
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

    let data = db::service::clipboard::toggle_favorite(&db, id)
        .await
        .map_err(|e| {
            error!("toggle_favorite failed: id={id}, error={e}");
            AppError::from(e)
        })?;
    Ok(data)
}

/// 分享和取消分享
#[tauri::command]
pub async fn toggle_share(app: tauri::AppHandle, id: i32) -> Result<bool, ApiError> {
    let db = app.state::<DbState>();

    let data = db::service::clipboard::toggle_share(&db, id)
        .await
        .map_err(|e| {
            error!("toggle_share failed: id={id}, error={e}");
            AppError::from(e)
        })?;
    if data {
        if let Err(error) = crate::server::sync::scan_local_shares_once(&db.conn).await {
            error!("scan local shares after toggle_share failed: id={id}, error={error}");
        }
    }
    crate::app::events::emit_local_files_changed(
        &app,
        Vec::new(),
        if data {
            "clipboard_share_enabled"
        } else {
            "clipboard_share_disabled"
        },
    );
    Ok(data)
}

/// 删除
#[tauri::command]
pub async fn delete_clipboard_record(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = app.state::<DbState>();
    let config = app.state::<AppConfigStore>().get();

    match db::service::clipboard::delete_item(&db, id, &config.cache_dir).await {
        Ok(()) => Ok(()),
        Err(AppError::NotFound) => {
            // 业务正常情况，记录 info 或 debug
            info!(
                "delete_clipboard_record: item {} not found, maybe already deleted",
                id
            );
            Err(ApiError::from(AppError::NotFound))
        }
        Err(e) => {
            // 真正的错误，记录 error
            error!("delete_clipboard_record failed: id={id}, error={e}");
            Err(ApiError::from(e))
        }
    }
}
