use std::collections::HashSet;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::entity::clipboard_record;
use crate::models::clipboard::ClipboardType;
use crate::utils::format::normalize_file_uri;

#[derive(Clone)]
pub struct HttpState {
    pub db: DatabaseConnection,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct SharedClipboardItem {
    id: i32,
    r#type: i32,
    preview: Option<String>,
    created_at: i64,
}

#[derive(Serialize)]
struct SharedClipboardContent {
    id: i32,
    r#type: i32,
    content: Value,
}

#[derive(Serialize)]
struct FileNode {
    name: String,
    path: String,
    is_dir: bool,
    size: Option<u64>,
}

#[derive(Serialize)]
struct FileListResponse {
    current_path: Option<String>,
    items: Vec<FileNode>,
}

#[derive(Deserialize)]
struct FileListRequest {
    path: Option<String>,
}

#[derive(Deserialize)]
struct ClipboardContentRequest {
    id: i32,
}

#[derive(Deserialize)]
struct DownloadQuery {
    path: String,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/clipboard/list", get(list_shared_clipboard))
        .route("/clipboard/content", get(get_shared_clipboard_content))
        .route("/files/list", get(list_shared_roots).post(list_files))
        .route("/files/download", get(download_file))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

async fn list_shared_clipboard(State(state): State<HttpState>) -> Response {
    let models = match clipboard_record::Entity::find()
        .filter(clipboard_record::Column::IsShared.eq(1))
        .order_by_desc(clipboard_record::Column::CreatedAt)
        .all(&state.db)
        .await
    {
        Ok(models) => models,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    let items = models
        .into_iter()
        .map(|item| SharedClipboardItem {
            id: item.id,
            r#type: item.r#type,
            preview: item.preview,
            created_at: item.created_at,
        })
        .collect::<Vec<_>>();

    Json(items).into_response()
}

async fn get_shared_clipboard_content(
    State(state): State<HttpState>,
    Query(params): Query<ClipboardContentRequest>,
) -> Response {
    let item = match clipboard_record::Entity::find_by_id(params.id)
        .one(&state.db)
        .await
    {
        Ok(Some(model)) => model,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "clipboard record not found"),
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    if item.is_shared != 1 {
        return json_error(StatusCode::FORBIDDEN, "clipboard record is not shared");
    }

    let content = match parse_clipboard_content(item.r#type, item.data.as_deref()) {
        Ok(content) => content,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, e),
    };

    Json(SharedClipboardContent {
        id: item.id,
        r#type: item.r#type,
        content,
    })
    .into_response()
}

async fn list_shared_roots(State(state): State<HttpState>) -> Response {
    let roots = match load_shared_roots(&state.db).await {
        Ok(roots) => roots,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };

    let items = roots
        .iter()
        .map(|path| path_to_node(path))
        .collect::<Vec<_>>();
    Json(FileListResponse {
        current_path: None,
        items,
    })
    .into_response()
}

async fn list_files(
    State(state): State<HttpState>,
    Json(payload): Json<FileListRequest>,
) -> Response {
    let roots = match load_shared_roots(&state.db).await {
        Ok(roots) => roots,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };

    let Some(path) = payload.path else {
        let items = roots
            .iter()
            .map(|path| path_to_node(path))
            .collect::<Vec<_>>();
        return Json(FileListResponse {
            current_path: None,
            items,
        })
        .into_response();
    };

    let canonical = match canonicalize_existing(Path::new(&path)) {
        Ok(path) => path,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "path not found"),
    };

    if !is_path_allowed(&canonical, &roots) {
        return json_error(StatusCode::FORBIDDEN, "path is not in shared scope");
    }

    if canonical.is_file() {
        return Json(FileListResponse {
            current_path: Some(canonical.to_string_lossy().to_string()),
            items: vec![path_to_node(&canonical)],
        })
        .into_response();
    }

    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(&canonical) {
        Ok(entries) => entries,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read directory: {e}"),
            )
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if let Ok(canonical_entry) = canonicalize_existing(&path) {
            entries.push(path_to_node(&canonical_entry));
        }
    }

    entries.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .reverse()
            .then_with(|| a.name.cmp(&b.name))
    });

    Json(FileListResponse {
        current_path: Some(canonical.to_string_lossy().to_string()),
        items: entries,
    })
    .into_response()
}

