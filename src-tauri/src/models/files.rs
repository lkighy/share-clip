use serde::Serialize;

/// 目录列表响应（当请求路径是一个目录时）
#[derive(Debug, Serialize)]
pub struct DirectoryList {
    /// 固定为 "directory"，便于前端区分类型
    #[serde(rename = "type")]
    pub type_: String,
    /// 当前目录的路径（相对路径）
    pub path: String,
    /// 目录下的条目列表
    pub entries: Vec<DirEntry>,
}

/// 目录中的单个条目（文件或子目录）
#[derive(Debug, Serialize)]
pub struct DirEntry {
    /// 文件名（不包含路径）
    pub name: String,
    /// 完整相对路径（可用于后续请求）
    pub path: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件大小（字节），目录此字段为 null
    pub size: Option<u64>,
}
