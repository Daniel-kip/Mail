//! Mailbox-user account features: first-login activation, self-service
//! password recovery, preferences (signature, out-of-office), personal
//! contacts and the organization directory.
//!
//! Activation and recovery are public (the user cannot sign in yet) and
//! protected by single-use, short-lived, hashed tokens; the forgot-password
//! endpoint always answers the same way so it cannot reveal which addresses
//! exist. Every credential change revokes webmail sessions and kicks IMAP
//! connections so the old password stops working everywhere at once.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{empty_as_none, ApiJson, ApiQuery};
use crate::{
    auth::{ClientInfo, MailboxSession},
    error::{ApiResult, AppError},
    password,
    services::{audit::Entry, doveadm, mailbox_tokens, sieve, system_mail},
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/activation", get(view_token))
        .route("/activate", post(activate))
        .route("/password/forgot", post(forgot))
        .route("/password/reset", post(reset))
        .route("/preferences", get(get_preferences).put(put_preferences))
        .route("/contacts", get(list_contacts).post(create_contact))
        .route("/contacts/{id}", patch(update_contact).delete(delete_contact))
        .route("/directory", get(directory))
        .route("/recipients/suggest", get(suggest))
}

fn mailbox_entry<'a>(org: Uuid, mailbox: Uuid, address: &str, client: &'a ClientInfo, action: &'a str) -> Entry<'a> {
    Entry {
        organization_id: Some(org),
        actor_type: "mailbox",
        actor_id: Some(mailbox),
        actor_label: Some(address.to_string()),
        action,
        target_type: Some("mailbox"),
        target_id: Some(mailbox.to_string()),
        client: Some(client),
        details: json!({}),
    }
}

// ── activation & recovery (public) ───────────────────────────────────────

#[derive(Deserialize)]
struct TokenQuery {
    token: String,
}

async fn view_token(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiQuery(q): ApiQuery<TokenQuery>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mbx-token-view:{}", client.ip_key()), 60, 3600)?;
    let mut conn = state.db.acquire().await?;
    let (mailbox_id, purpose, expires_at) = mailbox_tokens::peek(&mut conn, &q.token).await?;
    let (address, display_name, org, status): (String, Option<String>, String, String) = sqlx::query_as(
        "SELECT m.address::text, m.display_name, o.name, m.status FROM mailboxes m JOIN organizations o ON o.id = m.organization_id WHERE m.id = $1",
    )
    .bind(mailbox_id)
    .fetch_one(&mut *conn)
    .await?;
    if status == "deleted" || status == "disabled" {
        return Err(AppError::bad_request("invalid_token", "This mailbox is not available. Contact your IT team."));
    }
    Ok(Json(json!({
        "purpose": purpose, "address": address, "display_name": display_name,
        "organization": org, "expires_at": expires_at,
    })))
}

#[derive(Deserialize)]
struct SetPasswordBody {
    token: String,
    password: String,
}

