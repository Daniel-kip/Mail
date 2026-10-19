//! Platform-operator endpoints (guide 05: separate and strongly protected).
//!
//! Requires `users.is_platform_admin` and, in production, enrolled MFA.
//! Every action is written to the audit log with actor type
//! `platform_admin`. Suspension is the emergency brake of guide 07: it stops
//! API keys, outbound sending, inbound acceptance and mailbox logins at once.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{empty_as_none, ApiJson, ApiQuery};
use crate::{
    auth::{ClientInfo, PlatformAdmin},
    error::{ApiResult, AppError},
    services::{
        audit::Entry,
        mail_client::{self, ProfileInput},
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/overview", get(overview))
        .route("/organizations", get(organizations))
        .route("/organizations/{id}", get(organization_detail).patch(update_organization))
        .route("/users", get(users))
        .route("/mail-client-settings", get(client_settings).put(put_default_client_settings))
        .route(
            "/mail-client-settings/domains/{domain_id}",
            axum::routing::put(put_domain_client_settings).delete(delete_domain_client_settings),
        )
        .route("/onboarding", get(onboarding_metrics))
        .route("/security", get(security_overview))
        .route("/organizations/{id}/suspend", post(suspend))
        .route("/organizations/{id}/unsuspend", post(unsuspend))
        .route("/jobs/dead", get(dead_jobs))
        .route("/jobs/{id}/requeue", post(requeue_job))
        .route("/users/{id}/disable", post(disable_user))
        .route("/users/{id}/enable", post(enable_user))
}

fn entry<'a>(admin: &'a PlatformAdmin, client: &'a ClientInfo, org: Option<Uuid>, action: &'a str) -> Entry<'a> {
    Entry {
        organization_id: org,
        actor_type: "platform_admin",
        actor_id: Some(admin.0.user_id),
        actor_label: Some(admin.0.email.clone()),
        action,
        target_type: None,
        target_id: None,
        client: Some(client),
        details: json!({}),
    }
}

async fn overview(State(state): State<AppState>, _admin: PlatformAdmin) -> ApiResult<Json<Value>> {
    #[derive(sqlx::FromRow)]
    struct O {
        organizations: i64,
        suspended: i64,
        users: i64,
        mailboxes: i64,
        messages_24h: i64,
        bounced_24h: i64,
        failed_24h: i64,
        jobs_pending: i64,
        jobs_processing: i64,
        jobs_dead: i64,
        oldest_pending_seconds: Option<f64>,
        webhook_failures_24h: i64,
    }
    let o: O = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM organizations) AS organizations,
           (SELECT count(*) FROM organizations WHERE status = 'suspended') AS suspended,
           (SELECT count(*) FROM users) AS users,
           (SELECT count(*) FROM mailboxes WHERE status <> 'deleted') AS mailboxes,
           (SELECT count(*) FROM messages WHERE created_at > now() - interval '24 hours') AS messages_24h,
           (SELECT count(*) FROM messages WHERE created_at > now() - interval '24 hours' AND status = 'bounced') AS bounced_24h,
           (SELECT count(*) FROM messages WHERE created_at > now() - interval '24 hours' AND status = 'failed') AS failed_24h,
           (SELECT count(*) FROM outbox_jobs WHERE status = 'pending') AS jobs_pending,
           (SELECT count(*) FROM outbox_jobs WHERE status = 'processing') AS jobs_processing,
           (SELECT count(*) FROM outbox_jobs WHERE status = 'dead') AS jobs_dead,
           (SELECT extract(epoch FROM now() - min(run_at))::float8 FROM outbox_jobs WHERE status = 'pending' AND run_at <= now()) AS oldest_pending_seconds,
           (SELECT count(*) FROM webhook_deliveries WHERE status = 'failed' AND created_at > now() - interval '24 hours') AS webhook_failures_24h",
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({
        "organizations": o.organizations, "suspended_organizations": o.suspended, "users": o.users, "mailboxes": o.mailboxes,
        "messages_24h": o.messages_24h, "bounced_24h": o.bounced_24h, "failed_24h": o.failed_24h,
        "queue": { "pending": o.jobs_pending, "processing": o.jobs_processing, "dead": o.jobs_dead,
                   "oldest_pending_seconds": o.oldest_pending_seconds.unwrap_or(0.0).round() },
        "webhook_failures_24h": o.webhook_failures_24h,
        "smtp_relay": format!("{}:{}", state.cfg.smtp.host, state.cfg.smtp.port),
        "mail_storage_configured": state.cfg.imap.is_some(),
        "auto_reply_configured": state.cfg.sieve.is_some(),
        "billing": billing_summary(&state).await?,
    })))
}

