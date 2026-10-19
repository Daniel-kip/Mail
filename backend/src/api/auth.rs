//! Console authentication: registration (username + phone or Google),
//! login by email, username or phone (+TOTP), SMS and email verification,
//! sessions and password management.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{AppendHeaders, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{
        session::{self, SessionKind, SessionOwner},
        ClientInfo, ConsoleSession, PendingSession, Role,
    },
    crypto::{self, purpose},
    error::{ApiResult, AppError},
    password,
    services::{audit::Entry, breach, google, sms, system_mail, totp},
    state::AppState,
    validation,
};

const MAX_FAILED_LOGINS: i32 = 10;
const LOCKOUT_MINUTES: i64 = 15;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/session", get(current_session))
        .route("/profile", axum::routing::patch(update_profile))
        .route("/switch-organization", post(switch_organization))
        .route("/password", post(change_password))
        .route("/password/forgot", post(forgot_password))
        .route("/password/reset", post(reset_password))
        .route("/email/verify", post(verify_email))
        .route("/email/resend", post(resend_verification))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", delete(revoke_session))
        .route("/mfa/verify", post(mfa_verify))
        .route("/mfa/setup", post(mfa_setup))
        .route("/mfa/enable", post(mfa_enable))
        .route("/mfa/disable", post(mfa_disable))
        .route("/providers", get(providers))
        .route("/phone/send", post(send_phone_code))
        .route("/phone/verify", post(verify_phone_code))
        .route("/google", post(google_sign_in))
}

// ── helpers ──────────────────────────────────────────────────────────────

fn with_cookie(state: &AppState, token: &str, body: Value) -> Response {
    let cookie = session::set_cookie(&state.cfg, SessionKind::Console, token, state.cfg.session_ttl_hours * 3600);
    (AppendHeaders([(header::SET_COOKIE, cookie)]), Json(body)).into_response()
}

#[derive(sqlx::FromRow)]
struct OrgListRow {
    id: Uuid,
    name: String,
    slug: String,
    role: String,
    status: String,
    plan_id: String,
    account_type: String,
}

async fn organizations_of(state: &AppState, user_id: Uuid) -> ApiResult<Vec<Value>> {
    let rows: Vec<OrgListRow> = sqlx::query_as(
        "SELECT o.id, o.name, o.slug::text AS slug, m.role, o.status, o.plan_id, o.account_type
         FROM memberships m JOIN organizations o ON o.id = m.organization_id
         WHERE m.user_id = $1 ORDER BY o.name",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({ "id": r.id, "name": r.name, "slug": r.slug, "role": r.role, "status": r.status,
                    "plan": r.plan_id, "account_type": r.account_type })
        })
        .collect())
}

async fn session_payload(state: &AppState, s: &ConsoleSession) -> ApiResult<Value> {
    let organization = s.active_org.as_ref().map(|o| {
        json!({
            "id": o.id, "name": o.name, "role": o.role.as_str(), "status": o.status, "plan": o.plan_id,
            "account_type": o.account_type, "permissions": o.role.permissions(),
        })
    });
    let identity: (Option<String>, Option<String>, bool, bool, bool) = sqlx::query_as(
        "SELECT username::text, phone, phone_verified_at IS NOT NULL, email_verified_at IS NOT NULL, google_sub IS NOT NULL
         FROM users WHERE id = $1",
    )
    .bind(s.user_id)
    .fetch_one(&state.db)
    .await?;
    let (username, phone, phone_verified, email_confirmed, google_linked) = identity;
    Ok(json!({
        "user": {
            "id": s.user_id, "email": s.email, "full_name": s.full_name, "username": username,
            "phone": phone, "phone_verified": phone_verified, "email_confirmed": email_confirmed,
            "google_linked": google_linked,
            // Verified by any channel: phone code, email link or Google.
            "email_verified": s.email_verified, "account_verified": s.email_verified,
            "mfa_enabled": s.mfa_enabled, "is_platform_admin": s.is_platform_admin,
        },
        "organization": organization,
        "organizations": organizations_of(state, s.user_id).await?,
        "csrf_token": s.csrf_token,
        "expires_at": s.expires_at,
        "environment": if state.cfg.is_production() { "production" } else { "development" },
    }))
}

