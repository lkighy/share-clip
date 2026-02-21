use serde::Serialize;
use thiserror::Error;

//
// ========================
// 1️⃣ 内部错误类型（业务层使用）
// ========================
//

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error")]
    Database(#[from] sea_orm::DbErr),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found")]
    NotFound,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}

//
// ========================
// 2️⃣ 对前端暴露的 API 错误
// ========================
//

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

//
// ========================
// 3️⃣ 统一转换规则
// ========================
//

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Database(e) => ApiError {
                code: "DATABASE_ERROR",
                message: e.to_string(),
            },
            AppError::InvalidInput(msg) => ApiError {
                code: "INVALID_INPUT",
                message: msg,
            },
            AppError::NotFound => ApiError {
                code: "NOT_FOUND",
                message: "Resource not found".into(),
            },
            AppError::Unauthorized => ApiError {
                code: "UNAUTHORIZED",
                message: "Unauthorized".into(),
            },
            AppError::Internal(e) => ApiError {
                code: "INTERNAL_ERROR",
                message: e.to_string(),
            },
        }
    }
}