async fn billing_summary(state: &AppState) -> ApiResult<Value> {
    let (open, open_total, overdue, paid_30d): (i64, Option<i64>, i64, Option<i64>) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'open'),
                (sum(total_cents) FILTER (WHERE status = 'open'))::bigint,
                count(*) FILTER (WHERE status = 'open' AND due_at < now()),
                (sum(total_cents) FILTER (WHERE status = 'paid' AND paid_at > now() - interval '30 days'))::bigint
         FROM invoices",
    )
    .fetch_one(&state.db)
    .await?;
    let by_plan: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT p.id, p.name, count(o.id) FROM plans p LEFT JOIN organizations o ON o.plan_id = p.id
         GROUP BY p.id, p.name, p.sort_order ORDER BY p.audience, p.sort_order",
    )
    .fetch_all(&state.db)
    .await?;
    let past_due: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subscriptions WHERE status = 'past_due'").fetch_one(&state.db).await?;
    let trialing: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subscriptions WHERE status = 'trialing'").fetch_one(&state.db).await?;
    Ok(json!({
        "currency": state.cfg.billing.currency,
        "open_invoices": open, "open_total_cents": open_total.unwrap_or(0), "overdue_invoices": overdue,
        "paid_last_30_days_cents": paid_30d.unwrap_or(0), "past_due_subscriptions": past_due, "trialing": trialing,
        "organizations_by_plan": by_plan.into_iter().map(|(id, name, n)| json!({ "plan": id, "name": name, "organizations": n })).collect::<Vec<_>>(),
    }))
}

async fn organization_detail(
    State(state): State<AppState>,
    _admin: PlatformAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    #[derive(sqlx::FromRow)]
    struct O {
        name: String,
        slug: String,
        status: String,
        suspended_reason: Option<String>,
        account_type: String,
        plan_id: String,
        billing_email: Option<String>,
        created_at: DateTime<Utc>,
        sub_status: Option<String>,
        billing_cycle: Option<String>,
        current_period_end: Option<DateTime<Utc>>,
        pending_plan_id: Option<String>,
    }
    let o: O = sqlx::query_as(
        "SELECT o.name, o.slug::text AS slug, o.status, o.suspended_reason, o.account_type, o.plan_id,
                o.billing_email::text AS billing_email, o.created_at, s.status AS sub_status, s.billing_cycle,
                s.current_period_end, s.pending_plan_id
         FROM organizations o LEFT JOIN subscriptions s ON s.organization_id = o.id WHERE o.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Organization"))?;
    type MemberRow = (Uuid, String, String, String, bool, Option<DateTime<Utc>>);
    let members: Vec<MemberRow> = sqlx::query_as(
        "SELECT u.id, u.email::text, u.full_name, m.role, u.mfa_enabled, u.last_login_at
         FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.organization_id = $1 ORDER BY m.created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let domains: Vec<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT id, domain_name::text, status, mail_hosting_enabled FROM domains WHERE organization_id = $1 ORDER BY domain_name",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted'),
                (SELECT count(*) FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL),
                (SELECT count(*) FROM messages WHERE organization_id = $1 AND created_at > now() - interval '30 days')",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let recent_audit: Vec<(String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT action, actor_label, created_at FROM audit_logs WHERE organization_id = $1 ORDER BY id DESC LIMIT 15",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "id": id, "name": o.name, "slug": o.slug, "status": o.status, "suspended_reason": o.suspended_reason,
        "account_type": o.account_type, "plan": o.plan_id, "billing_email": o.billing_email, "created_at": o.created_at,
        "subscription": { "status": o.sub_status, "billing_cycle": o.billing_cycle, "current_period_end": o.current_period_end, "pending_plan": o.pending_plan_id },
        "members": members.into_iter().map(|(uid, e, n, r, mfa, last)| json!({ "user_id": uid, "email": e, "full_name": n, "role": r, "mfa_enabled": mfa, "last_login_at": last })).collect::<Vec<_>>(),
        "domains": domains.into_iter().map(|(did, n, s, h)| json!({ "id": did, "domain": n, "status": s, "mail_hosting_enabled": h })).collect::<Vec<_>>(),
        "mailboxes": counts.0, "active_api_keys": counts.1, "messages_30d": counts.2,
        "recent_activity": recent_audit.into_iter().map(|(a, who, at)| json!({ "action": a, "actor": who, "at": at })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct UserQuery {
    q: Option<String>,
}

async fn users(
    State(state): State<AppState>,
    _admin: PlatformAdmin,
    ApiQuery(q): ApiQuery<UserQuery>,
) -> ApiResult<Json<Value>> {
    #[derive(sqlx::FromRow)]
    struct U {
        id: Uuid,
        email: String,
        full_name: String,
        status: String,
        is_platform_admin: bool,
        mfa_enabled: bool,
        email_verified_at: Option<DateTime<Utc>>,
        last_login_at: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
        organizations: i64,
    }
    let pattern = empty_as_none(q.q).map(|s| format!("%{}%", s.replace('%', "\\%").replace('_', "\\_")));
    let rows: Vec<U> = sqlx::query_as(
        "SELECT u.id, u.email::text AS email, u.full_name, u.status, u.is_platform_admin, u.mfa_enabled, u.email_verified_at,
                u.last_login_at, u.created_at, (SELECT count(*) FROM memberships m WHERE m.user_id = u.id) AS organizations
         FROM users u WHERE ($1::text IS NULL OR u.email::text ILIKE $1 OR u.full_name ILIKE $1)
         ORDER BY u.created_at DESC LIMIT 300",
    )
    .bind(&pattern)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.into_iter().map(|u| json!({
        "id": u.id, "email": u.email, "full_name": u.full_name, "status": u.status, "is_platform_admin": u.is_platform_admin,
        "mfa_enabled": u.mfa_enabled, "email_verified": u.email_verified_at.is_some(), "last_login_at": u.last_login_at,
        "created_at": u.created_at, "organizations": u.organizations,
    })).collect::<Vec<_>>() })))
}

