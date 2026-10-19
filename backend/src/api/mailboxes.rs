//! Hosted mailbox and alias provisioning.
//!
//! Provisioning is a database operation: Postfix and Dovecot read the
//! `mail_*` views live, and Dovecot creates the maildir on first delivery or
//! login. There is therefore no separate storage step that could fail after
//! the database commit (guide 04: recoverable provisioning).
//!
//! Account lifecycle (guide 04 §5): a mailbox can be created
//! * `invite`   – pending; a single-use activation link goes to the
//!   employee's personal (recovery) address and they choose the password;
//! * `link`     – pending; the one-time activation link is returned once so
//!   IT can hand it over in person (printed handout / QR code);
//! * `password` – active with a password set by an administrator;
//! * `generate` – active with a random password shown once.
//!
//! ICT support (`mailboxes:assist`) can resend activation links, email
//! reset links to the recovery address and unlock sign-ins. They can never
//! set, see or receive a password, and cannot redirect links to another
//! address (guide 04: "ICT support must not retrieve existing passwords").

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{session, ClientInfo, Permission, Principal},
    crypto,
    error::{on_unique_violation, ApiResult, AppError},
    password,
    services::{audit::Entry, doveadm, imap, mail_client, mailbox_tokens, system_mail},
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mailboxes", get(list_mailboxes).post(create_mailbox))
        .route("/mailboxes/onboarding", get(onboarding_report))
        .route("/mailboxes/{id}", get(get_mailbox).patch(update_mailbox).delete(delete_mailbox))
        .route("/mailboxes/{id}/reset-password", post(reset_password))
        .route("/mailboxes/{id}/activation", post(send_activation))
        .route("/mailboxes/{id}/send-reset", post(send_reset))
        .route("/mailboxes/{id}/unlock", post(unlock))
        .route("/aliases", get(list_aliases).post(create_alias))
        .route("/aliases/{id}", axum::routing::patch(update_alias).delete(delete_alias))
}

/// Placeholder that never verifies; pending mailboxes cannot sign in.
const PENDING_HASH: &str = "!pending-activation";

