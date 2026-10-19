//! Authentication extractors.
//!
//! * [`ClientInfo`]       – caller IP and user agent (for audit and limits).
//! * [`ConsoleSession`]   – a logged-in console user (cookie + CSRF).
//! * [`PendingSession`]   – a console session still awaiting its MFA code.
//! * [`Principal`]        – the tenant-scoped actor for `/v1` resources,
//!   either a console user or an API key.
//! * [`PlatformAdmin`]    – a platform operator (MFA required in production).
//! * [`MailboxSession`]   – a webmail user (separate cookie and CSRF token).
//!
//! The tenant is always derived from the credential (membership or key),
//! never from a caller-supplied organization id (guide 07, tenant isolation).

pub mod api_key;
pub mod permissions;
pub mod session;

use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts, Method},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub use permissions::{Permission, Role};
use session::SessionKind;

use crate::{crypto, error::AppError, state::AppState};

// ── Client info ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

impl ClientInfo {
    pub fn ip_network(&self) -> Option<ipnetwork::IpNetwork> {
        self.ip.map(ipnetwork::IpNetwork::from)
    }
    pub fn ip_key(&self) -> String {
        self.ip.map(|i| i.to_string()).unwrap_or_else(|| "unknown".into())
    }
}

pub fn client_info(parts: &Parts, state: &AppState) -> ClientInfo {
    let forwarded = if state.cfg.trust_proxy_headers {
        // Nginx sets X-Real-IP to the address it accepted the connection from.
        parts.headers.get("x-real-ip").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<IpAddr>().ok())
    } else {
        None
    };
    let ip = forwarded.or_else(|| parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|ci| ci.0.ip()));
    let user_agent =
        parts.headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|s| s.chars().take(400).collect());
    ClientInfo { ip, user_agent }
}

impl FromRequestParts<AppState> for ClientInfo {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(client_info(parts, state))
    }
}

fn is_unsafe_method(m: &Method) -> bool {
    !matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn check_csrf(parts: &Parts, expected: &str) -> Result<(), AppError> {
    if !is_unsafe_method(&parts.method) {
        return Ok(());
    }
    let provided = parts.headers.get("x-csrf-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if crypto::constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::forbidden("Missing or invalid CSRF token").with_code("csrf_failed"))
    }
}

// ── Console sessions ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ActiveOrg {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
    pub status: String,
    pub plan_id: String,
    pub account_type: String,
}

#[derive(Debug, Clone)]
pub struct ConsoleSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub is_platform_admin: bool,
    pub email_verified: bool,
    pub mfa_enabled: bool,
    pub mfa_pending: bool,
    pub csrf_token: String,
    pub active_org: Option<ActiveOrg>,
    pub expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    csrf_token: String,
    mfa_pending: bool,
    expires_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    email: String,
    full_name: String,
    user_status: String,
    is_platform_admin: bool,
    email_verified_at: Option<DateTime<Utc>>,
    mfa_enabled: bool,
    org_id: Option<Uuid>,
    org_name: Option<String>,
    role: Option<String>,
    org_status: Option<String>,
    plan_id: Option<String>,
    account_type: Option<String>,
}