async fn set_password_with_token(
    state: &AppState,
    client: &ClientInfo,
    body: &SetPasswordBody,
    purpose: &str,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mbx-token-use:{}", client.ip_key()), 20, 3600)?;
    let mut tx = state.db.begin().await?;
    let (mailbox_id, _) = mailbox_tokens::consume(&mut tx, &body.token, &[purpose]).await?;
    let (address, org_id, status): (String, Uuid, String) =
        sqlx::query_as("SELECT address::text, organization_id, status FROM mailboxes WHERE id = $1 FOR UPDATE")
            .bind(mailbox_id)
            .fetch_one(&mut *tx)
            .await?;
    let allowed = if purpose == mailbox_tokens::ACTIVATE {
        status == "pending" || status == "active"
    } else {
        status == "active"
    };
    if !allowed {
        return Err(AppError::bad_request("invalid_token", "This mailbox is not available. Contact your IT team."));
    }
    password::check_policy(&body.password, &address)?;
    crate::services::breach::ensure_not_breached(&state.cfg, &body.password).await?;
    let hash = password::hash_for_mailbox(&body.password).await?;
    sqlx::query(
        "UPDATE mailboxes SET password_hash = $2, status = 'active', activated_at = COALESCE(activated_at, now()),
                password_changed_at = now(), failed_login_count = 0, locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(mailbox_id)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE mailbox_id = $1 AND revoked_at IS NULL")
        .bind(mailbox_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE mailbox_tokens SET used_at = now() WHERE mailbox_id = $1 AND used_at IS NULL")
        .bind(mailbox_id)
        .execute(&mut *tx)
        .await?;
    let action =
        if purpose == mailbox_tokens::ACTIVATE { "mailbox.activated" } else { "mailbox.password_reset_by_user" };
    mailbox_entry(org_id, mailbox_id, &address, client, action).record(&mut *tx).await?;
    tx.commit().await?;
    doveadm::revoke_access(state, vec![address.clone()]).await;
    Ok(Json(json!({
        "ok": true, "address": address,
        "next": "Sign in to webmail with your email address and new password, then connect your phone from the setup guide.",
    })))
}

async fn activate(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<SetPasswordBody>,
) -> ApiResult<Json<Value>> {
    set_password_with_token(&state, &client, &body, mailbox_tokens::ACTIVATE).await
}

async fn reset(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<SetPasswordBody>,
) -> ApiResult<Json<Value>> {
    set_password_with_token(&state, &client, &body, mailbox_tokens::RESET).await
}

#[derive(Deserialize)]
struct ForgotBody {
    address: String,
}

/// Always 202 with the same message: the response never reveals whether the
/// mailbox exists or has a recovery address.
async fn forgot(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<ForgotBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("mbx-forgot-ip:{}", client.ip_key()), 10, 3600)?;
    let address = body.address.trim().to_lowercase();
    state.limiter.check(&format!("mbx-forgot:{address}"), 3, 3600)?;
    let row: Option<(Uuid, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT m.id, m.organization_id, m.recovery_email::text FROM mailboxes m
         WHERE m.address = $1 AND m.status = 'active'
           AND EXISTS (SELECT 1 FROM mail_mailboxes mm WHERE mm.address = m.address::text)",
    )
    .bind(&address)
    .fetch_optional(&state.db)
    .await?;
    if let Some((mailbox_id, org_id, Some(recovery))) = row {
        let mut tx = state.db.begin().await?;
        let (token, _) =
            mailbox_tokens::issue(&mut tx, mailbox_id, mailbox_tokens::RESET, "self_service", None).await?;
        let link = format!("{}/mail/reset-password?token={token}", state.cfg.public_base_url);
        system_mail::enqueue(
            &mut *tx,
            system_mail::mailbox_password_reset(&state.cfg.system_from_name, &recovery, &address, &link),
        )
        .await?;
        mailbox_entry(org_id, mailbox_id, &address, &client, "mailbox.password_reset_requested")
            .record(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "message": "If this mailbox has a personal recovery email on file, a reset link has been sent there. If you receive nothing within a few minutes, contact your IT team.",
        })),
    ))
}

// ── preferences ──────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct Prefs {
    signature: Option<String>,
    messages_per_page: i32,
    auto_reply_enabled: bool,
    auto_reply_subject: Option<String>,
    auto_reply_body: Option<String>,
    auto_reply_starts_on: Option<NaiveDate>,
    auto_reply_ends_on: Option<NaiveDate>,
    auto_reply_synced_at: Option<DateTime<Utc>>,
    auto_reply_sync_error: Option<String>,
}

