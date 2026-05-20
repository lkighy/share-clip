use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::info;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use tauri::Manager;
use uuid::Uuid;

use crate::app::config::AppConfigStore;
use crate::db::service::clipboard_formats::{self, ClipboardFormats};
use crate::db::service::local_files::{
    has_direct_source, parse_source_clipboard_ids, source_type_after_adding_clipboard,
    SHARE_MODE_MANUAL, SHARE_MODE_TEMP, SOURCE_CLIPBOARD,
};
use crate::db::DbState;
use crate::entity::{clipboard_record, local_files};
use crate::models::clipboard::ClipboardType;
use crate::services::clipboard_watcher::ClipboardChangeEvent;
use crate::utils::format::{generate_image_thumbnail, normalize_file_uri};
use crate::utils::image::format_file_size;

type StorageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

// TODO: 将查询、更新等操作移动到 db 中
#[allow(dead_code)]
pub async fn save_clipboard_item(
    app_handle: tauri::AppHandle,
    event: ClipboardChangeEvent,
) -> StorageResult<()> {
    let db = &app_handle.state::<DbState>().conn;
    let config = app_handle.state::<AppConfigStore>().get();

    let (type_code, data, preview, hash, size, formats) = match event {
        ClipboardChangeEvent::Text(content) => {
            let size = content.len() as i64;
            let hash = hash_bytes(content.as_bytes());
            let preview = preview_from_plain_text(&content).unwrap_or_default();
            if (content.len() as u64) > config.clipboard_text_max_bytes {
                info!(
                    "externalize oversized text clipboard: size={} bytes, inline_limit={} bytes",
                    content.len(),
                    config.clipboard_text_max_bytes
                );
                (
                    i32::from(ClipboardType::Text),
                    None,
                    Some(preview),
                    hash,
                    size,
                    Some(ClipboardFormats {
                        text: Some(content),
                        html: None,
                        rtf: None,
                    }),
                )
            } else {
                (
                    i32::from(ClipboardType::Text),
                    Some(content.into_bytes()),
                    Some(preview),
                    hash,
                    size,
                    None,
                )
            }
        }
        ClipboardChangeEvent::RichText { text, html, rtf } => {
            let raw_formats = ClipboardFormats { text, html, rtf };
            let raw_size = raw_formats.total_size();
            let Some(formats) = limit_formats_for_storage(raw_formats, &config) else {
                info!(
                    "skip oversized rich clipboard: size={} bytes, text_limit={} bytes, format_limit={} bytes, total_limit={} bytes",
                    raw_size,
                    config.clipboard_text_max_bytes,
                    config.clipboard_rich_format_max_bytes,
                    config.clipboard_total_max_bytes
                );
                return Ok(());
            };
            let stored_size = formats.total_size();
            let primary_inline_limit = if formats.primary_type() == ClipboardType::Text {
                config.clipboard_text_max_bytes
            } else {
                config.clipboard_rich_format_max_bytes
            };
            if stored_size < raw_size {
                info!(
                    "trim oversized rich clipboard: raw_size={} bytes, stored_size={} bytes",
                    raw_size, stored_size
                );
            }
            let data_bytes = formats.primary_data();
            let preview = preview_from_plain_text(&formats.primary_text());
            let size = stored_size;
            let hash = formats.combined_hash();
            let data = if (data_bytes.len() as u64) <= primary_inline_limit {
                Some(data_bytes)
            } else {
                None
            };
            (
                i32::from(formats.primary_type()),
                data,
                preview,
                hash,
                size,
                Some(formats),
            )
        }
        ClipboardChangeEvent::Image => {
            let image_data = read_image_from_clipboard()?;
            let size = image_data.len() as i64;
            let hash = hash_bytes(&image_data);
            let preview = generate_image_thumbnail(&image_data, 10).ok();
            let path = cache_image(&config.cache_dir, &hash, &image_data)?;
            (
                i32::from(ClipboardType::Image),
                Some(path.into_bytes()),
                preview,
                hash,
                size,
                None,
            )
        }
        ClipboardChangeEvent::Files {
            files,
            file_count,
            folder_count,
        } => {
            let normalized_files = files
                .iter()
                .map(|path| normalize_file_uri(path).to_string())
                .collect::<Vec<_>>();
            let preview = build_files_preview(&normalized_files);

            let files_json = serde_json::to_string(&normalized_files)?;
            let data_bytes = files_json.into_bytes();
            let size = normalized_files
                .iter()
                .filter_map(|path| std::fs::metadata(path).ok())
                .filter(|metadata| metadata.is_file())
                .map(|metadata| metadata.len() as i64)
                .sum::<i64>();

            let mut sorted = normalized_files;
            sorted.sort();
            let hash_input = sorted.join("\0");
            let hash = hash_bytes(hash_input.as_bytes());

            let file_type = if folder_count > 0 && file_count == 0 {
                ClipboardType::Folder
            } else {
                ClipboardType::File
            };

            (
                i32::from(file_type),
                Some(data_bytes),
                Some(preview),
                hash,
                size,
                None,
            )
        }
        ClipboardChangeEvent::Unknown { formats } => {
            info!("skip unknown clipboard format(s): {:?}", formats);
            return Ok(());
        }
    };
    let default_is_shared = match type_code {
        t if t == ClipboardType::Text as i32
            || t == ClipboardType::Html as i32
            || t == ClipboardType::Rtf as i32 =>
        {
            config.default_share_text
        }
        t if t == ClipboardType::Image as i32 => config.default_share_image,
        t if t == ClipboardType::File as i32 => config.default_share_file,
        t if t == ClipboardType::Folder as i32 => config.default_share_folder,
        _ => false,
    };

    let existing = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Hash.eq(hash.as_str()))
        .one(db)
        .await?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let mut inserted_new_record = false;
    let saved_record = if let Some(existing_model) = existing {
        let existing_id = existing_model.id;
        let mut active: clipboard_record::ActiveModel = existing_model.into();
        active.last_accessed_at = Set(now);
        active.update(db).await?;
        clipboard_record::Entity::find_by_id(existing_id)
            .one(db)
            .await?
            .ok_or("updated clipboard record not found")?
    } else {
        inserted_new_record = true;
        let new_item = clipboard_record::ActiveModel {
            r#type: Set(type_code),
            data: Set(data),
            preview: Set(preview),
            hash: Set(Some(hash)),
            size: Set(Some(size)),
            source_app: Set(None),
            created_at: Set(now),
            last_accessed_at: Set(now),
            access_count: Set(0),
            is_favorite: Set(0),
            is_shared: Set(if default_is_shared { 1 } else { 0 }),
            ..Default::default()
        };
        new_item.insert(db).await?
    };

    if let Some(formats) = formats.as_ref() {
        if inserted_new_record
            || !clipboard_formats::has_stored_formats(db, saved_record.id).await?
        {
            clipboard_formats::save_formats(
                db,
                saved_record.id,
                formats,
                &config.cache_dir,
                config.clipboard_text_max_bytes,
                config.clipboard_rich_format_max_bytes,
            )
            .await?;
        }
    }

    if saved_record.is_shared == 1 {
        upsert_temp_local_files_for_clipboard(db, &saved_record).await?;
    }
    crate::app::events::emit_clipboard_changed(
        &app_handle,
        vec![saved_record.id.to_string()],
        "clipboard_saved",
    );
    crate::app::events::emit_local_files_changed(&app_handle, Vec::new(), "clipboard_changed");

    Ok(())
}

