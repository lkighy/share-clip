use base64::Engine;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::db::service::local_files::{
    parse_source_clipboard_ids, source_type_after_adding_direct, SHARE_MODE_MANUAL, SOURCE_DIRECT,
};
use crate::entity::clipboard_record;
use crate::entity::inbound_connections;
use crate::entity::local_file_index;
use crate::entity::local_files;
use crate::entity::outbound_connections;
use crate::entity::shared_file_index;
use crate::error::{ApiError, AppError};
use crate::models::clipboard::ClipboardType;
use crate::utils::format::{generate_image_thumbnail, normalize_file_uri};

#[derive(Debug, Serialize)]
pub struct RemoteShareUser {
    pub user_id: String,
    pub user_name: String,
    pub ip: String,
    pub password: Option<String>,
    pub device_id: Option<String>,
    pub auth_status: i32,
    pub last_connected_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct InboundConnectionRequest {
    pub user_id: String,
    pub user_name: Option<String>,
    pub ip: String,
    pub device_id: Option<String>,
    pub auth_status: i32,
    pub last_seen_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WebAccessRequest {
    pub id: String,
    pub client_label: String,
    pub ip: String,
    pub user_agent: Option<String>,
    pub scopes: Vec<String>,
    pub auth_status: i32,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Serialize)]
pub struct LocalSharedFileItem {
    pub id: String,
    pub path: String,
    pub r#type: i32,
    pub size: Option<i64>,
    pub created_at: i64,
    pub source_type: i32,
    pub source_clipboard_id: Option<String>,
    pub is_favorite: i32,
    pub share_mode: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpsertRemoteShareUserPayload {
    pub user_id: String,
    pub user_name: String,
    pub ip: String,
    pub password: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddManualSharedPathsPayload {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRemoteAuthStatusPayload {
    pub user_id: String,
    pub auth_status: i32,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InboundAuthDecisionPayload {
    pub user_id: String,
    pub auth_status: i32,
}

#[derive(Debug, Deserialize)]
pub struct WebAccessDecisionPayload {
    pub id: String,
    pub auth_status: i32,
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CacheRemoteSharedFilePayload {
    pub remote_user_id: String,
    pub share_id: String,
    pub share_name: String,
    pub relative_path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
    pub data_base64: Option<String>,
    pub destination_dir: Option<String>,
    pub destination_root_relative_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadRemoteSharedFilePayload {
    pub remote_user_id: String,
    pub base_url: String,
    pub auth_user_id: String,
    pub auth_device_id: String,
    pub share_id: String,
    pub share_name: String,
    pub relative_path: String,
    pub name: String,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
    pub transfer_task_id: Option<String>,
    pub destination_dir: Option<String>,
    pub destination_root_relative_path: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RemoteDownloadProgressPayload {
    pub transfer_task_id: String,
    pub relative_path: String,
    pub loaded: i64,
    pub total: Option<i64>,
    pub progress: f64,
}

#[derive(Debug, Deserialize)]
pub struct RemoteCacheTargetPayload {
    pub remote_user_id: String,
    pub share_id: String,
    pub relative_path: String,
}

#[derive(Debug, Deserialize)]
pub struct ListRemoteCachedFilesPayload {
    pub remote_user_id: String,
    pub share_id: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteCacheStatusPayload {
    pub remote_user_id: String,
    pub share_id: String,
    pub relative_path: String,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RemoteCacheStatus {
    pub cached: bool,
    pub local_cache_path: Option<String>,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateEmptyDirectoryPayload {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct MoveRemoteCachePayload {
    pub remote_user_id: String,
    pub share_id: String,
    pub relative_path: String,
    pub destination_dir: String,
}

#[derive(Debug, Serialize)]
pub struct RemoteCachedFileItem {
    pub remote_user_id: String,
    pub share_id: String,
    pub share_name: String,
    pub relative_path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
    pub local_cache_path: Option<String>,
    pub remote_deleted: bool,
    pub cache_status: i32,
    pub updated_at: Option<i64>,
}

const REMOTE_DOWNLOAD_PROGRESS_EVENT: &str = "share://remote-download-progress";

#[tauri::command]
pub async fn list_remote_share_users(
    app: tauri::AppHandle,
) -> Result<Vec<RemoteShareUser>, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let rows = outbound_connections::Entity::find()
        .all(db)
        .await
        .map_err(AppError::from)?;
    Ok(rows
        .into_iter()
        .map(|row| RemoteShareUser {
            user_id: row.user_id,
            user_name: row.user_name,
            ip: row.ip,
            password: row.password,
            device_id: row.device_id,
            auth_status: row.auth_status,
            last_connected_at: row.last_connected_at,
        })
        .collect())
}

#[tauri::command]
pub async fn list_local_shared_files(
    app: tauri::AppHandle,
) -> Result<Vec<LocalSharedFileItem>, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .order_by_desc(local_files::Column::IsFavorite)
        .order_by_desc(local_files::Column::CreatedAt)
        .all(db)
        .await
        .map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(|row| LocalSharedFileItem {
            id: row.id,
            path: row.path,
            r#type: row.r#type,
            size: row.size,
            created_at: row.created_at,
            source_type: row.source_type,
            source_clipboard_id: row.source_clipboard_id,
            is_favorite: row.is_favorite,
            share_mode: row.share_mode,
        })
        .collect())
}

#[tauri::command]
pub async fn toggle_local_shared_file_favorite(
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = local_files::Entity::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;

    let next_favorite = if row.is_favorite == 1 { 0 } else { 1 };
    let mut am: local_files::ActiveModel = row.into();
    am.is_favorite = Set(next_favorite);
    am.update(db).await.map_err(AppError::from)?;

    crate::app::events::emit_local_files_changed(&app, vec![id], "local_file_favorite_toggled");
    Ok(next_favorite == 1)
}

#[tauri::command]
pub async fn upsert_remote_share_user(
    app: tauri::AppHandle,
    payload: UpsertRemoteShareUserPayload,
) -> Result<RemoteShareUser, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let user_id = payload.user_id.trim().to_string();
    let user_name = payload.user_name.trim().to_string();
    let ip = payload.ip.trim().to_string();
    let password = payload
        .password
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let device_id = payload
        .device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if user_id.is_empty() || user_name.is_empty() || ip.is_empty() {
        return Err(
            AppError::InvalidInput("设备ID、设备名称、访问地址不能为空".to_string()).into(),
        );
    }

    let updated = if let Some(existing) = outbound_connections::Entity::find_by_id(user_id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
    {
        let auth_changed = existing.ip != ip || existing.password != password;
        let mut am: outbound_connections::ActiveModel = existing.into();
        am.user_name = Set(user_name.clone());
        am.ip = Set(ip.clone());
        am.password = Set(password.clone());
        am.device_id = Set(device_id.clone());
        if auth_changed {
            am.auth_status = Set(0);
            am.auth_token = Set(None);
            am.last_connected_at = Set(None);
        }
        am.update(db).await.map_err(AppError::from)?
    } else {
        let am = outbound_connections::ActiveModel {
            user_id: Set(user_id.clone()),
            user_name: Set(user_name.clone()),
            ip: Set(ip.clone()),
            password: Set(password.clone()),
            device_id: Set(device_id.clone()),
            display_name: Set(None),
            auth_token: Set(None),
            auth_status: Set(0),
            last_connected_at: Set(None),
        };
        am.insert(db).await.map_err(AppError::from)?
    };

    crate::app::events::emit_connection_status_changed(
        &app,
        vec![updated.user_id.clone()],
        "remote_connection_saved",
    );

    Ok(RemoteShareUser {
        user_id: updated.user_id,
        user_name: updated.user_name,
        ip: updated.ip,
        password: updated.password,
        device_id: updated.device_id,
        auth_status: updated.auth_status,
        last_connected_at: updated.last_connected_at,
    })
}

#[tauri::command]
pub async fn update_remote_share_user_auth_status(
    app: tauri::AppHandle,
    payload: UpdateRemoteAuthStatusPayload,
) -> Result<RemoteShareUser, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let user_id = payload.user_id.trim().to_string();
    if user_id.is_empty() {
        return Err(AppError::InvalidInput("user_id 不能为空".to_string()).into());
    }
    if !(0..=4).contains(&payload.auth_status) {
        return Err(AppError::InvalidInput("无效的认证状态".to_string()).into());
    }

    let row = outbound_connections::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    let mut am: outbound_connections::ActiveModel = row.into();
    am.auth_status = Set(payload.auth_status);
    am.auth_token = Set(payload.auth_token);
    if payload.auth_status == 2 {
        am.last_connected_at = Set(Some(chrono::Utc::now().timestamp()));
    }
    let updated = am.update(db).await.map_err(AppError::from)?;
    crate::app::events::emit_connection_status_changed(
        &app,
        vec![updated.user_id.clone()],
        "remote_connection_status_changed",
    );

    Ok(RemoteShareUser {
        user_id: updated.user_id,
        user_name: updated.user_name,
        ip: updated.ip,
        password: updated.password,
        device_id: updated.device_id,
        auth_status: updated.auth_status,
        last_connected_at: updated.last_connected_at,
    })
}

#[tauri::command]
pub async fn list_inbound_connection_requests(
    app: tauri::AppHandle,
) -> Result<Vec<InboundConnectionRequest>, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let rows = inbound_connections::Entity::find()
        .filter(inbound_connections::Column::AuthStatus.eq(1))
        .order_by_desc(inbound_connections::Column::LastSeenAt)
        .all(db)
        .await
        .map_err(AppError::from)?;
    Ok(rows
        .into_iter()
        .map(|row| InboundConnectionRequest {
            user_id: row.user_id,
            user_name: row.user_name,
            ip: row.ip,
            device_id: row.device_id,
            auth_status: row.auth_status,
            last_seen_at: row.last_seen_at,
        })
        .collect())
}

#[tauri::command]
pub async fn set_inbound_connection_auth_status(
    app: tauri::AppHandle,
    payload: InboundAuthDecisionPayload,
) -> Result<InboundConnectionRequest, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    if payload.auth_status != 2 && payload.auth_status != 3 {
        return Err(AppError::InvalidInput("只能同意或拒绝连接请求".to_string()).into());
    }

    let row = inbound_connections::Entity::find_by_id(payload.user_id)
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    let now = chrono::Utc::now().timestamp();
    let mut am: inbound_connections::ActiveModel = row.into();
    am.auth_status = Set(payload.auth_status);
    if payload.auth_status == 2 {
        am.is_shared = Set(1);
        am.is_trusted = Set(1);
        am.granted_at = Set(Some(now));
        am.revoked_at = Set(None);
    } else {
        am.is_shared = Set(0);
        am.is_trusted = Set(0);
        am.revoked_at = Set(Some(now));
    }
    let updated = am.update(db).await.map_err(AppError::from)?;
    let pending_count = inbound_connections::Entity::find()
        .filter(inbound_connections::Column::AuthStatus.eq(1))
        .count(db)
        .await
        .map_err(AppError::from)?;
    crate::app::ui::tray::update_connect_device_menu_pending_count(&app, pending_count);
    crate::app::events::emit_connection_status_changed(
        &app,
        vec![updated.user_id.clone()],
        "inbound_connection_status_changed",
    );
    Ok(InboundConnectionRequest {
        user_id: updated.user_id,
        user_name: updated.user_name,
        ip: updated.ip,
        device_id: updated.device_id,
        auth_status: updated.auth_status,
        last_seen_at: updated.last_seen_at,
    })
}

#[tauri::command]
pub async fn list_web_access_requests(
    app: tauri::AppHandle,
) -> Result<Vec<WebAccessRequest>, ApiError> {
    let auth = app.state::<crate::server::web_auth::WebAuthState>();
    Ok(auth
        .pending_requests()
        .into_iter()
        .map(web_access_request_response)
        .collect())
}

#[tauri::command]
pub async fn set_web_access_request_auth_status(
    app: tauri::AppHandle,
    payload: WebAccessDecisionPayload,
) -> Result<WebAccessRequest, ApiError> {
    let config = app.state::<crate::app::config::AppConfigStore>().get();
    let scopes = match payload.scopes.as_deref() {
        Some(scopes) => Some(
            crate::server::web_auth::scopes_from_optional_names(Some(scopes), &config)
                .map_err(AppError::InvalidInput)?,
        ),
        None => None,
    };
    let auth = app.state::<crate::server::web_auth::WebAuthState>();
    let request = auth
        .decide_request(
            payload.id.trim(),
            payload.auth_status,
            scopes,
            config.web_access_cookie_ttl_seconds,
        )
        .map_err(AppError::InvalidInput)?;
    crate::app::events::emit_web_access_requested(
        &app,
        vec![request.id.clone()],
        "web_access_status_changed",
    );
    Ok(web_access_request_response(request))
}

fn web_access_request_response(
    request: crate::server::web_auth::WebAccessRequest,
) -> WebAccessRequest {
    WebAccessRequest {
        id: request.id,
        client_label: request.client_label,
        ip: request.ip,
        user_agent: request.user_agent,
        scopes: crate::server::web_auth::scope_names(&request.scopes),
        auth_status: request.auth_status,
        created_at: request.created_at,
        expires_at: request.expires_at,
    }
}

#[tauri::command]
pub async fn remove_remote_share_user(
    app: tauri::AppHandle,
    user_id: String,
) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let model = outbound_connections::Entity::find()
        .filter(outbound_connections::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(AppError::from)?;
    if let Some(model) = model {
        let id = model.user_id.clone();
        model.delete(db).await.map_err(AppError::from)?;
        crate::app::events::emit_connection_status_changed(
            &app,
            vec![id],
            "remote_connection_removed",
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn reveal_shared_clipboard_item(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = clipboard_record::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;

    let path = match row.r#type {
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let bytes = row
                .data
                .ok_or_else(|| AppError::InvalidInput("文件记录缺少数据".to_string()))?;
            let paths: Vec<String> = serde_json::from_slice(&bytes).map_err(AppError::from)?;
            paths
                .into_iter()
                .next()
                .ok_or_else(|| AppError::InvalidInput("文件记录为空".to_string()))?
        }
        t if t == ClipboardType::Image as i32 => {
            let bytes = row
                .data
                .ok_or_else(|| AppError::InvalidInput("图片记录缺少数据".to_string()))?;
            String::from_utf8(bytes).map_err(AppError::from)?
        }
        _ => return Err(AppError::InvalidInput("该记录不是文件/文件夹/图片".to_string()).into()),
    };

    let target = std::path::PathBuf::from(path);
    if !target.exists() {
        return Err(AppError::InvalidInput("目标路径不存在".to_string()).into());
    }

    reveal_in_file_manager(&target)?;
    Ok(())
}

#[tauri::command]
pub async fn reveal_local_shared_file(app: tauri::AppHandle, id: String) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = local_files::Entity::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    let target = std::path::PathBuf::from(row.path);
    if !target.exists() {
        return Err(AppError::InvalidInput("目标路径不存在".to_string()).into());
    }
    reveal_in_file_manager(&target)?;
    Ok(())
}

#[tauri::command]
pub async fn get_local_shared_file_thumbnail(
    app: tauri::AppHandle,
    id: String,
) -> Result<String, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = local_files::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    if row.is_valid != 1 || row.r#type != 2 {
        return Err(AppError::InvalidInput("该共享项不是有效图片".to_string()).into());
    }

    let target = std::path::PathBuf::from(row.path);
    if !target.is_file() {
        return Err(AppError::InvalidInput("目标图片不存在".to_string()).into());
    }

    let image_data = std::fs::read(&target).map_err(AppError::from)?;
    let thumbnail = generate_image_thumbnail(&image_data, 48)
        .map_err(|e| AppError::InvalidInput(format!("生成缩略图失败: {e}")))?;
    Ok(format!("data:image/jpeg;base64,{thumbnail}"))
}

#[tauri::command]
pub async fn unshare_local_shared_file(app: tauri::AppHandle, id: String) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = local_files::Entity::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    let mut am: local_files::ActiveModel = row.clone().into();
    am.is_valid = Set(0);
    am.source_clipboard_id = Set(None);
    am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
    am.update(db).await.map_err(AppError::from)?;
    mark_local_share_index_removed(db, &id).await?;
    crate::app::events::emit_local_files_changed(&app, vec![id], "local_file_unshared");
    Ok(())
}

async fn mark_local_share_index_removed(
    db: &sea_orm::DatabaseConnection,
    local_file_id: &str,
) -> Result<(), ApiError> {
    let rows = local_file_index::Entity::find()
        .filter(local_file_index::Column::LocalFileId.eq(local_file_id.to_string()))
        .all(db)
        .await
        .map_err(AppError::from)?;

    for row in rows {
        let mut am: local_file_index::ActiveModel = row.into();
        am.exists_flag = Set(0);
        am.dirty = Set(0);
        am.update(db).await.map_err(AppError::from)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn add_manual_shared_paths(
    app: tauri::AppHandle,
    payload: AddManualSharedPathsPayload,
) -> Result<usize, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let mut added = 0usize;
    let mut changed_ids = Vec::new();
    for raw in payload.paths {
        let normalized = normalize_file_uri(raw.trim().trim_matches('"')).to_string();
        let path = std::path::PathBuf::from(normalized);
        if !path.exists() {
            continue;
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let path_text = canonical.to_string_lossy().to_string();
        let metadata = std::fs::metadata(&canonical).ok();
        let size = metadata.as_ref().and_then(|m| {
            if m.is_file() {
                Some(m.len() as i64)
            } else {
                None
            }
        });
        let file_type = if canonical.is_dir() {
            1
        } else if canonical
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
                )
            })
            .unwrap_or(false)
        {
            2
        } else {
            0
        };

        let existed = local_files::Entity::find()
            .filter(local_files::Column::Path.eq(path_text.clone()))
            .one(db)
            .await
            .map_err(AppError::from)?;
        if let Some(existing) = existed {
            let existing_id = existing.id.clone();
            let source_type = source_type_after_adding_direct(existing.source_type);
            let clipboard_ids = parse_source_clipboard_ids(existing.source_clipboard_id.as_deref());
            let mut am: local_files::ActiveModel = existing.into();
            am.is_valid = Set(1);
            am.source_type = Set(source_type);
            if clipboard_ids.is_empty() {
                am.source_clipboard_id = Set(None);
            }
            am.size = Set(size);
            am.r#type = Set(file_type);
            am.share_mode = Set(SHARE_MODE_MANUAL);
            am.expires_at = Set(None);
            am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
            am.update(db).await.map_err(AppError::from)?;
            changed_ids.push(existing_id);
        } else {
            let now = chrono::Utc::now().timestamp();
            let id = uuid::Uuid::new_v4().to_string();
            let am = local_files::ActiveModel {
                id: Set(id.clone()),
                path: Set(path_text),
                r#type: Set(file_type),
                created_at: Set(now),
                access_count: Set(0),
                is_valid: Set(1),
                size: Set(size),
                source_clipboard_id: Set(None),
                source_type: Set(SOURCE_DIRECT),
                is_favorite: Set(0),
                share_mode: Set(SHARE_MODE_MANUAL),
                expires_at: Set(None),
                updated_at: Set(Some(now)),
            };
            am.insert(db).await.map_err(AppError::from)?;
            changed_ids.push(id);
        }
        added += 1;
    }
    if !changed_ids.is_empty() {
        crate::app::events::emit_local_files_changed(&app, changed_ids, "manual_share_changed");
    }
    Ok(added)
}

#[tauri::command]
pub async fn refresh_local_share_indexes(app: tauri::AppHandle) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    crate::server::sync::scan_local_shares_once(db)
        .await
        .map_err(AppError::InvalidInput)?;
    Ok(())
}

#[tauri::command]
pub async fn get_remote_cache_status(
    app: tauri::AppHandle,
    payload: RemoteCacheStatusPayload,
) -> Result<RemoteCacheStatus, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    let row = shared_file_index::Entity::find_by_id((
        payload.remote_user_id.trim().to_string(),
        payload.share_id.trim().to_string(),
        relative_path,
    ))
    .one(db)
    .await
    .map_err(AppError::from)?;