/// Build the payload for a session created moments ago in this request.
async fn fresh_session_payload(state: &AppState, user_id: Uuid, session_id: Uuid, csrf: &str) -> ApiResult<Value> {
    #[derive(sqlx::FromRow)]
    struct Row {
        email: String,
        full_name: String,
        email_verified_at: Option<DateTime<Utc>>,
        mfa_enabled: bool,
        is_platform_admin: bool,
        expires_at: DateTime<Utc>,
        org_id: Option<Uuid>,
        org_name: Option<String>,
        role: Option<String>,
        org_status: Option<String>,
        plan_id: Option<String>,
        account_type: Option<String>,
    }
    let r: Row = sqlx::query_as(
        "SELECT u.email::text AS email, u.full_name, COALESCE(u.email_verified_at, u.phone_verified_at) AS email_verified_at,
                u.mfa_enabled, u.is_platform_admin, s.expires_at,
                o.id AS org_id, o.name AS org_name, m.role, o.status AS org_status, o.plan_id, o.account_type
         FROM sessions s JOIN users u ON u.id = s.user_id
         LEFT JOIN memberships m ON m.user_id = u.id AND m.organization_id = s.active_organization_id
         LEFT JOIN organizations o ON o.id = m.organization_id
         WHERE s.id = $1",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;
    let active_org = match (r.org_id, r.role.as_deref().and_then(Role::parse)) {
        (Some(id), Some(role)) => Some(crate::auth::ActiveOrg {
            id,
            name: r.org_name.unwrap_or_default(),
            role,
            status: r.org_status.unwrap_or_default(),
            plan_id: r.plan_id.unwrap_or_default(),
            account_type: r.account_type.unwrap_or_else(|| "institution".into()),
        }),
        _ => None,
    };
    let s = ConsoleSession {
        session_id,
        user_id,
        email: r.email,
        full_name: r.full_name,
        is_platform_admin: r.is_platform_admin,
        email_verified: r.email_verified_at.is_some(),
        mfa_enabled: r.mfa_enabled,
        mfa_pending: false,
        csrf_token: csrf.to_string(),
        active_org,
        expires_at: r.expires_at,
    };
    session_payload(state, &s).await
}

async fn default_org(state: &AppState, user_id: Uuid) -> ApiResult<Option<Uuid>> {
    Ok(sqlx::query_scalar("SELECT organization_id FROM memberships WHERE user_id = $1 ORDER BY created_at LIMIT 1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?)
}

/// Create a single-use emailed token; returns the raw token for the link.
async fn issue_user_token(
    tx: &mut sqlx::PgConnection,
    user_id: Uuid,
    purpose: &str,
    ttl: Duration,
) -> ApiResult<String> {
    // Invalidate older outstanding tokens of the same purpose.
    sqlx::query("UPDATE user_tokens SET used_at = now() WHERE user_id = $1 AND purpose = $2 AND used_at IS NULL")
        .bind(user_id)
        .bind(purpose)
        .execute(&mut *tx)
        .await?;
    let token = crypto::random_token(32);
    sqlx::query("INSERT INTO user_tokens (user_id, purpose, token_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(purpose)
        .bind(crypto::sha256(&token))
        .bind(Utc::now() + ttl)
        .execute(&mut *tx)
        .await?;
    Ok(token)
}

/// Consume a token atomically; returns the user id if it was valid.
async fn consume_user_token(tx: &mut sqlx::PgConnection, token: &str, purpose: &str) -> ApiResult<Uuid> {
    sqlx::query_scalar(
        "UPDATE user_tokens SET used_at = now()
         WHERE token_hash = $1 AND purpose = $2 AND used_at IS NULL AND expires_at > now()
         RETURNING user_id",
    )
    .bind(crypto::sha256(token.trim()))
    .bind(purpose)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::bad_request("invalid_token", "This link is invalid or has expired"))
}

pub async fn unique_org_slug(executor: &mut sqlx::PgConnection, name: &str) -> ApiResult<String> {
    let base = validation::slugify(name);
    for attempt in 0..6 {
        let candidate = if attempt == 0 { base.clone() } else { format!("{base}-{}", crypto::random_lower_alnum(4)) };
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM organizations WHERE slug = $1)")
            .bind(&candidate)
            .fetch_one(&mut *executor)
            .await?;
        if !taken {
            return Ok(candidate);
        }
    }
    Err(AppError::internal("could not allocate organization slug"))
}

/// Parse an account type; `None` keeps the historical default (institution).
pub fn parse_account_type(raw: Option<&str>) -> ApiResult<&'static str> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("institution") => Ok("institution"),
        Some("individual") => Ok("individual"),
        Some(_) => Err(AppError::validation("account_type must be 'individual' or 'institution'")),
    }
}

/// Create a tenant with its owner membership and subscription in `tx`.
/// Individuals start on the free Developer plan, institutions on the free
/// institutional plan.
pub async fn create_tenant(
    tx: &mut sqlx::PgConnection,
    name: &str,
    account_type: &str,
    owner: Uuid,
) -> ApiResult<(Uuid, String)> {
    let slug = unique_org_slug(tx, name).await?;
    let plan = if account_type == "individual" { "developer" } else { "free" };
    let org_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organizations (name, slug, account_type, plan_id) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(name)
    .bind(&slug)
    .bind(account_type)
    .bind(plan)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(org_id)
        .bind(owner)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO subscriptions (organization_id, plan_id) VALUES ($1, $2)")
        .bind(org_id)
        .bind(plan)
        .execute(&mut *tx)
        .await?;
    Ok((org_id, slug))
}

// ── registration & login ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterBody {
    full_name: String,
    username: Option<String>,
    email: String,
    phone: Option<String>,
    password: String,
    organization_name: Option<String>,
    account_type: Option<String>,
}

/// Usernames: 3-32 characters, lowercase letters, digits, dot, dash, underscore.
fn parse_username(raw: &str) -> ApiResult<String> {
    let u = raw.trim().trim_start_matches('@').to_lowercase();
    let valid = (3..=32).contains(&u.len())
        && u.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && u.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(AppError::validation(
            "Username must be 3-32 characters: letters, numbers, dots, dashes or underscores, starting with a letter or number",
        ));
    }
    Ok(u)
}

