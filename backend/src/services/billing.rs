//! Subscriptions, invoices and plan limits.
//!
//! The platform issues invoices and tracks their status; collecting money is
//! deliberately left to the operator's existing channel (bank transfer,
//! mobile money, card terminal...), recorded with a payment reference when
//! the operator marks an invoice paid. Pricing is a monthly base fee plus a
//! per-mailbox seat fee for institutions; annual billing applies the plan's
//! discount. All amounts are integer minor units of `BILLING_CURRENCY`.
//!
//! Renewal is idempotent: a unique index allows one subscription invoice per
//! organization and period, and subscriptions are claimed with
//! `FOR UPDATE SKIP LOCKED`, so overlapping maintenance passes cannot double
//! bill.

use chrono::{DateTime, Duration, Months, Utc};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    config::BillingConfig,
    error::{ApiResult, AppError},
    services::system_mail,
    state::AppState,
};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub audience: String,
    pub description: String,
    pub daily_send_limit: i32,
    pub monthly_send_limit: i32,
    pub max_domains: i32,
    pub max_mailboxes: i32,
    pub default_mailbox_quota_bytes: i64,
    pub max_mailbox_quota_bytes: i64,
    pub max_recipients_per_request: i32,
    pub api_requests_per_minute: i32,
    pub price_monthly_cents: i32,
    pub price_per_mailbox_cents: i32,
    pub annual_discount_percent: i32,
    pub max_members: i32,
    pub max_api_keys: i32,
    pub max_webhooks: i32,
    pub trial_days: i32,
    pub is_public: bool,
    pub self_service: bool,
    pub sort_order: i32,
}

pub const PLAN_COLUMNS: &str = "id, name, audience, description, daily_send_limit, monthly_send_limit, max_domains,
    max_mailboxes, default_mailbox_quota_bytes, max_mailbox_quota_bytes, max_recipients_per_request,
    api_requests_per_minute, price_monthly_cents, price_per_mailbox_cents, annual_discount_percent, max_members,
    max_api_keys, max_webhooks, trial_days, is_public, self_service, sort_order";

impl Plan {
    pub fn is_paid(&self) -> bool {
        self.price_monthly_cents > 0 || self.price_per_mailbox_cents > 0
    }

    /// Price of one period for the given cycle, before seats and tax.
    pub fn base_price(&self, cycle: &str) -> i64 {
        cycle_price(self.price_monthly_cents as i64, cycle, self.annual_discount_percent)
    }

    pub fn seat_price(&self, cycle: &str) -> i64 {
        cycle_price(self.price_per_mailbox_cents as i64, cycle, self.annual_discount_percent)
    }

    pub fn to_json(&self, currency: &str) -> Value {
        json!({
            "id": self.id, "name": self.name, "audience": self.audience, "description": self.description,
            "currency": currency,
            "price_monthly_cents": self.price_monthly_cents,
            "price_per_mailbox_cents": self.price_per_mailbox_cents,
            "price_annual_cents": self.base_price("annual"),
            "price_per_mailbox_annual_cents": self.seat_price("annual"),
            "annual_discount_percent": self.annual_discount_percent,
            "trial_days": self.trial_days, "self_service": self.self_service, "is_public": self.is_public,
            "limits": {
                "daily_send_limit": self.daily_send_limit, "monthly_send_limit": self.monthly_send_limit,
                "max_domains": self.max_domains, "max_mailboxes": self.max_mailboxes,
                "default_mailbox_quota_bytes": self.default_mailbox_quota_bytes,
                "max_mailbox_quota_bytes": self.max_mailbox_quota_bytes,
                "max_recipients_per_request": self.max_recipients_per_request,
                "api_requests_per_minute": self.api_requests_per_minute,
                "max_members": self.max_members, "max_api_keys": self.max_api_keys, "max_webhooks": self.max_webhooks,
            },
        })
    }
}