    let Some(row) = row else {
        return Ok(RemoteCacheStatus {
            cached: false,
            local_cache_path: None,
            size: None,
            mtime: None,
            hash: None,
            updated_at: None,
        });
    };

    let local_cache_path = row.local_cache_path.clone();
    let path_exists = local_cache_path
        .as_deref()
        .map(Path::new)
        .map(|path| path.exists())
        .unwrap_or(false);
    let size_matches = match (payload.size, row.size) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        _ => true,
    };
    let mtime_matches = match (payload.mtime, row.mtime) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        _ => true,
    };
    let hash_matches = payload
        .hash
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|expected| row.hash.as_deref() == Some(expected))
        .unwrap_or(false);
    let remote_meta_matches = if payload
        .hash
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        hash_matches
    } else {
        size_matches && mtime_matches
    };

    Ok(RemoteCacheStatus {
        cached: row.cache_status == 2
            && row.remote_deleted == 0
            && path_exists
            && remote_meta_matches,
        local_cache_path,
        size: row.size,
        mtime: row.mtime,
        hash: row.hash,
        updated_at: row.updated_at,
    })
}

#[tauri::command]
pub async fn list_remote_cached_files(
    app: tauri::AppHandle,
    payload: ListRemoteCachedFilesPayload,
) -> Result<Vec<RemoteCachedFileItem>, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let remote_user_id = payload.remote_user_id.trim().to_string();
    if remote_user_id.is_empty() {
        return Err(AppError::InvalidInput("远程用户不能为空".to_string()).into());
    }

    let rows = shared_file_index::Entity::find()
        .filter(shared_file_index::Column::UserId.eq(remote_user_id.clone()))
        .filter(shared_file_index::Column::CacheStatus.eq(2))
        .all(db)
        .await
        .map_err(AppError::from)?;

    if let Some(share_id) = payload
        .share_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let parent = normalize_remote_relative_path(payload.path.as_deref().unwrap_or("."))?;
        let mut items = rows
            .iter()
            .filter(|row| row.shared_file_id == share_id)
            .filter(|row| is_direct_remote_child(&parent, &row.relative_path))
            .map(|row| remote_cached_file_item(&remote_user_id, row, &rows))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| {
            a.is_dir
                .cmp(&b.is_dir)
                .reverse()
                .then_with(|| a.name.cmp(&b.name))
        });
        return Ok(items);
    }

    let mut roots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut sorted_rows = rows.iter().collect::<Vec<_>>();
    sorted_rows.sort_by(|a, b| {
        let depth_a = remote_path_depth(&a.relative_path);
        let depth_b = remote_path_depth(&b.relative_path);
        depth_a
            .cmp(&depth_b)
            .then_with(|| a.shared_file_id.cmp(&b.shared_file_id))
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });

    for row in sorted_rows {
        if !seen.insert(row.shared_file_id.clone()) {
            continue;
        }
        roots.push(remote_cached_file_item(&remote_user_id, row, &rows));
    }
    roots.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(roots)
}

