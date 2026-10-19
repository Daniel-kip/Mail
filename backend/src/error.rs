//! API error model.
//!
//! Every error response has the shape
//! `{"error": {"code": "...", "message": "...", "request_id": "..."}}`.
//! `code` is stable and machine-readable; `message` is safe to show users.
//! Internal details are logged with the request id and never returned.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

tokio::task_local! {
    /// Set by the request-id middleware for the duration of each request.
    pub static REQUEST_ID: String;
}

pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(|id| id.clone()).ok()
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Client { status: StatusCode, code: &'static str, message: String, details: Option<Value> },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub type ApiResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        AppError::Client { status, code, message: message.into(), details: None }
    }

    pub fn with_details(self, details: Value) -> Self {
        match self {
            AppError::Client { status, code, message, .. } => {
                AppError::Client { status, code, message, details: Some(details) }
            }
            other => other,
        }
    }

    /// Replace the machine-readable code, keeping status and message.
    pub fn with_code(self, code: &'static str) -> Self {
        match self {
            AppError::Client { status, message, details, .. } => AppError::Client { status, code, message, details },
            other => other,
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed", message)
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthenticated", "Authentication is required")
    }
    pub fn invalid_credentials() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "Invalid email or password")
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    pub fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", format!("{what} not found"))
    }
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", message)
    }
    pub fn quota_exceeded(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "quota_exceeded", message)
    }
    pub fn unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }
    pub fn internal(message: impl std::fmt::Display) -> Self {
        AppError::Internal(anyhow::anyhow!("{message}"))
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Client { status, .. } => *status,
            AppError::Database(sqlx::Error::PoolTimedOut) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let request_id = current_request_id();
        let status = self.status();
        let (code, message, details) = match self {
            AppError::Client { code, message, details, .. } => (code, message, details),
            AppError::Database(sqlx::Error::PoolTimedOut) => {
                tracing::error!(request_id = ?request_id, "database pool exhausted");
                ("service_unavailable", "The service is temporarily overloaded. Retry shortly.".into(), None)
            }
            AppError::Database(e) => {
                tracing::error!(request_id = ?request_id, error = %e, "database error");
                ("internal_error", "An internal error occurred".into(), None)
            }
            AppError::Internal(e) => {
                tracing::error!(request_id = ?request_id, error = ?e, "internal error");
                ("internal_error", "An internal error occurred".into(), None)
            }
        };
        let mut body = json!({ "code": code, "message": message, "request_id": request_id });
        if let Some(d) = details {
            body["details"] = d;
        }
        (status, Json(json!({ "error": body }))).into_response()
    }
}

/// Map unique-constraint violations to a friendly conflict error.
pub fn on_unique_violation(err: sqlx::Error, code: &'static str, message: &str) -> AppError {
    if let sqlx::Error::Database(db) = &err {
        if db.code().as_deref() == Some("23505") {
            return AppError::conflict(code, message);
        }
    }
    AppError::Database(err)
}
