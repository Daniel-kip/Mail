//! Signed webhook delivery with retries.

use serde_json::{json, Value};
use uuid::Uuid;

use super::queue::{self, Job};
use crate::{crypto::purpose, services::webhooks, state::AppState};

/// Endpoints are disabled after this many consecutive dead deliveries.
const DISABLE_AFTER_FAILURES: i32 = 15;

#[derive(sqlx::FromRow)]
struct Delivery {
    id: Uuid,
    organization_id: Uuid,
    endpoint_id: Uuid,
    event_id: Uuid,
    event_type: String,
    payload: Value,
    status: String,
    url: String,
    secret_enc: Vec<u8>,
    endpoint_status: String,
}

pub async fn deliver(state: &AppState, client: &reqwest::Client, job: &Job) -> anyhow::Result<()> {
    let id = job.webhook_delivery_id.ok_or_else(|| anyhow::anyhow!("webhook job {} has no delivery", job.id))?;
    let d: Option<Delivery> = sqlx::query_as(
        "SELECT d.id, d.organization_id, d.endpoint_id, d.event_id, d.event_type, d.payload, d.status,
                e.url, e.secret_enc, e.status AS endpoint_status
         FROM webhook_deliveries d JOIN webhook_endpoints e ON e.id = d.endpoint_id
         WHERE d.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some(d) = d else {
        queue::complete(&state.db, job).await?;
        return Ok(());
    };
    if d.status == "succeeded" {
        queue::complete(&state.db, job).await?;
        return Ok(());
    }
    if d.endpoint_status != "active" && d.event_type != "webhook.test" {
        finish(state, &d, "failed", None, None, Some("Endpoint is disabled")).await?;
        queue::complete(&state.db, job).await?;
        return Ok(());
    }

    let secret = state.secrets.open_string(purpose::WEBHOOK_SECRET, &d.secret_enc)?;
    let body = serde_json::to_vec(&d.payload)?;
    let timestamp = chrono::Utc::now().timestamp();
    let result = client
        .post(&d.url)
        .header("content-type", "application/json")
        .header("mailvora-signature", webhooks::signature_header(&secret, timestamp, &body))
        .header("mailvora-event-id", d.event_id.to_string())
        .header("mailvora-event-type", &d.event_type)
        .header("mailvora-delivery-id", d.id.to_string())
        .header("mailvora-delivery-attempt", job.attempts.to_string())
        .body(body)
        .send()
        .await;

    let attempt_error = match result {
        Ok(resp) => {
            let status = resp.status();
            let excerpt = resp.text().await.unwrap_or_default();
            let excerpt = queue::truncate(&excerpt, 500);
            if status.is_success() {
                finish(state, &d, "succeeded", Some(status.as_u16() as i32), Some(&excerpt), None).await?;
                sqlx::query("UPDATE webhook_endpoints SET consecutive_failures = 0 WHERE id = $1")
                    .bind(d.endpoint_id)
                    .execute(&state.db)
                    .await?;
                queue::complete(&state.db, job).await?;
                return Ok(());
            }
            record_attempt(state, &d, Some(status.as_u16() as i32), Some(&excerpt), &format!("HTTP {status}")).await?;
            format!("endpoint responded HTTP {}", status.as_u16())
        }
        Err(e) => {
            // Avoid echoing internal resolver details beyond the error kind.
            let msg = if e.is_timeout() {
                "request timed out".to_string()
            } else if e.is_connect() {
                format!("connection failed: {}", root_cause(&e))
            } else {
                format!("request failed: {}", root_cause(&e))
            };
            record_attempt(state, &d, None, None, &msg).await?;
            msg
        }
    };

    // Retry schedule: 10s, 20s, 40s … capped at 6h (≈ 1 day across 12 attempts).
    let dead = queue::retry_or_dead(&state.db, job, queue::backoff(job.attempts, 10, 6 * 3600), &attempt_error).await?;
    if dead {
        finish(state, &d, "failed", None, None, Some(&attempt_error)).await?;
        if d.event_type != "webhook.test" {
            let failures: i32 = sqlx::query_scalar(
                "UPDATE webhook_endpoints SET consecutive_failures = consecutive_failures + 1 WHERE id = $1 RETURNING consecutive_failures",
            )
            .bind(d.endpoint_id)
            .fetch_one(&state.db)
            .await?;
            if failures >= DISABLE_AFTER_FAILURES {
                sqlx::query("UPDATE webhook_endpoints SET status = 'disabled', updated_at = now() WHERE id = $1")
                    .bind(d.endpoint_id)
                    .execute(&state.db)
                    .await?;
                crate::services::audit::Entry {
                    organization_id: Some(d.organization_id),
                    actor_type: "system",
                    actor_id: None,
                    actor_label: Some("webhook-worker".into()),
                    action: "webhook.auto_disabled",
                    target_type: Some("webhook"),
                    target_id: Some(d.endpoint_id.to_string()),
                    client: None,
                    details: json!({ "consecutive_failures": failures }),
                }
                .record(&state.db)
                .await?;
                tracing::warn!(endpoint = %d.endpoint_id, "webhook endpoint disabled after repeated failures");
            }
        }
    }
    Ok(())
}

fn root_cause(e: &reqwest::Error) -> String {
    let mut source: &dyn std::error::Error = e;
    while let Some(next) = source.source() {
        source = next;
    }
    queue::truncate(&source.to_string(), 200)
}

async fn record_attempt(
    state: &AppState,
    d: &Delivery,
    status: Option<i32>,
    excerpt: Option<&str>,
    error: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE webhook_deliveries SET attempts = attempts + 1, response_status = $2, response_excerpt = $3, last_error = $4 WHERE id = $1")
        .bind(d.id)
        .bind(status)
        .bind(excerpt)
        .bind(error)
        .execute(&state.db)
        .await?;
    Ok(())
}

async fn finish(
    state: &AppState,
    d: &Delivery,
    status: &str,
    http: Option<i32>,
    excerpt: Option<&str>,
    error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE webhook_deliveries SET status = $2,
                attempts = attempts + CASE WHEN $2 = 'succeeded' THEN 1 ELSE 0 END,
                response_status = COALESCE($3, response_status), response_excerpt = COALESCE($4, response_excerpt),
                last_error = COALESCE($5, last_error), completed_at = now()
         WHERE id = $1",
    )
    .bind(d.id)
    .bind(status)
    .bind(http)
    .bind(excerpt)
    .bind(error)
    .execute(&state.db)
    .await?;
    Ok(())
}