#[tauri::command]
pub async fn validate_empty_directory(
    payload: ValidateEmptyDirectoryPayload,
) -> Result<(), ApiError> {
    let path = PathBuf::from(payload.path.trim());
    ensure_empty_directory(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn cache_remote_shared_file(
    app: tauri::AppHandle,
    payload: CacheRemoteSharedFilePayload,
) -> Result<String, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let config = app.state::<crate::app::config::AppConfigStore>().get();
    let remote_user_id = payload.remote_user_id.trim().to_string();
    let share_id = payload.share_id.trim().to_string();
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    if remote_user_id.is_empty() || share_id.is_empty() {
        return Err(AppError::InvalidInput("远程用户和共享 ID 不能为空".to_string()).into());
    }

    let cache_path = remote_cache_path_for_payload(
        &config.remote_cache_dir,
        &remote_user_id,
        &share_id,
        payload.share_name.trim(),
        &relative_path,
        payload.name.trim(),
        payload.is_dir,
        payload.destination_dir.as_deref(),
        payload.destination_root_relative_path.as_deref(),
    )
    .await?;

    if payload.is_dir {
        tokio::fs::create_dir_all(&cache_path)
            .await
            .map_err(AppError::from)?;
    } else {
        let data = payload
            .data_base64
            .as_deref()
            .ok_or_else(|| AppError::InvalidInput("缓存文件缺少内容".to_string()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.as_bytes())
            .map_err(|e| AppError::InvalidInput(format!("文件内容解码失败: {e}")))?;
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(AppError::from)?;
        }
        tokio::fs::write(&cache_path, bytes)
            .await
            .map_err(AppError::from)?;
    }

    let path_text = cache_path.to_string_lossy().to_string();
    upsert_remote_cache_index(
        db,
        &remote_user_id,
        &share_id,
        relative_path.clone(),
        payload.name,
        payload.is_dir,
        Some(path_text.clone()),
        payload.size,
        payload.mtime,
        payload.hash,
    )
    .await?;

    crate::app::events::emit_shared_file_index_changed(
        &app,
        vec![format!("{remote_user_id}:{share_id}:{relative_path}")],
        "remote_cache_updated",
    );
    Ok(path_text)
}

#[tauri::command]
pub async fn download_remote_shared_file(
    app: tauri::AppHandle,
    payload: DownloadRemoteSharedFilePayload,
) -> Result<String, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let config = app.state::<crate::app::config::AppConfigStore>().get();
    let remote_user_id = payload.remote_user_id.trim().to_string();
    let share_id = payload.share_id.trim().to_string();
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    let share_name = payload.share_name.trim().to_string();
    let name = payload.name.trim().to_string();
    if remote_user_id.is_empty() || share_id.is_empty() {
        return Err(AppError::InvalidInput("远程用户和共享 ID 不能为空".to_string()).into());
    }
    if name.is_empty() {
        return Err(AppError::InvalidInput("远程文件名不能为空".to_string()).into());
    }

    let cache_path = remote_cache_path_for_payload(
        &config.remote_cache_dir,
        &remote_user_id,
        &share_id,
        &share_name,
        &relative_path,
        &name,
        false,
        payload.destination_dir.as_deref(),
        payload.destination_root_relative_path.as_deref(),
    )
    .await?;
    download_remote_file_to_path(&app, &payload, &share_id, &relative_path, &cache_path).await?;

    let actual_size = tokio::fs::metadata(&cache_path)
        .await
        .map(|metadata| metadata.len() as i64)
        .ok();
    let size = payload.size.or(actual_size);
    let path_text = cache_path.to_string_lossy().to_string();
    upsert_remote_cache_index(
        db,
        &remote_user_id,
        &share_id,
        relative_path.clone(),
        name,
        false,
        Some(path_text.clone()),
        size,
        payload.mtime,
        payload.hash,
    )
    .await?;

    crate::app::events::emit_shared_file_index_changed(
        &app,
        vec![format!("{remote_user_id}:{share_id}:{relative_path}")],
        "remote_cache_updated",
    );
    Ok(path_text)
}

