use std::collections::HashMap;
use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{json_error, HttpState};
use crate::entity::{local_file_index, local_files};
use crate::server::share::{relative_path_for, resolve_share_path, ROOT_RELATIVE_PATH};

#[derive(Deserialize)]
struct RelativePathQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
struct IndexQuery {
    path: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Serialize)]
struct RemoteShareItem {
    id: String,
    name: String,
    r#type: i32,
    size: Option<i64>,
    updated_at: Option<i64>,
}

#[derive(Serialize)]
struct SyncIndexItem {
    relative_path: String,
    name: String,
    is_dir: bool,
    size: i64,
    mtime: i64,
    hash: Option<String>,
    dirty: i32,
}

#[derive(Deserialize)]
struct DiffRequest {
    files: Vec<ClientFileMeta>,
    path: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Deserialize)]
struct ClientFileMeta {
    relative_path: String,
    size: i64,
    mtime: i64,
    hash: Option<String>,
}

#[derive(Serialize)]
struct DiffResponse {
    page: u64,
    page_size: u64,
    total: u64,
    missing_on_client: Vec<SyncIndexItem>,
    need_download_to_client: Vec<SyncIndexItem>,
    conflict_candidates: Vec<SyncIndexItem>,
}

pub fn router() -> Router<HttpState> {
    Router::new()
        .route("/api/client/shares", get(list_client_shares))
        .route("/api/client/shares/{id}/index", get(index_share))
        .route("/api/client/shares/{id}/download", get(download_share_file))
        .route(
            "/api/client/shares/{id}/diff",
            axum::routing::post(diff_share),
        )
        .route("/api/files/{id}/download", get(download_share_file))
}

async fn list_client_shares(State(state): State<HttpState>) -> Response {
    let rows = match local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .order_by_desc(local_files::Column::UpdatedAt)
        .order_by_desc(local_files::Column::CreatedAt)
        .all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    Json(rows.into_iter().map(remote_share_item).collect::<Vec<_>>()).into_response()
}

