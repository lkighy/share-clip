use std::collections::BTreeMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::entity::{clipboard_record, clipboard_record_format};
use crate::models::clipboard::ClipboardType;
use crate::utils::text::{html_to_plain_text, rtf_to_plain_text};

pub const FORMAT_TEXT: &str = "text/plain";
pub const FORMAT_HTML: &str = "text/html";
pub const FORMAT_RTF: &str = "text/rtf";

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
) -> Result<(), DbErr> {
    let now = chrono::Utc::now().timestamp();
    for (format, value, priority) in formats.ordered_entries() {
        let data = value.as_bytes().to_vec();
        let active = clipboard_record_format::ActiveModel {
            clipboard_id: Set(clipboard_id),
            format: Set(format.to_string()),
            format_name: Set(Some(format.to_string())),
            hash: Set(Some(hash_bytes(&data))),
            size: Set(Some(data.len() as i64)),
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
        if let Ok(value) = String::from_utf8(row.data) {
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