#[tauri::command]
pub async fn reveal_remote_shared_cache(
    app: tauri::AppHandle,
    payload: RemoteCacheTargetPayload,
) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    let row = shared_file_index::Entity::find_by_id((
        payload.remote_user_id.trim().to_string(),
        payload.share_id.trim().to_string(),
        relative_path,
    ))
    .one(db)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::InvalidInput("该远程项还没有缓存，请先同步".to_string()))?;

    let path = row
        .local_cache_path
        .map(PathBuf::from)
        .ok_or_else(|| AppError::InvalidInput("该远程项还没有缓存，请先同步".to_string()))?;
    if !path.exists() {
        return Err(AppError::InvalidInput("缓存路径不存在，请重新同步".to_string()).into());
    }
    reveal_in_file_manager(&path)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_remote_shared_cache(
    app: tauri::AppHandle,
    payload: RemoteCacheTargetPayload,
) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let config = app.state::<crate::app::config::AppConfigStore>().get();
    let remote_user_id = payload.remote_user_id.trim().to_string();
    let share_id = payload.share_id.trim().to_string();
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    if remote_user_id.is_empty() || share_id.is_empty() {
        return Err(AppError::InvalidInput("远程用户和共享 ID 不能为空".to_string()).into());
    }

    let rows = shared_file_index::Entity::find()
        .filter(shared_file_index::Column::UserId.eq(remote_user_id.clone()))
        .filter(shared_file_index::Column::SharedFileId.eq(share_id.clone()))
        .all(db)
        .await
        .map_err(AppError::from)?;
    let targets = rows
        .into_iter()
        .filter(|row| remote_cache_row_in_scope(&relative_path, &row.relative_path))
        .collect::<Vec<_>>();

    let cache_root = resolve_cache_root(&config.remote_cache_dir);
    let mut paths = targets
        .iter()
        .filter_map(|row| row.local_cache_path.as_deref())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    paths.sort_by(|a, b| b.components().count().cmp(&a.components().count()));
    paths.dedup();

    for path in paths {
        remove_cached_path(&cache_root, &path).await?;
    }

    for row in targets {
        row.delete(db).await.map_err(AppError::from)?;
    }

    crate::app::events::emit_shared_file_index_changed(
        &app,
        vec![format!("{remote_user_id}:{share_id}:{relative_path}")],
        "remote_cache_removed",
    );
    Ok(())
}