async fn load_prefs(state: &AppState, mailbox_id: Uuid) -> ApiResult<Prefs> {
    let p: Option<Prefs> = sqlx::query_as(
        "SELECT signature, messages_per_page, auto_reply_enabled, auto_reply_subject, auto_reply_body, auto_reply_starts_on,
                auto_reply_ends_on, auto_reply_synced_at, auto_reply_sync_error
         FROM mailbox_preferences WHERE mailbox_id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(p.unwrap_or(Prefs {
        signature: None,
        messages_per_page: 50,
        auto_reply_enabled: false,
        auto_reply_subject: None,
        auto_reply_body: None,
        auto_reply_starts_on: None,
        auto_reply_ends_on: None,
        auto_reply_synced_at: None,
        auto_reply_sync_error: None,
    }))
}

fn prefs_json(state: &AppState, p: &Prefs) -> Value {
    let today = Utc::now().date_naive();
    let active_now = p.auto_reply_enabled
        && p.auto_reply_starts_on.map_or(true, |d| d <= today)
        && p.auto_reply_ends_on.map_or(true, |d| d >= today);
    json!({
        "signature": p.signature,
        "messages_per_page": p.messages_per_page,
        "auto_reply": {
            "enabled": p.auto_reply_enabled, "active_now": active_now,
            "subject": p.auto_reply_subject, "body": p.auto_reply_body,
            "starts_on": p.auto_reply_starts_on, "ends_on": p.auto_reply_ends_on,
            "synced_at": p.auto_reply_synced_at, "sync_error": p.auto_reply_sync_error,
            "server_supported": state.cfg.sieve.is_some() && state.cfg.imap.is_some(),
        },
    })
}

async fn get_preferences(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    let p = load_prefs(&state, s.mailbox_id).await?;
    Ok(Json(prefs_json(&state, &p)))
}

#[derive(Deserialize)]
struct PrefsBody {
    signature: Option<String>,
    messages_per_page: Option<i32>,
    auto_reply_enabled: Option<bool>,
    auto_reply_subject: Option<String>,
    auto_reply_body: Option<String>,
    auto_reply_starts_on: Option<NaiveDate>,
    auto_reply_ends_on: Option<NaiveDate>,
}

fn limited(field: &str, v: Option<String>, max: usize) -> ApiResult<Option<String>> {
    match v.map(|s| s.replace("\r\n", "\n")).filter(|s| !s.trim().is_empty()) {
        None => Ok(None),
        Some(s) if s.chars().count() > max => {
            Err(AppError::validation(format!("{field} must be at most {max} characters")))
        }
        Some(s) if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') => {
            Err(AppError::validation(format!("{field} contains control characters")))
        }
        Some(s) => Ok(Some(s)),
    }
}

