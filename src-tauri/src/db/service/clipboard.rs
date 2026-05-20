#![allow(dead_code)]

use log::{debug, warn};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, QueryFilter,
    Set,
};
use uuid::Uuid;

use crate::db::repository::clipboard_record;
use crate::db::service::clipboard_formats;
use crate::db::service::local_files::{
    cleanup_orphaned_clipboard_local_files, has_clipboard_source, has_direct_source,
    parse_source_clipboard_ids, source_type_after_adding_clipboard,
    source_type_after_removing_clipboard, SHARE_MODE_MANUAL, SHARE_MODE_TEMP, SOURCE_CLIPBOARD,
};
use crate::db::DbState;
use crate::entity::clipboard_record::{ActiveModel, Entity, Model};
use crate::entity::local_files;
use crate::error::AppError;
use crate::models::clipboard::{ClipboardFileItem, ClipboardResponse, ClipboardType};
use crate::utils::format::normalize_file_uri;

pub async fn list_records(
    db: &DbState,
    page: u64,
    page_size: u64,
) -> Result<Vec<ClipboardResponse>, DbErr> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;

    let rows = clipboard_record::list_latest_summaries(&db.conn, page_size, offset)
        .await
        .map_err(|e| {
            debug!("list_records failed: page={page}, page_size={page_size}, offset={offset}, error={e}");
            e
        })?;

    let mut responses = Vec::with_capacity(rows.len());
    for summary in rows {
        let row = model_from_summary_for_response(&db.conn, summary).await?;
        responses.push(clipboard_response_from_model_with_db(&db.conn, row).await?);
    }
    Ok(responses)
}

pub async fn list_shared_records(
    conn: &DatabaseConnection,
    page: u64,
    page_size: u64,
) -> Result<Vec<ClipboardResponse>, DbErr> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;

    let rows = clipboard_record::list_shared_summaries(conn, page_size, offset)
        .await
        .map_err(|e| {
            debug!("list_shared_records failed: page={page}, page_size={page_size}, offset={offset}, error={e}");
            e
        })?;

    let mut responses = Vec::with_capacity(rows.len());
    for summary in rows {
        let row = model_from_summary_for_response(conn, summary).await?;
        responses.push(clipboard_response_from_model_with_db(conn, row).await?);
    }
    Ok(responses)
}

async fn model_from_summary_for_response(
    conn: &DatabaseConnection,
    summary: clipboard_record::RecordSummary,
) -> Result<Model, DbErr> {
    let mut record = summary.into_model();
    if record.r#type == ClipboardType::File as i32 || record.r#type == ClipboardType::Folder as i32
    {
        if let Some(full) = clipboard_record::select_by_id(conn, record.id).await? {
            record.data = full.data;
        }
    }
    Ok(record)
}