#[derive(sqlx::FromRow)]
struct MailboxRow {
    id: Uuid,
    domain_id: Uuid,
    domain: String,
    local_part: String,
    address: String,
    display_name: Option<String>,
    status: String,
    quota_bytes: i64,
    owner_email: Option<String>,
    recovery_email: Option<String>,
    department: Option<String>,
    job_title: Option<String>,
    locked_until: Option<DateTime<Utc>>,
    last_login_at: Option<DateTime<Utc>>,
    activated_at: Option<DateTime<Utc>>,
    password_changed_at: Option<DateTime<Utc>>,
    incoming_verified_at: Option<DateTime<Utc>>,
    outgoing_verified_at: Option<DateTime<Utc>>,
    last_setup_client: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const SELECT_MAILBOX: &str = "
    SELECT m.id, m.domain_id, d.domain_name::text AS domain, m.local_part::text AS local_part, m.address::text AS address,
           m.display_name, m.status, m.quota_bytes, u.email::text AS owner_email, m.recovery_email::text AS recovery_email,
           m.department, m.job_title, m.locked_until, m.last_login_at, m.activated_at, m.password_changed_at,
           (SELECT max(c.verified_at) FROM mailbox_connection_checks c WHERE c.mailbox_id = m.id AND c.direction = 'incoming' AND c.status = 'verified') AS incoming_verified_at,
           (SELECT max(c.verified_at) FROM mailbox_connection_checks c WHERE c.mailbox_id = m.id AND c.direction = 'outgoing' AND c.status = 'verified') AS outgoing_verified_at,
           (SELECT e.client FROM mailbox_setup_events e WHERE e.mailbox_id = m.id AND e.client IS NOT NULL ORDER BY e.id DESC LIMIT 1) AS last_setup_client,
           m.created_at, m.updated_at
    FROM mailboxes m JOIN domains d ON d.id = m.domain_id LEFT JOIN users u ON u.id = m.owner_user_id";

/// Show the recovery address in full to administrators, masked to support.
pub fn mask_email(e: &str) -> String {
    match e.split_once('@') {
        Some((local, domain)) => {
            let first: String = local.chars().take(1).collect();
            format!("{first}{}@{domain}", "*".repeat(local.chars().count().clamp(1, 6).saturating_sub(1).max(2)))
        }
        None => "***".into(),
    }
}

fn mailbox_json(m: &MailboxRow, p: &Principal) -> Value {
    let full = p.can(Permission::MailboxesWrite);
    let connection = if m.incoming_verified_at.is_some() && m.outgoing_verified_at.is_some() {
        "verified"
    } else if m.incoming_verified_at.is_some() || m.outgoing_verified_at.is_some() {
        "partially_verified"
    } else if m.last_setup_client.is_some() {
        "setup_started"
    } else {
        "not_started"
    };
    json!({
        "id": m.id, "domain_id": m.domain_id, "domain": m.domain, "local_part": m.local_part, "address": m.address,
        "display_name": m.display_name, "status": m.status, "quota_bytes": m.quota_bytes, "owner": m.owner_email,
        "recovery_email": m.recovery_email.as_deref().map(|e| if full { e.to_string() } else { mask_email(e) }),
        "has_recovery_email": m.recovery_email.is_some(),
        "department": m.department, "job_title": m.job_title,
        "locked": m.locked_until.is_some_and(|t| t > Utc::now()),
        "last_login_at": m.last_login_at, "activated_at": m.activated_at, "password_changed_at": m.password_changed_at,
        "client_setup": {
            "state": connection, "last_client": m.last_setup_client,
            "incoming_verified_at": m.incoming_verified_at, "outgoing_verified_at": m.outgoing_verified_at,
        },
        "created_at": m.created_at, "updated_at": m.updated_at,
    })
}

async fn load(state: &AppState, org_id: Uuid, id: Uuid) -> ApiResult<MailboxRow> {
    sqlx::query_as(&format!("{SELECT_MAILBOX} WHERE m.id = $1 AND m.organization_id = $2 AND m.status <> 'deleted'"))
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Mailbox"))
}

/// Resolve a hosting-enabled, verified domain of the tenant.
async fn hosting_domain(
    state: &AppState,
    org_id: Uuid,
    domain_id: Option<Uuid>,
    domain_name: Option<&str>,
) -> ApiResult<(Uuid, String)> {
    let row: Option<(Uuid, String, String, bool)> = match (domain_id, domain_name) {
        (Some(id), _) => sqlx::query_as("SELECT id, domain_name::text, status, mail_hosting_enabled FROM domains WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(org_id)
            .fetch_optional(&state.db)
            .await?,
        (None, Some(name)) => sqlx::query_as("SELECT id, domain_name::text, status, mail_hosting_enabled FROM domains WHERE domain_name = $1 AND organization_id = $2")
            .bind(validation::normalize_domain(name)?)
            .bind(org_id)
            .fetch_optional(&state.db)
            .await?,
        (None, None) => return Err(AppError::validation("domain_id (or domain) is required")),
    };
    let (id, name, status, hosting) = row.ok_or_else(|| AppError::not_found("Domain"))?;
    if status != "verified" {
        return Err(AppError::conflict(
            "domain_not_verified",
            format!("{name} must be verified before creating mailboxes"),
        ));
    }
    if !hosting {
        return Err(AppError::conflict("mail_hosting_disabled", format!("Enable hosted mailboxes for {name} first")));
    }
    Ok((id, name))
}

async fn address_taken(state: &AppState, address: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM mailboxes WHERE address = $1) OR EXISTS (SELECT 1 FROM aliases WHERE address = $1)",
    )
    .bind(address)
    .fetch_one(&state.db)
    .await?)
}

/// Returns (password, generated?) — 20 random alphanumerics ≈ 119 bits.
fn choose_password(provided: Option<String>, address: &str) -> ApiResult<(String, bool)> {
    match provided.filter(|p| !p.is_empty()) {
        Some(p) => {
            password::check_policy(&p, address)?;
            Ok((p, false))
        }
        None => Ok((crypto::random_alphanumeric(20), true)),
    }
}

fn validate_quota(quota: i64, max: i64) -> ApiResult<()> {
    const MIN: i64 = 50 * 1024 * 1024;
    if quota < MIN || quota > max {
        return Err(AppError::validation(format!("quota_bytes must be between {MIN} and {max} on your plan")));
    }
    Ok(())
}

fn optional_text(field: &str, v: Option<&str>, max: usize) -> ApiResult<Option<String>> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => {
            validation::header_safe(field, s, max)?;
            Ok(Some(s.to_string()))
        }
    }
}