fn cycle_price(monthly: i64, cycle: &str, discount: i32) -> i64 {
    if cycle == "annual" {
        // Round to the nearest minor unit.
        (monthly * 12 * (100 - discount as i64) + 50) / 100
    } else {
        monthly
    }
}

pub fn add_period(start: DateTime<Utc>, cycle: &str) -> DateTime<Utc> {
    let months = if cycle == "annual" { 12 } else { 1 };
    start.checked_add_months(Months::new(months)).unwrap_or(start + Duration::days(30 * months as i64))
}

pub fn format_amount(currency: &str, cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.abs();
    format!("{currency} {sign}{}.{:02}", c / 100, c % 100)
}

pub async fn load_plan(executor: &mut PgConnection, id: &str) -> ApiResult<Plan> {
    sqlx::query_as(&format!("SELECT {PLAN_COLUMNS} FROM plans WHERE id = $1"))
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?
        .ok_or_else(|| AppError::not_found("Plan"))
}

/// Billable mailbox seats: every mailbox that is not deleted (pending ones
/// hold a reserved address).
pub async fn seats(executor: &mut PgConnection, org_id: Uuid) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted'")
        .bind(org_id)
        .fetch_one(&mut *executor)
        .await?)
}

/// Resources that exceed a target plan's limits, as human-readable notes.
pub async fn usage_violations(executor: &mut PgConnection, org_id: Uuid, plan: &Plan) -> ApiResult<Vec<String>> {
    #[derive(sqlx::FromRow)]
    struct U {
        domains: i64,
        mailboxes: i64,
        members: i64,
        api_keys: i64,
        webhooks: i64,
    }
    let u: U = sqlx::query_as(
        "SELECT (SELECT count(*) FROM domains WHERE organization_id = $1) AS domains,
                (SELECT count(*) FROM mailboxes WHERE organization_id = $1 AND status <> 'deleted') AS mailboxes,
                (SELECT count(*) FROM memberships WHERE organization_id = $1) AS members,
                (SELECT count(*) FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL) AS api_keys,
                (SELECT count(*) FROM webhook_endpoints WHERE organization_id = $1) AS webhooks",
    )
    .bind(org_id)
    .fetch_one(&mut *executor)
    .await?;
    let mut out = Vec::new();
    for (label, used, max) in [
        ("domains", u.domains, plan.max_domains),
        ("mailboxes", u.mailboxes, plan.max_mailboxes),
        ("team members", u.members, plan.max_members),
        ("active API keys", u.api_keys, plan.max_api_keys),
        ("webhooks", u.webhooks, plan.max_webhooks),
    ] {
        if used > max as i64 {
            out.push(format!("{used} {label} in use; {} allows {max}", plan.name));
        }
    }
    Ok(out)
}

pub struct Line {
    pub description: String,
    pub quantity: i64,
    pub unit_cents: i64,
}

impl Line {
    fn amount(&self) -> i64 {
        self.quantity * self.unit_cents
    }
}

/// Lines for one full period of a plan.
pub fn period_lines(plan: &Plan, cycle: &str, seats: i64) -> Vec<Line> {
    let label = if cycle == "annual" { "annual" } else { "monthly" };
    let mut lines = Vec::new();
    if plan.price_monthly_cents > 0 {
        lines.push(Line {
            description: format!("{} plan ({label})", plan.name),
            quantity: 1,
            unit_cents: plan.base_price(cycle),
        });
    }
    if plan.price_per_mailbox_cents > 0 && seats > 0 {
        lines.push(Line {
            description: format!("Work mailboxes ({label}, per mailbox)"),
            quantity: seats,
            unit_cents: plan.seat_price(cycle),
        });
    }
    lines
}

pub struct IssuedInvoice {
    pub id: Uuid,
    pub number: String,
    pub total_cents: i64,
    pub due_at: DateTime<Utc>,
}