async fn put_preferences(
    State(state): State<AppState>,
    client: ClientInfo,
    s: MailboxSession,
    ApiJson(b): ApiJson<PrefsBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mbx-prefs:{}", s.mailbox_id), 30, 600)?;
    let current = load_prefs(&state, s.mailbox_id).await?;
    let signature = match b.signature {
        Some(v) => limited("signature", Some(v), 4000)?,
        None => current.signature.clone(),
    };
    let per_page = b.messages_per_page.unwrap_or(current.messages_per_page);
    if !(10..=100).contains(&per_page) {
        return Err(AppError::validation("messages_per_page must be between 10 and 100"));
    }
    let enabled = b.auto_reply_enabled.unwrap_or(current.auto_reply_enabled);
    let subject = match b.auto_reply_subject {
        Some(v) => limited("auto_reply_subject", Some(v), 200)?,
        None => current.auto_reply_subject.clone(),
    };
    if let Some(sub) = &subject {
        validation::header_safe("auto_reply_subject", sub, 200)?;
    }
    let body = match b.auto_reply_body {
        Some(v) => limited("auto_reply_body", Some(v), 4000)?,
        None => current.auto_reply_body.clone(),
    };
    let starts = if b.auto_reply_enabled.is_some() || b.auto_reply_starts_on.is_some() {
        b.auto_reply_starts_on
    } else {
        current.auto_reply_starts_on
    };
    let ends = if b.auto_reply_enabled.is_some() || b.auto_reply_ends_on.is_some() {
        b.auto_reply_ends_on
    } else {
        current.auto_reply_ends_on
    };
    if let (Some(a), Some(z)) = (starts, ends) {
        if z < a {
            return Err(AppError::validation("The auto-reply end date must be on or after the start date"));
        }
    }
    if enabled && body.is_none() {
        return Err(AppError::validation("Write the auto-reply message before turning it on"));
    }
    let auto_reply_changed = enabled != current.auto_reply_enabled
        || subject != current.auto_reply_subject
        || body != current.auto_reply_body
        || starts != current.auto_reply_starts_on
        || ends != current.auto_reply_ends_on;

    // Apply to the mail server first so the stored state reflects reality.
    let (synced_at, sync_error) = if auto_reply_changed {
        match (&state.cfg.sieve, &state.cfg.imap) {
            (Some(sv), Some(im)) => {
                let result = if enabled {
                    sieve::install(
                        sv,
                        im,
                        &s.address,
                        &sieve::Vacation {
                            subject: subject.as_deref().unwrap_or("Out of office"),
                            body: body.as_deref().unwrap_or(""),
                            starts_on: starts,
                            ends_on: ends,
                        },
                    )
                    .await
                } else {
                    sieve::remove(sv, im, &s.address).await
                };
                match result {
                    Ok(()) => (Some(Utc::now()), None),
                    Err(e) => {
                        tracing::warn!(mailbox = %s.mailbox_id, error = %e, "auto-reply sync failed");
                        return Err(AppError::unavailable(
                            "auto_reply_unavailable",
                            "The mail server could not save your auto-reply right now. Try again shortly.",
                        ));
                    }
                }
            }
            _ if enabled => {
                return Err(AppError::unavailable(
                    "auto_reply_unsupported",
                    "Automatic replies are not enabled on this mail server. Ask your IT team.",
                ))
            }
            _ => (current.auto_reply_synced_at, None),
        }
    } else {
        (current.auto_reply_synced_at, current.auto_reply_sync_error.clone())
    };

    sqlx::query(
        "INSERT INTO mailbox_preferences (mailbox_id, signature, messages_per_page, auto_reply_enabled, auto_reply_subject,
                                          auto_reply_body, auto_reply_starts_on, auto_reply_ends_on, auto_reply_synced_at,
                                          auto_reply_sync_error, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now())
         ON CONFLICT (mailbox_id) DO UPDATE SET signature = EXCLUDED.signature, messages_per_page = EXCLUDED.messages_per_page,
             auto_reply_enabled = EXCLUDED.auto_reply_enabled, auto_reply_subject = EXCLUDED.auto_reply_subject,
             auto_reply_body = EXCLUDED.auto_reply_body, auto_reply_starts_on = EXCLUDED.auto_reply_starts_on,
             auto_reply_ends_on = EXCLUDED.auto_reply_ends_on, auto_reply_synced_at = EXCLUDED.auto_reply_synced_at,
             auto_reply_sync_error = EXCLUDED.auto_reply_sync_error, updated_at = now()",
    )
    .bind(s.mailbox_id)
    .bind(&signature)
    .bind(per_page)
    .bind(enabled)
    .bind(&subject)
    .bind(&body)
    .bind(starts)
    .bind(ends)
    .bind(synced_at)
    .bind(&sync_error)
    .execute(&state.db)
    .await?;
    if auto_reply_changed {
        mailbox_entry(s.organization_id, s.mailbox_id, &s.address, &client, "mailbox.auto_reply_updated")
            .details(json!({ "enabled": enabled, "starts_on": starts, "ends_on": ends }))
            .record(&state.db)
            .await?;
    }
    let p = load_prefs(&state, s.mailbox_id).await?;
    Ok(Json(prefs_json(&state, &p)))
}