async fn index_share(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<IndexQuery>,
) -> Response {
    if crate::server::share::load_local_share(&state.db, &id)
        .await
        .is_err()
    {
        return json_error(StatusCode::NOT_FOUND, "share not found");
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(200).clamp(1, 2000);
    let offset = (page - 1) * page_size;
    let parent = normalize_parent_filter(query.path.as_deref());

    let mut select = local_file_index::Entity::find()
        .filter(local_file_index::Column::LocalFileId.eq(id))
        .filter(local_file_index::Column::ExistsFlag.eq(1));

    if let Some(parent) = parent {
        if parent == ROOT_RELATIVE_PATH {
            select = select.filter(local_file_index::Column::RelativePath.eq(ROOT_RELATIVE_PATH));
        } else {
            let prefix = format!("{}/%", parent.trim_end_matches('/'));
            select = select.filter(local_file_index::Column::RelativePath.like(prefix));
        }
    }

    let rows = match select
        .order_by_asc(local_file_index::Column::RelativePath)
        .offset(offset)
        .limit(page_size)
        .all(&state.db)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    Json(rows.into_iter().map(index_item).collect::<Vec<_>>()).into_response()
}

async fn diff_share(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<DiffRequest>,
) -> Response {
    if crate::server::share::load_local_share(&state.db, &id)
        .await
        .is_err()
    {
        return json_error(StatusCode::NOT_FOUND, "share not found");
    }

    let page = payload.page.unwrap_or(1).max(1);
    let page_size = payload.page_size.unwrap_or(200).clamp(1, 2000);
    let offset = (page - 1) * page_size;
    let parent = normalize_parent_filter(payload.path.as_deref());

    let mut select = local_file_index::Entity::find()
        .filter(local_file_index::Column::LocalFileId.eq(id))
        .filter(local_file_index::Column::ExistsFlag.eq(1))
        .filter(local_file_index::Column::IsDir.eq(0));

    if let Some(parent) = parent {
        if parent == ROOT_RELATIVE_PATH {
            select = select.filter(local_file_index::Column::RelativePath.eq(ROOT_RELATIVE_PATH));
        } else {
            let prefix = format!("{}/%", parent.trim_end_matches('/'));
            select = select.filter(local_file_index::Column::RelativePath.like(prefix));
        }
    }

    let total = match select.clone().count(&state.db).await {
        Ok(v) => v,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    let rows = match select
        .order_by_asc(local_file_index::Column::RelativePath)
        .offset(offset)
        .limit(page_size)
        .all(&state.db)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    let mut client_map = HashMap::new();
    for f in payload.files {
        client_map.insert(f.relative_path.clone(), f);
    }

    let mut missing_on_client = Vec::new();
    let mut need_download_to_client = Vec::new();
    let mut conflict_candidates = Vec::new();

    for row in rows {
        let item = index_item(row.clone());
        if let Some(client) = client_map.remove(&row.relative_path) {
            let same_meta = client.size == row.size && client.mtime == row.mtime;
            let same_hash = client.hash.is_some() && client.hash == row.hash;
            if !same_meta && !same_hash {
                conflict_candidates.push(item);
            }
        } else {
            need_download_to_client.push(item);
        }
    }

    for (_, client_only) in client_map {
        missing_on_client.push(SyncIndexItem {
            name: name_from_relative_path(&client_only.relative_path),
            relative_path: client_only.relative_path,
            is_dir: false,
            size: client_only.size,
            mtime: client_only.mtime,
            hash: client_only.hash,
            dirty: 0,
        });
    }

    Json(DiffResponse {
        page,
        page_size,
        total,
        missing_on_client,
        need_download_to_client,
        conflict_candidates,
    })
    .into_response()
}

async fn download_share_file(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<RelativePathQuery>,
    headers: HeaderMap,
) -> Response {
    let share = match crate::server::share::load_local_share(&state.db, &id).await {
        Ok(share) => share,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "share not found"),
    };
    let root = PathBuf::from(&share.path);
    let canonical = match resolve_share_path(&root, query.path.as_deref()) {
        Ok(path) => path,
        Err(e) if e.contains("outside") || e.contains("absolute") || e.contains("invalid") => {
            return json_error(StatusCode::FORBIDDEN, e)
        }
        Err(e) => return json_error(StatusCode::NOT_FOUND, e),
    };

    if !canonical.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
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

    let chunk_len = end - start + 1;
    let filename = canonical
        .file_name()
        .map(|name| name.to_string_lossy().replace('"', "_"))
        .unwrap_or_else(|| "download.bin".to_string());

    let body = if partial || chunk_len != total_size {
        Body::from_stream(ReaderStream::new(file.take(chunk_len)))
    } else {
        Body::from_stream(ReaderStream::new(file))
    };

    let mut response = Response::new(body);
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

fn remote_share_item(row: local_files::Model) -> RemoteShareItem {
    RemoteShareItem {
        id: row.id,
        name: std::path::Path::new(&row.path)
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or(row.path),
        r#type: row.r#type,
        size: row.size,
        updated_at: row.updated_at,
    }
}

fn index_item(row: local_file_index::Model) -> SyncIndexItem {
    SyncIndexItem {
        name: name_from_relative_path(&row.relative_path),
        relative_path: row.relative_path,
        is_dir: row.is_dir == 1,
        size: row.size,
        mtime: row.mtime,
        hash: row.hash,
        dirty: row.dirty,
    }
}

fn normalize_parent_filter(path: Option<&str>) -> Option<String> {
    path.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.replace('\\', "/"))
}

fn name_from_relative_path(path: &str) -> String {
    if path == ROOT_RELATIVE_PATH {
        return ROOT_RELATIVE_PATH.to_string();
    }
    path.rsplit('/')
        .next()
        .filter(|v| !v.is_empty())
        .unwrap_or(path)
        .to_string()
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
            (total_size.saturating_sub(suffix), total_size - 1)
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