// ── mail-client settings (public endpoints advertised to email apps) ─────

async fn client_settings(State(state): State<AppState>, _admin: PlatformAdmin) -> ApiResult<Json<Value>> {
    let default = mail_client::platform_default(&state).await?;
    let environment = mail_client::ClientSettings::from_config(&state.cfg);
    #[derive(sqlx::FromRow)]
    struct Override {
        domain_id: Uuid,
        domain: String,
        organization: String,
        imap_host: String,
        imap_port: i32,
        smtp_host: String,
        smtp_port: i32,
        smtp_security: String,
        smtp_alt_port: Option<i32>,
        smtp_alt_security: Option<String>,
        version: i32,
        updated_at: DateTime<Utc>,
    }
    let overrides: Vec<Override> = sqlx::query_as(
        "SELECT p.domain_id AS domain_id, d.domain_name::text AS domain, o.name AS organization, p.imap_host, p.imap_port,
                p.smtp_host, p.smtp_port, p.smtp_security, p.smtp_alt_port, p.smtp_alt_security, p.version, p.updated_at
         FROM mail_client_profiles p JOIN domains d ON d.id = p.domain_id JOIN organizations o ON o.id = d.organization_id
         ORDER BY d.domain_name",
    )
    .fetch_all(&state.db)
    .await?;
    type HistoryRow = (i64, Option<String>, i32, String, Value, Option<String>, Option<String>, DateTime<Utc>);
    let history: Vec<HistoryRow> = sqlx::query_as(
        "SELECT id, domain_name, version, action, settings, change_note, changed_by_label, created_at
         FROM mail_client_profile_history ORDER BY id DESC LIMIT 50",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "default": default.to_json(None),
        "environment": environment.to_json(None),
        "overrides": overrides.into_iter().map(|o| json!({
            "domain_id": o.domain_id, "domain": o.domain, "organization": o.organization,
            "imap_host": o.imap_host, "imap_port": o.imap_port, "smtp_host": o.smtp_host, "smtp_port": o.smtp_port,
            "smtp_security": o.smtp_security, "smtp_alt_port": o.smtp_alt_port, "smtp_alt_security": o.smtp_alt_security,
            "version": o.version, "updated_at": o.updated_at,
        })).collect::<Vec<_>>(),
        "history": history.into_iter().map(|(hid, domain, v, action, settings, note, by, at)| json!({
            "id": hid, "scope": domain.unwrap_or_else(|| "platform default".into()), "version": v, "action": action,
            "settings": settings, "change_note": note, "changed_by": by, "at": at,
        })).collect::<Vec<_>>(),
    })))
}

