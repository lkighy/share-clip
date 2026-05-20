use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{json_error, HttpState};
use crate::db::service::local_files::{has_clipboard_source, parse_source_clipboard_ids};
use crate::entity::{clipboard_record, inbound_connections, local_file_index, local_files};
use crate::models::clipboard::ClipboardType;
use crate::server::share::{resolve_share_path, ROOT_RELATIVE_PATH};
use crate::utils::format::normalize_file_uri;

const AUTH_STATUS_UNAUTHENTICATED: i32 = 0;
const AUTH_STATUS_PENDING: i32 = 1;
const AUTH_STATUS_APPROVED: i32 = 2;
const AUTH_STATUS_REJECTED: i32 = 3;

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
struct FileNode {
    name: String,
    relative_path: String,
    is_dir: bool,
    size: Option<u64>,
    mtime: Option<i64>,
    hash: Option<String>,
}

#[derive(Serialize)]
struct FileListResponse {
    share_id: String,
    current_path: String,
    items: Vec<FileNode>,
}

#[derive(Serialize)]
struct DeviceInfoResponse {
    device_id: String,
    device_name: String,
}

#[derive(Serialize)]
pub(super) struct ClipboardContentResponse {
    id: i32,
    r#type: i32,
    text: Option<String>,
    html: Option<String>,
    rtf: Option<String>,
    image_base64: Option<String>,
    files: Option<Vec<String>>,
    sync_targets: Option<Vec<ClipboardFileSyncTarget>>,
}

#[derive(Serialize)]
pub(super) struct ClipboardFileSyncTarget {
    share_id: String,
    share_name: String,
    relative_path: String,
    name: String,
    is_dir: bool,
    size: Option<i64>,
    mtime: Option<i64>,
    hash: Option<String>,
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

#[derive(Deserialize)]
struct ConnectionRequest {
    user_id: String,
    user_name: Option<String>,
    device_id: Option<String>,
    password: Option<String>,
    ip: Option<String>,
}

#[derive(Serialize)]
struct ConnectionStatusResponse {
    auth_status: i32,
    message: String,
    poll_after_ms: u64,
    auth_token: Option<String>,
    device_id: String,
    device_name: String,
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
        .route("/api/client/device", get(device_info))
        .route("/api/client/connect/request", post(request_connection))
        .route(
            "/api/client/connect/status/{user_id}",
            get(connection_status),
        )
        .route("/api/client/shares", get(list_client_shares))
        .route("/api/client/shares/{id}/list", get(list_client_files))
        .route("/api/client/shares/{id}/index", get(index_share))
        .route(
            "/api/client/shares/{id}/download",
            get(download_client_share_file),
        )
        .route(
            "/api/client/shares/{id}/diff",
            axum::routing::post(diff_share),
        )
        .route("/api/client/clipboard/list", get(list_client_clipboard))
        .route(
            "/api/client/clipboard/{id}/content",
            get(get_client_clipboard_content),
        )
        .route("/api/files/{id}/download", get(download_share_file))
}

async fn device_info(State(state): State<HttpState>) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    Json(DeviceInfoResponse {
        device_id: config.local_device_id,
        device_name: config.local_device_name,
    })
    .into_response()
}

