//! Private endpoints for the mail tier. Not exposed through the public
//! proxy (see infra/nginx) and protected by `INTERNAL_API_TOKEN`.
//!
//! `POST /internal/v1/bounces?recipient=<envelope recipient>` with the raw
//! message as the body. Postfix's `pipe` transport for the return-path
//! domain calls this through `infra/scripts/bounce-to-api.sh`.

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{header, HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    crypto,
    error::{ApiResult, AppError},
    mail::bounce::{self, FeedbackKind},
    services::events,
    state::AppState,
    workers::queue::truncate,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/bounces", post(ingest_bounce))
        .route("/metrics", axum::routing::get(metrics))
        .layer(DefaultBodyLimit::max(12 * 1024 * 1024))
        .fallback(crate::http::api_not_found)
}

fn authorize(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = state.cfg.internal_api_token.as_deref() else {
        return Err(AppError::not_found("Resource"));
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if crypto::constant_time_eq(given.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::unauthorized())
    }
}

#[derive(Deserialize)]
struct BounceQuery {
    recipient: String,
}

#[derive(sqlx::FromRow)]
struct Target {
    organization_id: Uuid,
    status: String,
    envelope_recipients: Vec<String>,
}

async fn ingest_bounce(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<BounceQuery>,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    authorize(&state, &headers)?;
    let key = bounce::verp_key(&state.cfg.data_encryption_key);
    let Some(message_id) = bounce::parse_verp(&key, &q.recipient) else {
        tracing::warn!(recipient = %truncate(&q.recipient, 120), "bounce for unknown or forged return path ignored");
        // 2xx so Postfix does not retry or bounce the bounce.
        return Ok((StatusCode::ACCEPTED, Json(json!({ "status": "ignored", "reason": "unrecognized return path" }))));
    };
    let target: Option<Target> =
        sqlx::query_as("SELECT organization_id, status, envelope_recipients FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(&state.db)
            .await?;
    let Some(t) = target else {
        return Ok((StatusCode::ACCEPTED, Json(json!({ "status": "ignored", "reason": "message not found" }))));
    };

    let feedback = bounce::parse_feedback(&body);
    let mut tx = state.db.begin().await?;
    if feedback.is_empty() {
        events::message_event(&mut tx, t.organization_id, message_id, "bounce_unparsed", json!({ "size": body.len() }))
            .await?;
        tx.commit().await?;
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({ "status": "recorded", "message_id": message_id, "parsed": 0 })),
        ));
    }

    for f in &feedback {
        // Only trust a recipient named in the report if it was actually one
        // of this message's envelope recipients.
        let recipient = f
            .recipient
            .as_ref()
            .filter(|r| t.envelope_recipients.iter().any(|e| e.eq_ignore_ascii_case(r)))
            .cloned()
            .or_else(|| (t.envelope_recipients.len() == 1).then(|| t.envelope_recipients[0].clone()));
        let details = json!({
            "recipient": recipient, "status": f.status, "diagnostic": f.diagnostic.as_deref().map(|d| truncate(d, 500)),
            "remote_mta": f.remote_mta,
        });
        match f.kind {
            FeedbackKind::HardBounce => {
                let code = f
                    .status
                    .as_deref()
                    .and_then(|s| s.split('.').next())
                    .and_then(|c| c.parse::<i32>().ok())
                    .map(|c| c * 100);
                sqlx::query(
                    "UPDATE messages SET status = 'bounced', bounced_at = now(), last_error = $2, last_smtp_code = COALESCE($3, last_smtp_code), updated_at = now()
                     WHERE id = $1",
                )
                .bind(message_id)
                .bind(f.diagnostic.as_deref().map(|d| truncate(d, 500)))
                .bind(code)
                .execute(&mut *tx)
                .await?;
                if let Some(r) = &recipient {
                    sqlx::query(
                        "INSERT INTO suppressions (organization_id, email, reason, source_message_id, details) VALUES ($1, $2, 'hard_bounce', $3, $4)
                         ON CONFLICT (organization_id, email) DO NOTHING",
                    )
                    .bind(t.organization_id)
                    .bind(r.to_lowercase())
                    .bind(message_id)
                    .bind(f.diagnostic.as_deref().map(|d| truncate(d, 300)))
                    .execute(&mut *tx)
                    .await?;
                }
                events::message_event(&mut tx, t.organization_id, message_id, "bounced", details).await?;
            }
            FeedbackKind::Delayed => {
                if t.status == "accepted_by_mta" {
                    sqlx::query(
                        "UPDATE messages SET status = 'deferred', last_error = $2, updated_at = now() WHERE id = $1",
                    )
                    .bind(message_id)
                    .bind(f.diagnostic.as_deref().map(|d| truncate(d, 500)))
                    .execute(&mut *tx)
                    .await?;
                }
                events::message_event(&mut tx, t.organization_id, message_id, "deferred", details).await?;
            }
            FeedbackKind::Delivered => {
                sqlx::query("UPDATE messages SET status = 'delivered', updated_at = now() WHERE id = $1 AND status IN ('accepted_by_mta', 'deferred')")
                    .bind(message_id)
                    .execute(&mut *tx)
                    .await?;
                events::message_event(&mut tx, t.organization_id, message_id, "delivered", details).await?;
            }
            FeedbackKind::Complaint => {
                if let Some(r) = &recipient {
                    sqlx::query(
                        "INSERT INTO suppressions (organization_id, email, reason, source_message_id, details) VALUES ($1, $2, 'complaint', $3, 'Recipient reported this mail as spam')
                         ON CONFLICT (organization_id, email) DO UPDATE SET reason = 'complaint'",
                    )
                    .bind(t.organization_id)
                    .bind(r.to_lowercase())
                    .bind(message_id)
                    .execute(&mut *tx)
                    .await?;
                }
                events::message_event(&mut tx, t.organization_id, message_id, "complained", details).await?;
            }
        }
    }
    tx.commit().await?;
    tracing::info!(%message_id, reports = feedback.len(), "delivery feedback processed");
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "status": "processed", "message_id": message_id, "parsed": feedback.len() })),
    ))
}