async fn register(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<RegisterBody>,
) -> ApiResult<Response> {
    if !state.cfg.allow_signup {
        return Err(AppError::forbidden("Self-service sign-up is disabled on this platform").with_details(json!({})));
    }
    state.limiter.check(&format!("register:{}", client.ip_key()), 10, 3600)?;

    let full_name = validation::non_empty("full_name", &body.full_name, 200)?;
    let email = validation::parse_email("email", &body.email)?.as_string().to_lowercase();
    let username = body.username.as_deref().map(parse_username).transpose()?;
    let phone = match body.phone.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(sms::normalize(p)?),
        None => None,
    };
    password::check_policy(&body.password, &email)?;
    if let Some(u) = &username {
        password::check_policy(&body.password, u)?;
    }
    breach::ensure_not_breached(&state.cfg, &body.password).await?;
    let account_type = parse_account_type(body.account_type.as_deref())?;
    let org_name = match body.organization_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => validation::non_empty("organization_name", n, 200)?,
        None if account_type == "individual" => full_name.clone(),
        None => format!("{full_name}'s organization"),
    };

    let password_hash = password::hash(&body.password).await?;
    let mut tx = state.db.begin().await?;

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, full_name, password_hash, username, phone) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&email)
    .bind(&full_name)
    .bind(&password_hash)
    .bind(&username)
    .bind(&phone)
    .fetch_one(&mut *tx)
    .await
    .map_err(unique_identity_violation)?;

    let (org_id, slug) = create_tenant(&mut tx, &org_name, account_type, user_id).await?;

    // Verify by SMS code when a phone was given and SMS is available;
    // otherwise fall back to an emailed link.
    let by_phone = phone.is_some() && sms::available(&state);
    if !by_phone {
        let token = issue_user_token(&mut tx, user_id, "verify_email", Duration::hours(48)).await?;
        let link = format!("{}/verify-email?token={token}", state.cfg.public_base_url);
        system_mail::enqueue(
            &mut *tx,
            system_mail::verify_email(&state.cfg.system_from_name, &email, &full_name, &link),
        )
        .await?;
    }

    Entry::user(Some(org_id), user_id, &email, &client, "user.registered")
        .target("organization", org_id)
        .details(json!({ "organization_slug": slug, "account_type": account_type }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let s = session::create(
        &state.db,
        &state.cfg,
        SessionKind::Console,
        SessionOwner {
            user_id: Some(user_id),
            mailbox_id: None,
            active_organization_id: Some(org_id),
            mfa_pending: false,
        },
        client.ip,
        client.user_agent.as_deref(),
    )
    .await?;
    let mut payload = fresh_session_payload(&state, user_id, s.id, &s.csrf_token).await?;
    payload["verification"] = match (&phone, by_phone) {
        (Some(p), true) => match issue_phone_code(&state, user_id, p).await {
            Ok(()) => json!({ "channel": "phone", "sent_to": sms::mask(p) }),
            Err(e) => json!({ "channel": "phone", "sent_to": sms::mask(p), "error": e.to_string() }),
        },
        _ => json!({ "channel": "email", "sent_to": email }),
    };
    let mut res = with_cookie(&state, &s.token, payload);
    *res.status_mut() = StatusCode::CREATED;
    Ok(res)
}

fn unique_identity_violation(e: sqlx::Error) -> AppError {
    let constraint = match &e {
        sqlx::Error::Database(db) => db.constraint().unwrap_or("").to_string(),
        _ => String::new(),
    };
    if constraint.contains("username") {
        crate::error::on_unique_violation(e, "username_taken", "That username is taken")
    } else if constraint.contains("phone") {
        crate::error::on_unique_violation(e, "phone_taken", "That phone number is already linked to an account")
    } else {
        crate::error::on_unique_violation(e, "email_taken", "An account with this email already exists")
    }
}

#[derive(Deserialize)]
struct LoginBody {
    /// Email address, username or phone number.
    #[serde(alias = "login", alias = "identifier")]
    email: String,
    password: String,
}

#[derive(sqlx::FromRow)]
struct LoginUser {
    id: Uuid,
    email: String,
    password_hash: String,
    status: String,
    mfa_enabled: bool,
    failed_login_count: i32,
    locked_until: Option<DateTime<Utc>>,
}

async fn login(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<LoginBody>,
) -> ApiResult<Response> {
    let identifier = body.email.trim().trim_start_matches('@').to_lowercase();
    state.limiter.check(&format!("login-ip:{}", client.ip_key()), 30, 600)?;
    state.limiter.check(&format!("login-email:{identifier}"), 10, 600)?;
    let phone = if identifier.contains('@') { None } else { sms::normalize(&identifier).ok() };

    let user: Option<LoginUser> = sqlx::query_as(
        "SELECT id, email::text AS email, password_hash, status, mfa_enabled, failed_login_count, locked_until
         FROM users WHERE email = $1 OR username = $1 OR ($2::text IS NOT NULL AND phone = $2) LIMIT 1",
    )
    .bind(&identifier)
    .bind(&phone)
    .fetch_optional(&state.db)
    .await?;

    let Some(user) = user else {
        password::verify_dummy(&body.password).await;
        return Err(AppError::invalid_credentials());
    };
    if user.locked_until.is_some_and(|t| t > Utc::now()) {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "account_locked",
            "Too many failed sign-in attempts. Try again later or reset your password.",
        ));
    }
    if !password::verify(&body.password, &user.password_hash).await {
        let failures = user.failed_login_count + 1;
        let lock = (failures >= MAX_FAILED_LOGINS).then(|| Utc::now() + Duration::minutes(LOCKOUT_MINUTES));
        sqlx::query("UPDATE users SET failed_login_count = $2, locked_until = $3 WHERE id = $1")
            .bind(user.id)
            .bind(if lock.is_some() { 0 } else { failures })
            .bind(lock)
            .execute(&state.db)
            .await?;
        Entry::user(None, user.id, &user.email, &client, "user.login_failed").record(&state.db).await?;
        return Err(AppError::invalid_credentials());
    }
    if user.status != "active" {
        return Err(AppError::forbidden("This account is disabled").with_details(json!({})));
    }

    sqlx::query("UPDATE users SET failed_login_count = 0, locked_until = NULL WHERE id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;

    let org = default_org(&state, user.id).await?;
    let s = session::create(
        &state.db,
        &state.cfg,
        SessionKind::Console,
        SessionOwner {
            user_id: Some(user.id),
            mailbox_id: None,
            active_organization_id: org,
            mfa_pending: user.mfa_enabled,
        },
        client.ip,
        client.user_agent.as_deref(),
    )
    .await?;

    if user.mfa_enabled {
        return Ok(with_cookie(&state, &s.token, json!({ "mfa_required": true, "csrf_token": s.csrf_token })));
    }
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1").bind(user.id).execute(&state.db).await?;
    Entry::user(org, user.id, &user.email, &client, "user.login").record(&state.db).await?;
    let payload = fresh_session_payload(&state, user.id, s.id, &s.csrf_token).await?;
    Ok(with_cookie(&state, &s.token, payload))
}

