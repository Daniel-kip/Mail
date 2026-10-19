//! Opaque server-side sessions carried in HttpOnly cookies.
//!
//! * The cookie holds 256 bits of randomness; the database holds its SHA-256.
//! * Each session has a CSRF token that must be echoed in `X-CSRF-Token` on
//!   every state-changing request (synchronizer-token pattern). The cookie is
//!   also `SameSite=Lax`, so this is defense in depth.
//! * Sessions are revoked on logout, password change/reset and disablement.

use axum::http::{header, HeaderMap, HeaderValue};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{config::Config, crypto, error::ApiResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Console,
    Webmail,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Console => "console",
            SessionKind::Webmail => "webmail",
        }
    }

    pub fn cookie_name(self, cfg: &Config) -> &'static str {
        // The __Host- prefix pins the cookie to this exact origin over HTTPS.
        match (self, cfg.cookie_secure) {
            (SessionKind::Console, true) => "__Host-mv_session",
            (SessionKind::Console, false) => "mv_session",
            (SessionKind::Webmail, true) => "__Host-mv_mail",
            (SessionKind::Webmail, false) => "mv_mail",
        }
    }
}

pub struct NewSession {
    pub id: Uuid,
    pub token: String,
    pub csrf_token: String,
}

pub struct SessionOwner {
    pub user_id: Option<Uuid>,
    pub mailbox_id: Option<Uuid>,
    pub active_organization_id: Option<Uuid>,
    pub mfa_pending: bool,
}

pub async fn create(
    db: &PgPool,
    cfg: &Config,
    kind: SessionKind,
    owner: SessionOwner,
    ip: Option<std::net::IpAddr>,
    user_agent: Option<&str>,
) -> ApiResult<NewSession> {
    let token = crypto::random_token(32);
    let csrf_token = crypto::random_token(24);
    // A session waiting for its second factor gets a short life.
    let ttl = if owner.mfa_pending { Duration::minutes(10) } else { Duration::hours(cfg.session_ttl_hours) };
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (kind, token_hash, csrf_token, user_id, mailbox_id, active_organization_id,
                               mfa_pending, ip, user_agent, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(kind.as_str())
    .bind(crypto::sha256(&token))
    .bind(&csrf_token)
    .bind(owner.user_id)
    .bind(owner.mailbox_id)
    .bind(owner.active_organization_id)
    .bind(owner.mfa_pending)
    .bind(ip.map(ipnetwork::IpNetwork::from))
    .bind(user_agent.map(|ua| ua.chars().take(400).collect::<String>()))
    .bind(Utc::now() + ttl)
    .fetch_one(db)
    .await?;
    Ok(NewSession { id, token, csrf_token })
}

pub fn token_from_headers(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == cookie_name)
        .map(|(_, value)| value.to_string())
}

pub fn set_cookie(cfg: &Config, kind: SessionKind, token: &str, max_age_secs: i64) -> HeaderValue {
    let secure = if cfg.cookie_secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}{secure}",
        kind.cookie_name(cfg)
    ))
    .expect("cookie value is ASCII")
}

pub fn clear_cookie(cfg: &Config, kind: SessionKind) -> HeaderValue {
    set_cookie(cfg, kind, "", 0)
}

pub async fn revoke(db: &PgPool, session_id: Uuid) -> ApiResult<()> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(session_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Revoke every console session of a user, optionally keeping one.
pub async fn revoke_all_for_user(db: &PgPool, user_id: Uuid, except: Option<Uuid>) -> ApiResult<u64> {
    let r = sqlx::query(
        "UPDATE sessions SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)",
    )
    .bind(user_id)
    .bind(except)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

pub async fn revoke_all_for_mailbox(db: &PgPool, mailbox_id: Uuid, except: Option<Uuid>) -> ApiResult<u64> {
    let r = sqlx::query(
        "UPDATE sessions SET revoked_at = now()
         WHERE mailbox_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)",
    )
    .bind(mailbox_id)
    .bind(except)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

/// Transactional variant used when revocation must commit with another change.
pub async fn revoke_all_for_mailbox_tx(tx: &mut sqlx::PgConnection, mailbox_id: Uuid) -> ApiResult<u64> {
    let r = sqlx::query("UPDATE sessions SET revoked_at = now() WHERE mailbox_id = $1 AND revoked_at IS NULL")
        .bind(mailbox_id)
        .execute(&mut *tx)
        .await?;
    Ok(r.rows_affected())
}
