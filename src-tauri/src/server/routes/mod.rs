mod browser;
mod client;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::DatabaseConnection;
use serde::Serialize;

#[derive(Clone)]
pub struct HttpState {
    pub db: DatabaseConnection,
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
        .with_state(state)
}

async fn health(State(_state): State<HttpState>) -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
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
