//! Webhook endpoint management and delivery history.

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
    auth::{ClientInfo, Permission, Principal},
    crypto::purpose,
    error::{ApiResult, AppError},
    services::{audit::Entry, events, webhooks},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhooks", get(list).post(create))
        .route("/webhooks/events", get(event_types))
        .route("/webhooks/{id}", get(get_one).patch(update).delete(remove))
        .route("/webhooks/{id}/rotate-secret", post(rotate_secret))
        .route("/webhooks/{id}/test", post(send_test))
        .route("/webhooks/{id}/deliveries", get(deliveries))
        .route("/webhooks/{id}/deliveries/{delivery_id}/retry", post(retry_delivery))
}

#[derive(sqlx::FromRow)]
struct EndpointRow {
    id: Uuid,
    url: String,
    description: Option<String>,
    events: Vec<String>,
    status: String,
    consecutive_failures: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const SELECT: &str =
    "SELECT id, url, description, events, status, consecutive_failures, created_at, updated_at FROM webhook_endpoints";

fn endpoint_json(e: &EndpointRow) -> Value {
    json!({
        "id": e.id, "url": e.url, "description": e.description, "events": e.events, "status": e.status,
        "consecutive_failures": e.consecutive_failures, "created_at": e.created_at, "updated_at": e.updated_at,
    })
}

async fn load(state: &AppState, org_id: Uuid, id: Uuid) -> ApiResult<EndpointRow> {
    sqlx::query_as(&format!("{SELECT} WHERE id = $1 AND organization_id = $2"))
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Webhook endpoint"))
}

fn validate_events(events: &[String]) -> ApiResult<Vec<String>> {
    if events.is_empty() {
        return Err(AppError::validation("Subscribe to at least one event (or \"*\")"));
    }
    let mut out = Vec::new();
    for e in events {
        if !events::is_known_event(e) {
            return Err(AppError::validation(format!("Unknown event type {e:?}")));
        }
        if !out.contains(e) {
            out.push(e.clone());
        }
    }
    Ok(out)
}

async fn event_types() -> Json<Value> {
    Json(json!({ "data": events::EVENT_TYPES }))
}

async fn list(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksRead)?;
    let rows: Vec<EndpointRow> =
        sqlx::query_as(&format!("{SELECT} WHERE organization_id = $1 ORDER BY created_at DESC"))
            .bind(p.org_id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(json!({ "data": rows.iter().map(endpoint_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct CreateBody {
    url: String,
    events: Vec<String>,
    description: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::WebhooksWrite)?;
    let url = webhooks::validate_url(&state.cfg, &body.url)?;
    let events = validate_events(&body.events)?;
    let (count, max): (i64, i32) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM webhook_endpoints WHERE organization_id = $1), p.max_webhooks
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if count >= max as i64 {
        return Err(AppError::quota_exceeded(format!(
            "Your plan allows {max} webhook endpoint(s). Remove one or upgrade the plan."
        ))
        .with_details(json!({ "limit": "webhooks", "max": max })));
    }
    let secret = webhooks::new_secret();
    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO webhook_endpoints (organization_id, url, description, events, secret_enc) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(p.org_id)
    .bind(&url)
    .bind(body.description.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&events)
    .bind(state.secrets.seal(purpose::WEBHOOK_SECRET, secret.as_bytes()))
    .fetch_one(&mut *tx)
    .await?;
    Entry::by(&p, &client, "webhook.created")
        .target("webhook", id)
        .details(json!({ "url": url, "events": events }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut out = endpoint_json(&load(&state, p.org_id, id).await?);
    out["secret"] = json!(secret);
    out["notice"] = json!("Store this signing secret now; it is shown only once.");
    Ok((StatusCode::CREATED, Json(out)))
}

async fn get_one(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksRead)?;
    Ok(Json(endpoint_json(&load(&state, p.org_id, id).await?)))
}

#[derive(Deserialize)]
struct UpdateBody {
    url: Option<String>,
    events: Option<Vec<String>>,
    description: Option<String>,
    status: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksWrite)?;
    let current = load(&state, p.org_id, id).await?;
    let url = match &body.url {
        Some(u) => webhooks::validate_url(&state.cfg, u)?,
        None => current.url.clone(),
    };
    let events = match &body.events {
        Some(e) => validate_events(e)?,
        None => current.events.clone(),
    };
    let status = match body.status.as_deref() {
        None => current.status.clone(),
        Some(s @ ("active" | "disabled")) => s.to_string(),
        Some(_) => return Err(AppError::validation("status must be 'active' or 'disabled'")),
    };
    let description = body.description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).or(current.description);
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE webhook_endpoints SET url = $3, events = $4, description = $5, status = $6,
                consecutive_failures = CASE WHEN $6 = 'active' THEN 0 ELSE consecutive_failures END, updated_at = now()
         WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(&url)
    .bind(&events)
    .bind(&description)
    .bind(&status)
    .execute(&mut *tx)
    .await?;
    Entry::by(&p, &client, "webhook.updated")
        .target("webhook", id)
        .details(json!({ "url": url, "events": events, "status": status }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(endpoint_json(&load(&state, p.org_id, id).await?)))
}

async fn remove(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksWrite)?;
    let e = load(&state, p.org_id, id).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(p.org_id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "webhook.deleted")
        .target("webhook", id)
        .details(json!({ "url": e.url }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "deleted": true })))
}

async fn rotate_secret(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksWrite)?;
    load(&state, p.org_id, id).await?;
    let secret = webhooks::new_secret();
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE webhook_endpoints SET secret_enc = $3, updated_at = now() WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(state.secrets.seal(purpose::WEBHOOK_SECRET, secret.as_bytes()))
    .execute(&mut *tx)
    .await?;
    Entry::by(&p, &client, "webhook.secret_rotated").target("webhook", id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "secret": secret, "notice": "The previous secret stopped working immediately." })))
}