pub fn clipboard_response_from_model(record: Model) -> ClipboardResponse {
    let file_items = build_clipboard_file_items(&record);
    ClipboardResponse {
        id: record.id,
        r#type: record.r#type,
        preview: record.preview,
        hash: record.hash,
        size: record.size,
        source_app: record.source_app,
        created_at: record.created_at,
        last_accessed_at: record.last_accessed_at,
        access_count: record.access_count,
        is_favorite: record.is_favorite,
        is_shared: record.is_shared,
        is_valid: record.is_valid,
        file_items,
        available_formats: clipboard_formats::legacy_format_name(record.r#type)
            .into_iter()
            .collect(),
    }
}

pub async fn clipboard_response_from_model_with_db(
    db: &sea_orm::DatabaseConnection,
    record: Model,
) -> Result<ClipboardResponse, DbErr> {
    let mut response = clipboard_response_from_model(record.clone());
    let formats = clipboard_formats::list_format_names(db, record.id).await?;
    if !formats.is_empty() {
        response.available_formats = formats;
    }
    Ok(response)
}

fn build_clipboard_file_items(record: &Model) -> Option<Vec<ClipboardFileItem>> {
    if record.r#type != ClipboardType::File as i32 && record.r#type != ClipboardType::Folder as i32
    {
        return None;
    }

    let bytes = record.data.as_ref()?;
    let paths: Vec<String> = serde_json::from_slice(bytes).ok()?;
    let items = paths
        .into_iter()
        .map(|raw| {
            let normalized = normalize_file_uri(&raw).to_string();
            let path = std::path::PathBuf::from(&normalized);
            let canonical = std::fs::canonicalize(&path).unwrap_or(path);
            let exists = canonical.exists();
            let metadata = std::fs::metadata(&canonical).ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().and_then(|m| {
                if m.is_file() {
                    Some(m.len() as i64)
                } else {
                    None
                }
            });
            let name = canonical
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or(&normalized)
                .to_string();
            let file_type = if is_dir {
                1
            } else if is_image_path(&canonical) {
                2
            } else {
                0
            };
            ClipboardFileItem {
                name,
                path: canonical.to_string_lossy().to_string(),
                r#type: file_type,
                is_dir,
                size,
                exists,
            }
        })
        .collect::<Vec<_>>();

    Some(items)
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

            let any_missing = paths
                .iter()
                .any(|p| !std::path::Path::new(normalize_file_uri(p)).exists());
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
            let path = std::path::Path::new(normalize_file_uri(&path_str));
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
    sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
    if auto_cleanup {
        clipboard_formats::delete_payload_files(&db.conn, id).await?;
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

pub async fn mark_record_accessed(db: &DbState, id: i32) -> Result<(), AppError> {
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await
        .map_err(|e| {
            debug!("mark_record_accessed query failed: id={id}, error={e}");
            AppError::from(e)
        })?
        .ok_or(AppError::NotFound)?;

    let next_access_count = record.access_count.saturating_add(1);
    let mut active: ActiveModel = record.into();
    active.last_accessed_at = Set(chrono::Utc::now().timestamp());
    active.access_count = Set(next_access_count);
    active.update(&db.conn).await.map_err(|e| {
        debug!("mark_record_accessed update failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    Ok(())
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
    let updated = active.update(&db.conn).await.map_err(|e| {
        debug!("toggle_share update failed: id={id}, error={e}");
        AppError::from(e)
    })?;
    if new_share == 1 {
        upsert_local_files_for_shared_clipboard(db, &updated).await?;
    } else {
        sync_local_files_on_clipboard_unshared_or_invalid(db, &updated).await?;
    }
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
                    warn!(
                        "delete_item remove cache file failed: id={id}, path={}, error={e}",
                        path.display()
                    );
                }
            }
        }
    }

    clipboard_formats::delete_payload_files(&db.conn, record.id).await?;
    sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
    record.delete(&db.conn).await.map_err(|e| {
        debug!("delete_item delete record failed: id={id}, error={e}");
        AppError::from(e)
    })?;

    Ok(())
}

fn extract_paths_from_clipboard_record(record: &Model) -> Result<Vec<String>, AppError> {
    if record.r#type == ClipboardType::File as i32 || record.r#type == ClipboardType::Folder as i32
    {
        let bytes = record
            .data
            .clone()
            .ok_or_else(|| AppError::InvalidInput("clipboard file data is empty".to_string()))?;
        let paths: Vec<String> = serde_json::from_slice(&bytes).map_err(AppError::from)?;
        return Ok(paths
            .into_iter()
            .map(|path| normalize_file_uri(&path).to_string())
            .collect());
    }
    if record.r#type == ClipboardType::Image as i32 {
        let bytes = record
            .data
            .clone()
            .ok_or_else(|| AppError::InvalidInput("clipboard image data is empty".to_string()))?;
        let path = String::from_utf8(bytes).map_err(AppError::from)?;
        return Ok(vec![normalize_file_uri(&path).to_string()]);
    }
    Ok(Vec::new())
}

fn local_file_type_from_clipboard_type(clipboard_type: i32, path: &std::path::Path) -> i32 {
    if clipboard_type == ClipboardType::Folder as i32 || path.is_dir() {
        return 1;
    }
    if clipboard_type == ClipboardType::Image as i32 {
        return 2;
    }
    0
}

fn is_image_path(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    matches!(
        ext.as_deref(),
        Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("gif")
            | Some("bmp")
            | Some("webp")
            | Some("tif")
            | Some("tiff")
            | Some("ico")
            | Some("heic")
            | Some("avif")
            | Some("svg")
    )
}

async fn upsert_local_files_for_shared_clipboard(
    db: &DbState,
    record: &Model,
) -> Result<(), AppError> {
    let paths = extract_paths_from_clipboard_record(record)?;
    for raw_path in paths {
        let path = std::path::PathBuf::from(normalize_file_uri(&raw_path));
        if !path.exists() {
            continue;
        }
        let metadata = std::fs::metadata(&path).ok();
        let size = metadata.as_ref().and_then(|m| {
            if m.is_file() {
                Some(m.len() as i64)
            } else {
                None
            }
        });
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let path_text = canonical.to_string_lossy().to_string();
        let existed = local_files::Entity::find()
            .filter(local_files::Column::Path.eq(path_text.clone()))
            .one(&db.conn)
            .await
            .map_err(AppError::from)?;

        if let Some(existing) = existed {
            let mut ids = parse_source_clipboard_ids(existing.source_clipboard_id.as_deref());
            if !ids.contains(&record.id) {
                ids.push(record.id);
            }
            let source_type = source_type_after_adding_clipboard(existing.source_type);
            let share_mode = if has_direct_source(existing.source_type) {
                SHARE_MODE_MANUAL
            } else {
                SHARE_MODE_TEMP
            };
            let mut am: local_files::ActiveModel = existing.clone().into();
            am.is_valid = Set(1);
            am.size = Set(size);
            am.r#type = Set(local_file_type_from_clipboard_type(
                record.r#type,
                &canonical,
            ));
            am.source_clipboard_id =
                Set(Some(serde_json::to_string(&ids).map_err(AppError::from)?));
            am.source_type = Set(source_type);
            am.share_mode = Set(share_mode);
            am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
            am.update(&db.conn).await.map_err(AppError::from)?;
        } else {
            let now = chrono::Utc::now().timestamp();
            let new_item = local_files::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                path: Set(path_text),
                r#type: Set(local_file_type_from_clipboard_type(
                    record.r#type,
                    &canonical,
                )),
                created_at: Set(now),
                access_count: Set(0),
                is_valid: Set(1),
                size: Set(size),
                source_clipboard_id: Set(Some(
                    serde_json::to_string(&vec![record.id]).map_err(AppError::from)?,
                )),
                source_type: Set(SOURCE_CLIPBOARD),
                is_favorite: Set(0),
                share_mode: Set(SHARE_MODE_TEMP),
                expires_at: Set(None),
                updated_at: Set(Some(now)),
            };
            new_item.insert(&db.conn).await.map_err(AppError::from)?;
        }
    }
    Ok(())
}

