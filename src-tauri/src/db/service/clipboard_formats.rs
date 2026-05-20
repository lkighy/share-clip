use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::entity::{clipboard_record, clipboard_record_format};
use crate::models::clipboard::ClipboardType;
use crate::utils::text::{html_to_plain_text, rtf_to_plain_text};

pub const FORMAT_TEXT: &str = "text/plain";
pub const FORMAT_HTML: &str = "text/html";
pub const FORMAT_RTF: &str = "text/rtf";
const PAYLOAD_REF_PREFIX: &[u8] = b"share-clip:payload-file:v1\n";

#[derive(Debug, Serialize, Deserialize)]
struct PayloadRef {
    path: String,
    size: usize,
    hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardFormats {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
}

impl ClipboardFormats {
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.html.is_none() && self.rtf.is_none()
    }

    pub fn primary_type(&self) -> ClipboardType {
        if self.html.is_some() {
            ClipboardType::Html
        } else if self.rtf.is_some() {
            ClipboardType::Rtf
        } else {
            ClipboardType::Text
        }
    }

    pub fn primary_text(&self) -> String {
        if let Some(text) = self.text.as_ref() {
            return text.clone();
        }
        if let Some(html) = self.html.as_ref() {
            return html_to_plain_text(html);
        }
        if let Some(rtf) = self.rtf.as_ref() {
            return rtf_to_plain_text(rtf);
        }
        String::new()
    }

    pub fn primary_data(&self) -> Vec<u8> {
        if let Some(html) = self.html.as_ref() {
            html.as_bytes().to_vec()
        } else if let Some(rtf) = self.rtf.as_ref() {
            rtf.as_bytes().to_vec()
        } else {
            self.text.as_deref().unwrap_or_default().as_bytes().to_vec()
        }
    }

    pub fn total_size(&self) -> i64 {
        self.text
            .as_ref()
            .map(|value| value.len() as i64)
            .unwrap_or(0)
            + self
                .html
                .as_ref()
                .map(|value| value.len() as i64)
                .unwrap_or(0)
            + self
                .rtf
                .as_ref()
                .map(|value| value.len() as i64)
                .unwrap_or(0)
    }

    pub fn combined_hash(&self) -> String {
        let mut hash = blake3::Hasher::new();
        if let Some(text) = self.text.as_ref() {
            update_hash(&mut hash, FORMAT_TEXT, text.as_bytes());
        }
        if let Some(html) = self.html.as_ref() {
            update_hash(&mut hash, FORMAT_HTML, html.as_bytes());
        }
        if let Some(rtf) = self.rtf.as_ref() {
            update_hash(&mut hash, FORMAT_RTF, rtf.as_bytes());
        }
        hash.finalize().to_hex().to_string()
    }

    pub fn from_legacy_record(record: &clipboard_record::Model) -> Option<Self> {
        let data = record.data.clone()?;
        let value = String::from_utf8(data).ok()?;
        let mut formats = Self::default();
        if record.r#type == ClipboardType::Text as i32 {
            formats.text = Some(value);
        } else if record.r#type == ClipboardType::Html as i32 {
            formats.html = Some(value);
        } else if record.r#type == ClipboardType::Rtf as i32 {
            formats.rtf = Some(value);
        }
        if formats.is_empty() {
            None
        } else {
            Some(formats)
        }
    }

    fn ordered_entries(&self) -> Vec<(&'static str, &str, i32)> {
        let mut entries = Vec::with_capacity(3);
        if let Some(text) = self.text.as_deref() {
            entries.push((FORMAT_TEXT, text, 10));
        }
        if let Some(rtf) = self.rtf.as_deref() {
            entries.push((FORMAT_RTF, rtf, 20));
        }
        if let Some(html) = self.html.as_deref() {
            entries.push((FORMAT_HTML, html, 30));
        }
        entries
    }
}

pub async fn save_formats(
    db: &DatabaseConnection,
    clipboard_id: i32,
    formats: &ClipboardFormats,
    cache_dir: &str,
    text_inline_max_bytes: u64,
    rich_inline_max_bytes: u64,
) -> Result<(), DbErr> {
    let now = chrono::Utc::now().timestamp();
    for (format, value, priority) in formats.ordered_entries() {
        let content = value.as_bytes();
        let inline_max_bytes = if format == FORMAT_TEXT {
            text_inline_max_bytes
        } else {
            rich_inline_max_bytes
        };
        let data = format_storage_data(cache_dir, clipboard_id, format, value, inline_max_bytes)?;
        let active = clipboard_record_format::ActiveModel {
            clipboard_id: Set(clipboard_id),
            format: Set(format.to_string()),
            format_name: Set(Some(format.to_string())),
            hash: Set(Some(hash_bytes(content))),
            size: Set(Some(content.len() as i64)),
            priority: Set(priority),
            created_at: Set(now),
            data: Set(data),
            ..Default::default()
        };
        active.insert(db).await?;
    }
    Ok(())
}

