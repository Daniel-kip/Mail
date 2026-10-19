//! Domain events and webhook fan-out.
//!
//! `emit` must be called inside the transaction that makes the change. It
//! creates one `webhook_deliveries` row and one outbox job per subscribed
//! endpoint, so an event is delivered if and only if its change commits.

use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::ApiResult;

pub const EVENT_TYPES: &[&str] = &[
    "message.scheduled",
    "message.cancelled",
    "message.unsubscribed",
    "message.queued",
    "message.accepted",
    "message.delivered",
    "message.deferred",
    "message.bounced",
    "message.failed",
    "message.suppressed",
    "message.complained",
    "domain.verified",
    "domain.verification_failed",
];

pub fn is_known_event(e: &str) -> bool {
    e == "*" || EVENT_TYPES.contains(&e)
}

/// Fan an event out to every active endpoint subscribed to it.
pub async fn emit(tx: &mut PgConnection, org_id: Uuid, event_type: &str, data: Value) -> ApiResult<u64> {
    let event_id = Uuid::now_v7();
    let payload = json!({
        "id": event_id,
        "type": event_type,
        "created_at": chrono::Utc::now(),
        "organization_id": org_id,
        "data": data,
    });
    let deliveries: Vec<Uuid> = sqlx::query_scalar(
        "INSERT INTO webhook_deliveries (endpoint_id, organization_id, event_id, event_type, payload)
         SELECT e.id, e.organization_id, $2, $3, $4
         FROM webhook_endpoints e
         WHERE e.organization_id = $1 AND e.status = 'active' AND ($3 = ANY(e.events) OR '*' = ANY(e.events))
         RETURNING id",
    )
    .bind(org_id)
    .bind(event_id)
    .bind(event_type)
    .bind(&payload)
    .fetch_all(&mut *tx)
    .await?;
    if !deliveries.is_empty() {
        sqlx::query(
            "INSERT INTO outbox_jobs (kind, organization_id, webhook_delivery_id, max_attempts)
             SELECT 'deliver_webhook', $1, d, 12 FROM unnest($2::uuid[]) AS d",
        )
        .bind(org_id)
        .bind(&deliveries)
        .execute(&mut *tx)
        .await?;
    }
    Ok(deliveries.len() as u64)
}

/// Append to a message's timeline and emit the matching webhook event.
pub async fn message_event(
    tx: &mut PgConnection,
    org_id: Uuid,
    message_id: Uuid,
    event_type: &str,
    details: Value,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO message_events (message_id, organization_id, event_type, details) VALUES ($1, $2, $3, $4)",
    )
    .bind(message_id)
    .bind(org_id)
    .bind(event_type)
    .bind(&details)
    .execute(&mut *tx)
    .await?;

    let webhook_type = format!("message.{event_type}");
    if EVENT_TYPES.contains(&webhook_type.as_str()) {
        #[derive(sqlx::FromRow)]
        struct M {
            from_address: String,
            to_address: String,
            subject: String,
            status: String,
            smtp_message_id: String,
            tags: Vec<String>,
            metadata: Value,
        }
        let m: M = sqlx::query_as(
            "SELECT from_address::text AS from_address, to_address, subject, status, smtp_message_id, tags, metadata
             FROM messages WHERE id = $1",
        )
        .bind(message_id)
        .fetch_one(&mut *tx)
        .await?;
        emit(
            tx,
            org_id,
            &webhook_type,
            json!({
                "message_id": message_id, "status": m.status, "from": m.from_address, "to": m.to_address,
                "subject": m.subject, "smtp_message_id": m.smtp_message_id, "tags": m.tags,
                "metadata": m.metadata, "details": details,
            }),
        )
        .await?;
    }
    Ok(())
}