/// Create or update the platform-wide default profile (one row, enforced by
/// a partial unique index on `domain_id IS NULL`).
async fn put_default_client_settings(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    ApiJson(input): ApiJson<ProfileInput>,
) -> ApiResult<Json<Value>> {
    let v = input.validate()?;
    let mut tx = state.db.begin().await?;
    let (profile_id, version): (Uuid, i32) = sqlx::query_as(
        "INSERT INTO mail_client_profiles (domain_id, imap_host, imap_port, imap_security, smtp_host, smtp_port, smtp_security,
                                           smtp_alt_port, smtp_alt_security, updated_by)
         VALUES (NULL, $1, $2, 'tls', $3, $4, $5, $6, $7, $8)
         ON CONFLICT ((true)) WHERE domain_id IS NULL DO UPDATE SET
             imap_host = EXCLUDED.imap_host, imap_port = EXCLUDED.imap_port, smtp_host = EXCLUDED.smtp_host,
             smtp_port = EXCLUDED.smtp_port, smtp_security = EXCLUDED.smtp_security, smtp_alt_port = EXCLUDED.smtp_alt_port,
             smtp_alt_security = EXCLUDED.smtp_alt_security, updated_by = EXCLUDED.updated_by,
             version = mail_client_profiles.version + 1, updated_at = now()
         RETURNING id, version",
    )
    .bind(&v.imap_host)
    .bind(v.imap_port)
    .bind(&v.smtp_host)
    .bind(v.smtp_port)
    .bind(v.smtp_security)
    .bind(v.smtp_alt_port)
    .bind(v.smtp_alt_security)
    .bind(admin.0.user_id)
    .fetch_one(&mut *tx)
    .await?;
    let settings = json!({
        "imap_host": v.imap_host, "imap_port": v.imap_port, "imap_security": "tls",
        "smtp_host": v.smtp_host, "smtp_port": v.smtp_port, "smtp_security": v.smtp_security,
        "smtp_alt_port": v.smtp_alt_port, "smtp_alt_security": v.smtp_alt_security,
    });
    sqlx::query(
        "INSERT INTO mail_client_profile_history (profile_id, domain_id, domain_name, version, action, settings, change_note, changed_by, changed_by_label)
         VALUES ($1, NULL, NULL, $2, CASE WHEN $2 = 1 THEN 'created' ELSE 'updated' END, $3, $4, $5, $6)",
    )
    .bind(profile_id)
    .bind(version)
    .bind(&settings)
    .bind(&v.change_note)
    .bind(admin.0.user_id)
    .bind(&admin.0.email)
    .execute(&mut *tx)
    .await?;
    entry(&admin, &client, None, "mail_client_settings.updated")
        .target("mail_client_profile", profile_id)
        .details(json!({ "scope": "platform", "version": version, "settings": settings }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": profile_id, "version": version, "settings": settings })))
}

async fn put_domain_client_settings(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(domain_id): Path<Uuid>,
    ApiJson(input): ApiJson<ProfileInput>,
) -> ApiResult<Json<Value>> {
    let (name, org): (String, Uuid) =
        sqlx::query_as("SELECT domain_name::text, organization_id FROM domains WHERE id = $1 AND status = 'verified'")
            .bind(domain_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::not_found("Verified domain"))?;
    // Domain overrides upsert on their own unique key.
    let v = input.validate()?;
    let mut tx = state.db.begin().await?;
    let (profile_id, version): (Uuid, i32) = sqlx::query_as(
        "INSERT INTO mail_client_profiles (domain_id, imap_host, imap_port, imap_security, smtp_host, smtp_port, smtp_security,
                                           smtp_alt_port, smtp_alt_security, updated_by)
         VALUES ($1, $2, $3, 'tls', $4, $5, $6, $7, $8, $9)
         ON CONFLICT (domain_id) DO UPDATE SET
             imap_host = EXCLUDED.imap_host, imap_port = EXCLUDED.imap_port, smtp_host = EXCLUDED.smtp_host,
             smtp_port = EXCLUDED.smtp_port, smtp_security = EXCLUDED.smtp_security, smtp_alt_port = EXCLUDED.smtp_alt_port,
             smtp_alt_security = EXCLUDED.smtp_alt_security, updated_by = EXCLUDED.updated_by,
             version = mail_client_profiles.version + 1, updated_at = now()
         RETURNING id, version",
    )
    .bind(domain_id)
    .bind(&v.imap_host)
    .bind(v.imap_port)
    .bind(&v.smtp_host)
    .bind(v.smtp_port)
    .bind(v.smtp_security)
    .bind(v.smtp_alt_port)
    .bind(v.smtp_alt_security)
    .bind(admin.0.user_id)
    .fetch_one(&mut *tx)
    .await?;
    let settings = json!({
        "imap_host": v.imap_host, "imap_port": v.imap_port, "imap_security": "tls",
        "smtp_host": v.smtp_host, "smtp_port": v.smtp_port, "smtp_security": v.smtp_security,
        "smtp_alt_port": v.smtp_alt_port, "smtp_alt_security": v.smtp_alt_security,
    });
    sqlx::query(
        "INSERT INTO mail_client_profile_history (profile_id, domain_id, domain_name, version, action, settings, change_note, changed_by, changed_by_label)
         VALUES ($1, $2, $3, $4, CASE WHEN $4 = 1 THEN 'created' ELSE 'updated' END, $5, $6, $7, $8)",
    )
    .bind(profile_id)
    .bind(domain_id)
    .bind(&name)
    .bind(version)
    .bind(&settings)
    .bind(&v.change_note)
    .bind(admin.0.user_id)
    .bind(&admin.0.email)
    .execute(&mut *tx)
    .await?;
    entry(&admin, &client, Some(org), "mail_client_settings.updated")
        .target("mail_client_profile", profile_id)
        .details(json!({ "scope": name, "version": version, "settings": settings }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": profile_id, "version": version, "domain": name, "settings": settings })))
}

async fn delete_domain_client_settings(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(domain_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, i32, String, Uuid)> = sqlx::query_as(
        "DELETE FROM mail_client_profiles p USING domains d WHERE p.domain_id = $1 AND d.id = p.domain_id
         RETURNING p.id, p.version, d.domain_name::text, d.organization_id",
    )
    .bind(domain_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (profile_id, version, name, org) = row.ok_or_else(|| AppError::not_found("Domain override"))?;
    sqlx::query(
        "INSERT INTO mail_client_profile_history (profile_id, domain_id, domain_name, version, action, settings, changed_by, changed_by_label)
         VALUES (NULL, $1, $2, $3, 'removed', '{}'::jsonb, $4, $5)",
    )
    .bind(domain_id)
    .bind(&name)
    .bind(version + 1)
    .bind(admin.0.user_id)
    .bind(&admin.0.email)
    .execute(&mut *tx)
    .await?;
    entry(&admin, &client, Some(org), "mail_client_settings.override_removed")
        .target("mail_client_profile", profile_id)
        .details(json!({ "scope": name }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "domain": name, "removed": true })))
}

async fn onboarding_metrics(State(state): State<AppState>, _admin: PlatformAdmin) -> ApiResult<Json<Value>> {
    let funnel: Vec<(String, i64)> = sqlx::query_as(
        "SELECT event, count(DISTINCT mailbox_id) FROM mailbox_setup_events WHERE created_at > now() - interval '30 days' GROUP BY event",
    )
    .fetch_all(&state.db)
    .await?;
    let by_client: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT client, count(DISTINCT mailbox_id) FROM mailbox_setup_events
         WHERE event = 'client_selected' AND created_at > now() - interval '30 days' GROUP BY client ORDER BY 2 DESC",
    )
    .fetch_all(&state.db)
    .await?;
    let verified: Vec<(String, i64)> = sqlx::query_as(
        "SELECT direction, count(DISTINCT mailbox_id) FROM mailbox_connection_checks
         WHERE status = 'verified' AND verified_at > now() - interval '30 days' GROUP BY direction",
    )
    .fetch_all(&state.db)
    .await?;
    let readiness: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT detail, count(*) FROM mailbox_setup_events WHERE event = 'readiness_checked' AND created_at > now() - interval '30 days' GROUP BY detail",
    )
    .fetch_all(&state.db)
    .await?;
    let help: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT detail, count(*) FROM mailbox_setup_events WHERE event IN ('help_viewed', 'connection_failed')
         AND created_at > now() - interval '30 days' GROUP BY detail ORDER BY 2 DESC LIMIT 15",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "window_days": 30,
        "funnel": funnel.into_iter().map(|(e, n)| json!({ "event": e, "mailboxes": n })).collect::<Vec<_>>(),
        "by_client": by_client.into_iter().map(|(c, n)| json!({ "client": c, "mailboxes": n })).collect::<Vec<_>>(),
        "verified": verified.into_iter().map(|(d, n)| json!({ "direction": d, "mailboxes": n })).collect::<Vec<_>>(),
        "readiness": readiness.into_iter().map(|(d, n)| json!({ "result": d, "checks": n })).collect::<Vec<_>>(),
        "help_topics": help.into_iter().map(|(d, n)| json!({ "topic": d, "count": n })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct OrgQuery {
    q: Option<String>,
    status: Option<String>,
}

#[derive(sqlx::FromRow)]
struct OrgRow {
    id: Uuid,
    name: String,
    slug: String,
    status: String,
    suspended_reason: Option<String>,
    plan_id: String,
    created_at: DateTime<Utc>,
    members: i64,
    domains: i64,
    mailboxes: i64,
    messages_7d: i64,
    bounced_7d: i64,
}

async fn organizations(
    State(state): State<AppState>,
    _admin: PlatformAdmin,
    ApiQuery(q): ApiQuery<OrgQuery>,
) -> ApiResult<Json<Value>> {
    let rows: Vec<OrgRow> = sqlx::query_as(
        "SELECT o.id, o.name, o.slug::text AS slug, o.status, o.suspended_reason, o.plan_id, o.created_at,
                (SELECT count(*) FROM memberships m WHERE m.organization_id = o.id) AS members,
                (SELECT count(*) FROM domains d WHERE d.organization_id = o.id) AS domains,
                (SELECT count(*) FROM mailboxes x WHERE x.organization_id = o.id AND x.status <> 'deleted') AS mailboxes,
                (SELECT count(*) FROM messages g WHERE g.organization_id = o.id AND g.created_at > now() - interval '7 days') AS messages_7d,
                (SELECT count(*) FROM messages g WHERE g.organization_id = o.id AND g.created_at > now() - interval '7 days' AND g.status = 'bounced') AS bounced_7d
         FROM organizations o
         WHERE ($1::text IS NULL OR o.name ILIKE '%' || $1 || '%' OR o.slug::text ILIKE '%' || $1 || '%')
           AND ($2::text IS NULL OR o.status = $2)
         ORDER BY o.created_at DESC LIMIT 500",
    )
    .bind(empty_as_none(q.q))
    .bind(empty_as_none(q.status))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "data": rows.into_iter().map(|o| json!({
            "id": o.id, "name": o.name, "slug": o.slug, "status": o.status, "suspended_reason": o.suspended_reason,
            "plan": o.plan_id, "created_at": o.created_at, "members": o.members, "domains": o.domains,
            "mailboxes": o.mailboxes, "messages_7d": o.messages_7d, "bounced_7d": o.bounced_7d,
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct UpdateOrgBody {
    plan: String,
}

async fn update_organization(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateOrgBody>,
) -> ApiResult<Json<Value>> {
    let audience: Option<String> = sqlx::query_scalar("SELECT audience FROM plans WHERE id = $1")
        .bind(&body.plan)
        .fetch_optional(&state.db)
        .await?;
    let audience = audience.ok_or_else(|| AppError::validation("Unknown plan"))?;
    let mut tx = state.db.begin().await?;
    let account_type: Option<String> =
        sqlx::query_scalar("SELECT account_type FROM organizations WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let account_type = account_type.ok_or_else(|| AppError::not_found("Organization"))?;
    if account_type != audience {
        return Err(AppError::validation(format!(
            "That plan is for {audience} accounts; this organization is an {account_type} account"
        )));
    }
    sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&body.plan)
        .execute(&mut *tx)
        .await?;
    // Operator assignment (e.g. Enterprise contracts): a fresh period, no
    // automatic invoice; contract invoicing is handled by the operator.
    sqlx::query(
        "INSERT INTO subscriptions (organization_id, plan_id) VALUES ($1, $2)
         ON CONFLICT (organization_id) DO UPDATE SET plan_id = EXCLUDED.plan_id, status = 'active', pending_plan_id = NULL,
             cancel_at_period_end = false, current_period_start = now(), current_period_end = now() + interval '1 month', updated_at = now()",
    )
    .bind(id)
    .bind(&body.plan)
    .execute(&mut *tx)
    .await?;
    entry(&admin, &client, Some(id), "organization.plan_changed")
        .target("organization", id)
        .details(json!({ "plan": body.plan }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "plan": body.plan })))
}

#[derive(Deserialize)]
struct SuspendBody {
    reason: String,
}

async fn suspend(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<SuspendBody>,
) -> ApiResult<Json<Value>> {
    let reason = validation::non_empty("reason", &body.reason, 500)?;
    let mut tx = state.db.begin().await?;
    let n = sqlx::query("UPDATE organizations SET status = 'suspended', suspended_reason = $2, suspended_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&reason)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Organization"));
    }
    // Stop queued mail immediately rather than waiting for workers to see it.
    let halted: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE messages SET status = 'failed', last_error = 'Organization suspended by platform operator', updated_at = now()
         WHERE organization_id = $1 AND status IN ('queued', 'deferred') RETURNING id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query("UPDATE outbox_jobs SET status = 'dead', last_error = 'organization suspended', updated_at = now() WHERE organization_id = $1 AND kind = 'send_message' AND status = 'pending'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // Webmail sessions end now; console users keep read-only access.
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE mailbox_id IN (SELECT id FROM mailboxes WHERE organization_id = $1) AND revoked_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    entry(&admin, &client, Some(id), "organization.suspended")
        .target("organization", id)
        .details(json!({ "reason": reason, "messages_halted": halted.len() }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::warn!(organization = %id, admin = %admin.0.email, "organization suspended");
    let addresses = crate::services::doveadm::tenant_addresses(&state, id).await;
    crate::services::doveadm::revoke_access(&state, addresses).await;
    Ok(Json(json!({ "id": id, "status": "suspended", "messages_halted": halted.len() })))
}

async fn unsuspend(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.db.begin().await?;
    let n = sqlx::query("UPDATE organizations SET status = 'active', suspended_reason = NULL, suspended_at = NULL, updated_at = now() WHERE id = $1 AND status = 'suspended'")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Suspended organization"));
    }
    entry(&admin, &client, Some(id), "organization.unsuspended").target("organization", id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "active" })))
}