async fn request_connection(State(state): State<HttpState>, body: String) -> Response {
    let payload = match serde_json::from_str::<ConnectionRequest>(&body) {
        Ok(payload) => payload,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, format!("invalid request: {e}")),
    };
    let user_id = payload.user_id.trim().to_string();
    if user_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "user_id is required");
    }

    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    if !config.sync_access_enabled {
        return json_error(StatusCode::FORBIDDEN, "client sync access is disabled");
    }
    if config.share_server_password_enabled {
        let expected = config
            .share_server_password_hash
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let supplied = payload.password.as_deref().map(str::trim).unwrap_or("");
        if expected.is_none() || Some(supplied) != expected {
            return json_error(StatusCode::UNAUTHORIZED, "password required or invalid");
        }
    }

    let now = chrono::Utc::now().timestamp();
    let existing = match inbound_connections::Entity::find_by_id(user_id.clone())
        .one(&state.db)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    let auth_status = match existing.as_ref().map(|row| row.auth_status) {
        Some(AUTH_STATUS_APPROVED) => AUTH_STATUS_APPROVED,
        Some(AUTH_STATUS_REJECTED) => AUTH_STATUS_REJECTED,
        _ if config.share_server_auth_mode == 0 => AUTH_STATUS_APPROVED,
        _ => AUTH_STATUS_PENDING,
    };

    let user_name = payload
        .user_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let device_id = payload
        .device_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let ip = payload
        .ip
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown")
        .to_string();

    if let Some(existing) = existing {
        let mut am: inbound_connections::ActiveModel = existing.into();
        am.ip = Set(ip);
        am.device_id = Set(device_id);
        am.user_name = Set(user_name);
        am.auth_status = Set(auth_status);
        am.last_seen_at = Set(Some(now));
        if auth_status == AUTH_STATUS_APPROVED {
            am.is_shared = Set(1);
            am.is_trusted = Set(1);
            am.granted_at = Set(Some(now));
            am.revoked_at = Set(None);
        }
        if let Err(e) = am.update(&state.db).await {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            );
        }
    } else {
        let am = inbound_connections::ActiveModel {
            user_id: Set(user_id),
            is_shared: Set(if auth_status == AUTH_STATUS_APPROVED {
                1
            } else {
                0
            }),
            is_trusted: Set(if auth_status == AUTH_STATUS_APPROVED {
                1
            } else {
                0
            }),
            ip: Set(ip),
            device_id: Set(device_id),
            user_name: Set(user_name),
            auth_status: Set(auth_status),
            granted_at: Set(if auth_status == AUTH_STATUS_APPROVED {
                Some(now)
            } else {
                None
            }),
            revoked_at: Set(None),
            last_seen_at: Set(Some(now)),
        };
        if let Err(e) = am.insert(&state.db).await {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            );
        }
    }

    if auth_status == AUTH_STATUS_PENDING {
        let pending_count = match inbound_connections::Entity::find()
            .filter(inbound_connections::Column::AuthStatus.eq(AUTH_STATUS_PENDING))
            .count(&state.db)
            .await
        {
            Ok(count) => count,
            Err(e) => {
                log::error!("count pending inbound connections failed: {e}");
                1
            }
        };
        crate::app::ui::tray::update_connect_device_menu_pending_count(&state.app, pending_count);
        crate::app::events::emit_inbound_requested(
            &state.app,
            vec![payload.user_id.trim().to_string()],
            "inbound_connection_pending",
        );
        let config = state
            .app
            .state::<crate::app::config::AppConfigStore>()
            .get();
        if config.popup_on_inbound_request {
            let _ = crate::app::ui::window::open_or_create_window(
                &state.app,
                crate::models::window::WindowLabel::ShareFile,
            );
        }
    }

    Json(connection_status_body(auth_status, &config)).into_response()
}

async fn connection_status(
    State(state): State<HttpState>,
    AxumPath(user_id): AxumPath<String>,
) -> Response {
    let row = match inbound_connections::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    Json(connection_status_body(
        row.map(|row| row.auth_status)
            .unwrap_or(AUTH_STATUS_UNAUTHENTICATED),
        &config,
    ))
    .into_response()
}

async fn list_client_shares(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

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

async fn list_client_clipboard(
    State(state): State<HttpState>,
    Query(query): Query<IndexQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let responses = match crate::db::service::clipboard::list_shared_records(
        &state.db, page, page_size,
    )
    .await
    {
        Ok(responses) => responses,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };

    Json(responses).into_response()
}

async fn get_client_clipboard_content(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<i32>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

    let row = match clipboard_record::Entity::find_by_id(id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, "clipboard record not found"),
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        }
    };
    if row.is_shared != 1 {
        return json_error(StatusCode::NOT_FOUND, "clipboard record not found");
    }

    let content = match clipboard_content_response(&state.db, &row).await {
        Ok(content) => content,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, e),
    };
    Json(content).into_response()
}

async fn list_client_files(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<RelativePathQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

    let share = match crate::server::share::load_local_share(&state.db, &id).await {
        Ok(share) => share,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "share not found"),
    };
    let root = PathBuf::from(&share.path);
    let target = match resolve_share_path(&root, query.path.as_deref()) {
        Ok(target) => target,
        Err(e) if e.contains("outside") || e.contains("absolute") || e.contains("invalid") => {
            return json_error(StatusCode::FORBIDDEN, e)
        }
        Err(e) => return json_error(StatusCode::NOT_FOUND, e),
    };
    let root = match std::fs::canonicalize(root) {
        Ok(root) => root,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "share root not found"),
    };

    if target.is_file() {
        return Json(FileListResponse {
            share_id: id,
            current_path: crate::server::share::relative_path_for(&root, &target),
            items: vec![path_to_node(&root, &target)],
        })
        .into_response();
    }

    let read_dir = match std::fs::read_dir(&target) {
        Ok(entries) => entries,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read directory: {e}"),
            )
        }
    };

    let mut items = Vec::new();
    for entry in read_dir.flatten() {
        if let Ok(path) = std::fs::canonicalize(entry.path()) {
            if path.starts_with(&root) {
                items.push(path_to_node(&root, &path));
            }
        }
    }
    items.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .reverse()
            .then_with(|| a.name.cmp(&b.name))
    });

    Json(FileListResponse {
        share_id: id,
        current_path: crate::server::share::relative_path_for(&root, &target),
        items,
    })
    .into_response()
}

