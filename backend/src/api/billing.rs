//! Plans, subscriptions and invoices.
//!
//! * `GET /v1/plans` is public (pricing page).
//! * Tenant endpoints require `billing:read` / `billing:manage` (owners and
//!   admins) and an interactive session for changes.
//! * Operator endpoints (mounted under `/v1/admin`) record payments, void
//!   invoices and tune plan prices and limits.
//!
//! Plan changes: moving from a free to a paid plan starts a new period now
//! (or a trial, once per organization); upgrades between paid plans apply
//! immediately with a prorated invoice for the rest of the period;
//! downgrades are scheduled for the end of the period so nothing already
//! paid for is taken away mid-period.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{empty_as_none, ApiJson, ApiQuery};
use crate::{
    auth::{Actor, ClientInfo, Permission, PlatformAdmin, Principal},
    error::{ApiResult, AppError},
    services::{
        audit::Entry,
        billing::{self, Line, Plan, PLAN_COLUMNS},
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plans", get(public_plans))
        .route("/billing", get(overview))
        .route("/billing/details", patch(update_details))
        .route("/billing/subscription", post(change_plan))
        .route("/billing/subscription/cancel", post(cancel))
        .route("/billing/subscription/resume", post(resume))
        .route("/billing/invoices", get(list_invoices))
        .route("/billing/invoices/{id}", get(get_invoice))
}

pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/plans", get(admin_plans))
        .route("/plans/{id}", patch(admin_update_plan))
        .route("/invoices", get(admin_invoices))
        .route("/invoices/{id}", get(admin_get_invoice))
        .route("/invoices/{id}/mark-paid", post(admin_mark_paid))
        .route("/invoices/{id}/void", post(admin_void))
        .route("/billing/run", post(admin_run_billing))
}

// ── public catalogue ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PlansQuery {
    audience: Option<String>,
}

async fn public_plans(State(state): State<AppState>, ApiQuery(q): ApiQuery<PlansQuery>) -> ApiResult<Json<Value>> {
    let audience = empty_as_none(q.audience);
    if let Some(a) = &audience {
        if a != "individual" && a != "institution" {
            return Err(AppError::validation("audience must be 'individual' or 'institution'"));
        }
    }
    let plans: Vec<Plan> = sqlx::query_as(&format!(
        "SELECT {PLAN_COLUMNS} FROM plans WHERE is_public AND ($1::text IS NULL OR audience = $1) ORDER BY sort_order, id"
    ))
    .bind(&audience)
    .fetch_all(&state.db)
    .await?;
    let currency = &state.cfg.billing.currency;
    Ok(Json(json!({
        "currency": currency,
        "tax": { "rate_bps": state.cfg.billing.tax_rate_bps, "label": state.cfg.billing.tax_label },
        "data": plans.iter().map(|p| p.to_json(currency)).collect::<Vec<_>>(),
    })))
}

// ── tenant billing ───────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SubRow {
    plan_id: String,
    status: String,
    billing_cycle: String,
    current_period_start: DateTime<Utc>,
    current_period_end: DateTime<Utc>,
    trial_ends_at: Option<DateTime<Utc>>,
    trial_used: bool,
    pending_plan_id: Option<String>,
    cancel_at_period_end: bool,
}

async fn subscription(executor: &mut sqlx::PgConnection, org_id: Uuid, lock: bool) -> ApiResult<SubRow> {
    let sql = format!(
        "SELECT plan_id, status, billing_cycle, current_period_start, current_period_end, trial_ends_at, trial_used,
                pending_plan_id, cancel_at_period_end
         FROM subscriptions WHERE organization_id = $1{}",
        if lock { " FOR UPDATE" } else { "" }
    );
    sqlx::query_as(&sql)
        .bind(org_id)
        .fetch_optional(&mut *executor)
        .await?
        .ok_or_else(|| AppError::not_found("Subscription"))
}