#[derive(Deserialize)]
struct CodeBody {
    code: String,
}

async fn mfa_verify(
    State(state): State<AppState>,
    client: ClientInfo,
    PendingSession(s): PendingSession,
    ApiJson(body): ApiJson<CodeBody>,
) -> ApiResult<Response> {
    if !s.mfa_pending {
        return Err(AppError::bad_request("mfa_not_pending", "This session is already fully authenticated"));
    }
    state.limiter.check(&format!("mfa:{}", s.user_id), 6, 300)?;

    let (secret_enc, last_step): (Option<Vec<u8>>, Option<i64>) =
        sqlx::query_as("SELECT mfa_secret_enc, mfa_last_step FROM users WHERE id = $1")
            .bind(s.user_id)
            .fetch_one(&state.db)
            .await?;
    let secret = state
        .secrets
        .open(purpose::TOTP_SECRET, &secret_enc.ok_or_else(|| AppError::internal("MFA secret missing"))?)?;
    let Some(step) = totp::verify(&secret, &body.code, last_step) else {
        Entry::user(None, s.user_id, &s.email, &client, "user.mfa_failed").record(&state.db).await?;
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_mfa_code",
            "The authentication code is incorrect",
        ));
    };
    sqlx::query("UPDATE users SET mfa_last_step = $2, last_login_at = now() WHERE id = $1")
        .bind(s.user_id)
        .bind(step)
        .execute(&state.db)
        .await?;

    // Rotate: the pre-MFA session is discarded and a full session issued.
    session::revoke(&state.db, s.session_id).await?;
    let org = default_org(&state, s.user_id).await?;
    let fresh = session::create(
        &state.db,
        &state.cfg,
        SessionKind::Console,
        SessionOwner { user_id: Some(s.user_id), mailbox_id: None, active_organization_id: org, mfa_pending: false },
        client.ip,
        client.user_agent.as_deref(),
    )
    .await?;
    Entry::user(org, s.user_id, &s.email, &client, "user.login")
        .details(json!({ "mfa": true }))
        .record(&state.db)
        .await?;
    let payload = fresh_session_payload(&state, s.user_id, fresh.id, &fresh.csrf_token).await?;
    Ok(with_cookie(&state, &fresh.token, payload))
}

async fn logout(State(state): State<AppState>, PendingSession(s): PendingSession) -> ApiResult<Response> {
    session::revoke(&state.db, s.session_id).await?;
    let cookie = session::clear_cookie(&state.cfg, SessionKind::Console);
    Ok((AppendHeaders([(header::SET_COOKIE, cookie)]), Json(json!({ "ok": true }))).into_response())
}

async fn current_session(State(state): State<AppState>, s: ConsoleSession) -> ApiResult<Json<Value>> {
    Ok(Json(session_payload(&state, &s).await?))
}

#[derive(Deserialize)]
struct ProfileBody {
    full_name: String,
}

async fn update_profile(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(body): ApiJson<ProfileBody>,
) -> ApiResult<Json<Value>> {
    let full_name = validation::non_empty("full_name", &body.full_name, 200)?;
    sqlx::query("UPDATE users SET full_name = $2, updated_at = now() WHERE id = $1")
        .bind(s.user_id)
        .bind(&full_name)
        .execute(&state.db)
        .await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.profile_updated").record(&state.db).await?;
    Ok(Json(json!({ "ok": true, "full_name": full_name })))
}

#[derive(Deserialize)]
struct SwitchBody {
    organization_id: Uuid,
}

