use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;

use crate::db::service::local_files::{
    parse_source_clipboard_ids, source_type_after_adding_direct, source_type_after_removing_direct,
    SHARE_MODE_MANUAL, SHARE_MODE_TEMP, SOURCE_DIRECT,
};
use crate::entity::clipboard_record;
use crate::entity::inbound_connections;
use crate::entity::local_files;
use crate::entity::outbound_connections;
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
pub struct LocalSharedFileItem {
    pub id: String,
    pub path: String,
    pub r#type: i32,
    pub size: Option<i64>,
    pub created_at: i64,
    pub source_type: i32,
    pub source_clipboard_id: Option<String>,
    pub share_mode: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpsertRemoteShareUserPayload {
    pub user_id: String,
    pub user_name: String,
    pub ip: String,
    pub password: Option<String>,
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
            share_mode: row.share_mode,
        })
        .collect())
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
    if user_id.is_empty() || user_name.is_empty() || ip.is_empty() {
        return Err(AppError::InvalidInput("user_id/user_name/ip 不能为空".to_string()).into());
    }

    let updated = if let Some(existing) = outbound_connections::Entity::find_by_id(user_id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
    {
        let auth_changed = existing.ip != ip || existing.password != password;
        let device_id = existing
            .device_id
            .clone()
            .or_else(|| Some(uuid::Uuid::new_v4().to_string()));
        let mut am: outbound_connections::ActiveModel = existing.into();
        am.user_name = Set(user_name.clone());
        am.ip = Set(ip.clone());
        am.password = Set(password.clone());
        am.device_id = Set(device_id);
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
            device_id: Set(Some(uuid::Uuid::new_v4().to_string())),
            display_name: Set(None),
            auth_token: Set(None),
            auth_status: Set(0),
            last_connected_at: Set(None),
        };
        am.insert(db).await.map_err(AppError::from)?
    };

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
        model.delete(db).await.map_err(AppError::from)?;
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
    if let Some(next_source_type) = source_type_after_removing_direct(row.source_type) {
        let ids = parse_source_clipboard_ids(row.source_clipboard_id.as_deref());
        if ids.is_empty() {
            am.is_valid = Set(0);
            am.source_clipboard_id = Set(None);
        } else {
            am.source_type = Set(next_source_type);
            am.share_mode = Set(SHARE_MODE_TEMP);
        }
    } else {
        am.is_valid = Set(0);
        am.source_clipboard_id = Set(None);
    }
    am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
    am.update(db).await.map_err(AppError::from)?;
    crate::app::events::emit_local_files_changed(&app, vec![id], "local_file_unshared");
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

fn reveal_in_file_manager(target: &std::path::Path) -> Result<(), ApiError> {
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