pub async fn sync_local_files_on_clipboard_unshared_or_invalid(
    db: &DbState,
    record: &Model,
) -> Result<(), AppError> {
    let paths = extract_paths_from_clipboard_record(record)?;
    for raw_path in paths {
        let path = std::path::PathBuf::from(normalize_file_uri(&raw_path));
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let path_text = canonical.to_string_lossy().to_string();

        let existed = find_local_file_for_clipboard_path(db, record.id, &path_text).await?;

        let Some(existing) = existed else {
            continue;
        };
        if !has_clipboard_source(existing.source_type) {
            continue;
        }

        let mut ids = parse_source_clipboard_ids(existing.source_clipboard_id.as_deref());
        ids.retain(|x| *x != record.id);

        let mut am: local_files::ActiveModel = existing.clone().into();
        if ids.is_empty() {
            am.source_clipboard_id = Set(None);
            if let Some(next_source_type) =
                source_type_after_removing_clipboard(existing.source_type)
            {
                am.source_type = Set(next_source_type);
                am.share_mode = Set(SHARE_MODE_MANUAL);
                am.is_valid = Set(1);
            } else {
                am.is_valid = Set(0);
            }
        } else {
            am.source_clipboard_id =
                Set(Some(serde_json::to_string(&ids).map_err(AppError::from)?));
            am.is_valid = Set(1);
        }
        am.updated_at = Set(Some(chrono::Utc::now().timestamp()));
        am.update(&db.conn).await.map_err(AppError::from)?;
    }
    cleanup_orphaned_clipboard_local_files(&db.conn).await?;
    Ok(())
}