async fn switch_organization(
    State(state): State<AppState>,
    s: ConsoleSession,
    ApiJson(body): ApiJson<SwitchBody>,
) -> ApiResult<Json<Value>> {
    let member: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM memberships WHERE user_id = $1 AND organization_id = $2)")
            .bind(s.user_id)
            .bind(body.organization_id)
            .fetch_one(&state.db)
            .await?;
    if !member {
        return Err(AppError::not_found("Organization"));
    }
    sqlx::query("UPDATE sessions SET active_organization_id = $2 WHERE id = $1")
        .bind(s.session_id)
        .bind(body.organization_id)
        .execute(&state.db)
        .await?;
    Ok(Json(fresh_session_payload(&state, s.user_id, s.session_id, &s.csrf_token).await?))
}

// ── password management ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(body): ApiJson<ChangePasswordBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("pwchange:{}", s.user_id), 10, 3600)?;
    let current: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(s.user_id)
        .fetch_one(&state.db)
        .await?;
    if !password::verify(&body.current_password, &current).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "invalid_current_password", "Current password is incorrect"));
    }
    password::check_policy(&body.new_password, &s.email)?;
    breach::ensure_not_breached(&state.cfg, &body.new_password).await?;
    let hash = password::hash(&body.new_password).await?;
    sqlx::query("UPDATE users SET password_hash = $2, password_changed_at = now(), updated_at = now() WHERE id = $1")
        .bind(s.user_id)
        .bind(hash)
        .execute(&state.db)
        .await?;
    let revoked = session::revoke_all_for_user(&state.db, s.user_id, Some(s.session_id)).await?;
    security_alert(&state, s.user_id, "Your password was changed", &client).await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.password_changed")
        .details(json!({ "other_sessions_revoked": revoked }))
        .record(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true, "other_sessions_revoked": revoked })))
}

#[derive(Deserialize)]
struct ForgotBody {
    email: String,
}

/// Always answers 202 so the endpoint cannot be used to enumerate accounts.
async fn forgot_password(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<ForgotBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("forgot-ip:{}", client.ip_key()), 10, 3600)?;
    let email = body.email.trim().to_lowercase();
    state.limiter.check(&format!("forgot-email:{email}"), 3, 3600)?;
    let user: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, email::text FROM users WHERE email = $1 AND status = 'active'")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;
    if let Some((user_id, email)) = user {
        let mut tx = state.db.begin().await?;
        let token = issue_user_token(&mut tx, user_id, "reset_password", Duration::hours(1)).await?;
        let link = format!("{}/reset-password?token={token}", state.cfg.public_base_url);
        system_mail::enqueue(&mut *tx, system_mail::password_reset(&state.cfg.system_from_name, &email, &link)).await?;
        Entry::user(None, user_id, &email, &client, "user.password_reset_requested").record(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "ok": true, "message": "If an account exists for that address, a reset link has been sent." })),
    ))
}

#[derive(Deserialize)]
struct ResetBody {
    token: String,
    password: String,
}

async fn reset_password(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<ResetBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("reset-ip:{}", client.ip_key()), 20, 3600)?;
    let mut tx = state.db.begin().await?;
    let user_id = consume_user_token(&mut tx, &body.token, "reset_password").await?;
    let email: String =
        sqlx::query_scalar("SELECT email::text FROM users WHERE id = $1").bind(user_id).fetch_one(&mut *tx).await?;
    password::check_policy(&body.password, &email)?;
    breach::ensure_not_breached(&state.cfg, &body.password).await?;
    let hash = password::hash(&body.password).await?;
    sqlx::query(
        "UPDATE users SET password_hash = $2, password_changed_at = now(), failed_login_count = 0,
                          locked_until = NULL, updated_at = now() WHERE id = $1",
    )
    .bind(user_id)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    // Possession of the reset link also proves control of the address.
    sqlx::query("UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    Entry::user(None, user_id, &email, &client, "user.password_reset").record(&mut *tx).await?;
    tx.commit().await?;
    security_alert(&state, user_id, "Your password was reset", &client).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

async fn verify_email(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<TokenBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("verify-ip:{}", client.ip_key()), 30, 3600)?;
    let mut tx = state.db.begin().await?;
    let user_id = consume_user_token(&mut tx, &body.token, "verify_email").await?;
    let email: String = sqlx::query_scalar(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()), updated_at = now() WHERE id = $1 RETURNING email::text",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    Entry::user(None, user_id, &email, &client, "user.email_verified").record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn resend_verification(State(state): State<AppState>, s: ConsoleSession) -> ApiResult<Json<Value>> {
    if s.email_verified {
        return Ok(Json(json!({ "ok": true, "already_verified": true })));
    }
    state.limiter.check(&format!("resend-verify:{}", s.user_id), 3, 3600)?;
    let mut tx = state.db.begin().await?;
    let token = issue_user_token(&mut tx, s.user_id, "verify_email", Duration::hours(48)).await?;
    let link = format!("{}/verify-email?token={token}", state.cfg.public_base_url);
    system_mail::enqueue(
        &mut *tx,
        system_mail::verify_email(&state.cfg.system_from_name, &s.email, &s.full_name, &link),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

// ── session management ───────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SessionListRow {
    id: Uuid,
    ip: Option<ipnetwork::IpNetwork>,
    user_agent: Option<String>,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

async fn list_sessions(State(state): State<AppState>, s: ConsoleSession) -> ApiResult<Json<Value>> {
    let rows: Vec<SessionListRow> = sqlx::query_as(
        "SELECT id, ip, user_agent, created_at, last_seen_at, expires_at FROM sessions
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now() AND NOT mfa_pending
         ORDER BY last_seen_at DESC",
    )
    .bind(s.user_id)
    .fetch_all(&state.db)
    .await?;
    let data: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "ip": r.ip.map(|i| i.ip().to_string()), "user_agent": r.user_agent,
                "created_at": r.created_at, "last_seen_at": r.last_seen_at, "expires_at": r.expires_at,
                "current": r.id == s.session_id,
            })
        })
        .collect();
    Ok(Json(json!({ "data": data })))
}