async fn load_console_session(parts: &Parts, state: &AppState) -> Result<ConsoleSession, AppError> {
    let token = session::token_from_headers(&parts.headers, SessionKind::Console.cookie_name(&state.cfg))
        .ok_or_else(AppError::unauthorized)?;
    // Role and org status are re-read on every request, so role changes and
    // suspensions take effect immediately.
    let row: SessionRow = sqlx::query_as(
        "SELECT s.id, s.user_id, s.csrf_token, s.mfa_pending, s.expires_at, s.last_seen_at,
                u.email::text AS email, u.full_name, u.status AS user_status, u.is_platform_admin,
                COALESCE(u.email_verified_at, u.phone_verified_at) AS email_verified_at, u.mfa_enabled,
                o.id AS org_id, o.name AS org_name, m.role, o.status AS org_status, o.plan_id, o.account_type
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         LEFT JOIN memberships m ON m.user_id = s.user_id AND m.organization_id = s.active_organization_id
         LEFT JOIN organizations o ON o.id = m.organization_id
         WHERE s.token_hash = $1 AND s.kind = 'console' AND s.revoked_at IS NULL AND s.expires_at > now()",
    )
    .bind(crypto::sha256(&token))
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(AppError::unauthorized)?;

    if row.user_status != "active" {
        return Err(AppError::unauthorized());
    }
    check_csrf(parts, &row.csrf_token)?;

    if Utc::now() - row.last_seen_at > chrono::Duration::seconds(60) {
        let db = state.db.clone();
        let id = row.id;
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE id = $1").bind(id).execute(&db).await;
        });
    }

    let active_org = match (row.org_id, row.role.as_deref().and_then(Role::parse)) {
        (Some(id), Some(role)) => Some(ActiveOrg {
            id,
            name: row.org_name.unwrap_or_default(),
            role,
            status: row.org_status.unwrap_or_default(),
            plan_id: row.plan_id.unwrap_or_default(),
            account_type: row.account_type.unwrap_or_else(|| "institution".into()),
        }),
        _ => None,
    };

    Ok(ConsoleSession {
        session_id: row.id,
        user_id: row.user_id,
        email: row.email,
        full_name: row.full_name,
        is_platform_admin: row.is_platform_admin,
        email_verified: row.email_verified_at.is_some(),
        mfa_enabled: row.mfa_enabled,
        mfa_pending: row.mfa_pending,
        csrf_token: row.csrf_token,
        active_org,
        expires_at: row.expires_at,
    })
}

impl FromRequestParts<AppState> for ConsoleSession {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let s = load_console_session(parts, state).await?;
        if s.mfa_pending {
            return Err(AppError::new(
                axum::http::StatusCode::UNAUTHORIZED,
                "mfa_required",
                "Enter your authentication code to finish signing in",
            ));
        }
        Ok(s)
    }
}

/// A session that may still be waiting for its second factor.
pub struct PendingSession(pub ConsoleSession);

impl FromRequestParts<AppState> for PendingSession {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(PendingSession(load_console_session(parts, state).await?))
    }
}

pub struct PlatformAdmin(pub ConsoleSession);

impl FromRequestParts<AppState> for PlatformAdmin {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let s = ConsoleSession::from_request_parts(parts, state).await?;
        if !s.is_platform_admin {
            return Err(AppError::not_found("Resource"));
        }
        if state.cfg.is_production() && !s.mfa_enabled {
            return Err(AppError::forbidden("Platform operators must enable two-factor authentication")
                .with_code("mfa_enrollment_required"));
        }
        Ok(PlatformAdmin(s))
    }
}

// ── Principal (tenant-scoped actor) ──────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Actor {
    User {
        user_id: Uuid,
        email: String,
        role: Role,
        session_id: Uuid,
        email_verified: bool,
    },
    ApiKey {
        key_id: Uuid,
        prefix: String,
        scopes: Vec<Permission>,
        sandbox: bool,
        /// Sender domains this key may use; empty means any verified domain.
        sender_domains: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub org_id: Uuid,
    pub org_status: String,
    pub plan_id: String,
    pub account_type: String,
    pub actor: Actor,
}

impl Principal {
    pub fn can(&self, p: Permission) -> bool {
        match &self.actor {
            Actor::User { role, .. } => role.allows(p),
            Actor::ApiKey { scopes, .. } => scopes.contains(&p),
        }
    }

    pub fn require(&self, p: Permission) -> Result<(), AppError> {
        if !self.can(p) {
            return Err(AppError::forbidden(format!("This action requires the '{}' permission", p.as_str()))
                .with_code("insufficient_permissions"));
        }
        if p.is_write() && self.org_status != "active" {
            return Err(AppError::forbidden("This organization is suspended. Contact support.")
                .with_code("organization_suspended"));
        }
        Ok(())
    }

    pub fn user_id(&self) -> Option<Uuid> {
        match &self.actor {
            Actor::User { user_id, .. } => Some(*user_id),
            _ => None,
        }
    }