#[derive(sqlx::FromRow)]
struct DeadJob {
    id: i64,
    kind: String,
    organization_id: Option<Uuid>,
    message_id: Option<Uuid>,
    attempts: i32,
    last_error: Option<String>,
    updated_at: DateTime<Utc>,
}

async fn dead_jobs(State(state): State<AppState>, _admin: PlatformAdmin) -> ApiResult<Json<Value>> {
    let rows: Vec<DeadJob> = sqlx::query_as(
        "SELECT id, kind, organization_id, message_id, attempts, last_error, updated_at FROM outbox_jobs WHERE status = 'dead' ORDER BY updated_at DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "data": rows.into_iter().map(|j| json!({
            "id": j.id, "kind": j.kind, "organization_id": j.organization_id, "message_id": j.message_id,
            "attempts": j.attempts, "last_error": j.last_error, "updated_at": j.updated_at,
        })).collect::<Vec<_>>()
    })))
}

async fn requeue_job(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.db.begin().await?;
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "UPDATE outbox_jobs SET status = 'pending', attempts = 0, run_at = now(), last_error = NULL, updated_at = now()
         WHERE id = $1 AND status = 'dead' RETURNING organization_id, message_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (org, message) = row.ok_or_else(|| AppError::not_found("Dead job"))?;
    if let Some(m) = message {
        sqlx::query("UPDATE messages SET status = 'queued', updated_at = now() WHERE id = $1 AND status = 'failed'")
            .bind(m)
            .execute(&mut *tx)
            .await?;
    }
    entry(&admin, &client, org, "job.requeued").target("outbox_job", id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "pending" })))
}