// ── contacts ─────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ContactRow {
    id: Uuid,
    name: Option<String>,
    email: String,
    phone: Option<String>,
    organization: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn contact_json(c: &ContactRow) -> Value {
    json!({ "id": c.id, "name": c.name, "email": c.email, "phone": c.phone, "organization": c.organization,
            "notes": c.notes, "created_at": c.created_at, "updated_at": c.updated_at })
}

fn like(q: &str) -> String {
    format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

async fn list_contacts(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let pattern = empty_as_none(q.q).map(|v| like(&v));
    let rows: Vec<ContactRow> = sqlx::query_as(
        "SELECT id, name, email::text AS email, phone, organization, notes, created_at, updated_at FROM mailbox_contacts
         WHERE mailbox_id = $1 AND ($2::text IS NULL OR name ILIKE $2 OR email::text ILIKE $2 OR organization ILIKE $2)
         ORDER BY lower(coalesce(name, email::text)) LIMIT 1000",
    )
    .bind(s.mailbox_id)
    .bind(&pattern)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.iter().map(contact_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct ContactBody {
    name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    organization: Option<String>,
    notes: Option<String>,
}

struct CleanContact {
    name: Option<String>,
    email: String,
    phone: Option<String>,
    organization: Option<String>,
    notes: Option<String>,
}

fn clean_contact(b: ContactBody, existing_email: Option<&str>) -> ApiResult<CleanContact> {
    let email = match (b.email.as_deref().map(str::trim).filter(|e| !e.is_empty()), existing_email) {
        (Some(e), _) => validation::parse_email("email", e)?.as_string().to_lowercase(),
        (None, Some(e)) => e.to_string(),
        (None, None) => return Err(AppError::validation("email is required")),
    };
    let text = |field: &str, v: Option<String>, max: usize| -> ApiResult<Option<String>> {
        match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            None => Ok(None),
            Some(s) => {
                if field == "notes" {
                    limited(field, Some(s), max)
                } else {
                    validation::header_safe(field, &s, max)?;
                    Ok(Some(s))
                }
            }
        }
    };
    Ok(CleanContact {
        name: text("name", b.name, 200)?,
        email,
        phone: text("phone", b.phone, 40)?,
        organization: text("organization", b.organization, 200)?,
        notes: text("notes", b.notes, 2000)?,
    })
}

async fn create_contact(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiJson(b): ApiJson<ContactBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let c = clean_contact(b, None)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM mailbox_contacts WHERE mailbox_id = $1")
        .bind(s.mailbox_id)
        .fetch_one(&state.db)
        .await?;
    if count >= 5000 {
        return Err(AppError::conflict("contact_limit", "You can keep up to 5,000 contacts"));
    }
    let row: ContactRow = sqlx::query_as(
        "INSERT INTO mailbox_contacts (mailbox_id, name, email, phone, organization, notes) VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, name, email::text AS email, phone, organization, notes, created_at, updated_at",
    )
    .bind(s.mailbox_id)
    .bind(&c.name)
    .bind(&c.email)
    .bind(&c.phone)
    .bind(&c.organization)
    .bind(&c.notes)
    .fetch_one(&state.db)
    .await
    .map_err(|e| crate::error::on_unique_violation(e, "contact_exists", "A contact with this email already exists"))?;
    Ok((StatusCode::CREATED, Json(contact_json(&row))))
}

async fn update_contact(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(id): Path<Uuid>,
    ApiJson(b): ApiJson<ContactBody>,
) -> ApiResult<Json<Value>> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT email::text FROM mailbox_contacts WHERE id = $1 AND mailbox_id = $2")
            .bind(id)
            .bind(s.mailbox_id)
            .fetch_optional(&state.db)
            .await?;
    let existing = existing.ok_or_else(|| AppError::not_found("Contact"))?;
    let c = clean_contact(b, Some(&existing))?;
    let row: ContactRow = sqlx::query_as(
        "UPDATE mailbox_contacts SET name = $3, email = $4, phone = $5, organization = $6, notes = $7, updated_at = now()
         WHERE id = $1 AND mailbox_id = $2
         RETURNING id, name, email::text AS email, phone, organization, notes, created_at, updated_at",
    )
    .bind(id)
    .bind(s.mailbox_id)
    .bind(&c.name)
    .bind(&c.email)
    .bind(&c.phone)
    .bind(&c.organization)
    .bind(&c.notes)
    .fetch_one(&state.db)
    .await
    .map_err(|e| crate::error::on_unique_violation(e, "contact_exists", "A contact with this email already exists"))?;
    Ok(Json(contact_json(&row)))
}

async fn delete_contact(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let n = sqlx::query("DELETE FROM mailbox_contacts WHERE id = $1 AND mailbox_id = $2")
        .bind(id)
        .bind(s.mailbox_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Contact"));
    }
    Ok(Json(json!({ "id": id, "deleted": true })))
}

// ── organization directory & recipient suggestions ───────────────────────

async fn directory(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let pattern = empty_as_none(q.q).map(|v| like(&v));
    type PersonRow = (String, Option<String>, Option<String>, Option<String>);
    let rows: Vec<PersonRow> = sqlx::query_as(
        "SELECT address::text, display_name, department, job_title FROM mailboxes
         WHERE organization_id = $1 AND status = 'active'
           AND ($2::text IS NULL OR address::text ILIKE $2 OR display_name ILIKE $2 OR department ILIKE $2 OR job_title ILIKE $2)
         ORDER BY lower(coalesce(display_name, address::text)) LIMIT 500",
    )
    .bind(s.organization_id)
    .bind(&pattern)
    .fetch_all(&state.db)
    .await?;
    let aliases: Vec<String> = sqlx::query_scalar(
        "SELECT address::text FROM aliases WHERE organization_id = $1 AND status = 'active' AND ($2::text IS NULL OR address::text ILIKE $2) ORDER BY address LIMIT 200",
    )
    .bind(s.organization_id)
    .bind(&pattern)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "people": rows.into_iter().map(|(a, n, d, j)| json!({ "address": a, "name": n, "department": d, "job_title": j })).collect::<Vec<_>>(),
        "groups": aliases,
    })))
}