    pub fn api_key_id(&self) -> Option<Uuid> {
        match &self.actor {
            Actor::ApiKey { key_id, .. } => Some(*key_id),
            _ => None,
        }
    }

    pub fn role(&self) -> Option<Role> {
        match &self.actor {
            Actor::User { role, .. } => Some(*role),
            _ => None,
        }
    }

    pub fn is_sandbox(&self) -> bool {
        matches!(&self.actor, Actor::ApiKey { sandbox: true, .. })
    }

    pub fn is_institution(&self) -> bool {
        self.account_type == "institution"
    }

    /// Hosted mailboxes, aliases and mail hosting are institutional features.
    pub fn require_institution(&self) -> Result<(), AppError> {
        if self.is_institution() {
            return Ok(());
        }
        Err(AppError::forbidden(
            "Hosted work mailboxes are part of institutional accounts. Convert this workspace to an institution in Settings to use them.",
        )
        .with_code("institution_required"))
    }

    /// Enforce an API key's sender-domain restriction (no-op for users).
    pub fn check_sender_domain(&self, domain: &str) -> Result<(), AppError> {
        if let Actor::ApiKey { sender_domains, .. } = &self.actor {
            if !sender_domains.is_empty() && !sender_domains.iter().any(|d| d.eq_ignore_ascii_case(domain)) {
                return Err(AppError::forbidden(format!("This API key is not allowed to send from {domain}"))
                    .with_code("sender_domain_not_allowed"));
            }
        }
        Ok(())
    }

    /// (actor_type, actor_id, label) for audit records.
    pub fn audit_actor(&self) -> (&'static str, Uuid, String) {
        match &self.actor {
            Actor::User { user_id, email, .. } => ("user", *user_id, email.clone()),
            Actor::ApiKey { key_id, prefix, .. } => ("api_key", *key_id, prefix.clone()),
        }
    }
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: Uuid,
    organization_id: Uuid,
    prefix: String,
    secret_hash: Vec<u8>,
    environment: String,
    scopes: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    org_status: String,
    plan_id: String,
    account_type: String,
    api_requests_per_minute: i32,
    allowed_ips: Vec<ipnetwork::IpNetwork>,
    allowed_sender_domains: Vec<String>,
}

async fn principal_from_api_key(raw: &str, parts: &Parts, state: &AppState) -> Result<Principal, AppError> {
    let invalid = || AppError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid_api_key", "The API key is invalid");
    let prefix = api_key::prefix_of(raw).ok_or_else(invalid)?;
    let row: KeyRow = sqlx::query_as(
        "SELECT k.id, k.organization_id, k.prefix, k.secret_hash, k.environment, k.scopes,
                k.expires_at, k.revoked_at, o.status AS org_status, o.plan_id, o.account_type, p.api_requests_per_minute,
                k.allowed_ips, k.allowed_sender_domains::text[] AS allowed_sender_domains
         FROM api_keys k
         JOIN organizations o ON o.id = k.organization_id
         JOIN plans p ON p.id = o.plan_id
         WHERE k.prefix = $1",
    )
    .bind(prefix)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(invalid)?;

    if !crypto::constant_time_eq(&crypto::sha256(raw), &row.secret_hash) {
        return Err(invalid());
    }
    if row.revoked_at.is_some() {
        return Err(AppError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "api_key_revoked",
            "This API key has been revoked",
        ));
    }
    if row.expires_at.is_some_and(|e| e <= Utc::now()) {
        return Err(AppError::new(axum::http::StatusCode::UNAUTHORIZED, "api_key_expired", "This API key has expired"));
    }
    if row.org_status != "active" {
        return Err(
            AppError::forbidden("This organization is suspended. Contact support.").with_code("organization_suspended")
        );
    }

    let client = client_info(parts, state);
    if !row.allowed_ips.is_empty() {
        let allowed = client.ip.is_some_and(|ip| row.allowed_ips.iter().any(|net| net.contains(ip)));
        if !allowed {
            return Err(AppError::forbidden(
                "This API key cannot be used from this IP address. Update the key's IP allowlist in the console.",
            )
            .with_code("api_key_ip_not_allowed"));
        }
    }

    state.limiter.check(&format!("key:{}", row.id), row.api_requests_per_minute.max(1) as u32, 60)?;

    let ip = client.ip_network();
    let db = state.db.clone();
    let key_id = row.id;
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE api_keys SET last_used_at = now(), last_used_ip = $2
             WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')",
        )
        .bind(key_id)
        .bind(ip)
        .execute(&db)
        .await;
    });

    Ok(Principal {
        org_id: row.organization_id,
        org_status: row.org_status,
        plan_id: row.plan_id,
        account_type: row.account_type,
        actor: Actor::ApiKey {
            key_id: row.id,
            prefix: row.prefix,
            scopes: row.scopes.iter().filter_map(|s| Permission::parse(s)).collect(),
            sandbox: row.environment == "test",
            sender_domains: row.allowed_sender_domains,
        },
    })
}

