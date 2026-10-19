//! Single-use links for mailbox users: first-login activation and password
//! reset (guide 04 §5).
//!
//! Only SHA-256(token) is stored. Issuing a new token of a purpose
//! invalidates older ones, and consumption is an atomic UPDATE ... RETURNING,
//! so a link can never be used twice even under concurrent requests.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    crypto,
    error::{ApiResult, AppError},
};

pub const ACTIVATE: &str = "activate";
pub const RESET: &str = "reset_password";

pub fn ttl(purpose: &str) -> Duration {
    if purpose == ACTIVATE {
        Duration::days(7)
    } else {
        Duration::hours(1)
    }
}

/// Create a token and return (raw token, expiry).
pub async fn issue(
    tx: &mut PgConnection,
    mailbox_id: Uuid,
    purpose: &str,
    delivery: &str,
    created_by: Option<Uuid>,
) -> ApiResult<(String, DateTime<Utc>)> {
    sqlx::query("UPDATE mailbox_tokens SET used_at = now() WHERE mailbox_id = $1 AND purpose = $2 AND used_at IS NULL")
        .bind(mailbox_id)
        .bind(purpose)
        .execute(&mut *tx)
        .await?;
    let token = crypto::random_token(32);
    let expires_at = Utc::now() + ttl(purpose);
    sqlx::query(
        "INSERT INTO mailbox_tokens (mailbox_id, purpose, token_hash, delivery, created_by_user_id, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(mailbox_id)
    .bind(purpose)
    .bind(crypto::sha256(&token))
    .bind(delivery)
    .bind(created_by)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    Ok((token, expires_at))
}

/// Look up a live token without consuming it (for the confirmation page).
pub async fn peek(executor: &mut PgConnection, token: &str) -> ApiResult<(Uuid, String, DateTime<Utc>)> {
    sqlx::query_as(
        "SELECT mailbox_id, purpose, expires_at FROM mailbox_tokens
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(crypto::sha256(token.trim()))
    .fetch_optional(&mut *executor)
    .await?
    .ok_or_else(invalid)
}

/// Consume a token of one of the given purposes; returns (mailbox, purpose).
pub async fn consume(tx: &mut PgConnection, token: &str, purposes: &[&str]) -> ApiResult<(Uuid, String)> {
    let purposes: Vec<String> = purposes.iter().map(|p| p.to_string()).collect();
    sqlx::query_as(
        "UPDATE mailbox_tokens SET used_at = now()
         WHERE token_hash = $1 AND purpose = ANY($2) AND used_at IS NULL AND expires_at > now()
         RETURNING mailbox_id, purpose",
    )
    .bind(crypto::sha256(token.trim()))
    .bind(&purposes)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(invalid)
}

fn invalid() -> AppError {
    AppError::bad_request(
        "invalid_token",
        "This link is invalid, has expired or was already used. Ask your IT team for a new one.",
    )
}