/// A personal address for activation and recovery; it must not be a mailbox
/// hosted by this platform (an employee who cannot sign in could not read it).
async fn recovery_address(state: &AppState, raw: Option<&str>) -> ApiResult<Option<String>> {
    let Some(e) = raw.map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
    let e = validation::parse_email("recovery_email", e)?.as_string().to_lowercase();
    let hosted: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM mailboxes WHERE address = $1 AND status <> 'deleted')")
            .bind(&e)
            .fetch_one(&state.db)
            .await?;
    if hosted {
        return Err(AppError::validation(
            "recovery_email must be a personal address outside this platform (for example the employee's own Gmail)",
        ));
    }
    Ok(Some(e))
}

async fn list_mailboxes(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesRead)?;
    let rows: Vec<MailboxRow> = sqlx::query_as(&format!(
        "{SELECT_MAILBOX} WHERE m.organization_id = $1 AND m.status <> 'deleted' ORDER BY m.address"
    ))
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.iter().map(|m| mailbox_json(m, &p)).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct CreateMailboxBody {
    domain_id: Option<Uuid>,
    domain: Option<String>,
    local_part: String,
    display_name: Option<String>,
    password: Option<String>,
    quota_bytes: Option<i64>,
    owner_email: Option<String>,
    /// invite | link | password | generate (default: password if given, else generate)
    setup_method: Option<String>,
    recovery_email: Option<String>,
    department: Option<String>,
    job_title: Option<String>,
}