async fn revoke_session(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let n = sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL")
        .bind(id)
        .bind(s.user_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Session"));
    }
    Entry::user(None, s.user_id, &s.email, &client, "user.session_revoked")
        .target("session", id)
        .record(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ── MFA enrollment ───────────────────────────────────────────────────────

async fn mfa_setup(State(state): State<AppState>, s: ConsoleSession) -> ApiResult<Json<Value>> {
    if s.mfa_enabled {
        return Err(AppError::conflict("mfa_already_enabled", "Two-factor authentication is already enabled"));
    }
    let enrollment = totp::enroll(&state.cfg.system_from_name, &s.email)?;
    sqlx::query("UPDATE users SET mfa_pending_secret_enc = $2 WHERE id = $1")
        .bind(s.user_id)
        .bind(state.secrets.seal(purpose::TOTP_SECRET, &enrollment.secret_bytes))
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "secret": enrollment.secret_base32, "otpauth_url": enrollment.otpauth_url })))
}

async fn mfa_enable(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(body): ApiJson<CodeBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mfa:{}", s.user_id), 6, 300)?;
    let pending: Option<Vec<u8>> = sqlx::query_scalar("SELECT mfa_pending_secret_enc FROM users WHERE id = $1")
        .bind(s.user_id)
        .fetch_one(&state.db)
        .await?;
    let sealed = pending.ok_or_else(|| AppError::bad_request("mfa_not_started", "Start two-factor setup first"))?;
    let secret = state.secrets.open(purpose::TOTP_SECRET, &sealed)?;
    let step = totp::verify(&secret, &body.code, None).ok_or_else(|| {
        AppError::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_mfa_code", "The authentication code is incorrect")
    })?;
    sqlx::query(
        "UPDATE users SET mfa_enabled = true, mfa_secret_enc = mfa_pending_secret_enc, mfa_pending_secret_enc = NULL,
                          mfa_last_step = $2, updated_at = now() WHERE id = $1",
    )
    .bind(s.user_id)
    .bind(step)
    .execute(&state.db)
    .await?;
    let revoked = session::revoke_all_for_user(&state.db, s.user_id, Some(s.session_id)).await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.mfa_enabled").record(&state.db).await?;
    security_alert(&state, s.user_id, "Two-factor authentication was turned on", &client).await?;
    Ok(Json(json!({ "ok": true, "other_sessions_revoked": revoked })))
}

#[derive(Deserialize)]
struct DisableMfaBody {
    password: String,
    code: String,
}

async fn mfa_disable(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(body): ApiJson<DisableMfaBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mfa:{}", s.user_id), 6, 300)?;
    let (hash, secret_enc, last_step): (String, Option<Vec<u8>>, Option<i64>) =
        sqlx::query_as("SELECT password_hash, mfa_secret_enc, mfa_last_step FROM users WHERE id = $1")
            .bind(s.user_id)
            .fetch_one(&state.db)
            .await?;
    let sealed = secret_enc
        .ok_or_else(|| AppError::bad_request("mfa_not_enabled", "Two-factor authentication is not enabled"))?;
    if !password::verify(&body.password, &hash).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "invalid_current_password", "Password is incorrect"));
    }
    let secret = state.secrets.open(purpose::TOTP_SECRET, &sealed)?;
    if totp::verify(&secret, &body.code, last_step).is_none() {
        return Err(AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_mfa_code",
            "The authentication code is incorrect",
        ));
    }
    sqlx::query("UPDATE users SET mfa_enabled = false, mfa_secret_enc = NULL, mfa_last_step = NULL, updated_at = now() WHERE id = $1")
        .bind(s.user_id)
        .execute(&state.db)
        .await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.mfa_disabled").record(&state.db).await?;
    security_alert(&state, s.user_id, "Two-factor authentication was turned off", &client).await?;
    Ok(Json(json!({ "ok": true })))
}

// ── sign-in providers, phone verification, Google ────────────────────────

async fn providers(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "google": state.cfg.integrations.google.as_ref().map(|g| json!({ "client_id": g.client_id })),
        "phone_verification": sms::available(&state),
        "signup": state.cfg.allow_signup,
        "payments": { "mpesa": state.cfg.integrations.pesatide.is_some() },
    }))
}

const PHONE_CODE_TTL_MINUTES: i64 = 10;
const PHONE_CODE_MAX_ATTEMPTS: i32 = 5;

