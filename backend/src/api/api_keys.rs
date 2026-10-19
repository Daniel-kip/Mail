//! API key management (console only; keys cannot manage keys).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{
        api_key,
        permissions::{API_KEY_SCOPES, DEFAULT_API_KEY_SCOPES},
        Actor, ClientInfo, Permission, Principal,
    },
    error::{ApiResult, AppError},
    services::audit::Entry,
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api-keys", get(list_keys).post(create_key))
        .route("/api-keys/scopes", get(list_scopes))
        .route("/api-keys/{id}", delete(revoke_key).patch(update_key))
        .route("/api-keys/{id}/rotate", post(rotate_key))
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: Uuid,
    name: String,
    prefix: String,
    environment: String,
    scopes: Vec<String>,
    created_by_email: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    last_used_ip: Option<ipnetwork::IpNetwork>,
    created_at: DateTime<Utc>,
    description: Option<String>,
    allowed_ips: Vec<ipnetwork::IpNetwork>,
    allowed_sender_domains: Vec<String>,
    messages_30d: i64,
}

const KEY_SELECT: &str = "SELECT k.id, k.name, k.prefix, k.environment, k.scopes, u.email::text AS created_by_email,
        k.expires_at, k.revoked_at, k.last_used_at, k.last_used_ip, k.created_at, k.description, k.allowed_ips,
        k.allowed_sender_domains::text[] AS allowed_sender_domains,
        (SELECT count(*) FROM messages m WHERE m.api_key_id = k.id AND m.created_at > now() - interval '30 days') AS messages_30d
     FROM api_keys k LEFT JOIN users u ON u.id = k.created_by";

fn key_json(k: &KeyRow) -> Value {
    let status = if k.revoked_at.is_some() {
        "revoked"
    } else if k.expires_at.is_some_and(|e| e <= Utc::now()) {
        "expired"
    } else {
        "active"
    };
    json!({
        "id": k.id, "name": k.name, "prefix": k.prefix, "environment": k.environment,
        "scopes": k.scopes, "status": status, "created_by": k.created_by_email,
        "expires_at": k.expires_at, "revoked_at": k.revoked_at,
        "last_used_at": k.last_used_at, "last_used_ip": k.last_used_ip.map(|i| i.ip().to_string()),
        "created_at": k.created_at, "description": k.description,
        "allowed_ips": k.allowed_ips.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
        "allowed_sender_domains": k.allowed_sender_domains, "messages_30d": k.messages_30d,
    })
}

/// Parse an IP allowlist of addresses or CIDR blocks ("203.0.113.7",
/// "198.51.100.0/24", "2001:db8::/32").
fn parse_ips(input: Option<Vec<String>>) -> ApiResult<Vec<ipnetwork::IpNetwork>> {
    let raw = input.unwrap_or_default();
    if raw.len() > 50 {
        return Err(AppError::validation("At most 50 IP addresses or ranges"));
    }
    let mut out: Vec<ipnetwork::IpNetwork> = Vec::new();
    for s in raw.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let net: ipnetwork::IpNetwork =
            s.parse().map_err(|_| AppError::validation(format!("{s:?} is not a valid IP address or CIDR range")))?;
        if net.prefix() == 0 {
            return Err(AppError::validation("A /0 range allows every address; leave the list empty instead"));
        }
        if !out.contains(&net) {
            out.push(net);
        }
    }
    Ok(out)
}

/// Restrict a key to sender domains of this organization.
async fn parse_sender_domains(state: &AppState, org_id: Uuid, input: Option<Vec<String>>) -> ApiResult<Vec<String>> {
    let raw = input.unwrap_or_default();
    if raw.len() > 50 {
        return Err(AppError::validation("At most 50 sender domains"));
    }
    let mut out: Vec<String> = Vec::new();
    for d in raw.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let d = validation::normalize_domain(d)?;
        if !out.contains(&d) {
            out.push(d);
        }
    }
    if !out.is_empty() {
        let known: Vec<String> = sqlx::query_scalar(
            "SELECT domain_name::text FROM domains WHERE organization_id = $1 AND domain_name = ANY($2::citext[])",
        )
        .bind(org_id)
        .bind(&out)
        .fetch_all(&state.db)
        .await?;
        if let Some(missing) = out.iter().find(|d| !known.iter().any(|k| k.eq_ignore_ascii_case(d))) {
            return Err(AppError::validation(format!("{missing} is not a domain of this organization")));
        }
    }
    Ok(out)
}

fn clean_description(d: Option<String>) -> ApiResult<Option<String>> {
    match d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(s) if s.chars().count() > 300 => Err(AppError::validation("description must be at most 300 characters")),
        other => Ok(other),
    }
}

