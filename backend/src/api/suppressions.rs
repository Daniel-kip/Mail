//! Suppression list: addresses the platform will not send to.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{empty_as_none, page_limit, ApiJson, ApiQuery, Cursor};
use crate::{
    auth::{ClientInfo, Permission, Principal},
    error::{ApiResult, AppError},
    services::audit::Entry,
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/suppressions", get(list).post(add)).route("/suppressions/{email}", delete(remove))
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
    reason: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    email: String,
    reason: String,
    source_message_id: Option<Uuid>,
    details: Option<String>,
    created_at: DateTime<Utc>,
}

async fn list(State(state): State<AppState>, p: Principal, ApiQuery(q): ApiQuery<ListQuery>) -> ApiResult<Json<Value>> {
    p.require(Permission::SuppressionsRead)?;
    let limit = page_limit(q.limit, 50, 200);
    let cursor = Cursor::decode(q.cursor.as_deref())?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, email::text AS email, reason, source_message_id, details, created_at FROM suppressions
         WHERE organization_id = $1
           AND ($2::text IS NULL OR email::text ILIKE '%' || $2 || '%')
           AND ($3::text IS NULL OR reason = $3)
           AND ($4::timestamptz IS NULL OR (created_at, id) < ($4, $5))
         ORDER BY created_at DESC, id DESC LIMIT $6",
    )
    .bind(p.org_id)
    .bind(empty_as_none(q.q))
    .bind(empty_as_none(q.reason))
    .bind(cursor.as_ref().map(|c| c.t))
    .bind(cursor.as_ref().map(|c| c.id))
    .bind(limit + 1)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let page = &rows[..rows.len().min(limit as usize)];
    Ok(Json(json!({
        "data": page.iter().map(|r| json!({
            "id": r.id, "email": r.email, "reason": r.reason, "source_message_id": r.source_message_id,
            "details": r.details, "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "next_cursor": if has_more { page.last().map(|r| Cursor::encode(r.created_at, r.id)) } else { None },
    })))
}

#[derive(Deserialize)]
struct AddBody {
    email: String,
    reason: Option<String>,
    details: Option<String>,
}

async fn add(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<AddBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::SuppressionsWrite)?;
    let email = validation::parse_email("email", &body.email)?.as_string().to_lowercase();
    let reason = match body.reason.as_deref().unwrap_or("manual") {
        r @ ("manual" | "unsubscribe") => r.to_string(),
        _ => return Err(AppError::validation("reason must be 'manual' or 'unsubscribe'")),
    };
    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO suppressions (organization_id, email, reason, details) VALUES ($1, $2, $3, $4)
         ON CONFLICT (organization_id, email) DO UPDATE SET details = COALESCE(EXCLUDED.details, suppressions.details)
         RETURNING id",
    )
    .bind(p.org_id)
    .bind(&email)
    .bind(&reason)
    .bind(body.details.as_deref().map(|d| d.chars().take(500).collect::<String>()))
    .fetch_one(&mut *tx)
    .await?;
    Entry::by(&p, &client, "suppression.added")
        .target("suppression", id)
        .details(json!({ "email": email, "reason": reason }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "email": email, "reason": reason }))))
}

async fn remove(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(email): Path<String>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::SuppressionsWrite)?;
    let email = email.trim().to_lowercase();
    let mut tx = state.db.begin().await?;
    let removed: Option<(Uuid, String)> =
        sqlx::query_as("DELETE FROM suppressions WHERE organization_id = $1 AND email = $2 RETURNING id, reason")
            .bind(p.org_id)
            .bind(&email)
            .fetch_optional(&mut *tx)
            .await?;
    let (id, reason) = removed.ok_or_else(|| AppError::not_found("Suppression"))?;
    Entry::by(&p, &client, "suppression.removed")
        .target("suppression", id)
        .details(json!({ "email": email, "reason": reason }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}
