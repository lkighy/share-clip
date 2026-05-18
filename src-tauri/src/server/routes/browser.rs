use std::path::{Path, PathBuf};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use super::{
    authorize_browser_request, cookie_value, json_error, web_session_cookie, HttpState,
    WebAccessScope,
};
use crate::entity::{clipboard_record, local_files};
use crate::server::share::{relative_path_for, resolve_share_path};

const CLIPBOARD_HTML: &str = include_str!("static/clipboard.html");
const CLIPBOARD_CSS: &str = include_str!("static/clipboard.css");
const CLIPBOARD_JS: &str = include_str!("static/clipboard.js");
const AUTH_HTML: &str = include_str!("static/auth.html");
const AUTH_CSS: &str = include_str!("static/auth.css");
const AUTH_JS: &str = include_str!("static/auth.js");

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

#[derive(Deserialize)]
struct ClipboardListQuery {
    page: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Deserialize)]
struct AuthPasswordPayload {
    password: String,
    scopes: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AuthRequestPayload {
    client_label: Option<String>,
    scopes: Option<Vec<String>>,
}

#[derive(Serialize)]
struct AuthScopeOption {
    name: String,
    label: &'static str,
    enabled: bool,
}

#[derive(Serialize)]
struct AuthStatusResponse {
    auth_required: bool,
    authenticated: bool,
    password_enabled: bool,
    temp_approval_enabled: bool,
    cookie_ttl_seconds: u64,
    scopes: Vec<AuthScopeOption>,
    session_scopes: Vec<String>,
    expires_at: Option<i64>,
}

#[derive(Serialize)]
struct AuthSessionResponse {
    ok: bool,
    scopes: Vec<String>,
    expires_at: i64,
}

#[derive(Serialize)]
struct AuthRequestResponse {
    id: String,
    auth_status: i32,
    scopes: Vec<String>,
    created_at: i64,
    expires_at: i64,
    decided_at: Option<i64>,
    session_expires_at: Option<i64>,
}

pub fn router() -> Router<HttpState> {
    Router::new()
        .route("/auth", get(auth_page))
        .route("/auth/status", get(auth_status))
        .route("/auth/status/{id}", get(auth_request_status))
        .route("/auth/password", post(auth_password))
        .route("/auth/request", post(auth_request))
        .route("/", get(index_page))
        .route("/clipboard", get(clipboard_page_first))
        .route("/clipboard/{page}", get(clipboard_page))
        .route("/share/{id}", get(share_page))
        .route("/api/shares", get(list_shares))
        .route("/api/shares/{id}", get(get_share))
        .route("/api/files/{id}/list", get(list_files))
        .route("/api/clipboard", get(list_clipboard))
        .route("/api/clipboard/list", get(list_clipboard))
        .route("/api/clipboard/{id}/content", get(get_clipboard_content))
}

async fn auth_page(State(state): State<HttpState>) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    let scopes = auth_scope_options(&config);
    let scope_inputs = scopes
        .iter()
        .filter(|scope| scope.enabled)
        .map(|scope| {
            format!(
                r#"<label class="scope"><input type="checkbox" name="scope" value="{name}" checked><span>{label}</span></label>"#,
                name = scope.name,
                label = scope.label,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let password_panel = if config.web_access_password_enabled {
        r#"<form id="password-form" class="panel">
          <h2>固定访问密码</h2>
          <div class="row">
            <input id="password" type="password" autocomplete="current-password" placeholder="访问密码">
            <button type="submit">授权</button>
          </div>
        </form>"#
            .to_string()
    } else {
        r#"<div class="panel muted"><h2>固定访问密码</h2><p>当前未启用固定 Web 访问密码。</p></div>"#
            .to_string()
    };
    let temp_panel = if config.web_access_temp_approval_enabled {
        r#"<div class="panel">
          <h2>临时确认授权</h2>
          <button id="request-button" type="button">发送临时授权请求</button>
          <p id="request-state" class="state"></p>
        </div>"#
            .to_string()
    } else {
        r#"<div class="panel muted"><h2>临时确认授权</h2><p>当前未启用临时确认授权。</p></div>"#
            .to_string()
    };

    Html(auth_page_html(
        config.web_access_cookie_ttl_seconds.max(60),
        &scope_inputs,
        &password_panel,
        &temp_panel,
    ))
    .into_response()
}

async fn auth_status(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    let auth = state.app.state::<crate::server::web_auth::WebAuthState>();
    let session = cookie_value(&headers, crate::server::web_auth::COOKIE_NAME)
        .and_then(|token| auth.session_status(&token));
    Json(AuthStatusResponse {
        auth_required: config.web_access_auth_required,
        authenticated: session.is_some() || !config.web_access_auth_required,
        password_enabled: config.web_access_password_enabled,
        temp_approval_enabled: config.web_access_temp_approval_enabled,
        cookie_ttl_seconds: config.web_access_cookie_ttl_seconds.max(60),
        scopes: auth_scope_options(&config),
        session_scopes: session
            .as_ref()
            .map(|session| crate::server::web_auth::scope_names(&session.scopes))
            .unwrap_or_default(),
        expires_at: session.map(|session| session.expires_at),
    })
    .into_response()
}

async fn auth_password(
    State(state): State<HttpState>,
    Json(payload): Json<AuthPasswordPayload>,
) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    if !config.browser_access_enabled {
        return json_error(StatusCode::FORBIDDEN, "browser access is disabled");
    }
    if !config.web_access_password_enabled {
        return json_error(StatusCode::FORBIDDEN, "web password auth is disabled");
    }
    let expected = config
        .web_access_password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if expected.is_none() || Some(payload.password.trim()) != expected {
        return json_error(StatusCode::UNAUTHORIZED, "password required or invalid");
    }

    let scopes = match crate::server::web_auth::scopes_from_optional_names(
        payload.scopes.as_deref(),
        &config,
    ) {
        Ok(scopes) => scopes,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
    };
    let auth = state.app.state::<crate::server::web_auth::WebAuthState>();
    let session = auth.issue_session(scopes, config.web_access_cookie_ttl_seconds);
    session_json_response(&session, config.web_access_cookie_ttl_seconds)
}

async fn auth_request(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(payload): Json<AuthRequestPayload>,
) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    if !config.browser_access_enabled {
        return json_error(StatusCode::FORBIDDEN, "browser access is disabled");
    }
    if !config.web_access_temp_approval_enabled {
        return json_error(StatusCode::FORBIDDEN, "temporary web auth is disabled");
    }
    let scopes = match crate::server::web_auth::scopes_from_optional_names(
        payload.scopes.as_deref(),
        &config,
    ) {
        Ok(scopes) => scopes,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
    };
    let ip = header_text(&headers, "x-forwarded-for")
        .and_then(|value| {
            value
                .split(',')
                .next()
                .map(|value| value.trim().to_string())
        })
        .or_else(|| header_text(&headers, "x-real-ip"))
        .unwrap_or_else(|| "unknown".to_string());
    let user_agent = header_text(&headers, "user-agent");
    let client_label = payload
        .client_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Web 浏览器")
        .chars()
        .take(120)
        .collect::<String>();
    let auth = state.app.state::<crate::server::web_auth::WebAuthState>();
    let request = auth.create_request(client_label, ip, user_agent, scopes);
    crate::app::events::emit_web_access_requested(
        &state.app,
        vec![request.id.clone()],
        "web_access_pending",
    );
    if config.popup_on_inbound_request {
        let _ = crate::app::ui::window::open_or_create_window(
            &state.app,
            crate::models::window::WindowLabel::ShareFile,
        );
    }
    Json(auth_request_response(&request)).into_response()
}

async fn auth_request_status(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    let auth = state.app.state::<crate::server::web_auth::WebAuthState>();
    let Some(request) = auth.request_status(&id) else {
        return json_error(StatusCode::NOT_FOUND, "web auth request not found");
    };
    let mut response = Json(auth_request_response(&request)).into_response();
    if request.auth_status == crate::server::web_auth::AUTH_STATUS_APPROVED {
        if let Some(token) = request.session_token.as_deref() {
            if let Ok(value) = HeaderValue::from_str(&web_session_cookie(
                token,
                config.web_access_cookie_ttl_seconds,
            )) {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
        }
    }
    response
}

fn auth_page_html(
    ttl_seconds: u64,
    scope_inputs: &str,
    password_panel: &str,
    temp_panel: &str,
) -> String {
    AUTH_HTML
        .replace("__AUTH_CSS__", AUTH_CSS)
        .replace("__AUTH_JS__", AUTH_JS)
        .replace("__TTL__", &ttl_seconds.to_string())
        .replace("__SCOPE_INPUTS__", scope_inputs)
        .replace("__PASSWORD_PANEL__", password_panel)
        .replace("__TEMP_PANEL__", temp_panel)
}

fn session_json_response(
    session: &crate::server::web_auth::WebSession,
    ttl_seconds: u64,
) -> Response {
    let mut response = Json(AuthSessionResponse {
        ok: true,
        scopes: crate::server::web_auth::scope_names(&session.scopes),
        expires_at: session.expires_at,
    })
    .into_response();
    if let Ok(value) = HeaderValue::from_str(&web_session_cookie(&session.token, ttl_seconds)) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn auth_request_response(
    request: &crate::server::web_auth::WebAccessRequest,
) -> AuthRequestResponse {
    AuthRequestResponse {
        id: request.id.clone(),
        auth_status: request.auth_status,
        scopes: crate::server::web_auth::scope_names(&request.scopes),
        created_at: request.created_at,
        expires_at: request.expires_at,
        decided_at: request.decided_at,
        session_expires_at: request.session_expires_at,
    }
}

fn auth_scope_options(config: &crate::app::config::AppConfig) -> Vec<AuthScopeOption> {
    [
        WebAccessScope::Files,
        WebAccessScope::ClipboardList,
        WebAccessScope::ClipboardContent,
        WebAccessScope::Download,
    ]
    .into_iter()
    .map(|scope| AuthScopeOption {
        name: scope.as_str().to_string(),
        label: scope.label(),
        enabled: crate::server::web_auth::is_scope_enabled(config, scope),
    })
    .collect()
}

fn header_text(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

async fn index_page(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
        return response;
    }

    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    let rows = local_files::Entity::find()
        .filter(local_files::Column::IsValid.eq(1))
        .order_by_desc(local_files::Column::UpdatedAt)
        .order_by_desc(local_files::Column::CreatedAt)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let browser_status = if config.browser_access_enabled {
        "已开启"
    } else {
        "已关闭"
    };
    let sync_status = if config.sync_access_enabled {
        "已开启"
    } else {
        "已关闭"
    };
    let auth_status = if config.share_server_auth_mode == 0 {
        "自动授权"
    } else {
        "需要确认"
    };
    let password_status = if config.web_access_password_enabled {
        "已启用"
    } else {
        "未启用"
    };
    let share_count = rows.len();
    let rows_html = if rows.is_empty() {
        r#"<div class="empty">暂无共享项。可以在 Share Clip 的“文件共享”窗口拖入文件或文件夹。</div>"#.to_string()
    } else {
        rows.into_iter()
            .map(|row| {
                let item = share_to_item(row);
                let kind = share_kind_label(item.r#type);
                let size = format_size_i64(item.size);
                let updated = format_timestamp(item.updated_at);
                format!(
                    r#"<a class="row" href="/share/{id}">
  <span class="icon">{icon}</span>
  <span class="name">{name}</span>
  <span class="kind">{kind}</span>
  <span class="size">{size}</span>
  <span class="time">{updated}</span>
</a>"#,
                    id = url_encode(&item.id),
                    icon = share_icon(item.r#type),
                    name = escape_html(&item.name),
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    Html(format!(
        r#"<!doctype html>
<html lang="zh-CN">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Share Clip 共享服务</title>
<style>
  :root {{ color-scheme: light; font-family: "Segoe UI", system-ui, -apple-system, sans-serif; }}
  body {{ margin: 0; min-height: 100vh; background: #f6f8fb; color: #0f172a; }}
  main {{ box-sizing: border-box; width: min(760px, calc(100vw - 32px)); margin: 0 auto; padding: 48px 0; }}
  section {{ border: 1px solid #dbe3ee; border-radius: 10px; background: rgba(255,255,255,.86); box-shadow: 0 12px 32px rgba(15,23,42,.08); overflow: hidden; }}
  header {{ padding: 24px 28px 18px; border-bottom: 1px solid #e5ebf3; }}
  h1 {{ margin: 0 0 8px; font-size: 24px; line-height: 1.25; }}
  h2 {{ margin: 22px 0 10px; font-size: 16px; }}
  p {{ margin: 0; color: #475569; line-height: 1.7; }}
  .content {{ padding: 22px 28px 26px; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 10px; margin-bottom: 18px; }}
  .metric {{ border: 1px solid #e2e8f0; border-radius: 8px; background: #fff; padding: 12px; }}
  .label {{ font-size: 12px; color: #64748b; }}
  .value {{ margin-top: 4px; font-size: 16px; font-weight: 650; }}
  .links {{ display: flex; flex-wrap: wrap; gap: 10px; margin: 18px 0; }}
  a {{ color: #0369a1; text-decoration: none; }}
  a.button {{ border: 1px solid #bae6fd; border-radius: 8px; background: #f0f9ff; padding: 9px 12px; font-size: 14px; }}
  .table {{ border: 1px solid #e2e8f0; border-radius: 8px; overflow: hidden; background: #fff; }}
  .head, .row {{ display: grid; grid-template-columns: 32px minmax(0, 1fr) 78px 88px 140px; gap: 10px; align-items: center; padding: 10px 12px; }}
  .head {{ background: #f8fafc; color: #64748b; font-size: 12px; font-weight: 650; }}
  .row {{ border-top: 1px solid #e2e8f0; color: #0f172a; }}
  .row:hover {{ background: #f0f9ff; }}
  .name {{ min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }}
  .kind, .size, .time {{ color: #64748b; font-size: 13px; }}
  .empty {{ border: 1px dashed #cbd5e1; border-radius: 8px; padding: 18px; color: #64748b; background: #fff; }}
  code {{ border-radius: 6px; background: #eef2f7; padding: 2px 6px; font-size: 13px; }}
  .notice {{ border-left: 3px solid #f59e0b; background: #fffbeb; padding: 11px 13px; color: #78350f; font-size: 14px; line-height: 1.65; }}
  @media (max-width: 640px) {{ .head, .row {{ grid-template-columns: 28px minmax(0, 1fr) 70px; }} .size, .time {{ display: none; }} }}
</style>
<body>
  <main>
    <section>
      <header>
        <h1>Share Clip 共享服务正在运行</h1>
        <p>这表示当前设备正在对局域网开放共享服务。你可以在应用的“文件共享”和“设备连接”页面管理共享内容与授权。</p>
      </header>
      <div class="content">
        <div class="grid">
          <div class="metric"><div class="label">共享项</div><div class="value">{share_count}</div></div>
          <div class="metric"><div class="label">浏览器访问</div><div class="value">{browser_status}</div></div>
          <div class="metric"><div class="label">客户端同步</div><div class="value">{sync_status}</div></div>
          <div class="metric"><div class="label">连接授权</div><div class="value">{auth_status}</div></div>
          <div class="metric"><div class="label">Web 访问密码</div><div class="value">{password_status}</div></div>
        </div>
        <h2>可访问的共享</h2>
        <div class="table">
          <div class="head"><span></span><span>名称</span><span>类型</span><span>大小</span><span>更新时间</span></div>
          {rows_html}
        </div>
        <div class="links">
          <a class="button" href="/health">服务健康检查</a>
          <a class="button" href="/api/shares">JSON API</a>
          <a class="button" href="/clipboard/1">剪切板网页</a>
          <a class="button" href="/api/clipboard/list?page=1&page_size=20">剪切板 API</a>
        </div>
        <p class="notice">安全提醒：请只在可信网络中开放共享服务。若需要给其他设备访问，建议启用访问密码，并保持“连接授权”为需要确认。</p>
      </div>
    </section>
  </main>
</body>
</html>"#
    ))
    .into_response()
}

async fn share_page(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
        return response;
    }

    let share = match crate::server::share::load_local_share(&state.db, &id).await {
        Ok(share) => share,
        Err(_) => return page_shell("共享不存在", r#"<p class="empty">这个共享项不存在或已经停止共享。</p><p><a class="button" href="/">返回共享列表</a></p>"#).into_response(),
    };
    let share_name = Path::new(&share.path)
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| share.path.clone());
    let root = PathBuf::from(&share.path);
    let target = match resolve_share_path(&root, query.path.as_deref()) {
        Ok(target) => target,
        Err(_) => return page_shell("路径不可访问", r#"<p class="empty">路径不存在或不允许访问。</p><p><a class="button" href="/">返回共享列表</a></p>"#).into_response(),
    };
    let root = match std::fs::canonicalize(root) {
        Ok(root) => root,
        Err(_) => return page_shell("共享不可访问", r#"<p class="empty">共享根目录不存在。</p><p><a class="button" href="/">返回共享列表</a></p>"#).into_response(),
    };
    let current_path = relative_path_for(&root, &target);

    if target.is_file() {
        let file_name = target
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| share_name.clone());
        let download = download_href(&id, &current_path);
        let body = format!(
            r#"<p><a class="button" href="/">共享列表</a> <a class="button" href="/share/{id}">共享根目录</a></p>
<div class="file-card">
  <div class="file-icon">📄</div>
  <div>
    <h2>{name}</h2>
    <p>{path}</p>
    <p><a class="button" href="{download}">下载文件</a></p>
  </div>
</div>"#,
            id = url_encode(&id),
            name = escape_html(&file_name),
            path = escape_html(&current_path),
            download = download,
        );
        return page_shell(&share_name, &body).into_response();
    }

    let parent_link = parent_path(&current_path).map(|parent| {
        format!(
            r#"<a class="row" href="/share/{id}{query}"><span class="icon">↩</span><span class="name">上一级</span><span class="kind">目录</span><span class="size">-</span><span class="time">-</span></a>"#,
            id = url_encode(&id),
            query = path_query(&parent),
        )
    });
    let mut rows = Vec::new();
    if let Some(parent) = parent_link {
        rows.push(parent);
    }

    let entries = match std::fs::read_dir(&target) {
        Ok(entries) => entries,
        Err(_) => return page_shell("目录不可读", r#"<p class="empty">无法读取当前目录。</p><p><a class="button" href="/">返回共享列表</a></p>"#).into_response(),
    };
    let mut nodes = Vec::new();
    for entry in entries.flatten() {
        if let Ok(path) = std::fs::canonicalize(entry.path()) {
            if path.starts_with(&root) {
                nodes.push(path_to_node(&root, &path));
            }
        }
    }
    nodes.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    rows.extend(nodes.into_iter().map(|node| {
        let href = if node.is_dir {
            format!("/share/{}{}", url_encode(&id), path_query(&node.relative_path))
        } else {
            download_href(&id, &node.relative_path)
        };
        format!(
            r#"<a class="row" href="{href}"><span class="icon">{icon}</span><span class="name">{name}</span><span class="kind">{kind}</span><span class="size">{size}</span><span class="time">-</span></a>"#,
            href = href,
            icon = if node.is_dir { "📁" } else { "📄" },
            name = escape_html(&node.name),
            kind = if node.is_dir { "目录" } else { "文件" },
            size = if node.is_dir { "-".to_string() } else { format_size_u64(node.size) },
        )
    }));

    let list = if rows.is_empty() {
        r#"<div class="empty">这个目录是空的。</div>"#.to_string()
    } else {
        format!(
            r#"<div class="table"><div class="head"><span></span><span>名称</span><span>类型</span><span>大小</span><span>更新时间</span></div>{}</div>"#,
            rows.join("")
        )
    };
    let body = format!(
        r#"<p><a class="button" href="/">共享列表</a> <span class="crumb">{path}</span></p>{list}"#,
        path = escape_html(&current_path),
    );
    page_shell(&share_name, &body).into_response()
}

async fn clipboard_page_first(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    clipboard_page_response(state, 1, headers).await
}

async fn clipboard_page(
    State(state): State<HttpState>,
    AxumPath(page): AxumPath<u64>,
    headers: HeaderMap,
) -> Response {
    clipboard_page_response(state, page.max(1), headers).await
}

async fn clipboard_page_response(state: HttpState, page: u64, headers: HeaderMap) -> Response {
    if let Err(response) =
        authorize_browser_request(&state, &headers, WebAccessScope::ClipboardList)
    {
        return response;
    }

    Html(clipboard_page_html(page)).into_response()
}

fn clipboard_page_html(page: u64) -> String {
    CLIPBOARD_HTML
        .replace("__CLIPBOARD_CSS__", CLIPBOARD_CSS)
        .replace("__CLIPBOARD_JS__", CLIPBOARD_JS)
        .replace("__PAGE__", &page.to_string())
}

async fn list_shares(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
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

    let items = rows.into_iter().map(share_to_item).collect::<Vec<_>>();
    Json(items).into_response()
}

async fn get_share(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
        return response;
    }

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
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
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

async fn list_clipboard(
    State(state): State<HttpState>,
    Query(query): Query<ClipboardListQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) =
        authorize_browser_request(&state, &headers, WebAccessScope::ClipboardList)
    {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;
    let rows = match clipboard_record::Entity::find()
        .filter(clipboard_record::Column::IsShared.eq(1))
        .order_by_desc(clipboard_record::Column::IsFavorite)
        .order_by_desc(clipboard_record::Column::LastAccessedAt)
        .order_by_desc(clipboard_record::Column::CreatedAt)
        .offset(offset)
        .limit(page_size)
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

    let mut responses = Vec::with_capacity(rows.len());
    for row in rows {
        match crate::db::service::clipboard::clipboard_response_from_model_with_db(&state.db, row)
            .await
        {
            Ok(response) => responses.push(response),
            Err(e) => {
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("database error: {e}"),
                )
            }
        }
    }

    Json(responses).into_response()
}

async fn get_clipboard_content(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<i32>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) =
        authorize_browser_request(&state, &headers, WebAccessScope::ClipboardContent)
    {
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

    match super::client::clipboard_content_response(&state.db, &row).await {
        Ok(content) => Json(content).into_response(),
        Err(e) => json_error(StatusCode::BAD_REQUEST, e),
    }
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

fn page_shell(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!doctype html>
<html lang="zh-CN">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  :root {{ color-scheme: light; font-family: "Segoe UI", system-ui, -apple-system, sans-serif; }}
  body {{ margin: 0; min-height: 100vh; background: #f6f8fb; color: #0f172a; }}
  main {{ box-sizing: border-box; width: min(900px, calc(100vw - 32px)); margin: 0 auto; padding: 42px 0; }}
  section {{ border: 1px solid #dbe3ee; border-radius: 10px; background: rgba(255,255,255,.9); box-shadow: 0 12px 32px rgba(15,23,42,.08); overflow: hidden; }}
  header {{ padding: 22px 26px 16px; border-bottom: 1px solid #e5ebf3; }}
  h1 {{ margin: 0 0 8px; font-size: 24px; line-height: 1.25; }}
  h2 {{ margin: 0 0 8px; font-size: 18px; }}
  p {{ margin: 0 0 14px; color: #475569; line-height: 1.7; }}
  .content {{ padding: 20px 26px 26px; }}
  .button {{ display: inline-block; border: 1px solid #bae6fd; border-radius: 8px; background: #f0f9ff; padding: 8px 11px; color: #0369a1; text-decoration: none; font-size: 14px; }}
  .crumb {{ display: inline-block; margin-left: 8px; color: #64748b; font-size: 13px; }}
  .table {{ border: 1px solid #e2e8f0; border-radius: 8px; overflow: hidden; background: #fff; }}
  .head, .row {{ display: grid; grid-template-columns: 32px minmax(0, 1fr) 78px 88px 140px; gap: 10px; align-items: center; padding: 10px 12px; }}
  .head {{ background: #f8fafc; color: #64748b; font-size: 12px; font-weight: 650; }}
  .row {{ border-top: 1px solid #e2e8f0; color: #0f172a; text-decoration: none; }}
  .row:hover {{ background: #f0f9ff; }}
  .name {{ min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }}
  .kind, .size, .time {{ color: #64748b; font-size: 13px; }}
  .empty {{ border: 1px dashed #cbd5e1; border-radius: 8px; padding: 18px; color: #64748b; background: #fff; }}
  .file-card {{ display: flex; gap: 16px; border: 1px solid #e2e8f0; border-radius: 8px; background: #fff; padding: 18px; }}
  .file-icon {{ font-size: 36px; }}
  @media (max-width: 640px) {{ .head, .row {{ grid-template-columns: 28px minmax(0, 1fr) 70px; }} .size, .time {{ display: none; }} }}
</style>
<body>
  <main>
    <section>
      <header>
        <h1>{title}</h1>
        <p>Share Clip Web 文件浏览</p>
      </header>
      <div class="content">{body}</div>
    </section>
  </main>
</body>
</html>"#,
        title = escape_html(title),
        body = body,
    ))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        let b = *byte;
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn path_query(path: &str) -> String {
    if path == crate::server::share::ROOT_RELATIVE_PATH || path.trim().is_empty() {
        String::new()
    } else {
        format!("?path={}", url_encode(path))
    }
}

fn download_href(id: &str, path: &str) -> String {
    let mut href = format!("/api/files/{}/download", url_encode(id));
    if path != crate::server::share::ROOT_RELATIVE_PATH {
        href.push_str("?path=");
        href.push_str(&url_encode(path));
    }
    href
}

fn parent_path(path: &str) -> Option<String> {
    let value = path.trim_matches('/');
    if value.is_empty() || value == crate::server::share::ROOT_RELATIVE_PATH {
        return None;
    }
    value
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .or_else(|| Some(crate::server::share::ROOT_RELATIVE_PATH.to_string()))
}

fn share_icon(file_type: i32) -> &'static str {
    if file_type == 1 {
        "📁"
    } else {
        "📄"
    }
}

fn share_kind_label(file_type: i32) -> &'static str {
    match file_type {
        1 => "目录",
        2 => "图片",
        _ => "文件",
    }
}

fn format_size_i64(size: Option<i64>) -> String {
    size.and_then(|value| (value >= 0).then_some(value as u64))
        .map(|value| format_size_u64(Some(value)))
        .unwrap_or_else(|| "-".to_string())
}

fn format_size_u64(size: Option<u64>) -> String {
    let Some(mut value) = size else {
        return "-".to_string();
    };
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut unit = 0usize;
    let mut display = value as f64;
    while value >= 1024 && unit < units.len() - 1 {
        display /= 1024.0;
        value /= 1024;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", display as u64, units[unit])
    } else {
        format!("{display:.1} {}", units[unit])
    }
}

fn format_timestamp(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|value| chrono::DateTime::from_timestamp(value, 0))
        .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".to_string())
}
