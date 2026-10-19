//! Usage statistics, quotas and the tenant audit log.

use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{page_limit, ApiQuery};
use crate::{
    auth::{Permission, Principal},
    error::{ApiResult, AppError},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/usage", get(usage))
        .route("/audit-logs", get(audit_logs))
        .route("/audit-logs/export.csv", get(audit_export))
}

/// Up to 50,000 audit records as CSV (newest first), for compliance reviews.
async fn audit_export(State(state): State<AppState>, p: Principal) -> ApiResult<axum::response::Response> {
    use axum::response::IntoResponse;
    p.require(Permission::AuditRead)?;
    state.limiter.check(&format!("export:{}", p.org_id), 20, 3600)?;
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, actor_type, actor_id, actor_label, action, target_type, target_id, ip, user_agent, details, created_at
         FROM audit_logs WHERE organization_id = $1 ORDER BY id DESC LIMIT 50000",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    let mut out = String::from("id,created_at,actor_type,actor,action,target_type,target_id,ip,details\r\n");
    for r in &rows {
        let fields = [
            r.id.to_string(),
            r.created_at.to_rfc3339(),
            r.actor_type.clone(),
            r.actor_label.clone().unwrap_or_default(),
            r.action.clone(),
            r.target_type.clone().unwrap_or_default(),
            r.target_id.clone().unwrap_or_default(),
            r.ip.map(|i| i.ip().to_string()).unwrap_or_default(),
            r.details.to_string(),
        ];
        out.push_str(&fields.iter().map(|f| super::messages::csv_field(f)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    let filename = format!("audit-log-{}.csv", Utc::now().format("%Y%m%d-%H%M"));
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        out,
    )
        .into_response())
}

#[derive(Deserialize)]
struct UsageQuery {
    days: Option<i64>,
}

async fn usage(
    State(state): State<AppState>,
    p: Principal,
    ApiQuery(q): ApiQuery<UsageQuery>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::UsageRead)?;
    let days = q.days.unwrap_or(14).clamp(1, 90);

    let series: Vec<(NaiveDate, String, i64)> = sqlx::query_as(
        "SELECT (created_at AT TIME ZONE 'UTC')::date AS day, status, count(*)
         FROM messages
         WHERE organization_id = $1 AND created_at >= (now() AT TIME ZONE 'UTC')::date - ($2::int - 1)
         GROUP BY 1, 2 ORDER BY 1",
    )
    .bind(p.org_id)
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;

    let today = Utc::now().date_naive();
    let mut daily: Vec<Value> = Vec::new();
    for offset in (0..days).rev() {
        let day = today - chrono::Duration::days(offset);
        let mut counts = serde_json::Map::new();
        let mut total = 0i64;
        for (_, status, n) in series.iter().filter(|(d, _, _)| *d == day) {
            counts.insert(status.clone(), json!(n));
            total += n;
        }
        daily.push(json!({ "date": day, "total": total, "by_status": counts }));
    }

    let (daily_limit, monthly_limit, plan): (i32, i32, String) = sqlx::query_as(
        "SELECT p.daily_send_limit, p.monthly_send_limit, p.name FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    let (used_today, used_month): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(sum(messages_queued) FILTER (WHERE day = (now() AT TIME ZONE 'UTC')::date), 0)::bigint,
                COALESCE(sum(messages_queued), 0)::bigint
         FROM usage_daily WHERE organization_id = $1 AND day >= date_trunc('month', now() AT TIME ZONE 'UTC')::date",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;

    #[derive(sqlx::FromRow)]
    struct Counts {
        domains: i64,
        verified_domains: i64,
        mailboxes: i64,
        active_keys: i64,
        suppressions: i64,
        webhooks: i64,
        sent_7d: i64,
        bounced_7d: i64,
        failed_7d: i64,
        in_flight: i64,
    }
    let c: Counts = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM domains WHERE organization_id = $1) AS domains,
           (SELECT count(*) FROM domains WHERE organization_id = $1 AND status = 'verified') AS verified_domains,
           (SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted') AS mailboxes,
           (SELECT count(*) FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())) AS active_keys,
           (SELECT count(*) FROM suppressions WHERE organization_id = $1) AS suppressions,
           (SELECT count(*) FROM webhook_endpoints WHERE organization_id = $1 AND status = 'active') AS webhooks,
           (SELECT count(*) FROM messages WHERE organization_id = $1 AND created_at > now() - interval '7 days' AND status IN ('accepted_by_mta','delivered','bounced','deferred')) AS sent_7d,
           (SELECT count(*) FROM messages WHERE organization_id = $1 AND created_at > now() - interval '7 days' AND status = 'bounced') AS bounced_7d,
           (SELECT count(*) FROM messages WHERE organization_id = $1 AND created_at > now() - interval '7 days' AND status = 'failed') AS failed_7d,
           (SELECT count(*) FROM messages WHERE organization_id = $1 AND status IN ('queued','processing','deferred')) AS in_flight",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;

    let bounce_rate = if c.sent_7d > 0 { c.bounced_7d as f64 / c.sent_7d as f64 } else { 0.0 };
    Ok(Json(json!({
        "plan": plan,
        "quota": {
            "daily": { "used": used_today, "limit": daily_limit },
            "monthly": { "used": used_month, "limit": monthly_limit },
        },
        "daily": daily,
        "totals": {
            "domains": c.domains, "verified_domains": c.verified_domains, "mailboxes": c.mailboxes,
            "active_api_keys": c.active_keys, "suppressions": c.suppressions, "active_webhooks": c.webhooks,
            "in_flight": c.in_flight,
        },
        "last_7_days": {
            "sent": c.sent_7d, "bounced": c.bounced_7d, "failed": c.failed_7d,
            "bounce_rate": (bounce_rate * 10000.0).round() / 10000.0,
        },
    })))
}

#[derive(Deserialize)]
struct AuditQuery {
    before_id: Option<i64>,
    action: Option<String>,
    limit: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    id: i64,
    actor_type: String,
    actor_id: Option<Uuid>,
    actor_label: Option<String>,
    action: String,
    target_type: Option<String>,
    target_id: Option<String>,
    ip: Option<ipnetwork::IpNetwork>,
    user_agent: Option<String>,
    details: Value,
    created_at: DateTime<Utc>,
}

async fn audit_logs(
    State(state): State<AppState>,
    p: Principal,
    ApiQuery(q): ApiQuery<AuditQuery>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::AuditRead)?;
    let limit = page_limit(q.limit, 50, 200);
    if let Some(a) = &q.action {
        if a.len() > 100 {
            return Err(AppError::validation("action filter too long"));
        }
    }
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, actor_type, actor_id, actor_label, action, target_type, target_id, ip, user_agent, details, created_at
         FROM audit_logs
         WHERE organization_id = $1 AND ($2::bigint IS NULL OR id < $2) AND ($3::text IS NULL OR action LIKE $3 || '%')
         ORDER BY id DESC LIMIT $4",
    )
    .bind(p.org_id)
    .bind(q.before_id)
    .bind(q.action.filter(|a| !a.is_empty()))
    .bind(limit + 1)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let page = &rows[..rows.len().min(limit as usize)];
    Ok(Json(json!({
        "data": page.iter().map(|r| json!({
            "id": r.id, "actor_type": r.actor_type, "actor_id": r.actor_id, "actor": r.actor_label,
            "action": r.action, "target_type": r.target_type, "target_id": r.target_id,
            "ip": r.ip.map(|i| i.ip().to_string()), "user_agent": r.user_agent,
            "details": r.details, "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "next_before_id": if has_more { page.last().map(|r| r.id) } else { None },
    })))
}
