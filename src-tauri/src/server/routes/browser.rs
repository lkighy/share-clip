use std::path::{Path, PathBuf};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use super::{json_error, HttpState};
use crate::entity::local_files;
use crate::server::share::{relative_path_for, resolve_share_path};

#[derive(Serialize)]
struct ShareItem {
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
}

#[derive(Serialize)]
struct FileListResponse {
    share_id: String,
    current_path: String,
    items: Vec<FileNode>,
}

#[derive(Deserialize)]
struct ListQuery {
    path: Option<String>,
}

pub fn router() -> Router<HttpState> {
    Router::new()
        .route("/", get(index_page))
        .route("/api/shares", get(list_shares))
        .route("/api/shares/{id}", get(get_share))
        .route("/api/files/{id}/list", get(list_files))
}

async fn index_page() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<meta charset="utf-8">
<title>Share Clip</title>
<body>
  <main>
    <h1>Share Clip</h1>
    <p>Use <code>/api/shares</code> to browse shared files.</p>
  </main>
</body>"#,
    )
}

async fn list_shares(State(state): State<HttpState>) -> Response {
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

    let items = rows.into_iter().map(share_to_item).collect::<Vec<_>>();
    Json(items).into_response()
}

async fn get_share(State(state): State<HttpState>, AxumPath(id): AxumPath<String>) -> Response {
    let share = match crate::server::share::load_local_share(&state.db, &id).await {
        Ok(share) => share,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "share not found"),
    };
    Json(share_to_item(share)).into_response()
}

async fn list_files(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListQuery>,
) -> Response {
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
            current_path: relative_path_for(&root, &target),
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
        current_path: relative_path_for(&root, &target),
        items,
    })
    .into_response()
}

fn share_to_item(row: local_files::Model) -> ShareItem {
    ShareItem {
        id: row.id,
        name: Path::new(&row.path)
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or(row.path),
        r#type: row.r#type,
        size: row.size,
        updated_at: row.updated_at,
    }
}

fn path_to_node(root: &Path, path: &Path) -> FileNode {
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
        relative_path: relative_path_for(root, path),
        is_dir,
        size,
    }
}
