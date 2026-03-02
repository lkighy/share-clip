use sea_orm::FromQueryResult;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ClipboardType {
    Text = 0,
    Html = 1,
    Rtf = 2,
    Image = 3,
    File = 4,
    Folder = 5,
}

impl From<ClipboardType> for i32 {
    fn from(value: ClipboardType) -> Self {
        value as i32
    }
}

impl TryFrom<i32> for ClipboardType {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Text),
            1 => Ok(Self::Html),
            2 => Ok(Self::Rtf),
            3 => Ok(Self::Image),
            4 => Ok(Self::File),
            5 => Ok(Self::Folder),
            _ => Err(format!("invalid clipboard type: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, FromQueryResult)]
pub struct ClipboardResponse {
    pub id: i32,
    pub r#type: i32,
    pub preview: Option<String>,
    pub size: Option<i32>,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub last_accessed_at: i64,
    pub access_count: i32,
    pub is_favorite: i32,
    pub is_shared: i32,
    pub is_valid: i32,
}