#[tauri::command]
pub async fn move_remote_shared_cache(
    app: tauri::AppHandle,
    payload: MoveRemoteCachePayload,
) -> Result<String, ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let remote_user_id = payload.remote_user_id.trim().to_string();
    let share_id = payload.share_id.trim().to_string();
    let relative_path = normalize_remote_relative_path(&payload.relative_path)?;
    if remote_user_id.is_empty() || share_id.is_empty() {
        return Err(AppError::InvalidInput("远程用户和共享 ID 不能为空".to_string()).into());
    }

    let destination_root = PathBuf::from(payload.destination_dir.trim());
    ensure_empty_directory(&destination_root).await?;

    let rows = shared_file_index::Entity::find()
        .filter(shared_file_index::Column::UserId.eq(remote_user_id.clone()))
        .filter(shared_file_index::Column::SharedFileId.eq(share_id.clone()))
        .all(db)
        .await
        .map_err(AppError::from)?;
    let mut targets = rows
        .into_iter()
        .filter(|row| remote_cache_row_in_scope(&relative_path, &row.relative_path))
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Err(AppError::InvalidInput("该远程项还没有缓存，请先同步".to_string()).into());
    }
    targets.sort_by_key(|row| remote_path_depth(&row.relative_path));

    let root_row = targets
        .iter()
        .find(|row| row.relative_path == relative_path)
        .ok_or_else(|| AppError::InvalidInput("该远程项还没有缓存，请先同步".to_string()))?;
    let source_root = root_row
        .local_cache_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::InvalidInput("缓存路径不存在，请重新同步".to_string()))?;
    if !source_root.exists() {
        return Err(AppError::InvalidInput("缓存路径不存在，请重新同步".to_string()).into());
    }

    let source_root_canonical = std::fs::canonicalize(&source_root).map_err(AppError::from)?;
    if source_root_canonical == std::fs::canonicalize(&destination_root).map_err(AppError::from)? {
        return Err(AppError::InvalidInput("请选择不同的缓存位置".to_string()).into());
    }
    let root_destination = destination_cache_path(
        &destination_root,
        &root_row.relative_path,
        &root_row.name,
        root_row.is_dir == 1,
        Some(&relative_path),
    )?;
    move_cached_path(&source_root, &root_destination).await?;

    let now = chrono::Utc::now().timestamp();
    let mut changed_ids = Vec::new();
    for row in targets {
        let row_relative_path = row.relative_path.clone();
        let new_path = destination_cache_path(
            &destination_root,
            &row_relative_path,
            &row.name,
            row.is_dir == 1,
            Some(&relative_path),
        )?;
        let mut am: shared_file_index::ActiveModel = row.into();
        am.local_cache_path = Set(Some(new_path.to_string_lossy().to_string()));
        am.last_accessed_at = Set(Some(now));
        am.updated_at = Set(Some(now));
        am.update(db).await.map_err(AppError::from)?;
        changed_ids.push(format!("{remote_user_id}:{share_id}:{row_relative_path}"));
    }

    crate::app::events::emit_shared_file_index_changed(&app, changed_ids, "remote_cache_moved");
    Ok(destination_root.to_string_lossy().to_string())
}