async fn create_mailbox(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateMailboxBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::MailboxesWrite)?;
    p.require_institution()?;
    let (domain_id, domain) = hosting_domain(&state, p.org_id, body.domain_id, body.domain.as_deref()).await?;
    let local = validation::hosted_local_part(&body.local_part)?;
    let address = format!("{local}@{domain}");
    let display_name = optional_text("display_name", body.display_name.as_deref(), 120)?;
    let department = optional_text("department", body.department.as_deref(), 120)?;
    let job_title = optional_text("job_title", body.job_title.as_deref(), 120)?;
    let recovery_email = recovery_address(&state, body.recovery_email.as_deref()).await?;

    let method = match body.setup_method.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(m @ ("invite" | "link" | "password" | "generate")) => m.to_string(),
        Some(_) => return Err(AppError::validation("setup_method must be invite, link, password or generate")),
        None if body.password.as_deref().is_some_and(|p| !p.is_empty()) => "password".into(),
        None => "generate".into(),
    };
    if method == "invite" && recovery_email.is_none() {
        return Err(AppError::validation("An activation invite needs the employee's personal email (recovery_email)"));
    }
    if method == "password" && body.password.as_deref().map_or(true, str::is_empty) {
        return Err(AppError::validation("password is required for setup_method 'password'"));
    }

    let (count, max, default_quota, max_quota): (i64, i32, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted'),
                p.max_mailboxes, p.default_mailbox_quota_bytes, p.max_mailbox_quota_bytes
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if count >= max as i64 {
        return Err(AppError::quota_exceeded(format!(
            "Your plan allows {max} mailboxes. Upgrade the plan to add more."
        ))
        .with_details(json!({ "limit": "mailboxes", "max": max })));
    }
    let quota = body.quota_bytes.unwrap_or(default_quota);
    validate_quota(quota, max_quota)?;
    if address_taken(&state, &address).await? {
        return Err(AppError::conflict("address_taken", format!("{address} is already in use")));
    }
    let owner: Option<Uuid> = match body.owner_email.as_deref().filter(|e| !e.trim().is_empty()) {
        Some(email) => Some(
            sqlx::query_scalar("SELECT u.id FROM users u JOIN memberships m ON m.user_id = u.id WHERE u.email = $1 AND m.organization_id = $2")
                .bind(email.trim())
                .bind(p.org_id)
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| AppError::validation("owner_email must belong to a member of this organization"))?,
        ),
        None => None,
    };

    let pending = method == "invite" || method == "link";
    let (pw, generated) = if pending { (String::new(), false) } else { choose_password(body.password, &address)? };
    let hash = if pending { PENDING_HASH.to_string() } else { password::hash_for_mailbox(&pw).await? };
    let id = Uuid::now_v7();
    let storage_key = format!("{domain}/{}", id.simple());

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO mailboxes (id, organization_id, domain_id, local_part, address, display_name, password_hash, quota_bytes,
                                storage_key, owner_user_id, status, recovery_email, department, job_title, activated_at, password_changed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                 CASE WHEN $11 = 'active' THEN now() END, CASE WHEN $11 = 'active' THEN now() END)",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(domain_id)
    .bind(&local)
    .bind(&address)
    .bind(&display_name)
    .bind(&hash)
    .bind(quota)
    .bind(&storage_key)
    .bind(owner)
    .bind(if pending { "pending" } else { "active" })
    .bind(&recovery_email)
    .bind(&department)
    .bind(&job_title)
    .execute(&mut *tx)
    .await
    .map_err(|e| on_unique_violation(e, "address_taken", "This address is already in use"))?;

    let activation = if pending {
        Some(issue_activation(&state, &mut tx, &p, id, &address, &method, recovery_email.as_deref()).await?)
    } else {
        None
    };
    Entry::by(&p, &client, "mailbox.created")
        .target("mailbox", id)
        .details(json!({ "address": address, "quota_bytes": quota, "setup_method": method }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let mut out = mailbox_json(&load(&state, p.org_id, id).await?, &p);
    if generated {
        out["password"] = json!(pw);
        out["notice"] = json!("Give this password to the mailbox user over a secure channel. It is shown only once.");
    }
    if let Some(a) = activation {
        out["activation"] = a;
    }
    out["client_settings"] = mail_client::effective(&state, Some(domain_id)).await?.to_json(Some(&address));
    Ok((StatusCode::CREATED, Json(out)))
}

/// Issue an activation token and either email it or return it for handover.
async fn issue_activation(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    p: &Principal,
    mailbox_id: Uuid,
    address: &str,
    method: &str,
    recovery_email: Option<&str>,
) -> ApiResult<Value> {
    let delivery = if method == "invite" { "email" } else { "handover" };
    let (token, expires_at) =
        mailbox_tokens::issue(tx, mailbox_id, mailbox_tokens::ACTIVATE, delivery, p.user_id()).await?;
    let link = format!("{}/mail/activate?token={token}", state.cfg.public_base_url);
    if method == "invite" {
        let to = recovery_email.ok_or_else(|| AppError::validation("No personal email is on file for this mailbox"))?;
        let org_name: String = sqlx::query_scalar("SELECT name FROM organizations WHERE id = $1")
            .bind(p.org_id)
            .fetch_one(&mut *tx)
            .await?;
        system_mail::enqueue(
            &mut *tx,
            system_mail::mailbox_activation(&state.cfg.system_from_name, to, address, &org_name, &link),
        )
        .await?;
        Ok(json!({ "method": "invite", "sent_to": mask_email(to), "expires_at": expires_at }))
    } else {
        Ok(json!({
            "method": "link", "url": link, "expires_at": expires_at,
            "notice": "Give this link only to the employee. It works once and lets them choose their own password.",
        }))
    }
}

async fn get_mailbox(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesRead)?;
    let m = load(&state, p.org_id, id).await?;
    let mut out = mailbox_json(&m, &p);
    out["client_settings"] = mail_client::effective(&state, Some(m.domain_id)).await?.to_json(Some(&m.address));
    // Usage comes from Dovecot's quota accounting when the mail tier is configured.
    out["usage"] = match (&state.cfg.imap, m.status.as_str()) {
        (Some(cfg), "active") => match imap::quota_usage(cfg, &m.address).await {
            Ok(Some((used, limit))) => json!({ "used_bytes": used, "limit_bytes": limit }),
            Ok(None) => Value::Null,
            Err(e) => {
                tracing::warn!(mailbox = %id, error = %e, "quota lookup failed");
                json!({ "error": "Mail storage is temporarily unavailable" })
            }
        },
        _ => Value::Null,
    };
    let aliases: Vec<String> = sqlx::query_scalar(
        "SELECT address::text FROM aliases WHERE organization_id = $1 AND $2::citext = ANY(destinations)",
    )
    .bind(p.org_id)
    .bind(&m.address)
    .fetch_all(&state.db)
    .await?;
    out["aliases"] = json!(aliases);
    let pending_activation: Option<(String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT delivery, expires_at FROM mailbox_tokens WHERE mailbox_id = $1 AND purpose = 'activate' AND used_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    out["pending_activation"] =
        pending_activation.map(|(d, e)| json!({ "delivery": d, "expires_at": e })).unwrap_or(Value::Null);
    type EventRow = (String, Option<String>, Option<String>, DateTime<Utc>);
    let events: Vec<EventRow> = sqlx::query_as(
        "SELECT event, device, client, created_at FROM mailbox_setup_events WHERE mailbox_id = $1 ORDER BY id DESC LIMIT 15",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    out["setup_events"] = json!(events
        .into_iter()
        .map(|(e, d, c, at)| json!({ "event": e, "device": d, "client": c, "at": at }))
        .collect::<Vec<_>>());
    Ok(Json(out))
}

#[derive(Deserialize)]
struct UpdateMailboxBody {
    display_name: Option<String>,
    quota_bytes: Option<i64>,
    status: Option<String>,
    recovery_email: Option<String>,
    department: Option<String>,
    job_title: Option<String>,
}

async fn update_mailbox(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateMailboxBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesWrite)?;
    let m = load(&state, p.org_id, id).await?;
    if let Some(n) = &body.display_name {
        validation::header_safe("display_name", n, 120)?;
    }
    if let Some(q) = body.quota_bytes {
        let max: i64 = sqlx::query_scalar(
            "SELECT p.max_mailbox_quota_bytes FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
        )
        .bind(p.org_id)
        .fetch_one(&state.db)
        .await?;
        validate_quota(q, max)?;
    }
    let status = match body.status.as_deref() {
        None => m.status.clone(),
        Some(s @ ("active" | "disabled")) if m.status != "pending" => s.to_string(),
        Some("active" | "disabled") => {
            return Err(AppError::conflict(
                "mailbox_pending",
                "This mailbox has not been activated yet. Send a new activation link instead.",
            ))
        }
        Some(_) => return Err(AppError::validation("status must be 'active' or 'disabled'")),
    };
    // Empty strings clear optional fields; absent fields are unchanged.
    let recovery = match &body.recovery_email {
        None => m.recovery_email.clone(),
        Some(_) => recovery_address(&state, body.recovery_email.as_deref()).await?,
    };
    let department = match &body.department {
        None => m.department.clone(),
        Some(d) => optional_text("department", Some(d), 120)?,
    };
    let job_title = match &body.job_title {
        None => m.job_title.clone(),
        Some(j) => optional_text("job_title", Some(j), 120)?,
    };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE mailboxes SET display_name = COALESCE($3, display_name), quota_bytes = COALESCE($4, quota_bytes), status = $5,
                recovery_email = $6, department = $7, job_title = $8, updated_at = now()
         WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(body.display_name.as_deref().map(str::trim))
    .bind(body.quota_bytes)
    .bind(&status)
    .bind(&recovery)
    .bind(&department)
    .bind(&job_title)
    .execute(&mut *tx)
    .await?;
    if status == "disabled" && m.status != "disabled" {
        sqlx::query("UPDATE sessions SET revoked_at = now() WHERE mailbox_id = $1 AND revoked_at IS NULL")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    Entry::by(&p, &client, "mailbox.updated")
        .target("mailbox", id)
        .details(json!({ "address": m.address, "status": status, "quota_bytes": body.quota_bytes,
                         "recovery_email_changed": body.recovery_email.is_some() }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    if status != "active" && m.status == "active" {
        doveadm::revoke_access(&state, vec![m.address.clone()]).await;
    }
    Ok(Json(mailbox_json(&load(&state, p.org_id, id).await?, &p)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ResetPasswordBody {
    password: Option<String>,
}

/// Administrator sets (or generates) a new password. Use `send-reset` to let
/// the employee choose one instead.
async fn reset_password(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<ResetPasswordBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesWrite)?;
    let m = load(&state, p.org_id, id).await?;
    if m.status == "pending" {
        return Err(AppError::conflict("mailbox_pending", "Activate the mailbox first (send a new activation link)"));
    }
    let (pw, generated) = choose_password(body.password, &m.address)?;
    let hash = password::hash_for_mailbox(&pw).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE mailboxes SET password_hash = $3, failed_login_count = 0, locked_until = NULL, password_changed_at = now(), updated_at = now() WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(p.org_id)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE mailbox_id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // Outstanding self-service links are superseded.
    sqlx::query("UPDATE mailbox_tokens SET used_at = now() WHERE mailbox_id = $1 AND used_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "mailbox.password_reset")
        .target("mailbox", id)
        .details(json!({ "address": m.address, "method": if generated { "generated" } else { "set_by_admin" } }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    doveadm::revoke_access(&state, vec![m.address.clone()]).await;
    let mut out = json!({ "id": id, "ok": true, "webmail_sessions_revoked": true });
    if generated {
        out["password"] = json!(pw);
    }
    Ok(Json(out))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivationBody {
    /// invite (email to the recovery address) | link (handover)
    method: Option<String>,
}

/// Resend or regenerate the activation link of a pending mailbox.
async fn send_activation(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<ActivationBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesAssist)?;
    let m = load(&state, p.org_id, id).await?;
    if m.status != "pending" {
        return Err(AppError::conflict(
            "already_active",
            "This mailbox is already activated. Send a password reset instead.",
        ));
    }
    let method = body.method.as_deref().unwrap_or(if m.recovery_email.is_some() { "invite" } else { "link" });
    match method {
        "invite" => {}
        // A handover link lets the holder choose the password, so only
        // administrators may generate one.
        "link" => p.require(Permission::MailboxesWrite)?,
        _ => return Err(AppError::validation("method must be 'invite' or 'link'")),
    }
    state.limiter.check(&format!("mbx-activation:{id}"), 10, 3600)?;
    let mut tx = state.db.begin().await?;
    let activation = issue_activation(&state, &mut tx, &p, id, &m.address, method, m.recovery_email.as_deref()).await?;
    Entry::by(&p, &client, "mailbox.activation_sent")
        .target("mailbox", id)
        .details(json!({ "address": m.address, "method": method }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "activation": activation })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SendResetBody {
    /// email (to the recovery address) | link (handover, administrators only)
    method: Option<String>,
}

/// Let the employee choose a new password through a single-use link.
async fn send_reset(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<SendResetBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesAssist)?;
    let m = load(&state, p.org_id, id).await?;
    if m.status != "active" {
        return Err(AppError::conflict("mailbox_not_active", "Only active mailboxes can be reset"));
    }
    let method = body.method.as_deref().unwrap_or("email");
    match method {
        "email" if m.recovery_email.is_none() => {
            return Err(AppError::conflict(
                "no_recovery_email",
                "No personal email is on file for this mailbox. An administrator can add one or issue a handover link.",
            ))
        }
        "email" => {}
        "link" => p.require(Permission::MailboxesWrite)?,
        _ => return Err(AppError::validation("method must be 'email' or 'link'")),
    }
    state.limiter.check(&format!("mbx-reset:{id}"), 10, 3600)?;
    let mut tx = state.db.begin().await?;
    let delivery = if method == "email" { "email" } else { "handover" };
    let (token, expires_at) = mailbox_tokens::issue(&mut tx, id, mailbox_tokens::RESET, delivery, p.user_id()).await?;
    let link = format!("{}/mail/reset-password?token={token}", state.cfg.public_base_url);
    let result = if method == "email" {
        let to = m.recovery_email.clone().unwrap_or_default();
        system_mail::enqueue(
            &mut *tx,
            system_mail::mailbox_password_reset(&state.cfg.system_from_name, &to, &m.address, &link),
        )
        .await?;
        json!({ "method": "email", "sent_to": mask_email(&to), "expires_at": expires_at })
    } else {
        json!({ "method": "link", "url": link, "expires_at": expires_at,
                "notice": "Give this link only to the employee. It works once and expires in one hour." })
    };
    Entry::by(&p, &client, "mailbox.reset_link_sent")
        .target("mailbox", id)
        .details(json!({ "address": m.address, "method": method }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "reset": result })))
}

async fn unlock(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesAssist)?;
    let m = load(&state, p.org_id, id).await?;
    sqlx::query("UPDATE mailboxes SET failed_login_count = 0, locked_until = NULL, updated_at = now() WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(p.org_id)
        .execute(&state.db)
        .await?;
    Entry::by(&p, &client, "mailbox.unlocked")
        .target("mailbox", id)
        .details(json!({ "address": m.address }))
        .record(&state.db)
        .await?;
    Ok(Json(json!({ "id": id, "locked": false })))
}

/// Aggregated mail-app onboarding for the organization (last 90 days).
/// Counts only: no message content, credentials or device identifiers.
async fn onboarding_report(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesRead)?;
    #[derive(sqlx::FromRow)]
    struct Totals {
        mailboxes: i64,
        pending: i64,
        started: i64,
        completed: i64,
        incoming_verified: i64,
        outgoing_verified: i64,
        fully_verified: i64,
    }
    let t: Totals = sqlx::query_as(
        "WITH mb AS (SELECT id FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted')
         SELECT (SELECT count(*) FROM mb) AS mailboxes,
                (SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status = 'pending') AS pending,
                (SELECT count(DISTINCT mailbox_id) FROM mailbox_setup_events WHERE organization_id = $1 AND event = 'guide_started') AS started,
                (SELECT count(DISTINCT mailbox_id) FROM mailbox_setup_events WHERE organization_id = $1 AND event = 'guide_completed') AS completed,
                (SELECT count(DISTINCT mailbox_id) FROM mailbox_connection_checks WHERE organization_id = $1 AND direction = 'incoming' AND status = 'verified') AS incoming_verified,
                (SELECT count(DISTINCT mailbox_id) FROM mailbox_connection_checks WHERE organization_id = $1 AND direction = 'outgoing' AND status = 'verified') AS outgoing_verified,
                (SELECT count(*) FROM mb WHERE
                    EXISTS (SELECT 1 FROM mailbox_connection_checks c WHERE c.mailbox_id = mb.id AND c.direction = 'incoming' AND c.status = 'verified')
                AND EXISTS (SELECT 1 FROM mailbox_connection_checks c WHERE c.mailbox_id = mb.id AND c.direction = 'outgoing' AND c.status = 'verified')) AS fully_verified",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    let by_client: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT client, count(DISTINCT mailbox_id) FROM mailbox_setup_events
         WHERE organization_id = $1 AND event = 'client_selected' AND created_at > now() - interval '90 days'
         GROUP BY client ORDER BY 2 DESC",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    let by_device: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT device, count(DISTINCT mailbox_id) FROM mailbox_setup_events
         WHERE organization_id = $1 AND event = 'device_selected' AND created_at > now() - interval '90 days'
         GROUP BY device ORDER BY 2 DESC",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    let failures: Vec<(Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT client, detail, count(*) FROM mailbox_setup_events
         WHERE organization_id = $1 AND event IN ('connection_failed', 'help_viewed') AND created_at > now() - interval '90 days'
         GROUP BY client, detail ORDER BY 3 DESC LIMIT 20",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "totals": {
            "mailboxes": t.mailboxes, "pending_activation": t.pending, "guide_started": t.started,
            "guide_completed": t.completed, "incoming_verified": t.incoming_verified,
            "outgoing_verified": t.outgoing_verified, "fully_verified": t.fully_verified,
        },
        "by_client": by_client.into_iter().map(|(c, n)| json!({ "client": c, "mailboxes": n })).collect::<Vec<_>>(),
        "by_device": by_device.into_iter().map(|(d, n)| json!({ "device": d, "mailboxes": n })).collect::<Vec<_>>(),
        "help_topics": failures.into_iter().map(|(c, d, n)| json!({ "client": c, "topic": d, "count": n })).collect::<Vec<_>>(),
    })))
}

async fn delete_mailbox(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesWrite)?;
    let m = load(&state, p.org_id, id).await?;
    let in_alias: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM aliases WHERE organization_id = $1 AND $2::citext = ANY(destinations)",
    )
    .bind(p.org_id)
    .bind(&m.address)
    .fetch_one(&state.db)
    .await?;
    if in_alias > 0 {
        return Err(AppError::conflict(
            "mailbox_in_use",
            format!("Remove {} from its {in_alias} alias(es) first", m.address),
        ));
    }
    let tombstone = format!("deleted-{}", id.simple());
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE mailboxes SET status = 'deleted', deleted_at = now(), deleted_address = address,
                local_part = $3, address = $3 || '@' || $4, updated_at = now()
         WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(&tombstone)
    .bind(&m.domain)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE mailbox_tokens SET used_at = now() WHERE mailbox_id = $1 AND used_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    session::revoke_all_for_mailbox_tx(&mut tx, id).await?;
    Entry::by(&p, &client, "mailbox.deleted")
        .target("mailbox", id)
        .details(json!({ "address": m.address, "note": "Mail access stopped immediately; stored mail is purged per the retention runbook." }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    doveadm::revoke_access(&state, vec![m.address.clone()]).await;
    Ok(Json(json!({ "id": id, "deleted": true })))
}

// ── aliases ──────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct AliasRow {
    id: Uuid,
    domain_id: Uuid,
    address: String,
    destinations: Vec<String>,
    status: String,
    created_at: DateTime<Utc>,
}

fn alias_json(a: &AliasRow) -> Value {
    json!({ "id": a.id, "domain_id": a.domain_id, "address": a.address, "destinations": a.destinations, "status": a.status, "created_at": a.created_at })
}

async fn list_aliases(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesRead)?;
    let rows: Vec<AliasRow> = sqlx::query_as(
        "SELECT id, domain_id, address::text AS address, destinations::text[] AS destinations, status, created_at
         FROM aliases WHERE organization_id = $1 ORDER BY address",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.iter().map(alias_json).collect::<Vec<_>>() })))
}

/// Destinations must be active mailboxes of the same tenant.
async fn validate_destinations(
    state: &AppState,
    org_id: Uuid,
    input: &[String],
    self_address: &str,
) -> ApiResult<Vec<String>> {
    if input.is_empty() || input.len() > 50 {
        return Err(AppError::validation("Provide 1 to 50 destination mailboxes"));
    }
    let mut out: Vec<String> = Vec::new();
    for d in input {
        let a = validation::parse_email("destinations", d)?.as_string().to_lowercase();
        if a == self_address {
            return Err(AppError::validation("An alias cannot deliver to itself"));
        }
        if !out.contains(&a) {
            out.push(a);
        }
    }
    let found: Vec<String> = sqlx::query_scalar(
        "SELECT address::text FROM mailboxes WHERE organization_id = $1 AND status IN ('active', 'pending') AND address = ANY($2::citext[])",
    )
    .bind(org_id)
    .bind(&out)
    .fetch_all(&state.db)
    .await?;
    let missing: Vec<&String> = out.iter().filter(|a| !found.iter().any(|f| f.eq_ignore_ascii_case(a))).collect();
    if !missing.is_empty() {
        return Err(AppError::validation(format!(
            "Destinations must be mailboxes in this organization: {}",
            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(out)
}

#[derive(Deserialize)]
struct CreateAliasBody {
    domain_id: Option<Uuid>,
    domain: Option<String>,
    local_part: String,
    destinations: Vec<String>,
}

async fn create_alias(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateAliasBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::MailboxesWrite)?;
    p.require_institution()?;
    let (domain_id, domain) = hosting_domain(&state, p.org_id, body.domain_id, body.domain.as_deref()).await?;
    let local = validation::hosted_local_part(&body.local_part)?;
    let address = format!("{local}@{domain}");
    if address_taken(&state, &address).await? {
        return Err(AppError::conflict("address_taken", format!("{address} is already in use")));
    }
    let destinations = validate_destinations(&state, p.org_id, &body.destinations, &address).await?;
    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO aliases (organization_id, domain_id, local_part, address, destinations) VALUES ($1, $2, $3, $4, $5::citext[]) RETURNING id",
    )
    .bind(p.org_id)
    .bind(domain_id)
    .bind(&local)
    .bind(&address)
    .bind(&destinations)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| on_unique_violation(e, "address_taken", "This address is already in use"))?;
    Entry::by(&p, &client, "alias.created")
        .target("alias", id)
        .details(json!({ "address": address, "destinations": destinations }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({ "id": id, "domain_id": domain_id, "address": address, "destinations": destinations, "status": "active" }),
        ),
    ))
}

#[derive(Deserialize)]
struct UpdateAliasBody {
    destinations: Option<Vec<String>>,
    status: Option<String>,
}

async fn update_alias(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateAliasBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesWrite)?;
    let a: AliasRow = sqlx::query_as(
        "SELECT id, domain_id, address::text AS address, destinations::text[] AS destinations, status, created_at FROM aliases WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Alias"))?;
    let destinations = match &body.destinations {
        Some(d) => validate_destinations(&state, p.org_id, d, &a.address).await?,
        None => a.destinations.clone(),
    };
    let status = match body.status.as_deref() {
        None => a.status.clone(),
        Some(s @ ("active" | "disabled")) => s.to_string(),
        Some(_) => return Err(AppError::validation("status must be 'active' or 'disabled'")),
    };
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE aliases SET destinations = $3::citext[], status = $4, updated_at = now() WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(p.org_id)
        .bind(&destinations)
        .bind(&status)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "alias.updated")
        .target("alias", id)
        .details(json!({ "destinations": destinations, "status": status }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "address": a.address, "destinations": destinations, "status": status })))
}

async fn delete_alias(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesWrite)?;
    let mut tx = state.db.begin().await?;
    let address: Option<String> =
        sqlx::query_scalar("DELETE FROM aliases WHERE id = $1 AND organization_id = $2 RETURNING address::text")
            .bind(id)
            .bind(p.org_id)
            .fetch_optional(&mut *tx)
            .await?;
    let address = address.ok_or_else(|| AppError::not_found("Alias"))?;
    Entry::by(&p, &client, "alias.deleted")
        .target("alias", id)
        .details(json!({ "address": address }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "deleted": true })))
}

#[cfg(test)]
mod tests {
    use super::mask_email;

    #[test]
    fn masks_recovery_addresses() {
        assert_eq!(mask_email("amina@gmail.com"), "a****@gmail.com");
        assert_eq!(mask_email("a@b.co"), "a**@b.co");
        assert!(!mask_email("brian.kiprop@yahoo.com").contains("brian"));
    }
}