impl FromRequestParts<AppState> for Principal {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        if let Some(auth) = parts.headers.get(header::AUTHORIZATION) {
            let value = auth.to_str().map_err(|_| AppError::unauthorized())?;
            let token = value.strip_prefix("Bearer ").or_else(|| value.strip_prefix("bearer ")).ok_or_else(|| {
                AppError::bad_request("invalid_authorization", "Use 'Authorization: Bearer <api key>'")
            })?;
            return principal_from_api_key(token.trim(), parts, state).await;
        }

        let s = ConsoleSession::from_request_parts(parts, state).await?;
        let org = s.active_org.ok_or_else(|| {
            AppError::forbidden("Select or create an organization first").with_code("no_active_organization")
        })?;
        Ok(Principal {
            org_id: org.id,
            org_status: org.status,
            plan_id: org.plan_id,
            account_type: org.account_type,
            actor: Actor::User {
                user_id: s.user_id,
                email: s.email,
                role: org.role,
                session_id: s.session_id,
                email_verified: s.email_verified,
            },
        })
    }
}

// ── Webmail sessions ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MailboxSession {
    pub session_id: Uuid,
    pub mailbox_id: Uuid,
    pub organization_id: Uuid,
    pub domain_id: Uuid,
    pub address: String,
    pub display_name: Option<String>,
    pub quota_bytes: i64,
    pub csrf_token: String,
}

#[derive(sqlx::FromRow)]
struct MailboxSessionRow {
    id: Uuid,
    mailbox_id: Uuid,
    csrf_token: String,
    organization_id: Uuid,
    domain_id: Uuid,
    address: String,
    display_name: Option<String>,
    quota_bytes: i64,
    live: bool,
}

impl FromRequestParts<AppState> for MailboxSession {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = session::token_from_headers(&parts.headers, SessionKind::Webmail.cookie_name(&state.cfg))
            .ok_or_else(AppError::unauthorized)?;
        let row: MailboxSessionRow = sqlx::query_as(
            "SELECT s.id, s.mailbox_id, s.csrf_token, m.organization_id, m.domain_id,
                    m.address::text AS address, m.display_name, m.quota_bytes,
                    EXISTS (SELECT 1 FROM mail_mailboxes mm WHERE mm.address = m.address::text) AS live
             FROM sessions s
             JOIN mailboxes m ON m.id = s.mailbox_id
             WHERE s.token_hash = $1 AND s.kind = 'webmail' AND s.revoked_at IS NULL AND s.expires_at > now()",
        )
        .bind(crypto::sha256(&token))
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::unauthorized)?;
        // Disabled mailbox, unverified domain or suspended tenant ends access.
        if !row.live {
            return Err(AppError::unauthorized());
        }
        check_csrf(parts, &row.csrf_token)?;
        Ok(MailboxSession {
            session_id: row.id,
            mailbox_id: row.mailbox_id,
            organization_id: row.organization_id,
            domain_id: row.domain_id,
            address: row.address,
            display_name: row.display_name,
            quota_bytes: row.quota_bytes,
            csrf_token: row.csrf_token,
        })
    }
}