fn normalize_remote_relative_path(path: &str) -> Result<String, AppError> {
    let value = path.trim().replace('\\', "/");
    if value.is_empty() || value == "." {
        return Ok(".".to_string());
    }
    let value = value.trim_matches('/');
    if value.is_empty() {
        return Ok(".".to_string());
    }
    let mut parts = Vec::new();
    for part in value.split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(AppError::InvalidInput("远程路径不合法".to_string()));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(parts.join("/"))
    }
}

fn remote_cached_file_item(
    remote_user_id: &str,
    row: &shared_file_index::Model,
    rows: &[shared_file_index::Model],
) -> RemoteCachedFileItem {
    let share_name = rows
        .iter()
        .find(|item| item.shared_file_id == row.shared_file_id && item.relative_path == ".")
        .map(|item| item.name.clone())
        .unwrap_or_else(|| row.name.clone());

    RemoteCachedFileItem {
        remote_user_id: remote_user_id.to_string(),
        share_id: row.shared_file_id.clone(),
        share_name,
        relative_path: row.relative_path.clone(),
        name: row.name.clone(),
        is_dir: row.is_dir == 1,
        size: row.size,
        mtime: row.mtime,
        hash: row.hash.clone(),
        local_cache_path: row.local_cache_path.clone(),
        remote_deleted: row.remote_deleted == 1,
        cache_status: row.cache_status,
        updated_at: row.updated_at,
    }
}

fn remote_path_depth(path: &str) -> usize {
    if path == "." || path.trim().is_empty() {
        0
    } else {
        path.split('/')
            .filter(|part| !part.trim().is_empty())
            .count()
    }
}

fn is_direct_remote_child(parent: &str, child: &str) -> bool {
    let parent = normalize_remote_relative_path(parent).unwrap_or_else(|_| ".".to_string());
    let child = normalize_remote_relative_path(child).unwrap_or_else(|_| ".".to_string());
    if parent == "." {
        return child != "." && !child.contains('/');
    }
    let prefix = format!("{}/", parent.trim_end_matches('/'));
    let Some(rest) = child.strip_prefix(&prefix) else {
        return false;
    };
    !rest.is_empty() && !rest.contains('/')
}

fn remote_cache_row_in_scope(parent: &str, child: &str) -> bool {
    let parent = normalize_remote_relative_path(parent).unwrap_or_else(|_| ".".to_string());
    let child = normalize_remote_relative_path(child).unwrap_or_else(|_| ".".to_string());
    if parent == "." {
        return true;
    }
    child == parent || child.starts_with(&format!("{}/", parent.trim_end_matches('/')))
}

async fn download_remote_file_to_path(
    app: &tauri::AppHandle,
    payload: &DownloadRemoteSharedFilePayload,
    share_id: &str,
    relative_path: &str,
    cache_path: &Path,
) -> Result<(), ApiError> {
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AppError::from)?;
    }

    let url = remote_download_url(&payload.base_url, share_id, relative_path)?;
    let client = reqwest::Client::new();
    let mut response = client
        .get(url)
        .header("x-share-clip-user-id", payload.auth_user_id.trim())
        .header("x-share-clip-device-id", payload.auth_device_id.trim())
        .send()
        .await
        .map_err(|e| AppError::InvalidInput(format!("下载远程文件失败: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        let detail = response
            .text()
            .await
            .unwrap_or_default()
            .trim()
            .replace(char::is_whitespace, " ");
        let detail = parse_remote_error_detail(&detail);
        return Err(AppError::InvalidInput(format!(
            "下载远程文件失败: HTTP {}{}",
            status.as_u16(),
            if detail.is_empty() {
                String::new()
            } else {
                format!(" ({detail})")
            }
        ))
        .into());
    }

    let total = response
        .content_length()
        .and_then(|value| i64::try_from(value).ok())
        .or(payload.size);
    let mut loaded = 0i64;
    let temp_path = remote_download_temp_path(cache_path);
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(AppError::from)?;

    emit_remote_download_progress(app, payload, relative_path, loaded, total, false);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::InvalidInput(format!("读取远程文件失败: {e}")))?
    {
        file.write_all(&chunk).await.map_err(AppError::from)?;
        loaded = loaded.saturating_add(chunk.len() as i64);
        emit_remote_download_progress(app, payload, relative_path, loaded, total, false);
    }
    file.flush().await.map_err(AppError::from)?;
    drop(file);
    if cache_path.exists() {
        tokio::fs::remove_file(cache_path)
            .await
            .map_err(AppError::from)?;
    }
    tokio::fs::rename(&temp_path, cache_path)
        .await
        .map_err(AppError::from)?;
    emit_remote_download_progress(
        app,
        payload,
        relative_path,
        loaded,
        total.or(Some(loaded)),
        true,
    );
    Ok(())
}

