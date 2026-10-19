//! Shared request/response helpers for the API handlers.

use axum::{
    extract::{rejection::JsonRejection, rejection::QueryRejection, FromRequest, FromRequestParts},
    http::StatusCode,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// `Json<T>` whose rejections use the standard error envelope.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct ApiJson<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(rej: JsonRejection) -> Self {
        let status = match rej {
            JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            JsonRejection::BytesRejection(_) => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        let code = match status {
            StatusCode::UNSUPPORTED_MEDIA_TYPE => "unsupported_media_type",
            StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
            _ => "validation_failed",
        };
        AppError::new(status, code, rej.body_text())
    }
}

/// `Query<T>` with the standard error envelope.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct ApiQuery<T>(pub T);

impl From<QueryRejection> for AppError {
    fn from(rej: QueryRejection) -> Self {
        AppError::validation(rej.body_text())
    }
}

/// Opaque keyset cursor over (created_at DESC, id DESC).
#[derive(Debug, Serialize, Deserialize)]
pub struct Cursor {
    pub t: DateTime<Utc>,
    pub id: Uuid,
}

impl Cursor {
    pub fn encode(t: DateTime<Utc>, id: Uuid) -> String {
        B64URL.encode(serde_json::to_vec(&Cursor { t, id }).expect("cursor serializes"))
    }

    pub fn decode(raw: Option<&str>) -> Result<Option<Cursor>, AppError> {
        match raw.filter(|s| !s.is_empty()) {
            None => Ok(None),
            Some(s) => {
                let bytes = B64URL.decode(s).map_err(|_| AppError::validation("cursor is invalid"))?;
                serde_json::from_slice(&bytes).map(Some).map_err(|_| AppError::validation("cursor is invalid"))
            }
        }
    }
}

pub fn page_limit(requested: Option<i64>, default: i64, max: i64) -> i64 {
    requested.unwrap_or(default).clamp(1, max)
}

/// Deserialize helper: treat `""` as `None` for optional string query params.
pub fn empty_as_none(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}