async fn send_test(
    State(state): State<AppState>,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::WebhooksWrite)?;
    load(&state, p.org_id, id).await?;
    state.limiter.check(&format!("webhook-test:{id}"), 20, 600)?;
    let event_id = Uuid::now_v7();
    let payload = json!({
        "id": event_id, "type": "webhook.test", "created_at": Utc::now(), "organization_id": p.org_id,
        "data": { "message": format!("This is a test event from {}.", state.cfg.product_name) },
    });
    let mut tx = state.db.begin().await?;
    let delivery_id: Uuid = sqlx::query_scalar(
        "INSERT INTO webhook_deliveries (endpoint_id, organization_id, event_id, event_type, payload)
         VALUES ($1, $2, $3, 'webhook.test', $4) RETURNING id",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(event_id)
    .bind(&payload)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO outbox_jobs (kind, organization_id, webhook_delivery_id, max_attempts) VALUES ('deliver_webhook', $1, $2, 1)")
        .bind(p.org_id)
        .bind(delivery_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "delivery_id": delivery_id, "event_id": event_id }))))
}

#[derive(sqlx::FromRow)]
struct DeliveryRow {
    id: Uuid,
    event_id: Uuid,
    event_type: String,
    status: String,
    attempts: i32,
    response_status: Option<i32>,
    response_excerpt: Option<String>,
    last_error: Option<String>,
    payload: Value,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

async fn deliveries(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::WebhooksRead)?;
    load(&state, p.org_id, id).await?;
    let rows: Vec<DeliveryRow> = sqlx::query_as(
        "SELECT id, event_id, event_type, status, attempts, response_status, response_excerpt, last_error, payload, created_at, completed_at
         FROM webhook_deliveries WHERE endpoint_id = $1 AND organization_id = $2 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "data": rows.into_iter().map(|d| json!({
            "id": d.id, "event_id": d.event_id, "event_type": d.event_type, "status": d.status, "attempts": d.attempts,
            "response_status": d.response_status, "response_excerpt": d.response_excerpt, "last_error": d.last_error,
            "payload": d.payload, "created_at": d.created_at, "completed_at": d.completed_at,
        })).collect::<Vec<_>>()
    })))
}

async fn retry_delivery(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path((id, delivery_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::WebhooksWrite)?;
    let mut tx = state.db.begin().await?;
    let n = sqlx::query(
        "UPDATE webhook_deliveries SET status = 'pending', completed_at = NULL
         WHERE id = $1 AND endpoint_id = $2 AND organization_id = $3 AND status = 'failed'",
    )
    .bind(delivery_id)
    .bind(id)
    .bind(p.org_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Failed delivery"));
    }
    sqlx::query("INSERT INTO outbox_jobs (kind, organization_id, webhook_delivery_id, max_attempts) VALUES ('deliver_webhook', $1, $2, 3)")
        .bind(p.org_id)
        .bind(delivery_id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "webhook.delivery_retried").target("webhook_delivery", delivery_id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "delivery_id": delivery_id, "status": "pending" }))))
}