async fn index_share(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<IndexQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

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
    headers: HeaderMap,
    Json(payload): Json<DiffRequest>,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }

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
    if let Err(response) =
        super::authorize_browser_request(&state, &headers, super::WebAccessScope::Download)
    {
        return response;
    }

    serve_share_file(state, id, query, headers).await
}

async fn download_client_share_file(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<RelativePathQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_client_request(&state, &headers).await {
        return response;
    }
    serve_share_file(state, id, query, headers).await
}

async fn serve_share_file(
    state: HttpState,
    id: String,
    query: RelativePathQuery,
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

pub(super) async fn clipboard_content_response(
    db: &DatabaseConnection,
    record: &clipboard_record::Model,
) -> Result<ClipboardContentResponse, String> {
    let mut response = ClipboardContentResponse {
        id: record.id,
        r#type: record.r#type,
        text: None,
        html: None,
        rtf: None,
        image_base64: None,
        files: None,
        sync_targets: None,
    };

    if record.r#type == ClipboardType::Text as i32 {
        let formats = crate::db::service::clipboard_formats::load_formats(db, record)
            .await
            .map_err(|e| format!("database error: {e}"))?;
        response.text = formats
            .text
            .or_else(|| Some(bytes_to_string(record.data.clone()).unwrap_or_default()));
        response.html = formats.html;
        response.rtf = formats.rtf;
    } else if record.r#type == ClipboardType::Html as i32 {
        let formats = crate::db::service::clipboard_formats::load_formats(db, record)
            .await
            .map_err(|e| format!("database error: {e}"))?;
        response.text = formats.text;
        response.html = formats
            .html
            .or_else(|| Some(bytes_to_string(record.data.clone()).unwrap_or_default()));
        response.rtf = formats.rtf;
    } else if record.r#type == ClipboardType::Rtf as i32 {
        let formats = crate::db::service::clipboard_formats::load_formats(db, record)
            .await
            .map_err(|e| format!("database error: {e}"))?;
        response.text = formats.text;
        response.html = formats.html;
        response.rtf = formats
            .rtf
            .or_else(|| Some(bytes_to_string(record.data.clone()).unwrap_or_default()));
    } else if record.r#type == ClipboardType::Image as i32 {
        let path = bytes_to_string(record.data.clone())?;
        let bytes = tokio::fs::read(normalize_file_uri(&path))
            .await
            .map_err(|e| format!("failed to read image: {e}"))?;
        response.image_base64 = Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ));
    } else if record.r#type == ClipboardType::File as i32
        || record.r#type == ClipboardType::Folder as i32
    {
        let text = bytes_to_string(record.data.clone())?;
        let files = serde_json::from_str::<Vec<String>>(&text)
            .map_err(|e| format!("failed to parse file list: {e}"))?;
        response.sync_targets = Some(clipboard_file_sync_targets(db, record.id, &files).await?);
        response.files = Some(files);
    }

    Ok(response)
}

async fn clipboard_file_sync_targets(
    db: &DatabaseConnection,
    clipboard_id: i32,
    files: &[String],
) -> Result<Vec<ClipboardFileSyncTarget>, String> {
    let mut targets = Vec::new();
    for raw in files {
        if let Some(share) = find_local_share_for_clipboard_path(db, clipboard_id, raw).await? {
            targets.push(clipboard_sync_target_for_share(db, share).await?);
        }
    }
    Ok(targets)
}