async fn disable_user(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    if id == admin.0.user_id {
        return Err(AppError::forbidden("You cannot disable your own account"));
    }
    let mut tx = state.db.begin().await?;
    let email: Option<String> = sqlx::query_scalar(
        "UPDATE users SET status = 'disabled', updated_at = now() WHERE id = $1 RETURNING email::text",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let email = email.ok_or_else(|| AppError::not_found("User"))?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    entry(&admin, &client, None, "user.disabled")
        .target("user", id)
        .details(json!({ "email": email }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "disabled" })))
}

async fn enable_user(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.db.begin().await?;
    let n = sqlx::query("UPDATE users SET status = 'active', failed_login_count = 0, locked_until = NULL, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("User"));
    }
    entry(&admin, &client, None, "user.enabled").target("user", id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "active" })))
}

/// Security posture and signals for operators: authentication failures,
/// lockouts, privileged changes and accounts missing two-factor.
async fn security_overview(State(state): State<AppState>, _admin: PlatformAdmin) -> ApiResult<Json<Value>> {
    #[derive(sqlx::FromRow)]
    struct Counts {
        failed_logins_24h: i64,
        failed_mfa_24h: i64,
        password_resets_24h: i64,
        mfa_changes_7d: i64,
        role_changes_7d: i64,
        keys_created_24h: i64,
        locked_users: i64,
        locked_mailboxes: i64,
        admins_total: i64,
        admins_without_mfa: i64,
        owners_without_mfa: i64,
        unverified_users: i64,
    }
    let c: Counts = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM audit_logs WHERE action = 'user.login_failed' AND created_at > now() - interval '24 hours') AS failed_logins_24h,
           (SELECT count(*) FROM audit_logs WHERE action = 'user.mfa_failed' AND created_at > now() - interval '24 hours') AS failed_mfa_24h,
           (SELECT count(*) FROM audit_logs WHERE action IN ('user.password_reset', 'mailbox.password_reset') AND created_at > now() - interval '24 hours') AS password_resets_24h,
           (SELECT count(*) FROM audit_logs WHERE action IN ('user.mfa_enabled', 'user.mfa_disabled') AND created_at > now() - interval '7 days') AS mfa_changes_7d,
           (SELECT count(*) FROM audit_logs WHERE action = 'member.role_changed' AND created_at > now() - interval '7 days') AS role_changes_7d,
           (SELECT count(*) FROM audit_logs WHERE action = 'api_key.created' AND created_at > now() - interval '24 hours') AS keys_created_24h,
           (SELECT count(*) FROM users WHERE locked_until > now()) AS locked_users,
           (SELECT count(*) FROM mailboxes WHERE locked_until > now()) AS locked_mailboxes,
           (SELECT count(*) FROM users WHERE is_platform_admin) AS admins_total,
           (SELECT count(*) FROM users WHERE is_platform_admin AND NOT mfa_enabled) AS admins_without_mfa,
           (SELECT count(DISTINCT u.id) FROM users u JOIN memberships m ON m.user_id = u.id
             WHERE m.role = 'owner' AND NOT u.mfa_enabled) AS owners_without_mfa,
           (SELECT count(*) FROM users WHERE email_verified_at IS NULL AND phone_verified_at IS NULL) AS unverified_users",
    )
    .fetch_one(&state.db)
    .await?;

    let top_failures: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT actor_label, count(*) FROM audit_logs
         WHERE action IN ('user.login_failed', 'user.mfa_failed') AND created_at > now() - interval '24 hours'
         GROUP BY actor_label ORDER BY count(*) DESC LIMIT 10",
    )
    .fetch_all(&state.db)
    .await?;
    let failing_ips: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT host(ip), count(*) FROM audit_logs
         WHERE action IN ('user.login_failed', 'user.mfa_failed') AND created_at > now() - interval '24 hours' AND ip IS NOT NULL
         GROUP BY ip ORDER BY count(*) DESC LIMIT 10",
    )
    .fetch_all(&state.db)
    .await?;
    type OperatorRow = (Uuid, String, String, bool, Option<DateTime<Utc>>);
    type ActivityRow = (String, Option<String>, Option<String>, DateTime<Utc>);
    let admins: Vec<OperatorRow> = sqlx::query_as(
        "SELECT id, email::text, full_name, mfa_enabled, last_login_at FROM users WHERE is_platform_admin ORDER BY email",
    )
    .fetch_all(&state.db)
    .await?;
    let privileged: Vec<ActivityRow> = sqlx::query_as(
        "SELECT action, actor_label, host(ip), created_at FROM audit_logs
         WHERE actor_type = 'platform_admin' OR action IN ('member.role_changed', 'user.mfa_disabled', 'user.google_linked',
               'organization.suspended', 'organization.unsuspended', 'user.disabled', 'invoice.voided')
         ORDER BY created_at DESC LIMIT 25",
    )
    .fetch_all(&state.db)
    .await?;

    let mut findings = vec![];
    if c.admins_without_mfa > 0 {
        findings.push(json!({ "level": "fail", "message": format!("{} platform operator(s) without two-factor authentication. Operators are blocked in production until they enrol.", c.admins_without_mfa) }));
    }
    if c.failed_logins_24h > 50 {
        findings.push(json!({ "level": "warn", "message": format!("{} failed sign-ins in 24 hours: review the source addresses below.", c.failed_logins_24h) }));
    }
    if c.owners_without_mfa > 0 {
        findings.push(json!({ "level": "warn", "message": format!("{} organization owner(s) have not enabled two-factor authentication.", c.owners_without_mfa) }));
    }
    if !state.cfg.password_breach_check {
        findings.push(json!({ "level": "warn", "message": "Breached-password screening is disabled (PASSWORD_BREACH_CHECK=false)." }));
    }
    if findings.is_empty() {
        findings.push(json!({ "level": "pass", "message": "No security issues need attention right now." }));
    }

    Ok(Json(json!({
        "counts": {
            "failed_logins_24h": c.failed_logins_24h, "failed_mfa_24h": c.failed_mfa_24h, "password_resets_24h": c.password_resets_24h,
            "mfa_changes_7d": c.mfa_changes_7d, "role_changes_7d": c.role_changes_7d, "keys_created_24h": c.keys_created_24h,
            "locked_users": c.locked_users, "locked_mailboxes": c.locked_mailboxes, "admins_total": c.admins_total,
            "admins_without_mfa": c.admins_without_mfa, "owners_without_mfa": c.owners_without_mfa, "unverified_users": c.unverified_users,
        },
        "controls": {
            "breached_password_screening": state.cfg.password_breach_check,
            "operator_mfa_enforced": state.cfg.is_production(),
            "secure_cookies": state.cfg.is_production(),
            "sms_verification": state.cfg.integrations.sms.is_some(),
            "google_sign_in": state.cfg.integrations.google.is_some(),
            "mpesa_payments": state.cfg.integrations.pesatide.is_some(),
            "content_retention_days": state.cfg.content_retention_days,
        },
        "findings": findings,
        "top_failing_accounts": top_failures.into_iter().map(|(a, n)| json!({ "account": a, "count": n })).collect::<Vec<_>>(),
        "top_failing_ips": failing_ips.into_iter().map(|(ip, n)| json!({ "ip": ip, "count": n })).collect::<Vec<_>>(),
        "operators": admins.into_iter().map(|(id, email, name, mfa, last)| json!({ "id": id, "email": email, "full_name": name, "mfa_enabled": mfa, "last_login_at": last })).collect::<Vec<_>>(),
        "privileged_activity": privileged.into_iter().map(|(action, actor, ip, at)| json!({ "action": action, "actor": actor, "ip": ip, "at": at })).collect::<Vec<_>>(),
    })))
}
