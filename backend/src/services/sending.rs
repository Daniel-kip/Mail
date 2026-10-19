//! The send pipeline shared by the public API, console test sends and
//! webmail.
//!
//! Invariant (guide 01/02): a message row and its outbox job are inserted in
//! the same transaction, before the API acknowledges the request. If the
//! process dies after commit, the worker still delivers; if it dies before,
//! nothing was promised.

use lettre::Message;
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    error::{ApiResult, AppError},
    mail::{bounce, mime},
    services::{dkim, events},
    state::AppState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyMode {
    NoReply,
    ReplyToMailbox,
    ReplyToUser,
}

impl ReplyMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "no_reply" => Some(Self::NoReply),
            "reply_to_mailbox" => Some(Self::ReplyToMailbox),
            "reply_to_user" => Some(Self::ReplyToUser),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoReply => "no_reply",
            Self::ReplyToMailbox => "reply_to_mailbox",
            Self::ReplyToUser => "reply_to_user",
        }
    }
}

/// Signing material for one verified sending domain.
pub struct SendingDomain {
    pub id: Uuid,
    pub name: String,
    selector: String,
    sealed_key: Vec<u8>,
}

pub async fn sending_domain(state: &AppState, org_id: Uuid, domain: &str) -> ApiResult<SendingDomain> {
    let row: Option<(Uuid, String, String, Vec<u8>, String)> = sqlx::query_as(
        "SELECT id, domain_name::text, dkim_selector, dkim_private_key_enc, status
         FROM domains WHERE organization_id = $1 AND domain_name = $2",
    )
    .bind(org_id)
    .bind(domain)
    .fetch_optional(&state.db)
    .await?;
    match row {
        Some((id, name, selector, sealed_key, status)) if status == "verified" => {
            Ok(SendingDomain { id, name, selector, sealed_key })
        }
        Some(_) => Err(AppError::forbidden(format!(
            "The domain {domain} is not verified yet. Complete DNS verification before sending."
        ))
        .with_details(json!({ "reason": "domain_not_verified", "domain": domain }))),
        None => Err(AppError::forbidden(format!("The domain {domain} is not registered to this organization."))
            .with_details(json!({ "reason": "domain_not_registered", "domain": domain }))),
    }
}

/// DKIM-sign and serialize a built message.
pub fn sign(state: &AppState, domain: &SendingDomain, mut message: Message) -> ApiResult<Vec<u8>> {
    let cfg = dkim::signing_config(&state.secrets, &domain.sealed_key, &domain.selector, &domain.name)?;
    message.sign(&cfg);
    Ok(message.formatted())
}

pub fn new_ids(state: &AppState) -> (Uuid, String) {
    let id = Uuid::now_v7();
    (id, mime::new_message_id(id, &state.cfg.message_id_domain))
}

pub fn envelope_from(state: &AppState, message_id: Uuid) -> String {
    bounce::verp_address(&bounce::verp_key(&state.cfg.data_encryption_key), message_id, &state.cfg.return_path_domain)
}

/// Reserve `n` sends against the tenant's daily and monthly plan limits.
/// Serialized per tenant with a transaction-scoped advisory lock so
/// concurrent requests cannot overshoot.
pub async fn reserve_quota(tx: &mut PgConnection, org_id: Uuid, n: i32) -> ApiResult<()> {
    if n == 0 {
        return Ok(());
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 7))")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;
    let (daily_limit, monthly_limit, today, month): (i32, i32, i64, i64) = sqlx::query_as(
        "SELECT p.daily_send_limit, p.monthly_send_limit,
                COALESCE((SELECT sum(messages_queued) FROM usage_daily
                          WHERE organization_id = $1 AND day = (now() AT TIME ZONE 'UTC')::date), 0)::bigint,
                COALESCE((SELECT sum(messages_queued) FROM usage_daily
                          WHERE organization_id = $1 AND day >= date_trunc('month', now() AT TIME ZONE 'UTC')::date), 0)::bigint
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(org_id)
    .fetch_one(&mut *tx)
    .await?;
    if today + n as i64 > daily_limit as i64 {
        return Err(AppError::quota_exceeded(format!(
            "Daily sending limit reached ({today}/{daily_limit}). The limit resets at 00:00 UTC."
        ))
        .with_details(json!({ "limit": "daily", "used": today, "max": daily_limit, "requested": n })));
    }
    if month + n as i64 > monthly_limit as i64 {
        return Err(AppError::quota_exceeded(format!("Monthly sending limit reached ({month}/{monthly_limit})."))
            .with_details(json!({ "limit": "monthly", "used": month, "max": monthly_limit, "requested": n })));
    }
    sqlx::query(
        "INSERT INTO usage_daily (organization_id, day, messages_queued) VALUES ($1, (now() AT TIME ZONE 'UTC')::date, $2)
         ON CONFLICT (organization_id, day) DO UPDATE SET messages_queued = usage_daily.messages_queued + EXCLUDED.messages_queued",
    )
    .bind(org_id)
    .bind(n)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Recipients (lowercased) that are on the tenant's suppression list.
pub async fn suppressed_among(tx: &mut PgConnection, org_id: Uuid, recipients: &[String]) -> ApiResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT email::text FROM suppressions WHERE organization_id = $1 AND email = ANY($2::citext[])",
    )
    .bind(org_id)
    .bind(recipients)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|e: String| e.to_lowercase())
    .collect())
}