/// Key management requires an interactive session, never an API key.
fn require_human(p: &Principal) -> ApiResult<()> {
    p.require(Permission::ApiKeysManage)?;
    if !matches!(p.actor, Actor::User { .. }) {
        return Err(AppError::forbidden("API keys cannot manage API keys"));
    }
    Ok(())
}

async fn list_scopes() -> Json<Value> {
    Json(json!({
        "scopes": API_KEY_SCOPES.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "default": DEFAULT_API_KEY_SCOPES.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    }))
}

async fn list_keys(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    require_human(&p)?;
    let rows: Vec<KeyRow> = sqlx::query_as(&format!(
        "{KEY_SELECT} WHERE k.organization_id = $1 ORDER BY k.revoked_at IS NOT NULL, k.created_at DESC"
    ))
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.iter().map(key_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct CreateKeyBody {
    name: String,
    #[serde(default = "default_env")]
    environment: String,
    scopes: Option<Vec<String>>,
    expires_at: Option<DateTime<Utc>>,
    description: Option<String>,
    allowed_ips: Option<Vec<String>>,
    allowed_sender_domains: Option<Vec<String>>,
}

pub struct KeyRestrictions {
    description: Option<String>,
    allowed_ips: Vec<ipnetwork::IpNetwork>,
    allowed_sender_domains: Vec<String>,
}

fn default_env() -> String {
    "live".into()
}

fn parse_scopes(input: Option<Vec<String>>) -> ApiResult<Vec<String>> {
    let raw = input.unwrap_or_else(|| DEFAULT_API_KEY_SCOPES.iter().map(|s| s.as_str().to_string()).collect());
    if raw.is_empty() {
        return Err(AppError::validation("At least one scope is required"));
    }
    let mut out: Vec<String> = Vec::new();
    for s in raw {
        let perm = Permission::parse(s.trim())
            .filter(|p| API_KEY_SCOPES.contains(p))
            .ok_or_else(|| AppError::validation(format!("Unknown or non-grantable scope {s:?}")))?;
        if !out.iter().any(|x| x == perm.as_str()) {
            out.push(perm.as_str().to_string());
        }
    }
    Ok(out)
}

async fn insert_key(
    tx: &mut sqlx::PgConnection,
    p: &Principal,
    name: &str,
    environment: &str,
    scopes: &[String],
    expires_at: Option<DateTime<Utc>>,
    r: &KeyRestrictions,
) -> ApiResult<(KeyRow, String)> {
    let generated = api_key::generate(environment);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO api_keys (organization_id, name, prefix, secret_hash, environment, scopes, created_by, expires_at,
                               description, allowed_ips, allowed_sender_domains)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::citext[]) RETURNING id",
    )
    .bind(p.org_id)
    .bind(name)
    .bind(&generated.prefix)
    .bind(&generated.secret_hash)
    .bind(environment)
    .bind(scopes)
    .bind(p.user_id())
    .bind(expires_at)
    .bind(&r.description)
    .bind(&r.allowed_ips)
    .bind(&r.allowed_sender_domains)
    .fetch_one(&mut *tx)
    .await?;
    let row: KeyRow = sqlx::query_as(&format!("{KEY_SELECT} WHERE k.id = $1")).bind(id).fetch_one(&mut *tx).await?;
    Ok((row, generated.full))
}

