mod browser;
mod client;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::Manager;

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

async fn health(State(_state): State<HttpState>) -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
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

fn authorize_browser_request(state: &HttpState, headers: &HeaderMap) -> Result<(), Response> {
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

    if !config.share_server_password_enabled {
        return Ok(());
    }

    let expected = config
        .share_server_password_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(expected) = expected else {
        return Err(browser_auth_required());
    };

    if browser_basic_password(headers).as_deref() == Some(expected) {
        return Ok(());
    }

    Err(browser_auth_required())
}

fn browser_basic_password(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    decoded
        .split_once(':')
        .map(|(_, password)| password.to_string())
}

fn browser_auth_required() -> Response {
    let mut response = html_error(
        StatusCode::UNAUTHORIZED,
        "需要访问密码",
        "请输入 Share Clip 共享服务器访问密码。",
    );
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static(r#"Basic realm="Share Clip", charset="UTF-8""#),
    );
    response
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
</style>
<main><h1>{title}</h1><p>{message}</p></main>
</html>"#,
            title = escape_html(title),
            message = escape_html(message),
        ),
    )
        .into_response()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
