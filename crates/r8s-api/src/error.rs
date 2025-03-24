// crates/r8s-api/src/error.rs
//! Error types for the API layer.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// API error types.
#[derive(Error, Debug)]
pub enum ApiError {
    /// Not found error
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Authorization error
    #[error("Authorization error: {0}")]
    Authorization(String),

    /// Bad request error
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Conflict error
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Core domain error
    #[error("Domain error: {0}")]
    Domain(#[from] r8s_core::error::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Execution error
    #[error("Execution error: {0}")]
    Execution(String),
}

/// Result type for API operations
pub type Result<T> = std::result::Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Authentication(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Authorization(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Domain(err) => match err {
                r8s_core::error::Error::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
                r8s_core::error::Error::Unauthorized(msg) => (StatusCode::FORBIDDEN, msg),
                r8s_core::error::Error::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
                r8s_core::error::Error::Conflict(msg) => (StatusCode::CONFLICT, msg),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            },
            ApiError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Execution(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "status": status.as_u16()
            }
        }));

        (status, body).into_response()
    }
}

impl From<r8s_persistence::error::PersistenceError> for ApiError {
    fn from(err: r8s_persistence::error::PersistenceError) -> Self {
        ApiError::Database(err.to_string())
    }
}

impl From<r8s_execution::error::ExecutionError> for ApiError {
    fn from(err: r8s_execution::error::ExecutionError) -> Self {
        ApiError::Execution(err.to_string())
    }
}

impl From<r8s_plugin::error::PluginError> for ApiError {
    fn from(err: r8s_plugin::error::PluginError) -> Self {
        ApiError::Internal(format!("Plugin error: {}", err))
    }
}
