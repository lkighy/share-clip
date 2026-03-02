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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
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
            AppError::Io(e) => ApiError {
                code: "IO_ERROR",
                message: e.to_string(),
            },
            AppError::Json(e) => ApiError {
                code: "SERIALIZATION_ERROR",
                message: e.to_string(),
            }
        }
    }
}

impl From<std::string::FromUtf8Error> for AppError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        AppError::InvalidInput(format!("无效的 UTF-8 数据: {}", err))
    }
}