async fn create_key(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateKeyBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_human(&p)?;
    let name = validation::non_empty("name", &body.name, 100)?;
    let environment = match body.environment.as_str() {
        "live" | "test" => body.environment.clone(),
        _ => return Err(AppError::validation("environment must be 'live' or 'test'")),
    };
    if environment == "live" {
        if let Actor::User { email_verified: false, .. } = p.actor {
            return Err(AppError::forbidden("Confirm your email address before creating live API keys")
                .with_details(json!({ "reason": "email_unverified" })));
        }
    }
    if body.expires_at.is_some_and(|e| e <= Utc::now()) {
        return Err(AppError::validation("expires_at must be in the future"));
    }
    let scopes = parse_scopes(body.scopes)?;
    if !p.is_institution() && scopes.iter().any(|s| s.starts_with("mailboxes:")) {
        return Err(AppError::validation("Mailbox scopes are only available to institutional accounts"));
    }
    let restrictions = KeyRestrictions {
        description: clean_description(body.description)?,
        allowed_ips: parse_ips(body.allowed_ips)?,
        allowed_sender_domains: parse_sender_domains(&state, p.org_id, body.allowed_sender_domains).await?,
    };
    let (active, max_keys): (i64, i32) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL), p.max_api_keys
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if active >= max_keys as i64 {
        return Err(AppError::quota_exceeded(format!(
            "Your plan allows {max_keys} active API keys. Revoke unused keys or upgrade the plan."
        ))
        .with_details(json!({ "limit": "api_keys", "max": max_keys })));
    }

    let mut tx = state.db.begin().await?;
    let (row, full) = insert_key(&mut tx, &p, &name, &environment, &scopes, body.expires_at, &restrictions).await?;
    Entry::by(&p, &client, "api_key.created")
        .target("api_key", row.id)
        .details(json!({ "prefix": row.prefix, "environment": environment, "scopes": scopes,
                         "ip_restricted": !restrictions.allowed_ips.is_empty(),
                         "sender_domains": restrictions.allowed_sender_domains }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let mut out = key_json(&row);
    out["key"] = json!(full);
    out["notice"] = json!("Store this key securely. It is shown only once and cannot be recovered.");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
struct UpdateKeyBody {
    name: Option<String>,
    description: Option<String>,
    allowed_ips: Option<Vec<String>>,
    allowed_sender_domains: Option<Vec<String>>,
}

/// Rename a key or change its restrictions. Scopes and environment are
/// immutable (rotate to change them) so a key's power never grows silently.
async fn update_key(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateKeyBody>,
) -> ApiResult<Json<Value>> {
    require_human(&p)?;
    let name = match body.name.as_deref() {
        Some(n) => Some(validation::non_empty("name", n, 100)?),
        None => None,
    };
    let ips = match body.allowed_ips {
        Some(v) => Some(parse_ips(Some(v))?),
        None => None,
    };
    let domains = match body.allowed_sender_domains {
        Some(v) => Some(parse_sender_domains(&state, p.org_id, Some(v)).await?),
        None => None,
    };
    let description_set = body.description.is_some();
    let description = clean_description(body.description)?;
    let mut tx = state.db.begin().await?;
    let n = sqlx::query(
        "UPDATE api_keys SET name = COALESCE($3, name),
                description = CASE WHEN $4 THEN $5 ELSE description END,
                allowed_ips = COALESCE($6, allowed_ips),
                allowed_sender_domains = COALESCE($7::citext[], allowed_sender_domains)
         WHERE id = $1 AND organization_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(&name)
    .bind(description_set)
    .bind(&description)
    .bind(&ips)
    .bind(&domains)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Active API key"));
    }
    Entry::by(&p, &client, "api_key.updated")
        .target("api_key", id)
        .details(json!({ "renamed": name.is_some(), "ip_allowlist_changed": ips.is_some(), "sender_domains": domains }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    let row: KeyRow = sqlx::query_as(&format!("{KEY_SELECT} WHERE k.id = $1")).bind(id).fetch_one(&state.db).await?;
    Ok(Json(key_json(&row)))
}

async fn revoke_key(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    require_human(&p)?;
    let mut tx = state.db.begin().await?;
    let prefix: Option<String> = sqlx::query_scalar(
        "UPDATE api_keys SET revoked_at = now() WHERE id = $1 AND organization_id = $2 AND revoked_at IS NULL RETURNING prefix",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let prefix = prefix.ok_or_else(|| AppError::not_found("Active API key"))?;
    Entry::by(&p, &client, "api_key.revoked")
        .target("api_key", id)
        .details(json!({ "prefix": prefix }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "revoked" })))
}

/// Issue a replacement with identical settings and revoke the original in
/// one transaction.
async fn rotate_key(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_human(&p)?;
    let mut tx = state.db.begin().await?;
    #[derive(sqlx::FromRow)]
    struct Revoked {
        name: String,
        environment: String,
        scopes: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
        prefix: String,
        description: Option<String>,
        allowed_ips: Vec<ipnetwork::IpNetwork>,
        allowed_sender_domains: Vec<String>,
    }
    let old: Option<Revoked> = sqlx::query_as(
        "UPDATE api_keys SET revoked_at = now() WHERE id = $1 AND organization_id = $2 AND revoked_at IS NULL
         RETURNING name, environment, scopes, expires_at, prefix, description, allowed_ips,
                   allowed_sender_domains::text[] AS allowed_sender_domains",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Revoked {
        name,
        environment,
        scopes,
        expires_at,
        prefix: old_prefix,
        description,
        allowed_ips,
        allowed_sender_domains,
    } = old.ok_or_else(|| AppError::not_found("Active API key"))?;
    let expires_at = expires_at.filter(|e| *e > Utc::now());
    let restrictions = KeyRestrictions { description, allowed_ips, allowed_sender_domains };
    let (row, full) = insert_key(&mut tx, &p, &name, &environment, &scopes, expires_at, &restrictions).await?;
    Entry::by(&p, &client, "api_key.rotated")
        .target("api_key", row.id)
        .details(json!({ "replaced": id, "old_prefix": old_prefix, "new_prefix": row.prefix }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut out = key_json(&row);
    out["key"] = json!(full);
    out["replaced_key_id"] = json!(id);
    out["notice"] =
        json!("The previous key stopped working immediately. Store this key securely; it is shown only once.");
    Ok((StatusCode::CREATED, Json(out)))
}