async fn remote_cache_path_for_payload(
    configured_root: &str,
    remote_user_id: &str,
    share_id: &str,
    share_name: &str,
    relative_path: &str,
    item_name: &str,
    is_dir: bool,
    destination_dir: Option<&str>,
    destination_root_relative_path: Option<&str>,
) -> Result<PathBuf, ApiError> {
    if let Some(destination_dir) = destination_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let destination_root = PathBuf::from(destination_dir);
        if should_validate_destination_root(relative_path, destination_root_relative_path)? {
            ensure_empty_directory(&destination_root).await?;
        }
        return destination_cache_path(
            &destination_root,
            relative_path,
            item_name,
            is_dir,
            destination_root_relative_path,
        );
    }

    remote_cache_path(
        configured_root,
        remote_user_id,
        share_id,
        share_name,
        relative_path,
        item_name,
        is_dir,
    )
}

fn should_validate_destination_root(
    relative_path: &str,
    destination_root_relative_path: Option<&str>,
) -> Result<bool, ApiError> {
    let root_relative_path = destination_root_relative_path
        .map(normalize_remote_relative_path)
        .transpose()?
        .unwrap_or_else(|| relative_path.to_string());
    Ok(normalize_remote_relative_path(relative_path)? == root_relative_path)
}

fn destination_cache_path(
    destination_root: &Path,
    relative_path: &str,
    item_name: &str,
    is_dir: bool,
    destination_root_relative_path: Option<&str>,
) -> Result<PathBuf, ApiError> {
    let root_relative_path = destination_root_relative_path
        .map(normalize_remote_relative_path)
        .transpose()?
        .unwrap_or_else(|| relative_path.to_string());
    let relative_path = normalize_remote_relative_path(relative_path)?;

    if relative_path == root_relative_path {
        if is_dir {
            return Ok(destination_root.to_path_buf());
        }
        return Ok(destination_root.join(safe_path_segment(item_name, "download.bin")));
    }

    if root_relative_path != "." {
        let prefix = format!("{}/", root_relative_path.trim_end_matches('/'));
        let Some(rest) = relative_path.strip_prefix(&prefix) else {
            return Err(AppError::InvalidInput("目标路径不在选择的远程目录内".to_string()).into());
        };
        let mut target = destination_root.to_path_buf();
        append_safe_relative_path(&mut target, rest)?;
        return Ok(target);
    }

    let mut target = destination_root.to_path_buf();
    append_safe_relative_path(&mut target, &relative_path)?;
    Ok(target)
}

async fn ensure_empty_directory(path: &Path) -> Result<(), ApiError> {
    if path.as_os_str().is_empty() {
        return Err(AppError::InvalidInput("请选择有效的目标文件夹".to_string()).into());
    }
    let metadata = tokio::fs::metadata(path).await.map_err(AppError::from)?;
    if !metadata.is_dir() {
        return Err(AppError::InvalidInput("请选择文件夹作为目标位置".to_string()).into());
    }
    let mut entries = tokio::fs::read_dir(path).await.map_err(AppError::from)?;
    if entries
        .next_entry()
        .await
        .map_err(AppError::from)?
        .is_some()
    {
        return Err(AppError::InvalidInput("目标文件夹必须为空".to_string()).into());
    }
    Ok(())
}

async fn upsert_remote_cache_index(
    db: &sea_orm::DatabaseConnection,
    remote_user_id: &str,
    share_id: &str,
    relative_path: String,
    name: String,
    is_dir: bool,
    local_cache_path: Option<String>,
    size: Option<i64>,
    mtime: Option<i64>,
    hash: Option<String>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    let existing = shared_file_index::Entity::find_by_id((
        remote_user_id.to_string(),
        share_id.to_string(),
        relative_path.clone(),
    ))
    .one(db)
    .await
    .map_err(AppError::from)?;

    if let Some(row) = existing {
        let mut am: shared_file_index::ActiveModel = row.into();
        am.name = Set(name);
        am.is_dir = Set(if is_dir { 1 } else { 0 });
        am.local_cache_path = Set(local_cache_path);
        am.size = Set(size);
        am.mtime = Set(mtime);
        am.hash = Set(hash);
        am.remote_deleted = Set(0);
        am.cache_status = Set(2);
        am.last_accessed_at = Set(Some(now));
        am.updated_at = Set(Some(now));
        am.update(db).await.map_err(AppError::from)?;
    } else {
        let am = shared_file_index::ActiveModel {
            user_id: Set(remote_user_id.to_string()),
            shared_file_id: Set(share_id.to_string()),
            relative_path: Set(relative_path),
            name: Set(name),
            is_dir: Set(if is_dir { 1 } else { 0 }),
            local_cache_path: Set(local_cache_path),
            size: Set(size),
            mtime: Set(mtime),
            hash: Set(hash),
            remote_deleted: Set(0),
            cache_status: Set(2),
            last_accessed_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        };
        am.insert(db).await.map_err(AppError::from)?;
    }
    Ok(())
}

fn remote_download_url(
    base_url: &str,
    share_id: &str,
    relative_path: &str,
) -> Result<String, ApiError> {
    let mut url = reqwest::Url::parse(&normalized_remote_base_url(base_url))
        .map_err(|e| AppError::InvalidInput(format!("远程地址不合法: {e}")))?;
    let base_path = url.path().trim_end_matches('/').to_string();
    url.set_path(&format!(
        "{base_path}/api/client/shares/{}/download",
        encode_uri_component(share_id)
    ));
    url.query_pairs_mut().append_pair("path", relative_path);
    Ok(url.to_string())
}