pub async fn load_formats(
    db: &DatabaseConnection,
    record: &clipboard_record::Model,
) -> Result<ClipboardFormats, DbErr> {
    let rows = clipboard_record_format::Entity::find()
        .filter(clipboard_record_format::Column::ClipboardId.eq(record.id))
        .order_by_desc(clipboard_record_format::Column::Priority)
        .all(db)
        .await?;

    if rows.is_empty() {
        return Ok(ClipboardFormats::from_legacy_record(record).unwrap_or_default());
    }

    let mut map = BTreeMap::new();
    for row in rows {
        if let Ok(value) = load_format_data(row.data) {
            map.insert(row.format, value);
        }
    }

    Ok(ClipboardFormats {
        text: map.remove(FORMAT_TEXT),
        html: map.remove(FORMAT_HTML),
        rtf: map.remove(FORMAT_RTF),
    })
}

pub async fn has_stored_formats(db: &DatabaseConnection, clipboard_id: i32) -> Result<bool, DbErr> {
    let count = clipboard_record_format::Entity::find()
        .filter(clipboard_record_format::Column::ClipboardId.eq(clipboard_id))
        .count(db)
        .await?;
    Ok(count > 0)
}

pub async fn list_format_names(
    db: &DatabaseConnection,
    clipboard_id: i32,
) -> Result<Vec<String>, DbErr> {
    clipboard_record_format::Entity::find()
        .select_only()
        .column(clipboard_record_format::Column::Format)
        .filter(clipboard_record_format::Column::ClipboardId.eq(clipboard_id))
        .order_by_desc(clipboard_record_format::Column::Priority)
        .into_tuple::<String>()
        .all(db)
        .await
}

pub async fn delete_payload_files(db: &DatabaseConnection, clipboard_id: i32) -> Result<(), DbErr> {
    let rows = clipboard_record_format::Entity::find()
        .filter(clipboard_record_format::Column::ClipboardId.eq(clipboard_id))
        .all(db)
        .await?;

    for row in rows {
        if let Some(payload) = payload_ref_from_data(&row.data) {
            let _ = fs::remove_file(payload.path);
        }
    }

    Ok(())
}

pub fn legacy_format_name(record_type: i32) -> Option<String> {
    if record_type == ClipboardType::Text as i32 {
        Some(FORMAT_TEXT.to_string())
    } else if record_type == ClipboardType::Html as i32 {
        Some(FORMAT_HTML.to_string())
    } else if record_type == ClipboardType::Rtf as i32 {
        Some(FORMAT_RTF.to_string())
    } else {
        None
    }
}

fn update_hash(hash: &mut blake3::Hasher, format: &str, data: &[u8]) {
    hash.update(format.as_bytes());
    hash.update(&[0]);
    hash.update(data);
    hash.update(&[0]);
}

fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn format_storage_data(
    cache_dir: &str,
    clipboard_id: i32,
    format: &str,
    value: &str,
    inline_max_bytes: u64,
) -> Result<Vec<u8>, DbErr> {
    let bytes = value.as_bytes();
    if (bytes.len() as u64) <= inline_max_bytes {
        return Ok(bytes.to_vec());
    }

    let dir = resolve_payload_cache_dir(cache_dir);
    fs::create_dir_all(&dir)
        .map_err(|err| DbErr::Custom(format!("create clipboard payload dir failed: {err}")))?;

    let hash = hash_bytes(bytes);
    let extension = extension_for_format(format);
    let file_path = dir.join(format!("{}-{}.{extension}", clipboard_id, &hash[..16]));
    if !file_path.exists() {
        fs::write(&file_path, bytes)
            .map_err(|err| DbErr::Custom(format!("write clipboard payload failed: {err}")))?;
    }

    let payload = PayloadRef {
        path: file_path.to_string_lossy().into_owned(),
        size: bytes.len(),
        hash,
    };
    let mut data = PAYLOAD_REF_PREFIX.to_vec();
    data.extend(
        serde_json::to_vec(&payload)
            .map_err(|err| DbErr::Custom(format!("encode clipboard payload ref failed: {err}")))?,
    );
    Ok(data)
}

fn load_format_data(data: Vec<u8>) -> Result<String, DbErr> {
    if let Some(payload) = payload_ref_from_data(&data) {
        let bytes = fs::read(&payload.path)
            .map_err(|err| DbErr::Custom(format!("read clipboard payload failed: {err}")))?;
        return String::from_utf8(bytes)
            .map_err(|err| DbErr::Custom(format!("invalid clipboard payload utf8: {err}")));
    }

    String::from_utf8(data)
        .map_err(|err| DbErr::Custom(format!("invalid clipboard format utf8: {err}")))
}

fn payload_ref_from_data(data: &[u8]) -> Option<PayloadRef> {
    data.strip_prefix(PAYLOAD_REF_PREFIX)
        .and_then(|json| serde_json::from_slice::<PayloadRef>(json).ok())
}

fn extension_for_format(format: &str) -> &'static str {
    match format {
        FORMAT_HTML => "html",
        FORMAT_RTF => "rtf",
        _ => "txt",
    }
}

fn resolve_payload_cache_dir(cache_dir: &str) -> PathBuf {
    let cache_path = PathBuf::from(cache_dir);
    let base = if cache_path.is_absolute() {
        cache_path
    } else if let Ok(exe_path) = std::env::current_exe() {
        exe_path
            .parent()
            .map(|base_dir| base_dir.join(&cache_path))
            .unwrap_or(cache_path)
    } else {
        cache_path
    };

    base.join("clipboard-payloads")
}