fn sub_json(s: &SubRow, plan: &Plan, pending: Option<&Plan>, currency: &str) -> Value {
    json!({
        "status": s.status, "billing_cycle": s.billing_cycle,
        "current_period_start": s.current_period_start, "current_period_end": s.current_period_end,
        "trial_ends_at": s.trial_ends_at, "trial_used": s.trial_used,
        "cancel_at_period_end": s.cancel_at_period_end,
        "plan": plan.to_json(currency),
        "pending_plan": pending.map(|p| p.to_json(currency)),
    })
}

#[derive(sqlx::FromRow)]
struct InvoiceRow {
    id: Uuid,
    number: String,
    kind: String,
    status: String,
    currency: String,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    lines: Value,
    subtotal_cents: i64,
    tax_rate_bps: i32,
    tax_cents: i64,
    total_cents: i64,
    billing_snapshot: Value,
    issued_at: DateTime<Utc>,
    due_at: DateTime<Utc>,
    paid_at: Option<DateTime<Utc>>,
    payment_method: Option<String>,
    payment_reference: Option<String>,
    voided_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

const INVOICE_COLUMNS: &str = "id, number, kind, status, currency, period_start, period_end, lines, subtotal_cents,
    tax_rate_bps, tax_cents, total_cents, billing_snapshot, issued_at, due_at, paid_at, payment_method,
    payment_reference, voided_at, notes";

fn invoice_json(i: &InvoiceRow, full: bool) -> Value {
    let overdue = i.status == "open" && i.due_at < Utc::now();
    let mut v = json!({
        "id": i.id, "number": i.number, "kind": i.kind, "status": i.status, "overdue": overdue,
        "currency": i.currency, "period_start": i.period_start, "period_end": i.period_end,
        "subtotal_cents": i.subtotal_cents, "tax_rate_bps": i.tax_rate_bps, "tax_cents": i.tax_cents,
        "total_cents": i.total_cents, "issued_at": i.issued_at, "due_at": i.due_at, "paid_at": i.paid_at,
        "voided_at": i.voided_at,
    });
    if full {
        v["lines"] = i.lines.clone();
        v["billing"] = i.billing_snapshot.clone();
        v["payment_method"] = json!(i.payment_method);
        v["payment_reference"] = json!(i.payment_reference);
        v["notes"] = json!(i.notes);
    }
    v
}

async fn overview(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::BillingRead)?;
    let mut conn = state.db.acquire().await?;
    let s = subscription(&mut conn, p.org_id, false).await?;
    let plan = billing::load_plan(&mut conn, &s.plan_id).await?;
    let pending = match &s.pending_plan_id {
        Some(id) => Some(billing::load_plan(&mut conn, id).await?),
        None => None,
    };
    let seats = billing::seats(&mut conn, p.org_id).await?;
    let next_plan = pending.as_ref().unwrap_or(&plan);
    let next_plan = if s.cancel_at_period_end { None } else { Some(next_plan) };
    let estimate = next_plan.map(|np| {
        let lines = billing::period_lines(np, &s.billing_cycle, seats);
        let subtotal: i64 = lines.iter().map(|l| l.quantity * l.unit_cents).sum();
        let tax = (subtotal * state.cfg.billing.tax_rate_bps as i64 + 5000) / 10000;
        json!({
            "date": s.current_period_end, "subtotal_cents": subtotal, "tax_cents": tax, "total_cents": subtotal + tax,
            "lines": lines.iter().map(|l| json!({ "description": l.description, "quantity": l.quantity, "unit_cents": l.unit_cents, "amount_cents": l.quantity * l.unit_cents })).collect::<Vec<_>>(),
        })
    });
    #[derive(sqlx::FromRow)]
    struct Details {
        name: String,
        account_type: String,
        billing_name: Option<String>,
        billing_email: Option<String>,
        billing_address: Option<String>,
        tax_id: Option<String>,
        country: Option<String>,
    }
    let d: Details = sqlx::query_as(
        "SELECT name, account_type, billing_name, billing_email::text AS billing_email, billing_address, tax_id, country
         FROM organizations WHERE id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&mut *conn)
    .await?;
    let invoices: Vec<InvoiceRow> = sqlx::query_as(&format!(
        "SELECT {INVOICE_COLUMNS} FROM invoices WHERE organization_id = $1 ORDER BY issued_at DESC LIMIT 24"
    ))
    .bind(p.org_id)
    .fetch_all(&mut *conn)
    .await?;
    let balance: i64 = invoices.iter().filter(|i| i.status == "open").map(|i| i.total_cents).sum();
    let currency = state.cfg.billing.currency.clone();
    Ok(Json(json!({
        "account_type": d.account_type,
        "currency": currency,
        "subscription": sub_json(&s, &plan, pending.as_ref(), &currency),
        "seats": seats,
        "next_invoice": estimate,
        "open_balance_cents": balance,
        "details": {
            "organization": d.name, "billing_name": d.billing_name, "billing_email": d.billing_email,
            "billing_address": d.billing_address, "tax_id": d.tax_id, "country": d.country,
        },
        "payment_instructions": state.cfg.billing.payment_instructions,
        "invoices": invoices.iter().map(|i| invoice_json(i, false)).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct DetailsBody {
    billing_name: Option<String>,
    billing_email: Option<String>,
    billing_address: Option<String>,
    tax_id: Option<String>,
    country: Option<String>,
}

fn clean(field: &str, v: Option<String>, max: usize) -> ApiResult<Option<String>> {
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) if s.chars().count() > max => {
            Err(AppError::validation(format!("{field} must be at most {max} characters")))
        }
        Some(s) => Ok(Some(s)),
    }
}

async fn update_details(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<DetailsBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::BillingManage)?;
    let email = match body.billing_email.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(e) => Some(validation::parse_email("billing_email", e)?.as_string().to_lowercase()),
        None => None,
    };
    let name = clean("billing_name", body.billing_name, 200)?;
    if let Some(n) = &name {
        validation::header_safe("billing_name", n, 200)?;
    }
    let address = clean("billing_address", body.billing_address, 1000)?;
    let tax_id = clean("tax_id", body.tax_id, 64)?;
    let country = clean("country", body.country, 2)?.map(|c| c.to_ascii_uppercase());
    if country.as_ref().is_some_and(|c| c.len() != 2 || !c.bytes().all(|b| b.is_ascii_uppercase())) {
        return Err(AppError::validation("country must be a two-letter ISO code such as KE"));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE organizations SET billing_name = $2, billing_email = $3, billing_address = $4, tax_id = $5, country = $6,
                updated_at = now() WHERE id = $1",
    )
    .bind(p.org_id)
    .bind(&name)
    .bind(&email)
    .bind(&address)
    .bind(&tax_id)
    .bind(&country)
    .execute(&mut *tx)
    .await?;
    Entry::by(&p, &client, "billing.details_updated").target("organization", p.org_id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

fn require_human_manager(p: &Principal) -> ApiResult<()> {
    p.require(Permission::BillingManage)?;
    if !matches!(p.actor, Actor::User { .. }) {
        return Err(AppError::forbidden("Subscriptions can only be changed from the console"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct ChangePlanBody {
    plan_id: String,
    billing_cycle: Option<String>,
}

async fn change_plan(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<ChangePlanBody>,
) -> ApiResult<Json<Value>> {
    require_human_manager(&p)?;
    let cycle_req = match body.billing_cycle.as_deref() {
        None => None,
        Some(c @ ("monthly" | "annual")) => Some(c.to_string()),
        Some(_) => return Err(AppError::validation("billing_cycle must be 'monthly' or 'annual'")),
    };
    let mut tx = state.db.begin().await?;
    let s = subscription(&mut tx, p.org_id, true).await?;
    let current = billing::load_plan(&mut tx, &s.plan_id).await?;
    let target = billing::load_plan(&mut tx, body.plan_id.trim()).await?;
    if target.audience != p.account_type {
        return Err(AppError::validation(format!(
            "{} is a plan for {} accounts. This workspace is an {} account.",
            target.name,
            if target.audience == "individual" { "individual" } else { "institutional" },
            if p.account_type == "individual" { "individual" } else { "institutional" }
        )));
    }
    if !target.is_public || !target.self_service {
        return Err(AppError::forbidden(format!(
            "{} is arranged with the platform team. Contact support to switch.",
            target.name
        ))
        .with_code("plan_not_self_service"));
    }
    let cycle = cycle_req.unwrap_or_else(|| s.billing_cycle.clone());
    if target.id == current.id && cycle == s.billing_cycle && s.pending_plan_id.is_none() && !s.cancel_at_period_end {
        return Err(AppError::conflict("already_on_plan", format!("You are already on {}", target.name)));
    }
    let violations = billing::usage_violations(&mut tx, p.org_id, &target).await?;
    if !violations.is_empty() {
        return Err(AppError::conflict(
            "plan_limits_exceeded",
            format!("Your current usage does not fit {}: {}.", target.name, violations.join("; ")),
        )
        .with_details(json!({ "violations": violations })));
    }

    let seats = billing::seats(&mut tx, p.org_id).await?;
    let period_cost = |plan: &Plan, cyc: &str| -> i64 {
        billing::period_lines(plan, cyc, seats).iter().map(|l| l.quantity * l.unit_cents).sum()
    };
    let current_cost = period_cost(&current, &s.billing_cycle);
    let target_cost_now = period_cost(&target, &s.billing_cycle);
    let now = Utc::now();
    let mut invoice = None;
    let outcome;

    if s.status == "trialing" && target.is_paid() {
        // Switching between paid plans during a trial keeps the trial dates.
        sqlx::query(
            "UPDATE subscriptions SET plan_id = $2, billing_cycle = $3, pending_plan_id = NULL, cancel_at_period_end = false,
                    updated_at = now() WHERE organization_id = $1",
        )
        .bind(p.org_id)
        .bind(&target.id)
        .bind(&cycle)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = now() WHERE id = $1")
            .bind(p.org_id)
            .bind(&target.id)
            .execute(&mut *tx)
            .await?;
        outcome = "changed";
    } else if !current.is_paid() || s.status == "trialing" {
        if s.status == "past_due" && target.is_paid() {
            return Err(AppError::forbidden("Settle the overdue invoices before upgrading").with_code("past_due"));
        }
        // New period starting now (or a one-time trial).
        if target.is_paid() && target.trial_days > 0 && !s.trial_used && s.status != "trialing" {
            let trial_end = now + chrono::Duration::days(target.trial_days as i64);
            sqlx::query(
                "UPDATE subscriptions SET plan_id = $2, status = 'trialing', billing_cycle = $3, current_period_start = $4,
                        current_period_end = $5, trial_ends_at = $5, trial_used = true, pending_plan_id = NULL,
                        cancel_at_period_end = false, updated_at = now() WHERE organization_id = $1",
            )
            .bind(p.org_id)
            .bind(&target.id)
            .bind(&cycle)
            .bind(now)
            .bind(trial_end)
            .execute(&mut *tx)
            .await?;
            outcome = "trial_started";
        } else {
            let end = billing::add_period(now, &cycle);
            sqlx::query(
                "UPDATE subscriptions SET plan_id = $2, status = 'active', billing_cycle = $3, current_period_start = $4,
                        current_period_end = $5, trial_ends_at = CASE WHEN status = 'trialing' THEN now() ELSE trial_ends_at END,
                        pending_plan_id = NULL, cancel_at_period_end = false, updated_at = now() WHERE organization_id = $1",
            )
            .bind(p.org_id)
            .bind(&target.id)
            .bind(&cycle)
            .bind(now)
            .bind(end)
            .execute(&mut *tx)
            .await?;
            invoice = billing::issue_invoice(
                &mut tx,
                &state.cfg.billing,
                &state.cfg.public_base_url,
                p.org_id,
                "subscription",
                now,
                end,
                &billing::period_lines(&target, &cycle, seats),
            )
            .await?;
            outcome = "changed";
        }
        sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = now() WHERE id = $1")
            .bind(p.org_id)
            .bind(&target.id)
            .execute(&mut *tx)
            .await?;
    } else if target_cost_now >= current_cost && target.id != current.id {
        // Upgrade: immediate, prorated for the remainder of the period.
        let total = (s.current_period_end - s.current_period_start).num_seconds().max(1);
        let remaining = (s.current_period_end - now).num_seconds().clamp(0, total);
        let difference = target_cost_now - current_cost;
        let prorated = (difference as i128 * remaining as i128 / total as i128) as i64;
        sqlx::query(
            "UPDATE subscriptions SET plan_id = $2, billing_cycle = $3, pending_plan_id = NULL, cancel_at_period_end = false,
                    updated_at = now() WHERE organization_id = $1",
        )
        .bind(p.org_id)
        .bind(&target.id)
        .bind(&cycle)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = now() WHERE id = $1")
            .bind(p.org_id)
            .bind(&target.id)
            .execute(&mut *tx)
            .await?;
        if prorated > 0 {
            invoice = billing::issue_invoice(
                &mut tx,
                &state.cfg.billing,
                &state.cfg.public_base_url,
                p.org_id,
                "proration",
                now,
                s.current_period_end,
                &[Line {
                    description: format!(
                        "Upgrade from {} to {} for the rest of the current period",
                        current.name, target.name
                    ),
                    quantity: 1,
                    unit_cents: prorated,
                }],
            )
            .await?;
        }
        outcome = "changed";
    } else {
        // Downgrade or cycle change only: takes effect at renewal.
        let pending = if target.id == current.id { None } else { Some(target.id.clone()) };
        sqlx::query(
            "UPDATE subscriptions SET pending_plan_id = $2, billing_cycle = $3, cancel_at_period_end = false, updated_at = now()
             WHERE organization_id = $1",
        )
        .bind(p.org_id)
        .bind(&pending)
        .bind(&cycle)
        .execute(&mut *tx)
        .await?;
        outcome = "scheduled";
    }

    Entry::by(&p, &client, "subscription.changed")
        .target("organization", p.org_id)
        .details(json!({ "from": current.id, "to": target.id, "billing_cycle": cycle, "outcome": outcome,
                         "invoice": invoice.as_ref().map(|i| i.number.clone()) }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "outcome": outcome,
        "plan": target.id,
        "effective_at": if outcome == "scheduled" { s.current_period_end } else { now },
        "invoice": invoice.map(|i| json!({ "id": i.id, "number": i.number, "total_cents": i.total_cents, "due_at": i.due_at })),
    })))
}

async fn cancel(State(state): State<AppState>, client: ClientInfo, p: Principal) -> ApiResult<Json<Value>> {
    require_human_manager(&p)?;
    let mut tx = state.db.begin().await?;
    let s = subscription(&mut tx, p.org_id, true).await?;
    let plan = billing::load_plan(&mut tx, &s.plan_id).await?;
    if !plan.is_paid() {
        return Err(AppError::conflict("not_subscribed", "You are on a free plan; there is nothing to cancel"));
    }
    sqlx::query("UPDATE subscriptions SET cancel_at_period_end = true, pending_plan_id = NULL, updated_at = now() WHERE organization_id = $1")
        .bind(p.org_id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "subscription.cancel_scheduled").target("organization", p.org_id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true, "ends_at": s.current_period_end })))
}

async fn resume(State(state): State<AppState>, client: ClientInfo, p: Principal) -> ApiResult<Json<Value>> {
    require_human_manager(&p)?;
    let n = sqlx::query(
        "UPDATE subscriptions SET cancel_at_period_end = false, updated_at = now() WHERE organization_id = $1 AND cancel_at_period_end",
    )
    .bind(p.org_id)
    .execute(&state.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::conflict("not_cancelling", "The subscription is not scheduled to cancel"));
    }
    Entry::by(&p, &client, "subscription.resumed").target("organization", p.org_id).record(&state.db).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_invoices(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::BillingRead)?;
    let rows: Vec<InvoiceRow> = sqlx::query_as(&format!(
        "SELECT {INVOICE_COLUMNS} FROM invoices WHERE organization_id = $1 ORDER BY issued_at DESC LIMIT 200"
    ))
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "data": rows.iter().map(|i| invoice_json(i, false)).collect::<Vec<_>>() })))
}