async fn upsert_temp_local_files_for_clipboard(
    db: &sea_orm::DatabaseConnection,
    record: &clipboard_record::Model,
) -> StorageResult<()> {
    let paths = extract_share_paths(record)?;
    for raw_path in paths {
        let path = PathBuf::from(raw_path);
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
        let file_type = local_file_type_from_clipboard_type(record.r#type, &canonical);
        let now = now_ts();

        let existed = local_files::Entity::find()
            .filter(local_files::Column::Path.eq(path_text.clone()))
            .one(db)
            .await?;

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
            let mut am: local_files::ActiveModel = existing.into();
            am.is_valid = Set(1);
            am.size = Set(size);
            am.r#type = Set(file_type);
            am.source_type = Set(source_type);
            am.share_mode = Set(share_mode);
            am.source_clipboard_id = Set(Some(serde_json::to_string(&ids)?));
            am.updated_at = Set(Some(now));
            am.update(db).await?;
        } else {
            let am = local_files::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                path: Set(path_text),
                r#type: Set(file_type),
                created_at: Set(now),
                access_count: Set(0),
                is_valid: Set(1),
                size: Set(size),
                source_clipboard_id: Set(Some(serde_json::to_string(&vec![record.id])?)),
                source_type: Set(SOURCE_CLIPBOARD),
                is_favorite: Set(0),
                share_mode: Set(SHARE_MODE_TEMP),
                expires_at: Set(None),
                updated_at: Set(Some(now)),
            };
            am.insert(db).await?;
        }
    }
    Ok(())
}