/// Issue an invoice in `tx` and enqueue the billing notification. Returns
/// `None` when the lines total zero (nothing is invoiced).
#[allow(clippy::too_many_arguments)]
pub async fn issue_invoice(
    tx: &mut PgConnection,
    billing: &BillingConfig,
    public_base_url: &str,
    org_id: Uuid,
    kind: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    lines: &[Line],
) -> ApiResult<Option<IssuedInvoice>> {
    let subtotal: i64 = lines.iter().map(Line::amount).sum();
    if subtotal <= 0 {
        return Ok(None);
    }
    let tax = (subtotal * billing.tax_rate_bps as i64 + 5000) / 10000;
    let total = subtotal + tax;
    #[derive(sqlx::FromRow)]
    struct Billing {
        name: String,
        billing_name: Option<String>,
        billing_email: Option<String>,
        billing_address: Option<String>,
        tax_id: Option<String>,
        country: Option<String>,
    }
    let b: Billing = sqlx::query_as(
        "SELECT name, billing_name, billing_email::text AS billing_email, billing_address, tax_id, country
         FROM organizations WHERE id = $1",
    )
    .bind(org_id)
    .fetch_one(&mut *tx)
    .await?;
    let seq: i64 = sqlx::query_scalar("SELECT nextval('invoice_number_seq')").fetch_one(&mut *tx).await?;
    let number = format!("INV-{}-{seq:06}", Utc::now().format("%Y"));
    let due_at = Utc::now() + Duration::days(billing.due_days);
    let lines_json: Vec<Value> = lines
        .iter()
        .map(|l| json!({ "description": l.description, "quantity": l.quantity, "unit_cents": l.unit_cents, "amount_cents": l.amount() }))
        .collect();
    let snapshot = json!({
        "organization": b.name, "name": b.billing_name.clone().unwrap_or_else(|| b.name.clone()),
        "email": b.billing_email, "address": b.billing_address, "tax_id": b.tax_id, "country": b.country,
        "issuer": billing.issuer, "tax_label": billing.tax_label, "payment_instructions": billing.payment_instructions,
    });
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO invoices (organization_id, number, kind, currency, period_start, period_end, lines, subtotal_cents,
                               tax_rate_bps, tax_cents, total_cents, billing_snapshot, due_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
    )
    .bind(org_id)
    .bind(&number)
    .bind(kind)
    .bind(&billing.currency)
    .bind(period_start)
    .bind(period_end)
    .bind(Value::Array(lines_json))
    .bind(subtotal)
    .bind(billing.tax_rate_bps)
    .bind(tax)
    .bind(total)
    .bind(&snapshot)
    .bind(due_at)
    .fetch_one(&mut *tx)
    .await?;

    // Notify the billing contact, or the owners when none is set.
    let recipients: Vec<String> = match &b.billing_email {
        Some(e) => vec![e.clone()],
        None => {
            sqlx::query_scalar(
                "SELECT u.email::text FROM memberships m JOIN users u ON u.id = m.user_id
             WHERE m.organization_id = $1 AND m.role = 'owner' AND u.status = 'active' LIMIT 5",
            )
            .bind(org_id)
            .fetch_all(&mut *tx)
            .await?
        }
    };
    // The console opens records only through sealed in-app links, so the
    // email points at the billing page, where the invoice is listed first.
    let link = format!("{public_base_url}/console/billing");
    for to in recipients {
        system_mail::enqueue(
            &mut *tx,
            system_mail::invoice_issued(
                &billing.issuer,
                &to,
                &b.name,
                &number,
                &format_amount(&billing.currency, total),
                &due_at.format("%d %B %Y").to_string(),
                &link,
            ),
        )
        .await?;
    }
    Ok(Some(IssuedInvoice { id, number, total_cents: total, due_at }))
}

#[derive(sqlx::FromRow)]
struct DueSubscription {
    organization_id: Uuid,
    plan_id: String,
    status: String,
    billing_cycle: String,
    current_period_end: DateTime<Utc>,
    trial_ends_at: Option<DateTime<Utc>>,
    pending_plan_id: Option<String>,
    cancel_at_period_end: bool,
    account_type: String,
}