pub struct NewMessage<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub batch_id: Uuid,
    pub source: &'static str,
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub mailbox_id: Option<Uuid>,
    pub domain_id: Uuid,
    pub from_address: &'a str,
    pub from_name: Option<&'a str>,
    pub envelope_from: &'a str,
    pub to_display: &'a str,
    pub recipients: &'a [String],
    pub reply_to: Option<&'a str>,
    pub reply_mode: ReplyMode,
    pub subject: &'a str,
    pub template: Option<(Uuid, i32)>,
    pub sandbox: bool,
    pub smtp_message_id: &'a str,
    pub tags: &'a [String],
    pub metadata: &'a Value,
    pub idempotency_key: Option<&'a str>,
    pub raw: &'a [u8],
    pub html: Option<&'a str>,
    pub text: Option<&'a str>,
    pub suppressed: bool,
    /// Deliver no earlier than this time (status `scheduled` until then).
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub list_unsubscribe: bool,
}

/// Insert the message, its content, its first event and (if it will be
/// sent) its outbox job. Returns the initial status.
pub async fn insert(tx: &mut PgConnection, m: &NewMessage<'_>) -> ApiResult<&'static str> {
    let status = if m.suppressed {
        "suppressed"
    } else if m.sandbox {
        "sandboxed"
    } else if m.scheduled_at.is_some() {
        "scheduled"
    } else {
        "queued"
    };
    sqlx::query(
        "INSERT INTO messages (id, organization_id, batch_id, source, api_key_id, created_by_user_id, mailbox_id, domain_id,
                               from_address, from_name, envelope_from, to_address, envelope_recipients, reply_to, reply_mode,
                               subject, template_id, template_version, status, sandbox, smtp_message_id, tags, metadata,
                               idempotency_key, size_bytes, scheduled_at, list_unsubscribe)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27)",
    )
    .bind(m.id)
    .bind(m.org_id)
    .bind(m.batch_id)
    .bind(m.source)
    .bind(m.api_key_id)
    .bind(m.user_id)
    .bind(m.mailbox_id)
    .bind(m.domain_id)
    .bind(m.from_address)
    .bind(m.from_name)
    .bind(m.envelope_from)
    .bind(m.to_display)
    .bind(m.recipients)
    .bind(m.reply_to)
    .bind(m.reply_mode.as_str())
    .bind(m.subject)
    .bind(m.template.map(|t| t.0))
    .bind(m.template.map(|t| t.1))
    .bind(status)
    .bind(m.sandbox)
    .bind(m.smtp_message_id)
    .bind(m.tags)
    .bind(m.metadata)
    .bind(m.idempotency_key)
    .bind(m.raw.len() as i32)
    .bind(if m.suppressed || m.sandbox { None } else { m.scheduled_at })
    .bind(m.list_unsubscribe)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO message_contents (message_id, raw_mime, html_body, text_body) VALUES ($1, $2, $3, $4)")
        .bind(m.id)
        .bind(if m.suppressed { None } else { Some(m.raw) })
        .bind(m.html)
        .bind(m.text)
        .execute(&mut *tx)
        .await?;

    match status {
        "scheduled" => {
            let at = m.scheduled_at.expect("scheduled status implies a time");
            events::message_event(
                tx,
                m.org_id,
                m.id,
                "scheduled",
                json!({ "send_at": at, "recipients": m.recipients.len() }),
            )
            .await?;
            sqlx::query("INSERT INTO outbox_jobs (kind, organization_id, message_id, run_at) VALUES ('send_message', $1, $2, $3)")
                .bind(m.org_id)
                .bind(m.id)
                .bind(at)
                .execute(&mut *tx)
                .await?;
        }
        "queued" => {
            events::message_event(tx, m.org_id, m.id, "queued", json!({ "recipients": m.recipients.len() })).await?;
            sqlx::query("INSERT INTO outbox_jobs (kind, organization_id, message_id) VALUES ('send_message', $1, $2)")
                .bind(m.org_id)
                .bind(m.id)
                .execute(&mut *tx)
                .await?;
        }
        "sandboxed" => {
            events::message_event(tx, m.org_id, m.id, "queued", json!({ "sandbox": true })).await?;
            events::message_event(
                tx,
                m.org_id,
                m.id,
                "sandboxed",
                json!({ "reason": "Sent with a test API key: processed and stored, not delivered." }),
            )
            .await?;
        }
        _ => {
            events::message_event(
                tx,
                m.org_id,
                m.id,
                "suppressed",
                json!({ "reason": "Recipient is on the suppression list" }),
            )
            .await?;
        }
    }
    Ok(status)
}