fn phone_code_hash(user_id: Uuid, code: &str) -> Vec<u8> {
    crypto::sha256(format!("{user_id}:{code}"))
}

/// Issue a fresh six-digit code (older ones stop working) and text it.
async fn issue_phone_code(state: &AppState, user_id: Uuid, phone: &str) -> ApiResult<()> {
    state.limiter.check(&format!("phone-code-user:{user_id}"), 5, 3600)?;
    state.limiter.check(&format!("phone-code-number:{phone}"), 8, 3600)?;
    let n = u32::from_be_bytes(crypto::random_bytes::<4>()) % 1_000_000;
    let code = format!("{n:06}");
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE phone_codes SET consumed_at = now() WHERE user_id = $1 AND consumed_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO phone_codes (user_id, phone, code_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(phone)
        .bind(phone_code_hash(user_id, &code))
        .bind(Utc::now() + Duration::minutes(PHONE_CODE_TTL_MINUTES))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let text = format!(
        "{code} is your {} verification code. It expires in {PHONE_CODE_TTL_MINUTES} minutes. Never share it with anyone.",
        state.cfg.product_name
    );
    sms::send(state, phone, &text).await
}

#[derive(Deserialize)]
struct SendCodeBody {
    phone: Option<String>,
}

async fn send_phone_code(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(b): ApiJson<SendCodeBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("phone-code-burst:{}", s.user_id), 1, 45)?;
    let phone = match b.phone.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(raw) => {
            let phone = sms::normalize(raw)?;
            sqlx::query(
                "UPDATE users SET phone = $2, phone_verified_at = CASE WHEN phone = $2 THEN phone_verified_at END, updated_at = now()
                 WHERE id = $1",
            )
            .bind(s.user_id)
            .bind(&phone)
            .execute(&state.db)
            .await
            .map_err(unique_identity_violation)?;
            phone
        }
        None => sqlx::query_scalar::<_, Option<String>>("SELECT phone FROM users WHERE id = $1")
            .bind(s.user_id)
            .fetch_one(&state.db)
            .await?
            .ok_or_else(|| AppError::validation("Add a phone number first"))?,
    };
    issue_phone_code(&state, s.user_id, &phone).await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.phone_code_sent")
        .details(json!({ "phone": sms::mask(&phone) }))
        .record(&state.db)
        .await?;
    Ok(Json(json!({ "sent_to": sms::mask(&phone), "expires_in": PHONE_CODE_TTL_MINUTES * 60, "resend_after": 45 })))
}

async fn verify_phone_code(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(b): ApiJson<CodeBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("phone-verify:{}", s.user_id), 10, 600)?;
    let code: String = b.code.chars().filter(char::is_ascii_digit).collect();
    let row: Option<(Uuid, String, Vec<u8>, i32)> = sqlx::query_as(
        "SELECT c.id, c.phone, c.code_hash, c.attempts FROM phone_codes c JOIN users u ON u.id = c.user_id
         WHERE c.user_id = $1 AND c.consumed_at IS NULL AND c.expires_at > now() AND c.phone = u.phone
         ORDER BY c.created_at DESC LIMIT 1",
    )
    .bind(s.user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, phone, hash, attempts)) = row else {
        return Err(AppError::bad_request("code_expired", "This code has expired. Request a new one."));
    };
    if attempts >= PHONE_CODE_MAX_ATTEMPTS {
        return Err(AppError::bad_request("too_many_attempts", "Too many incorrect codes. Request a new one."));
    }
    if code.len() != 6 || !crypto::constant_time_eq(&phone_code_hash(s.user_id, &code), &hash) {
        sqlx::query("UPDATE phone_codes SET attempts = attempts + 1 WHERE id = $1").bind(id).execute(&state.db).await?;
        return Err(AppError::bad_request("invalid_code", "That code is not correct. Check the latest text message."));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE phone_codes SET consumed_at = now() WHERE id = $1").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET phone_verified_at = now(), updated_at = now() WHERE id = $1")
        .bind(s.user_id)
        .execute(&mut *tx)
        .await?;
    Entry::user(None, s.user_id, &s.email, &client, "user.phone_verified")
        .details(json!({ "phone": sms::mask(&phone) }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "verified": true, "phone": sms::mask(&phone) })))
}

#[derive(Deserialize)]
struct GoogleBody {
    code: String,
    code_verifier: String,
    redirect_uri: String,
    nonce: String,
    account_type: Option<String>,
    organization_name: Option<String>,
}

