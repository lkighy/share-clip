use serde::Serialize;

use crate::entity::clipboard_record;

#[derive(Serialize)]
pub struct ClipboardRecordDto {
    pub id: String,
    pub r#type: String,
    pub data: Option<Vec<u8>>,
    pub file_path: Option<String>,
    pub preview: Option<String>,
    pub size: Option<i64>,
    pub metadata: Option<String>,
    pub source_app: Option<String>,
    pub created_at: String,
    pub last_accessed_at: Option<String>,
    pub access_count: i32,
    pub is_favorite: i32,
    pub sync_status: String,
    pub sync_version: i32,
}

impl From<clipboard_record::Model> for ClipboardRecordDto {
    fn from(item: clipboard_record::Model) -> Self {
        Self {
            id: item.id,
            r#type: item.r#type,
            data: item.data,
            file_path: item.file_path,
            preview: item.preview,
            size: item.size,
            metadata: item.metadata,
            source_app: item.source_app,
            created_at: item.created_at.to_rfc3339(),
            last_accessed_at: item.last_accessed_at.map(|t| t.to_rfc3339()),
            access_count: item.access_count,
            is_favorite: item.is_favorite,
            sync_status: item.sync_status,
            sync_version: item.sync_version,
        }
    }
}
