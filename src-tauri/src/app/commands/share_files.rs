use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;

use crate::entity::clipboard_record;
use crate::entity::local_files;
use crate::entity::outbound_connections;
use crate::error::{ApiError, AppError};
use crate::models::clipboard::ClipboardType;
use crate::utils::format::normalize_file_uri;

#[derive(Debug, Serialize)]
pub struct RemoteShareUser {
    pub user_id: String,
    pub user_name: String,
    pub ip: String,
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
    if user_id.is_empty() || user_name.is_empty() || ip.is_empty() {
        return Err(AppError::InvalidInput("user_id/user_name/ip 不能为空".to_string()).into());
    }

    if let Some(existing) = outbound_connections::Entity::find_by_id(user_id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
    {
        let mut am: outbound_connections::ActiveModel = existing.into();
        am.user_name = Set(user_name.clone());
        am.ip = Set(ip.clone());
        am.password = Set(payload.password);
        am.update(db).await.map_err(AppError::from)?;
    } else {
        let am = outbound_connections::ActiveModel {
            user_id: Set(user_id.clone()),
            user_name: Set(user_name.clone()),
            ip: Set(ip.clone()),
            password: Set(payload.password),
            device_id: Set(None),
            display_name: Set(None),
            auth_token: Set(None),
            auth_status: Set(0),
            last_connected_at: Set(None),
        };
        am.insert(db).await.map_err(AppError::from)?;
    }

    Ok(RemoteShareUser {
        user_id,
        user_name,
        ip,
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
pub async fn unshare_local_shared_file(app: tauri::AppHandle, id: String) -> Result<(), ApiError> {
    let db = &app.state::<crate::db::DbState>().conn;
    let row = local_files::Entity::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;
    let mut am: local_files::ActiveModel = row.into();
    am.is_valid = Set(0);
    am.source_clipboard_id = Set(None);
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
            let mut am: local_files::ActiveModel = existing.into();
            am.is_valid = Set(1);
            am.source_type = Set(0);
            am.source_clipboard_id = Set(None);
            am.size = Set(size);
            am.r#type = Set(file_type);
            am.share_mode = Set(0);
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
                source_type: Set(0),
                is_favorite: Set(0),
                share_mode: Set(0),
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