async fn download_file(
    State(state): State<HttpState>,
    Query(params): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Response {
    let roots = match load_shared_roots(&state.db).await {
        Ok(roots) => roots,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };

    let canonical = match canonicalize_existing(Path::new(&params.path)) {
        Ok(path) => path,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "file not found"),
    };

    if !canonical.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }

    if !is_path_allowed(&canonical, &roots) {
        return json_error(StatusCode::FORBIDDEN, "file is not in shared scope");
    }

    let metadata = match tokio::fs::metadata(&canonical).await {
        Ok(metadata) => metadata,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read metadata: {e}"),
            )
        }
    };

    let total_size = metadata.len();
    if total_size == 0 {
        return (StatusCode::OK, Body::empty()).into_response();
    }

    let range_header = headers.get(header::RANGE).and_then(|h| h.to_str().ok());
    let (start, end, partial) = match parse_range(range_header, total_size) {
        Ok(tuple) => tuple,
        Err(_) => {
            let mut resp = json_error(StatusCode::RANGE_NOT_SATISFIABLE, "invalid range");
            if let Ok(value) = HeaderValue::from_str(&format!("bytes */{total_size}")) {
                resp.headers_mut().insert(header::CONTENT_RANGE, value);
            }
            return resp;
        }
    };

    let chunk_len = end - start + 1;
    if chunk_len > usize::MAX as u64 {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "range too large");
    }

    let mut file = match tokio::fs::File::open(&canonical).await {
        Ok(file) => file,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to open file: {e}"),
            )
        }
    };

    if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to seek file: {e}"),
        );
    }

    let mut buffer = vec![0u8; chunk_len as usize];
    if let Err(e) = file.read_exact(&mut buffer).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read file: {e}"),
        );
    }

    let filename = canonical
        .file_name()
        .map(|name| name.to_string_lossy().replace('"', "_"))
        .unwrap_or_else(|| "download.bin".to_string());

    let mut response = Response::new(Body::from(buffer));
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    if let Ok(value) = HeaderValue::from_str(&chunk_len.to_string()) {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    if partial {
        if let Ok(value) = HeaderValue::from_str(&format!("bytes {start}-{end}/{total_size}")) {
            response.headers_mut().insert(header::CONTENT_RANGE, value);
        }
    }

    response
}

fn parse_clipboard_content(type_code: i32, data: Option<&[u8]>) -> Result<Value, String> {
    let Some(bytes) = data else {
        return Ok(Value::Null);
    };

    if type_code == ClipboardType::Text as i32
        || type_code == ClipboardType::Html as i32
        || type_code == ClipboardType::Rtf as i32
        || type_code == ClipboardType::Image as i32
    {
        let text =
            String::from_utf8(bytes.to_vec()).map_err(|e| format!("invalid utf-8 data: {e}"))?;
        return Ok(Value::String(text));
    }

    if type_code == ClipboardType::File as i32 || type_code == ClipboardType::Folder as i32 {
        let paths: Vec<String> =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid file-list json: {e}"))?;
        return serde_json::to_value(paths).map_err(|e| format!("serialization error: {e}"));
    }

    Ok(Value::Null)
}

fn path_to_node(path: &Path) -> FileNode {
    let metadata = std::fs::metadata(path).ok();
    let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let size = metadata
        .as_ref()
        .and_then(|m| if m.is_file() { Some(m.len()) } else { None });
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir,
        size,
    }
}

async fn load_shared_roots(db: &DatabaseConnection) -> Result<Vec<PathBuf>, String> {
    let models = clipboard_record::Entity::find()
        .filter(clipboard_record::Column::IsShared.eq(1))
        .filter(
            clipboard_record::Column::Type
                .is_in([ClipboardType::File as i32, ClipboardType::Folder as i32]),
        )
        .all(db)
        .await
        .map_err(|e| format!("database error: {e}"))?;

    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    for item in models {
        let Some(raw) = item.data else {
            continue;
        };

        let paths: Vec<String> = match serde_json::from_slice(&raw) {
            Ok(paths) => paths,
            Err(_) => continue,
        };

        for raw_path in paths {
            let normalized = normalize_file_uri(&raw_path);
            let parsed = PathBuf::from(normalized);
            let canonical = match canonicalize_existing(&parsed) {
                Ok(path) => path,
                Err(_) => continue,
            };
            if seen.insert(canonical.clone()) {
                roots.push(canonical);
            }
        }
    }

    Ok(roots)
}

fn canonicalize_existing(path: &Path) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

fn is_path_allowed(target: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        if root.is_dir() {
            target.starts_with(root)
        } else {
            target == root
        }
    })
}

fn parse_range(range: Option<&str>, total_size: u64) -> Result<(u64, u64, bool), ()> {
    let Some(range) = range else {
        return Ok((0, total_size - 1, false));
    };

    let content = range.strip_prefix("bytes=").ok_or(())?;
    let first = content.split(',').next().ok_or(())?.trim();
    if first.is_empty() {
        return Err(());
    }

    let (start, end) = if let Some((left, right)) = first.split_once('-') {
        if left.is_empty() {
            let suffix: u64 = right.parse().map_err(|_| ())?;
            if suffix == 0 {
                return Err(());
            }
            let start = total_size.saturating_sub(suffix);
            (start, total_size - 1)
        } else {
            let start: u64 = left.parse().map_err(|_| ())?;
            let end = if right.is_empty() {
                total_size - 1
            } else {
                right.parse().map_err(|_| ())?
            };
            (start, end)
        }
    } else {
        return Err(());
    };

    if start >= total_size || end >= total_size || start > end {
        return Err(());
    }

    Ok((start, end, true))
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
        .into_response()
}
