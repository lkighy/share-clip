use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use html2text::from_read;
use regex::Regex;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use tauri::Manager;

use crate::app::config::AppConfig;
use crate::db::DbState;
use crate::entity::clipboard_record;
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
    let config = app_handle.state::<AppConfig>();

    let (type_code, data, preview, hash, size) = match event {
        ClipboardChangeEvent::Text(content) => {
            let data_bytes = content.into_bytes();
            let preview = preview_from_plain_text(&String::from_utf8_lossy(&data_bytes))
                .unwrap_or_default();
            let size = data_bytes.len() as i64;
            let hash = hash_bytes(&data_bytes);
            (
                i32::from(ClipboardType::Text),
                Some(data_bytes),
                Some(preview),
                hash,
                size,
            )
        }
        ClipboardChangeEvent::Html(content) => {
            let data_bytes = content.into_bytes();
            let preview = preview_from_plain_text(&html_to_plain_text(&String::from_utf8_lossy(&data_bytes)));
            let size = data_bytes.len() as i64;
            let hash = hash_bytes(&data_bytes);
            (
                i32::from(ClipboardType::Html),
                Some(data_bytes),
                preview,
                hash,
                size,
            )
        }
        ClipboardChangeEvent::Rtf(content) => {
            let data_bytes = content.into_bytes();
            let size = data_bytes.len() as i64;
            let hash = hash_bytes(&data_bytes);
            let preview = preview_from_plain_text(&rtf_to_plain_text(&String::from_utf8_lossy(&data_bytes)));
            (
                i32::from(ClipboardType::Rtf),
                Some(data_bytes),
                preview,
                hash,
                size,
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
            )
        }
        ClipboardChangeEvent::Files {
            files,
            file_count,
            folder_count,
            image_count,
        } => {
            if files.len() == 1 && file_count == 1 && image_count == 1 {
                let normalized = normalize_file_uri(&files[0]);
                let image_data = std::fs::read(normalized)?;
                let size = image_data.len() as i64;
                let hash = hash_bytes(&image_data);
                let preview = generate_image_thumbnail(&image_data, 10).ok();
                let data = if Path::new(normalized).is_file() {
                    normalized.as_bytes().to_vec()
                } else {
                    cache_image(&config.cache_dir, &hash, &image_data)?.into_bytes()
                };
                (
                    i32::from(ClipboardType::Image),
                    Some(data),
                    preview,
                    hash,
                    size,
                )
            } else {
                let preview = build_files_preview(&files);

                let files_json = serde_json::to_string(&files)?;
                let data_bytes = files_json.into_bytes();
                let size = data_bytes.len() as i64;

                let mut sorted = files;
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
                )
            }
        }
        ClipboardChangeEvent::Unknown { formats } => {
            println!("skip unknown clipboard format(s): {:?}", formats);
            return Ok(());
        }
    };

    let existing = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::Hash.eq(hash.as_str()))
        .one(db)
        .await?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    if let Some(existing_model) = existing {
        let mut active: clipboard_record::ActiveModel = existing_model.into();
        active.last_accessed_at = Set(now);
        active.update(db).await?;
    } else {
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
            is_shared: Set(0),
            ..Default::default()
        };
        new_item.insert(db).await?;
    }

    Ok(())
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
                preview_parts.push(format!("📄 {} ({})", item_label, format_file_size(metadata.len())));
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
    let mut lines = Vec::with_capacity(3);
    for line in text.split('\n').take(3) {
        lines.push(line.trim_end_matches('\r'));
    }

    let preview = lines.join("\n");
    if preview.trim().is_empty() {
        None
    } else {
        Some(preview.chars().take(100).collect::<String>())
    }
}

fn html_to_plain_text(html: &str) -> String {
    from_read(html.as_bytes(), usize::MAX)
}

fn rtf_to_plain_text(rtf: &str) -> String {
    let hex_re = Regex::new(r"\\'([0-9a-fA-F]{2})").expect("valid rtf hex regex");
    let mut decoded = String::with_capacity(rtf.len());
    let mut last = 0usize;
    for cap in hex_re.captures_iter(rtf) {
        let m = cap.get(0).expect("full match exists");
        decoded.push_str(&rtf[last..m.start()]);
        if let Some(hex) = cap.get(1) {
            if let Ok(value) = u8::from_str_radix(hex.as_str(), 16) {
                decoded.push(value as char);
            }
        }
        last = m.end();
    }
    decoded.push_str(&rtf[last..]);

    let ctrl_re = Regex::new(r"\\[a-zA-Z]+\d* ?|\\[{}\\]").expect("valid rtf control regex");
    let braces_re = Regex::new(r"[{}]").expect("valid brace regex");

    let text = ctrl_re.replace_all(&decoded, " ");
    braces_re.replace_all(&text, " ").into_owned()
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