/// Roll every subscription whose period ended: apply scheduled changes,
/// end trials and issue the next period's invoice. Returns renewals done.
pub async fn renew_due(state: &AppState) -> anyhow::Result<u64> {
    let mut renewed = 0;
    loop {
        let mut tx = state.db.begin().await?;
        let due: Vec<DueSubscription> = sqlx::query_as(
            "SELECT s.organization_id, s.plan_id, s.status, s.billing_cycle, s.current_period_end, s.trial_ends_at,
                    s.pending_plan_id, s.cancel_at_period_end, o.account_type
             FROM subscriptions s JOIN organizations o ON o.id = s.organization_id
             WHERE s.current_period_end <= now() OR (s.status = 'trialing' AND s.trial_ends_at <= now())
             ORDER BY s.current_period_end LIMIT 50 FOR UPDATE OF s SKIP LOCKED",
        )
        .fetch_all(&mut *tx)
        .await?;
        if due.is_empty() {
            break;
        }
        for s in &due {
            renew_one(state, &mut tx, s).await?;
            renewed += 1;
        }
        tx.commit().await?;
        if due.len() < 50 {
            break;
        }
    }
    Ok(renewed)
}

async fn renew_one(state: &AppState, tx: &mut PgConnection, s: &DueSubscription) -> anyhow::Result<()> {
    let now = Utc::now();
    let trial_ending = s.status == "trialing" && s.trial_ends_at.is_some_and(|t| t <= now);
    let period_over = s.current_period_end <= now;
    let mut plan_id = s.plan_id.clone();
    let mut note = None;

    if period_over && s.cancel_at_period_end {
        plan_id = if s.account_type == "individual" { "developer".into() } else { "free".into() };
        note = Some("cancelled at period end; moved to the free plan");
    } else if period_over {
        if let Some(pending) = &s.pending_plan_id {
            let target = load_plan(tx, pending).await?;
            if usage_violations(tx, s.organization_id, &target).await?.is_empty() {
                plan_id = pending.clone();
                note = Some("scheduled plan change applied");
            } else {
                note = Some("scheduled downgrade skipped: current usage exceeds the new plan's limits");
            }
        }
    }
    let plan = load_plan(tx, &plan_id).await?;

    // The new period starts where the old one ended, unless the platform was
    // offline for a whole period, in which case it starts now.
    let start = if !period_over {
        now
    } else if add_period(s.current_period_end, &s.billing_cycle) > now {
        s.current_period_end
    } else {
        now
    };
    let end = add_period(start, &s.billing_cycle);
    let status = if s.status == "past_due" { "past_due" } else { "active" };

    sqlx::query(
        "UPDATE subscriptions SET plan_id = $2, status = $3, current_period_start = $4, current_period_end = $5,
                pending_plan_id = NULL, cancel_at_period_end = false, updated_at = now()
         WHERE organization_id = $1",
    )
    .bind(s.organization_id)
    .bind(&plan_id)
    .bind(status)
    .bind(start)
    .bind(end)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = now() WHERE id = $1")
        .bind(s.organization_id)
        .bind(&plan_id)
        .execute(&mut *tx)
        .await?;

    if plan.is_paid() {
        let seats = seats(tx, s.organization_id).await?;
        let lines = period_lines(&plan, &s.billing_cycle, seats);
        // The unique index makes a repeated renewal a no-op.
        sqlx::query("SAVEPOINT renew_invoice").execute(&mut *tx).await?;
        match issue_invoice(
            tx,
            &state.cfg.billing,
            &state.cfg.public_base_url,
            s.organization_id,
            "subscription",
            start,
            end,
            &lines,
        )
        .await
        {
            Ok(_) => {
                sqlx::query("RELEASE SAVEPOINT renew_invoice").execute(&mut *tx).await?;
            }
            Err(AppError::Database(sqlx::Error::Database(db))) if db.code().as_deref() == Some("23505") => {
                sqlx::query("ROLLBACK TO SAVEPOINT renew_invoice").execute(&mut *tx).await?;
            }
            Err(e) => return Err(anyhow::anyhow!("issuing renewal invoice: {e}")),
        }
    }
    sqlx::query(
        "INSERT INTO audit_logs (organization_id, actor_type, action, target_type, target_id, details)
         VALUES ($1, 'system', 'subscription.renewed', 'organization', $1::text, $2)",
    )
    .bind(s.organization_id)
    .bind(
        json!({ "plan": plan_id, "period_start": start, "period_end": end, "trial_ended": trial_ending, "note": note }),
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Move subscriptions with invoices overdue beyond the grace period to
/// `past_due`, and back to `active` once nothing is overdue.
pub async fn update_payment_status(state: &AppState) -> anyhow::Result<()> {
    let grace = state.cfg.billing.grace_days.clamp(0, 90) as i32;
    sqlx::query(
        "UPDATE subscriptions s SET status = 'past_due', updated_at = now()
         WHERE s.status IN ('active', 'trialing')
           AND EXISTS (SELECT 1 FROM invoices i WHERE i.organization_id = s.organization_id
                       AND i.status = 'open' AND i.due_at + make_interval(days => $1) < now())",
    )
    .bind(grace)
    .execute(&state.db)
    .await?;
    sqlx::query(
        "UPDATE subscriptions s SET status = 'active', updated_at = now()
         WHERE s.status = 'past_due'
           AND NOT EXISTS (SELECT 1 FROM invoices i WHERE i.organization_id = s.organization_id
                           AND i.status = 'open' AND i.due_at + make_interval(days => $1) < now())",
    )
    .bind(grace)
    .execute(&state.db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(monthly: i32, seat: i32, discount: i32) -> Plan {
        Plan {
            id: "p".into(),
            name: "Standard".into(),
            audience: "institution".into(),
            description: String::new(),
            daily_send_limit: 1,
            monthly_send_limit: 1,
            max_domains: 1,
            max_mailboxes: 1,
            default_mailbox_quota_bytes: 1,
            max_mailbox_quota_bytes: 1,
            max_recipients_per_request: 1,
            api_requests_per_minute: 1,
            price_monthly_cents: monthly,
            price_per_mailbox_cents: seat,
            annual_discount_percent: discount,
            max_members: 1,
            max_api_keys: 1,
            max_webhooks: 1,
            trial_days: 0,
            is_public: true,
            self_service: true,
            sort_order: 0,
        }
    }

    #[test]
    fn annual_pricing_applies_discount() {
        let p = plan(1900, 150, 15);
        assert_eq!(p.base_price("monthly"), 1900);
        assert_eq!(p.base_price("annual"), 19380); // 1900 * 12 * 0.85
        assert_eq!(p.seat_price("annual"), 1530);
    }

    #[test]
    fn period_lines_include_seats_only_when_priced() {
        let p = plan(1900, 150, 15);
        let lines = period_lines(&p, "monthly", 12);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].amount(), 1800);
        assert!(period_lines(&plan(0, 0, 0), "monthly", 5).is_empty());
        assert_eq!(period_lines(&plan(1200, 0, 0), "monthly", 5).len(), 1);
    }

    #[test]
    fn amounts_and_periods() {
        assert_eq!(format_amount("USD", 19380), "USD 193.80");
        assert_eq!(format_amount("KES", 5), "KES 0.05");
        let start = DateTime::parse_from_rfc3339("2026-01-31T10:00:00Z").unwrap().with_timezone(&Utc);
        assert_eq!(add_period(start, "monthly").format("%Y-%m-%d").to_string(), "2026-02-28");
        assert_eq!(add_period(start, "annual").format("%Y-%m-%d").to_string(), "2027-01-31");
    }
}