async fn get_invoice(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::BillingRead)?;
    let row: InvoiceRow =
        sqlx::query_as(&format!("SELECT {INVOICE_COLUMNS} FROM invoices WHERE id = $1 AND organization_id = $2"))
            .bind(id)
            .bind(p.org_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::not_found("Invoice"))?;
    Ok(Json(invoice_json(&row, true)))
}

// ── operator endpoints ───────────────────────────────────────────────────

fn admin_entry<'a>(admin: &'a PlatformAdmin, client: &'a ClientInfo, org: Option<Uuid>, action: &'a str) -> Entry<'a> {
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

async fn admin_plans(State(state): State<AppState>, _a: PlatformAdmin) -> ApiResult<Json<Value>> {
    let plans: Vec<Plan> =
        sqlx::query_as(&format!("SELECT {PLAN_COLUMNS} FROM plans ORDER BY audience, sort_order, id"))
            .fetch_all(&state.db)
            .await?;
    let counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT plan_id, count(*) FROM organizations GROUP BY plan_id").fetch_all(&state.db).await?;
    let currency = &state.cfg.billing.currency;
    Ok(Json(json!({
        "currency": currency,
        "data": plans.iter().map(|p| {
            let mut v = p.to_json(currency);
            v["organizations"] = json!(counts.iter().find(|(id, _)| *id == p.id).map(|c| c.1).unwrap_or(0));
            v
        }).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct PlanUpdate {
    name: Option<String>,
    description: Option<String>,
    price_monthly_cents: Option<i32>,
    price_per_mailbox_cents: Option<i32>,
    annual_discount_percent: Option<i32>,
    daily_send_limit: Option<i32>,
    monthly_send_limit: Option<i32>,
    max_domains: Option<i32>,
    max_mailboxes: Option<i32>,
    max_members: Option<i32>,
    max_api_keys: Option<i32>,
    max_webhooks: Option<i32>,
    max_recipients_per_request: Option<i32>,
    api_requests_per_minute: Option<i32>,
    trial_days: Option<i32>,
    is_public: Option<bool>,
    self_service: Option<bool>,
}

async fn admin_update_plan(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<String>,
    ApiJson(b): ApiJson<PlanUpdate>,
) -> ApiResult<Json<Value>> {
    for (field, v) in [
        ("price_monthly_cents", b.price_monthly_cents),
        ("price_per_mailbox_cents", b.price_per_mailbox_cents),
        ("daily_send_limit", b.daily_send_limit),
        ("monthly_send_limit", b.monthly_send_limit),
        ("max_domains", b.max_domains),
        ("max_mailboxes", b.max_mailboxes),
        ("max_api_keys", b.max_api_keys),
        ("max_webhooks", b.max_webhooks),
    ] {
        if v.is_some_and(|n| n < 0) {
            return Err(AppError::validation(format!("{field} cannot be negative")));
        }
    }
    if b.max_members.is_some_and(|n| n < 1)
        || b.max_recipients_per_request.is_some_and(|n| n < 1)
        || b.api_requests_per_minute.is_some_and(|n| n < 1)
    {
        return Err(AppError::validation("Member, recipient and request limits must be at least 1"));
    }
    if b.annual_discount_percent.is_some_and(|n| !(0..=50).contains(&n))
        || b.trial_days.is_some_and(|n| !(0..=90).contains(&n))
    {
        return Err(AppError::validation("annual_discount_percent must be 0-50 and trial_days 0-90"));
    }
    let name = match b.name.as_deref() {
        Some(n) => Some(validation::non_empty("name", n, 80)?),
        None => None,
    };
    let mut tx = state.db.begin().await?;
    let n = sqlx::query(
        "UPDATE plans SET
            name = COALESCE($2, name), description = COALESCE($3, description),
            price_monthly_cents = COALESCE($4, price_monthly_cents), price_per_mailbox_cents = COALESCE($5, price_per_mailbox_cents),
            annual_discount_percent = COALESCE($6, annual_discount_percent), daily_send_limit = COALESCE($7, daily_send_limit),
            monthly_send_limit = COALESCE($8, monthly_send_limit), max_domains = COALESCE($9, max_domains),
            max_mailboxes = COALESCE($10, max_mailboxes), max_members = COALESCE($11, max_members),
            max_api_keys = COALESCE($12, max_api_keys), max_webhooks = COALESCE($13, max_webhooks),
            max_recipients_per_request = COALESCE($14, max_recipients_per_request),
            api_requests_per_minute = COALESCE($15, api_requests_per_minute), trial_days = COALESCE($16, trial_days),
            is_public = COALESCE($17, is_public), self_service = COALESCE($18, self_service)
         WHERE id = $1",
    )
    .bind(&id)
    .bind(&name)
    .bind(b.description.as_deref().map(str::trim))
    .bind(b.price_monthly_cents)
    .bind(b.price_per_mailbox_cents)
    .bind(b.annual_discount_percent)
    .bind(b.daily_send_limit)
    .bind(b.monthly_send_limit)
    .bind(b.max_domains)
    .bind(b.max_mailboxes)
    .bind(b.max_members)
    .bind(b.max_api_keys)
    .bind(b.max_webhooks)
    .bind(b.max_recipients_per_request)
    .bind(b.api_requests_per_minute)
    .bind(b.trial_days)
    .bind(b.is_public)
    .bind(b.self_service)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Plan"));
    }
    admin_entry(&admin, &client, None, "plan.updated").target("plan", &id).record(&mut *tx).await?;
    tx.commit().await?;
    let mut conn = state.db.acquire().await?;
    Ok(Json(billing::load_plan(&mut conn, &id).await?.to_json(&state.cfg.billing.currency)))
}

#[derive(Deserialize)]
struct AdminInvoiceQuery {
    status: Option<String>,
    organization_id: Option<Uuid>,
}

async fn admin_invoices(
    State(state): State<AppState>,
    _a: PlatformAdmin,
    ApiQuery(q): ApiQuery<AdminInvoiceQuery>,
) -> ApiResult<Json<Value>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        number: String,
        kind: String,
        status: String,
        currency: String,
        total_cents: i64,
        issued_at: DateTime<Utc>,
        due_at: DateTime<Utc>,
        paid_at: Option<DateTime<Utc>>,
        organization_id: Uuid,
        organization: String,
    }
    let status = empty_as_none(q.status);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT i.id, i.number, i.kind, i.status, i.currency, i.total_cents, i.issued_at, i.due_at, i.paid_at,
                i.organization_id, o.name AS organization
         FROM invoices i JOIN organizations o ON o.id = i.organization_id
         WHERE ($1::text IS NULL OR i.status = $1) AND ($2::uuid IS NULL OR i.organization_id = $2)
         ORDER BY i.issued_at DESC LIMIT 500",
    )
    .bind(&status)
    .bind(q.organization_id)
    .fetch_all(&state.db)
    .await?;
    let (open_count, open_total, overdue): (i64, Option<i64>, i64) = sqlx::query_as(
        "SELECT count(*), sum(total_cents)::bigint, count(*) FILTER (WHERE due_at < now()) FROM invoices WHERE status = 'open'",
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({
        "summary": { "open": open_count, "open_total_cents": open_total.unwrap_or(0), "overdue": overdue, "currency": state.cfg.billing.currency },
        "data": rows.into_iter().map(|r| json!({
            "id": r.id, "number": r.number, "kind": r.kind, "status": r.status, "currency": r.currency,
            "total_cents": r.total_cents, "issued_at": r.issued_at, "due_at": r.due_at, "paid_at": r.paid_at,
            "overdue": r.status == "open" && r.due_at < Utc::now(),
            "organization_id": r.organization_id, "organization": r.organization,
        })).collect::<Vec<_>>(),
    })))
}

