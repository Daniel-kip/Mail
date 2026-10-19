//! Delivery of queued messages and system mail to the relay.

use serde_json::json;
use uuid::Uuid;

use super::queue::{self, Job};
use crate::{
    mail::{
        mime::{self, Addr, OutgoingEmail},
        smtp::{SmtpRelay, SubmitError},
    },
    services::{dkim, events},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct Outgoing {
    organization_id: Uuid,
    status: String,
    envelope_from: String,
    envelope_recipients: Vec<String>,
    raw_mime: Option<Vec<u8>>,
    org_status: String,
}

/// Process one `send_message` job. Errors returned here are infrastructure
/// errors (database); delivery outcomes are recorded on the message.
pub async fn send_message(state: &AppState, relay: &SmtpRelay, job: &Job) -> anyhow::Result<()> {
    let message_id = job.message_id.ok_or_else(|| anyhow::anyhow!("send_message job {} has no message", job.id))?;
    let m: Option<Outgoing> = sqlx::query_as(
        "SELECT m.organization_id, m.status, m.envelope_from, m.envelope_recipients, c.raw_mime, o.status AS org_status
         FROM messages m
         JOIN organizations o ON o.id = m.organization_id
         LEFT JOIN message_contents c ON c.message_id = m.id
         WHERE m.id = $1",
    )
    .bind(message_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(m) = m else {
        queue::complete(&state.db, job).await?;
        return Ok(());
    };
    // Idempotent: a job reclaimed after a crash must not resend a message
    // that already reached a terminal or accepted state.
    if !matches!(m.status.as_str(), "scheduled" | "queued" | "processing" | "deferred") {
        queue::complete(&state.db, job).await?;
        return Ok(());
    }
    let org = m.organization_id;

    if m.org_status != "active" {
        fail(state, org, message_id, None, "Organization is suspended; sending stopped").await?;
        queue::complete(&state.db, job).await?;
        return Ok(());
    }
    let Some(raw) = m.raw_mime else {
        fail(state, org, message_id, None, "Message content is no longer available").await?;
        queue::complete(&state.db, job).await?;
        return Ok(());
    };

    sqlx::query("UPDATE messages SET status = 'processing', attempts = attempts + 1, updated_at = now() WHERE id = $1")
        .bind(message_id)
        .execute(&state.db)
        .await?;

    match relay.submit(&m.envelope_from, &m.envelope_recipients, &raw).await {
        Ok(accepted) => {
            let mut tx = state.db.begin().await?;
            sqlx::query(
                "UPDATE messages SET status = 'accepted_by_mta', accepted_at = now(), last_error = NULL,
                        last_smtp_code = 250, updated_at = now() WHERE id = $1",
            )
            .bind(message_id)
            .execute(&mut *tx)
            .await?;
            events::message_event(
                &mut tx,
                org,
                message_id,
                "accepted",
                json!({ "mta_queue_id": accepted.queue_id, "response": queue::truncate(&accepted.response, 300), "attempt": job.attempts }),
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            tx.commit().await?;
            queue::complete(&state.db, job).await?;
            tracing::info!(%message_id, queue_id = ?accepted.queue_id, "message accepted by relay");
        }
        Err(SubmitError::Permanent { code, message }) => {
            tracing::warn!(%message_id, ?code, error = %message, "relay rejected message permanently");
            fail(state, org, message_id, code, &message).await?;
            queue::complete(&state.db, job).await?;
        }
        Err(e @ SubmitError::Transient { .. }) => {
            let delay = queue::backoff(job.attempts, 30, 3600);
            let is_dead = queue::retry_or_dead(&state.db, job, delay, e.message()).await?;
            if is_dead {
                fail(
                    state,
                    org,
                    message_id,
                    e.code(),
                    &format!("Gave up after {} attempts: {}", job.attempts, e.message()),
                )
                .await?;
            } else {
                let mut tx = state.db.begin().await?;
                sqlx::query("UPDATE messages SET status = 'deferred', last_error = $2, last_smtp_code = $3, updated_at = now() WHERE id = $1")
                    .bind(message_id)
                    .bind(queue::truncate(e.message(), 500))
                    .bind(e.code().map(i32::from))
                    .execute(&mut *tx)
                    .await?;
                events::message_event(
                    &mut tx,
                    org,
                    message_id,
                    "deferred",
                    json!({ "reason": queue::truncate(e.message(), 300), "smtp_code": e.code(), "attempt": job.attempts, "retry_in_seconds": delay.as_secs() }),
                )
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
                tx.commit().await?;
            }
            tracing::warn!(%message_id, attempt = job.attempts, error = %e.message(), "relay temporarily unavailable");
        }
    }
    Ok(())
}

async fn fail(state: &AppState, org: Uuid, message_id: Uuid, code: Option<u16>, reason: &str) -> anyhow::Result<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE messages SET status = 'failed', last_error = $2, last_smtp_code = $3, updated_at = now() WHERE id = $1",
    )
    .bind(message_id)
    .bind(queue::truncate(reason, 500))
    .bind(code.map(i32::from))
    .execute(&mut *tx)
    .await?;
    events::message_event(
        &mut tx,
        org,
        message_id,
        "failed",
        json!({ "reason": queue::truncate(reason, 300), "smtp_code": code }),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    tx.commit().await?;
    Ok(())
}

/// Platform email (verification, reset, invitations). Signed with DKIM when
/// the platform's own sending domain is registered and verified as a tenant
/// domain (the recommended production setup).
pub async fn system_email(state: &AppState, relay: &SmtpRelay, job: &Job) -> anyhow::Result<()> {
    let to = job.payload["to"].as_str().unwrap_or_default().to_string();
    let subject = job.payload["subject"].as_str().unwrap_or_default().to_string();
    let email = OutgoingEmail {
        from: Some(Addr::named(Some(state.cfg.system_from_name.clone()), state.cfg.system_from_address.clone())),
        to: vec![Addr::new(to.clone())],
        subject,
        message_id: mime::new_message_id(Uuid::now_v7(), &state.cfg.message_id_domain),
        text: job.payload["text"].as_str().map(str::to_string),
        html: job.payload["html"].as_str().map(str::to_string),
        headers: vec![("X-Auto-Response-Suppress".into(), "All".into())],
        ..Default::default()
    };
    let mut message = match mime::build(&email) {
        Ok(m) => m,
        Err(e) => {
            queue::dead(&state.db, job, &format!("invalid system email: {e}")).await?;
            return Ok(());
        }
    };

    let system_domain = state.cfg.system_from_address.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    let key: Option<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT dkim_selector, dkim_private_key_enc FROM domains WHERE domain_name = $1 AND status = 'verified' LIMIT 1",
    )
    .bind(&system_domain)
    .fetch_optional(&state.db)
    .await?;
    if let Some((selector, sealed)) = key {
        match dkim::signing_config(&state.secrets, &sealed, &selector, &system_domain) {
            Ok(cfg) => message.sign(&cfg),
            Err(e) => tracing::error!(error = %e, "could not load DKIM key for system domain"),
        }
    }
    let raw = message.formatted();

    match relay.submit(&state.cfg.system_from_address, std::slice::from_ref(&to), &raw).await {
        Ok(_) => {
            queue::complete(&state.db, job).await?;
            tracing::info!(job = job.id, "system email accepted by relay");
        }
        Err(SubmitError::Permanent { message, .. }) => {
            tracing::error!(job = job.id, error = %message, "system email rejected");
            queue::dead(&state.db, job, &message).await?;
        }
        Err(e) => {
            queue::retry_or_dead(&state.db, job, queue::backoff(job.attempts, 15, 1800), e.message()).await?;
        }
    }
    Ok(())
}