async fn find_local_share_for_clipboard_path(
    db: &DatabaseConnection,
    clipboard_id: i32,
    raw_path: &str,
) -> Result<Option<local_files::Model>, String> {
    let path = PathBuf::from(normalize_file_uri(raw_path));
    let canonical = std::fs::canonicalize(&path).unwrap_or(path);
    let path_text = canonical.to_string_lossy().to_string();

    if let Some(row) = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .filter(local_files::Column::Path.eq(path_text.clone()))
        .one(db)
        .await
        .map_err(|e| format!("database error: {e}"))?
    {
        if local_share_matches_clipboard(&row, clipboard_id) {
            return Ok(Some(row));
        }
    }

    let candidates = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .filter(local_files::Column::SourceClipboardId.is_not_null())
        .all(db)
        .await
        .map_err(|e| format!("database error: {e}"))?;

    Ok(candidates.into_iter().find(|row| {
        local_share_matches_clipboard(row, clipboard_id) && path_text_matches(&row.path, &path_text)
    }))
}

fn local_share_matches_clipboard(row: &local_files::Model, clipboard_id: i32) -> bool {
    has_clipboard_source(row.source_type)
        && parse_source_clipboard_ids(row.source_clipboard_id.as_deref()).contains(&clipboard_id)
}

async fn clipboard_sync_target_for_share(
    db: &DatabaseConnection,
    share: local_files::Model,
) -> Result<ClipboardFileSyncTarget, String> {
    let root_index =
        local_file_index::Entity::find_by_id((share.id.clone(), ROOT_RELATIVE_PATH.to_string()))
            .one(db)
            .await
            .map_err(|e| format!("database error: {e}"))?;

    let path = Path::new(&share.path);
    let metadata = std::fs::metadata(path).ok();
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| share.path.clone());
    let is_dir = metadata
        .as_ref()
        .map(|metadata| metadata.is_dir())
        .unwrap_or(share.r#type == 1);
    let fallback_size = metadata.as_ref().and_then(|metadata| {
        if metadata.is_file() {
            Some(metadata.len() as i64)
        } else {
            None
        }
    });
    let fallback_mtime = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);

    Ok(ClipboardFileSyncTarget {
        share_id: share.id,
        share_name: name.clone(),
        relative_path: ROOT_RELATIVE_PATH.to_string(),
        name,
        is_dir,
        size: root_index
            .as_ref()
            .map(|index| index.size)
            .or(share.size)
            .or(fallback_size),
        mtime: root_index
            .as_ref()
            .map(|index| index.mtime)
            .or(fallback_mtime),
        hash: root_index.and_then(|index| index.hash),
    })
}

fn path_text_matches(left: &str, right: &str) -> bool {
    normalize_for_path_compare(left) == normalize_for_path_compare(right)
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

fn bytes_to_string(data: Option<Vec<u8>>) -> Result<String, String> {
    String::from_utf8(data.unwrap_or_default()).map_err(|e| format!("invalid utf8 data: {e}"))
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

fn path_to_node(root: &std::path::Path, path: &std::path::Path) -> FileNode {
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
        relative_path: crate::server::share::relative_path_for(root, path),
        is_dir,
        size,
        mtime: metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64),
        hash: None,
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

async fn authorize_client_request(state: &HttpState, headers: &HeaderMap) -> Result<(), Response> {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    if !config.sync_access_enabled {
        return Err(json_error(
            StatusCode::FORBIDDEN,
            "client sync access is disabled",
        ));
    }

    let user_id = headers
        .get("x-share-clip-user-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| json_error(StatusCode::UNAUTHORIZED, "missing user id"))?;
    let device_id = headers
        .get("x-share-clip-device-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| json_error(StatusCode::UNAUTHORIZED, "missing device id"))?;

    let row = inbound_connections::Entity::find_by_id(user_id.to_string())
        .one(&state.db)
        .await
        .map_err(|e| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            )
        })?
        .ok_or_else(|| json_error(StatusCode::UNAUTHORIZED, "connection is not approved"))?;

    if row.auth_status != AUTH_STATUS_APPROVED || row.is_shared != 1 {
        return Err(json_error(
            StatusCode::FORBIDDEN,
            "connection is not approved",
        ));
    }
    if row.device_id.as_deref() != Some(device_id) {
        return Err(json_error(StatusCode::FORBIDDEN, "device is not trusted"));
    }
    Ok(())
}

fn connection_status_body(
    auth_status: i32,
    config: &crate::app::config::AppConfig,
) -> ConnectionStatusResponse {
    let message = match auth_status {
        AUTH_STATUS_PENDING => "waiting for approval",
        AUTH_STATUS_APPROVED => "approved",
        AUTH_STATUS_REJECTED => "rejected",
        _ => "unauthenticated",
    };
    ConnectionStatusResponse {
        auth_status,
        message: message.to_string(),
        poll_after_ms: 2000,
        auth_token: None,
        device_id: config.local_device_id.clone(),
        device_name: config.local_device_name.clone(),
    }
}