async fn google_sign_in(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(b): ApiJson<GoogleBody>,
) -> ApiResult<Response> {
    let cfg = state.cfg.integrations.google.clone().ok_or_else(|| {
        AppError::unavailable("google_not_configured", "Google sign-in is not enabled on this platform")
    })?;
    state.limiter.check(&format!("google:{}", client.ip_key()), 20, 600)?;
    let expected_redirect = format!("{}/auth/google/callback", state.cfg.public_base_url.trim_end_matches('/'));
    if b.redirect_uri != expected_redirect {
        return Err(AppError::validation("redirect_uri does not match this platform"));
    }
    let claims = google::exchange(
        &cfg,
        &google::Exchange {
            code: &b.code,
            code_verifier: &b.code_verifier,
            redirect_uri: &b.redirect_uri,
            nonce: &b.nonce,
        },
    )
    .await?;
    let email = claims.email.clone().unwrap_or_default().to_lowercase();

    let existing: Option<(Uuid, String, bool, bool)> = sqlx::query_as(
        "SELECT id, status, mfa_enabled, google_sub IS NULL FROM users WHERE google_sub = $1 OR email = $2
         ORDER BY (google_sub = $1) DESC NULLS LAST LIMIT 1",
    )
    .bind(&claims.sub)
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, mfa_enabled, created) = match existing {
        Some((id, status, mfa, unlinked)) => {
            if status != "active" {
                return Err(AppError::forbidden("This account is disabled").with_details(json!({})));
            }
            sqlx::query(
                "UPDATE users SET google_sub = COALESCE(google_sub, $2), email_verified_at = COALESCE(email_verified_at, now()),
                        failed_login_count = 0, locked_until = NULL, updated_at = now() WHERE id = $1",
            )
            .bind(id)
            .bind(&claims.sub)
            .execute(&state.db)
            .await?;
            if unlinked {
                Entry::user(None, id, &email, &client, "user.google_linked").record(&state.db).await?;
                security_alert(&state, id, "A Google account was linked for sign-in", &client).await?;
            }
            (id, mfa, false)
        }
        None => {
            if !state.cfg.allow_signup {
                return Err(
                    AppError::forbidden("Self-service sign-up is disabled on this platform").with_details(json!({}))
                );
            }
            let name = claims.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| email.clone());
            let full_name: String = name.chars().take(200).collect();
            let account_type = parse_account_type(b.account_type.as_deref())?;
            let org_name = match b.organization_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(n) => validation::non_empty("organization_name", n, 200)?,
                None if account_type == "individual" => full_name.clone(),
                None => format!("{full_name}'s organization"),
            };
            // Google accounts sign in with Google; a random password keeps the
            // password path closed until the person sets one through a reset.
            let password_hash = password::hash(&crypto::random_token(32)).await?;
            let username = suggest_username(&state, &email).await?;
            let mut tx = state.db.begin().await?;
            let id: Uuid = sqlx::query_scalar(
                "INSERT INTO users (email, full_name, password_hash, username, google_sub, email_verified_at)
                 VALUES ($1, $2, $3, $4, $5, now()) RETURNING id",
            )
            .bind(&email)
            .bind(&full_name)
            .bind(&password_hash)
            .bind(&username)
            .bind(&claims.sub)
            .fetch_one(&mut *tx)
            .await
            .map_err(unique_identity_violation)?;
            let (org_id, slug) = create_tenant(&mut tx, &org_name, account_type, id).await?;
            Entry::user(Some(org_id), id, &email, &client, "user.registered")
                .target("organization", org_id)
                .details(json!({ "organization_slug": slug, "account_type": account_type, "via": "google" }))
                .record(&mut *tx)
                .await?;
            tx.commit().await?;
            (id, false, true)
        }
    };

    let org = default_org(&state, user_id).await?;
    let s = session::create(
        &state.db,
        &state.cfg,
        SessionKind::Console,
        SessionOwner {
            user_id: Some(user_id),
            mailbox_id: None,
            active_organization_id: org,
            mfa_pending: mfa_enabled,
        },
        client.ip,
        client.user_agent.as_deref(),
    )
    .await?;
    if mfa_enabled {
        return Ok(with_cookie(&state, &s.token, json!({ "mfa_required": true, "csrf_token": s.csrf_token })));
    }
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1").bind(user_id).execute(&state.db).await?;
    Entry::user(org, user_id, &email, &client, "user.login")
        .details(json!({ "via": "google" }))
        .record(&state.db)
        .await?;
    let mut payload = fresh_session_payload(&state, user_id, s.id, &s.csrf_token).await?;
    payload["created"] = json!(created);
    Ok(with_cookie(&state, &s.token, payload))
}

/// A free username derived from the email's local part.
async fn suggest_username(state: &AppState, email: &str) -> ApiResult<String> {
    let base: String = email
        .split('@')
        .next()
        .unwrap_or("user")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(24)
        .collect();
    let base = if base.len() < 3 || !base.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) {
        format!("user{base}")
    } else {
        base
    };
    for attempt in 0..20 {
        let candidate = if attempt == 0 { base.clone() } else { format!("{base}{}", crypto::random_lower_alnum(4)) };
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1)")
            .bind(&candidate)
            .fetch_one(&state.db)
            .await?;
        if !taken && parse_username(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Ok(format!("user{}", crypto::random_lower_alnum(10)))
}

/// Email the account owner about a security-sensitive change.
async fn security_alert(state: &AppState, user_id: Uuid, what: &str, client: &ClientInfo) -> ApiResult<()> {
    let (email, name): (String, String) = sqlx::query_as("SELECT email::text, full_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    let first = name.split_whitespace().next().unwrap_or(&name).to_string();
    let ip = client.ip.map(|ip| ip.to_string()).unwrap_or_else(|| "an unknown address".into());
    let context = format!("{} UTC from {ip}", Utc::now().format("%Y-%m-%d %H:%M"));
    let link = format!("{}/console/account", state.cfg.public_base_url);
    system_mail::enqueue(
        &state.db,
        system_mail::security_alert(&state.cfg.system_from_name, &email, &first, what, &context, &link),
    )
    .await
}