fn extract_share_paths(record: &clipboard_record::Model) -> StorageResult<Vec<String>> {
    if record.r#type == ClipboardType::File as i32 || record.r#type == ClipboardType::Folder as i32
    {
        let bytes = record.data.clone().unwrap_or_default();
        let paths: Vec<String> = serde_json::from_slice(&bytes)?;
        return Ok(paths
            .into_iter()
            .map(|p| normalize_file_uri(&p).to_string())
            .collect());
    }
    if record.r#type == ClipboardType::Image as i32 {
        let bytes = record.data.clone().unwrap_or_default();
        let path = String::from_utf8(bytes)?;
        return Ok(vec![normalize_file_uri(&path).to_string()]);
    }
    Ok(Vec::new())
}

fn local_file_type_from_clipboard_type(clipboard_type: i32, path: &Path) -> i32 {
    if clipboard_type == ClipboardType::Folder as i32 || path.is_dir() {
        return 1;
    }
    if clipboard_type == ClipboardType::Image as i32 {
        return 2;
    }
    0
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

fn limit_formats_for_storage(
    mut formats: ClipboardFormats,
    config: &crate::app::config::AppConfig,
) -> Option<ClipboardFormats> {
    while formats_total_size(&formats) > config.clipboard_total_max_bytes {
        if formats.rtf.take().is_some() {
            continue;
        }
        if formats.html.take().is_some() {
            continue;
        }
        if formats.text.take().is_some() {
            continue;
        }
        break;
    }

    if formats.is_empty() {
        None
    } else {
        Some(formats)
    }
}

fn option_len(value: &Option<String>) -> u64 {
    value.as_ref().map(|value| value.len() as u64).unwrap_or(0)
}

fn formats_total_size(formats: &ClipboardFormats) -> u64 {
    option_len(&formats.text) + option_len(&formats.html) + option_len(&formats.rtf)
}

fn build_files_preview(files: &[String]) -> String {
    let total = files.len();
    let display_count = if total > 3 { 2 } else { total };
    let mut preview_parts = Vec::with_capacity(display_count);

    for path_str in files.iter().take(display_count) {
        let normalized = normalize_file_uri(path_str);
        let path = Path::new(normalized);

        let item_label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(normalized)
            .to_string();

        if path.is_dir() {
            preview_parts.push(format!("📁 {}", item_label));
        } else if path.is_file() {
            if let Ok(metadata) = std::fs::metadata(path) {
                preview_parts.push(format!(
                    "📄 {} ({})",
                    item_label,
                    format_file_size(metadata.len())
                ));
            } else {
                preview_parts.push(item_label);
            }
        } else {
            preview_parts.push(item_label);
        }
    }

    let mut preview = preview_parts.join("\n");
    if total > 3 {
        preview.push_str(&format!("\n等 {} 个文件", total - 2));
    }
    preview
}

fn preview_from_plain_text(text: &str) -> Option<String> {
    let mut preview = String::new();
    let mut char_count = 0usize;

    for (line_index, line) in text.split('\n').take(3).enumerate() {
        if line_index > 0 {
            preview.push('\n');
            char_count += 1;
            if char_count >= 100 {
                break;
            }
        }

        for ch in line.trim_end_matches('\r').chars() {
            preview.push(ch);
            char_count += 1;
            if char_count >= 100 {
                break;
            }
        }

        if char_count >= 100 {
            break;
        }
    }

    if preview.trim().is_empty() {
        None
    } else {
        Some(preview)
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn read_image_from_clipboard() -> StorageResult<Vec<u8>> {
    use clipboard_rs::common::RustImage;
    use clipboard_rs::{Clipboard, ClipboardContext};

    let ctx = ClipboardContext::new()?;
    let image = ctx.get_image()?;
    Ok(image.to_png()?.get_bytes().to_vec())
}

fn cache_image(cache_dir: &str, hash: &str, image_data: &[u8]) -> StorageResult<String> {
    let dir = resolve_cache_dir(cache_dir);
    std::fs::create_dir_all(&dir)?;

    let file_path = dir.join(format!("{hash}.png"));
    if !file_path.exists() {
        std::fs::write(&file_path, image_data)?;
    }

    Ok(file_path.to_string_lossy().into_owned())
}

fn resolve_cache_dir(cache_dir: &str) -> PathBuf {
    let cache_path = PathBuf::from(cache_dir);
    if cache_path.is_absolute() {
        return cache_path;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(base_dir) = exe_path.parent() {
            return base_dir.join(cache_path);
        }
    }

    cache_path
}
