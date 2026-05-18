mod browser;
mod client;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::Manager;

pub use crate::server::web_auth::WebAccessScope;

#[derive(Clone)]
pub struct HttpState {
    pub db: DatabaseConnection,
    pub app: tauri::AppHandle,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(browser::router())
        .merge(client::router())
        .layer(middleware::from_fn(cors))
        .with_state(state)
}

async fn health(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if let Err(response) = authorize_browser_request(&state, &headers, WebAccessScope::Files) {
        return response;
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn cors(req: Request, next: Next) -> Response {
    let mut response = if req.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(req).await
    };
    apply_cors_headers(response.headers_mut());
    response
}

fn apply_cors_headers(headers: &mut HeaderMap) {
    headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static(
            "authorization, content-type, range, x-share-clip-user-id, x-share-clip-device-id",
        ),
    );
    headers.insert(
        "access-control-expose-headers",
        HeaderValue::from_static(
            "accept-ranges, content-disposition, content-length, content-range",
        ),
    );
    headers.insert(
        "access-control-allow-private-network",
        HeaderValue::from_static("true"),
    );
    headers.insert("access-control-max-age", HeaderValue::from_static("86400"));
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

fn authorize_browser_request(
    state: &HttpState,
    headers: &HeaderMap,
    scope: WebAccessScope,
) -> Result<(), Response> {
    let config = state
        .app
        .state::<crate::app::config::AppConfigStore>()
        .get();
    if !config.browser_access_enabled {
        return Err(html_error(
            StatusCode::FORBIDDEN,
            "浏览器访问已关闭",
            "请在 Share Clip 设置中开启浏览器访问。",
        ));
    }

    if !crate::server::web_auth::is_scope_enabled(&config, scope) {
        return Err(html_error(
            StatusCode::FORBIDDEN,
            "没有访问权限",
            "当前网页访问权限未启用，请在 Share Clip 设置中调整 Web 权限粒度。",
        ));
    }

    if !config.web_access_auth_required {
        return Ok(());
    }

    let auth = state.app.state::<crate::server::web_auth::WebAuthState>();
    if cookie_value(headers, crate::server::web_auth::COOKIE_NAME)
        .and_then(|token| auth.validate_session(&token, scope))
        .is_some()
    {
        return Ok(());
    }

    Err(browser_auth_required())
}

fn browser_auth_required() -> Response {
    html_error(
        StatusCode::UNAUTHORIZED,
        "需要授权",
        r#"请先打开 <a href="/auth">Web 授权页面</a>，使用访问密码或临时确认后再继续访问。"#,
    )
}

fn html_error(status: StatusCode, title: &str, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            r#"<!doctype html>
<html lang="zh-CN">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: #f6f8fb; color: #0f172a; font-family: "Segoe UI", system-ui, sans-serif; }}
  main {{ width: min(520px, calc(100vw - 32px)); border: 1px solid #dbe3ee; border-radius: 10px; background: white; padding: 26px; box-shadow: 0 12px 32px rgba(15,23,42,.08); }}
  h1 {{ margin: 0 0 10px; font-size: 22px; }}
  p {{ margin: 0; color: #475569; line-height: 1.7; }}
  a {{ color: #0369a1; text-decoration: none; }}
</style>
<main><h1>{title}</h1><p>{message}</p></main>
</html>"#,
            title = escape_html(title),
            message = message,
        ),
    )
        .into_response()
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

pub fn web_session_cookie(token: &str, max_age_seconds: u64) -> String {
    format!(
        "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        crate::server::web_auth::COOKIE_NAME,
        token,
        max_age_seconds.max(60)
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