async fn find_local_file_for_clipboard_path(
    db: &DbState,
    clipboard_id: i32,
    path_text: &str,
) -> Result<Option<local_files::Model>, AppError> {
    if let Some(exact) = local_files::Entity::find()
        .filter(local_files::Column::Path.eq(path_text.to_string()))
        .one(&db.conn)
        .await
        .map_err(AppError::from)?
    {
        return Ok(Some(exact));
    }

    let candidates = local_files::Entity::find()
        .filter(local_files::Column::SourceClipboardId.is_not_null())
        .all(&db.conn)
        .await
        .map_err(AppError::from)?;

    Ok(candidates.into_iter().find(|candidate| {
        if !has_clipboard_source(candidate.source_type) {
            return false;
        }
        let ids = parse_source_clipboard_ids(candidate.source_clipboard_id.as_deref());
        ids.contains(&clipboard_id) && path_text_matches(&candidate.path, path_text)
    }))
}

pub async fn invalidate_shared_clipboards_by_fs_change(
    db: &DbState,
    changed_path: &std::path::Path,
) -> Result<(), AppError> {
    let normalized_changed_path = changed_path
        .to_str()
        .map(|path| std::path::PathBuf::from(normalize_file_uri(path)))
        .unwrap_or_else(|| changed_path.to_path_buf());
    let changed_path_text = changed_path.to_string_lossy().to_string();
    let normalized_changed_path_text = normalized_changed_path.to_string_lossy().to_string();
    let changed_candidates = [changed_path.to_path_buf(), normalized_changed_path];

    let records = Entity::find()
        .filter(crate::entity::clipboard_record::Column::IsShared.eq(1))
        .filter(crate::entity::clipboard_record::Column::IsValid.eq(1))
        .filter(crate::entity::clipboard_record::Column::Type.is_in([
            ClipboardType::File as i32,
            ClipboardType::Folder as i32,
            ClipboardType::Image as i32,
        ]))
        .all(&db.conn)
        .await
        .map_err(AppError::from)?;

    for record in records {
        let paths = extract_paths_from_clipboard_record(&record)?;
        let mut should_check = false;
        for raw in &paths {
            let p = std::path::PathBuf::from(normalize_file_uri(raw));
            let canonical = std::fs::canonicalize(&p).unwrap_or(p);
            let canonical_text = canonical.to_string_lossy().to_string();
            if changed_candidates.iter().any(|candidate| {
                canonical == *candidate
                    || candidate.starts_with(&canonical)
                    || canonical.starts_with(candidate)
            }) || path_text_matches(&canonical_text, &changed_path_text)
                || path_text_matches(&canonical_text, &normalized_changed_path_text)
            {
                should_check = true;
                break;
            }
        }
        if !should_check {
            continue;
        }

        let invalid = if record.r#type == ClipboardType::Folder as i32 {
            // 文件夹模式：忽略文件夹内部变动，只在文件夹本身不存在时失效
            paths
                .iter()
                .any(|raw| !std::path::Path::new(normalize_file_uri(raw)).exists())
        } else {
            // 文件/图片/多文件：任何一个缺失即整条剪贴板失效
            paths
                .iter()
                .any(|raw| !std::path::Path::new(normalize_file_uri(raw)).exists())
        };

        if invalid {
            let mut am: ActiveModel = record.clone().into();
            am.is_valid = Set(0);
            am.update(&db.conn).await.map_err(AppError::from)?;
            sync_local_files_on_clipboard_unshared_or_invalid(db, &record).await?;
        }
    }

    Ok(())
}

fn path_text_matches(path: &str, changed_path: &str) -> bool {
    let path = normalize_for_path_compare(path);
    let changed_path = normalize_for_path_compare(changed_path);
    path == changed_path
        || changed_path.starts_with(&format!("{path}\\"))
        || path.starts_with(&format!("{changed_path}\\"))
}

fn normalize_for_path_compare(path: &str) -> String {
    let mut path = normalize_file_uri(path).replace('/', "\\");
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        path = format!(r"\\{rest}");
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        path = rest.to_string();
    }
    path.trim_end_matches('\\').to_ascii_lowercase()
}