fn normalized_remote_base_url(raw: &str) -> String {
    let value = raw.trim().trim_end_matches('/');
    if value.to_ascii_lowercase().starts_with("http://")
        || value.to_ascii_lowercase().starts_with("https://")
    {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

fn encode_uri_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn parse_remote_error_detail(detail: &str) -> String {
    if detail.is_empty() {
        return String::new();
    }
    serde_json::from_str::<serde_json::Value>(detail)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .or_else(|| value.get("message"))
                .and_then(|inner| inner.as_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| detail.chars().take(120).collect())
}

fn remote_download_temp_path(cache_path: &Path) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "download".into());
    cache_path.with_file_name(format!("{}.{}.download", file_name, uuid::Uuid::new_v4()))
}

fn emit_remote_download_progress(
    app: &tauri::AppHandle,
    payload: &DownloadRemoteSharedFilePayload,
    relative_path: &str,
    loaded: i64,
    total: Option<i64>,
    finished: bool,
) {
    let Some(transfer_task_id) = payload
        .transfer_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let progress = total
        .filter(|value| *value > 0)
        .map(|value| (loaded as f64 / value as f64) * 100.0)
        .unwrap_or(if finished { 100.0 } else { 0.0 })
        .clamp(0.0, 100.0);
    let _ = app.emit(
        REMOTE_DOWNLOAD_PROGRESS_EVENT,
        RemoteDownloadProgressPayload {
            transfer_task_id: transfer_task_id.to_string(),
            relative_path: relative_path.to_string(),
            loaded,
            total,
            progress,
        },
    );
}

async fn remove_cached_path(cache_root: &Path, path: &Path) -> Result<(), ApiError> {
    if !path.exists() {
        return Ok(());
    }
    let canonical_root =
        std::fs::canonicalize(cache_root).unwrap_or_else(|_| cache_root.to_path_buf());
    let canonical_path = std::fs::canonicalize(path).map_err(AppError::from)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(AppError::InvalidInput("缓存路径不在远程缓存目录内".to_string()).into());
    }

    let metadata = tokio::fs::metadata(&canonical_path)
        .await
        .map_err(AppError::from)?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(&canonical_path)
            .await
            .map_err(AppError::from)?;
    } else {
        tokio::fs::remove_file(&canonical_path)
            .await
            .map_err(AppError::from)?;
    }
    Ok(())
}

async fn move_cached_path(source: &Path, destination: &Path) -> Result<(), ApiError> {
    if !source.exists() {
        return Err(AppError::InvalidInput("缓存路径不存在，请重新同步".to_string()).into());
    }
    let source_metadata = tokio::fs::metadata(source).await.map_err(AppError::from)?;
    if source_metadata.is_dir() {
        if destination.exists() {
            ensure_empty_directory(destination).await?;
        } else {
            tokio::fs::create_dir_all(destination)
                .await
                .map_err(AppError::from)?;
        }
    } else {
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(AppError::from)?;
        }
        if destination.exists() {
            return Err(AppError::InvalidInput("目标文件已存在".to_string()).into());
        }
    }

    match tokio::fs::rename(source, destination).await {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            log::warn!("rename cache failed, fallback to copy: {rename_error}");
            copy_path_recursive(source, destination).await?;
            remove_path_recursive(source).await?;
            Ok(())
        }
    }
}

async fn copy_path_recursive(source: &Path, destination: &Path) -> Result<(), ApiError> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if source.is_dir() {
            copy_dir_recursive_blocking(&source, &destination)
        } else {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source, &destination)?;
            Ok(())
        }
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("copy cache task failed: {e}")))?
    .map_err(AppError::from)?;
    Ok(())
}

fn copy_dir_recursive_blocking(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive_blocking(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

async fn remove_path_recursive(path: &Path) -> Result<(), ApiError> {
    let metadata = tokio::fs::metadata(path).await.map_err(AppError::from)?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(AppError::from)?;
    } else {
        tokio::fs::remove_file(path).await.map_err(AppError::from)?;
    }
    Ok(())
}

fn remote_cache_path(
    configured_root: &str,
    remote_user_id: &str,
    share_id: &str,
    share_name: &str,
    relative_path: &str,
    item_name: &str,
    is_dir: bool,
) -> Result<PathBuf, ApiError> {
    let root = resolve_cache_root(configured_root)
        .join(safe_path_segment(remote_user_id, "remote"))
        .join(safe_path_segment(share_id, "share"));

    if relative_path == "." && !is_dir {
        return Ok(root.join(safe_path_segment(item_name, "download.bin")));
    }

    let mut target = root.join(safe_path_segment(share_name, "share"));
    if relative_path != "." {
        append_safe_relative_path(&mut target, relative_path)?;
    }
    Ok(target)
}

fn resolve_cache_root(configured_root: &str) -> PathBuf {
    let trimmed = configured_root.trim();
    let configured_path = if trimmed.is_empty() {
        PathBuf::from("remote")
    } else {
        PathBuf::from(trimmed)
    };
    if configured_path.is_absolute() {
        return configured_path;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join(&configured_path)))
        .unwrap_or(configured_path)
}

fn append_safe_relative_path(target: &mut PathBuf, relative_path: &str) -> Result<(), ApiError> {
    for part in relative_path.split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(AppError::InvalidInput("远程路径不合法".to_string()).into());
        }
        target.push(safe_path_segment(part, "item"));
    }
    Ok(())
}

fn safe_path_segment(value: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.trim().chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let out = out.trim().trim_end_matches([' ', '.']);
    if out.is_empty() {
        fallback.to_string()
    } else {
        out.to_string()
    }
}

fn reveal_in_file_manager(target: &Path) -> Result<(), ApiError> {
    #[cfg(target_os = "windows")]
    {
        if target.is_file() {
            Command::new("explorer")
                .args(["/select,", &target.to_string_lossy()])
                .spawn()
                .map_err(AppError::from)?;
        } else {
            Command::new("explorer")
                .arg(target.to_string_lossy().to_string())
                .spawn()
                .map_err(AppError::from)?;
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(target.to_string_lossy().to_string())
            .spawn()
            .map_err(AppError::from)?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let open_target = if target.is_file() {
            target.parent().unwrap_or(target)
        } else {
            target
        };
        Command::new("xdg-open")
            .arg(open_target.to_string_lossy().to_string())
            .spawn()
            .map_err(AppError::from)?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err(AppError::InvalidInput("当前平台不支持".to_string()).into())
}