async fn admin_get_invoice(
    State(state): State<AppState>,
    _a: PlatformAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let row: InvoiceRow = sqlx::query_as(&format!("SELECT {INVOICE_COLUMNS} FROM invoices WHERE id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Invoice"))?;
    Ok(Json(invoice_json(&row, true)))
}

#[derive(Deserialize)]
struct MarkPaidBody {
    method: String,
    reference: Option<String>,
    paid_at: Option<DateTime<Utc>>,
}

async fn admin_mark_paid(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
    ApiJson(b): ApiJson<MarkPaidBody>,
) -> ApiResult<Json<Value>> {
    let method = validation::non_empty("method", &b.method, 60)?;
    let reference = clean("reference", b.reference, 120)?;
    let paid_at = b.paid_at.unwrap_or_else(Utc::now);
    if paid_at > Utc::now() + chrono::Duration::minutes(5) {
        return Err(AppError::validation("paid_at cannot be in the future"));
    }
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "UPDATE invoices SET status = 'paid', paid_at = $2, payment_method = $3, payment_reference = $4
         WHERE id = $1 AND status = 'open' RETURNING organization_id, number",
    )
    .bind(id)
    .bind(paid_at)
    .bind(&method)
    .bind(&reference)
    .fetch_optional(&mut *tx)
    .await?;
    let (org, number) = row.ok_or_else(|| AppError::not_found("Open invoice"))?;
    admin_entry(&admin, &client, Some(org), "invoice.paid")
        .target("invoice", id)
        .details(json!({ "number": number, "method": method, "reference": reference }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    billing::update_payment_status(&state).await.map_err(AppError::Internal)?;
    Ok(Json(json!({ "id": id, "status": "paid" })))
}

#[derive(Deserialize)]
struct VoidBody {
    reason: String,
}

async fn admin_void(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
    Path(id): Path<Uuid>,
    ApiJson(b): ApiJson<VoidBody>,
) -> ApiResult<Json<Value>> {
    let reason = validation::non_empty("reason", &b.reason, 1000)?;
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "UPDATE invoices SET status = 'void', voided_at = now(), notes = $2 WHERE id = $1 AND status = 'open'
         RETURNING organization_id, number",
    )
    .bind(id)
    .bind(&reason)
    .fetch_optional(&mut *tx)
    .await?;
    let (org, number) = row.ok_or_else(|| AppError::not_found("Open invoice"))?;
    admin_entry(&admin, &client, Some(org), "invoice.voided")
        .target("invoice", id)
        .details(json!({ "number": number, "reason": reason }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    billing::update_payment_status(&state).await.map_err(AppError::Internal)?;
    Ok(Json(json!({ "id": id, "status": "void" })))
}

/// Run renewals and payment-status updates now (they also run every few
/// minutes from the maintenance worker).
async fn admin_run_billing(
    State(state): State<AppState>,
    client: ClientInfo,
    admin: PlatformAdmin,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let renewed = billing::renew_due(&state).await.map_err(AppError::Internal)?;
    billing::update_payment_status(&state).await.map_err(AppError::Internal)?;
    admin_entry(&admin, &client, None, "billing.run").details(json!({ "renewed": renewed })).record(&state.db).await?;
    Ok((StatusCode::OK, Json(json!({ "renewed": renewed }))))
}
