use base64::Engine;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Manager;

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

    let cache_path = remote_cache_path(
        &config.remote_cache_dir,
        &remote_user_id,
        &share_id,
        payload.share_name.trim(),
        &relative_path,
        payload.name.trim(),
        payload.is_dir,
    )?;

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

    let now = chrono::Utc::now().timestamp();
    let path_text = cache_path.to_string_lossy().to_string();
    let existing = shared_file_index::Entity::find_by_id((
        remote_user_id.clone(),
        share_id.clone(),
        relative_path.clone(),
    ))
    .one(db)
    .await
    .map_err(AppError::from)?;

    if let Some(row) = existing {
        let mut am: shared_file_index::ActiveModel = row.into();
        am.name = Set(payload.name);
        am.is_dir = Set(if payload.is_dir { 1 } else { 0 });
        am.local_cache_path = Set(Some(path_text.clone()));
        am.size = Set(payload.size);
        am.mtime = Set(payload.mtime);
        am.hash = Set(payload.hash);
        am.remote_deleted = Set(0);
        am.cache_status = Set(2);
        am.last_accessed_at = Set(Some(now));
        am.updated_at = Set(Some(now));
        am.update(db).await.map_err(AppError::from)?;
    } else {
        let am = shared_file_index::ActiveModel {
            user_id: Set(remote_user_id.clone()),
            shared_file_id: Set(share_id.clone()),
            relative_path: Set(relative_path.clone()),
            name: Set(payload.name),
            is_dir: Set(if payload.is_dir { 1 } else { 0 }),
            local_cache_path: Set(Some(path_text.clone())),
            size: Set(payload.size),
            mtime: Set(payload.mtime),
            hash: Set(payload.hash),
            remote_deleted: Set(0),
            cache_status: Set(2),
            last_accessed_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        };
        am.insert(db).await.map_err(AppError::from)?;
    }

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
    let out = out.trim_matches([' ', '.']).trim();
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