/// Prometheus text exposition for queue health and delivery outcomes
/// (guide 06 monitoring). Scrape from the private network only.
async fn metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<(StatusCode, [(header::HeaderName, &'static str); 1], String)> {
    authorize(&state, &headers)?;
    let jobs: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT kind, status, count(*) FROM outbox_jobs WHERE status <> 'done' GROUP BY 1, 2")
            .fetch_all(&state.db)
            .await?;
    let oldest: Option<f64> = sqlx::query_scalar(
        "SELECT extract(epoch FROM now() - min(run_at))::float8 FROM outbox_jobs WHERE status = 'pending' AND run_at <= now()",
    )
    .fetch_one(&state.db)
    .await?;
    let statuses: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, count(*) FROM messages WHERE created_at > now() - interval '1 hour' GROUP BY 1")
            .fetch_all(&state.db)
            .await?;
    let webhooks: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM webhook_deliveries WHERE created_at > now() - interval '1 hour' GROUP BY 1",
    )
    .fetch_all(&state.db)
    .await?;
    let suspended: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations WHERE status = 'suspended'")
        .fetch_one(&state.db)
        .await?;

    let mut out = String::new();
    out.push_str(
        "# HELP mv_outbox_jobs Outbox jobs by kind and status (excluding done).
# TYPE mv_outbox_jobs gauge
",
    );
    for (kind, status, n) in jobs {
        out.push_str(&format!(
            "mv_outbox_jobs{{kind=\"{kind}\",status=\"{status}\"}} {n}
"
        ));
    }
    out.push_str(
        "# HELP mv_outbox_oldest_pending_seconds Age of the oldest runnable job.
# TYPE mv_outbox_oldest_pending_seconds gauge
",
    );
    out.push_str(&format!(
        "mv_outbox_oldest_pending_seconds {}
",
        oldest.unwrap_or(0.0).round()
    ));
    out.push_str(
        "# HELP mv_messages_last_hour Messages created in the last hour by current status.
# TYPE mv_messages_last_hour gauge
",
    );
    for (status, n) in statuses {
        out.push_str(&format!(
            "mv_messages_last_hour{{status=\"{status}\"}} {n}
"
        ));
    }
    out.push_str(
        "# HELP mv_webhook_deliveries_last_hour Webhook deliveries in the last hour by status.
# TYPE mv_webhook_deliveries_last_hour gauge
",
    );
    for (status, n) in webhooks {
        out.push_str(&format!(
            "mv_webhook_deliveries_last_hour{{status=\"{status}\"}} {n}
"
        ));
    }
    out.push_str(&format!(
        "# HELP mv_organizations_suspended Suspended tenants.
# TYPE mv_organizations_suspended gauge
mv_organizations_suspended {suspended}
"
    ));
    out.push_str(&format!(
        "# HELP mv_db_pool_connections Open database connections.
# TYPE mv_db_pool_connections gauge
mv_db_pool_connections {}
",
        state.db.size()
    ));
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "text/plain; version=0.0.4")], out))
}