async fn suggest(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let Some(term) = empty_as_none(q.q).filter(|t| t.chars().count() >= 2) else {
        return Ok(Json(json!({ "data": [] })));
    };
    let pattern = like(&term);
    let rows: Vec<(String, Option<String>, String)> = sqlx::query_as(
        "(SELECT email::text, name, 'contact' FROM mailbox_contacts WHERE mailbox_id = $1 AND (email::text ILIKE $3 OR name ILIKE $3) LIMIT 8)
         UNION ALL
         (SELECT address::text, display_name, 'colleague' FROM mailboxes WHERE organization_id = $2 AND status = 'active' AND id <> $1
            AND (address::text ILIKE $3 OR display_name ILIKE $3) LIMIT 8)
         UNION ALL
         (SELECT to_address, NULL, 'recent' FROM messages WHERE mailbox_id = $1 AND to_address ILIKE $3
            GROUP BY to_address ORDER BY max(created_at) DESC LIMIT 8)",
    )
    .bind(s.mailbox_id)
    .bind(s.organization_id)
    .bind(&pattern)
    .fetch_all(&state.db)
    .await?;
    let mut seen = std::collections::HashSet::new();
    let data: Vec<Value> = rows
        .into_iter()
        .filter(|(e, _, _)| !e.contains(',') && seen.insert(e.to_lowercase()))
        .take(10)
        .map(|(e, n, k)| json!({ "email": e, "name": n, "source": k }))
        .collect();
    Ok(Json(json!({ "data": data })))
}
