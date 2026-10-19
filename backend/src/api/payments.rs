//! M-Pesa payment of invoices through PesaTide.
//!
//! * `POST /v1/billing/invoices/{id}/pay`    start an STK push to the payer's phone
//! * `GET  /v1/billing/payments/{id}`        current status (reconciles with PesaTide while pending)
//! * `POST /v1/payments/pesatide/webhook`    signed result callback (public, HMAC-verified)
//!
//! A payment succeeds only on a `SUCCESS` status, from the webhook or a
//! status query; the invoice is then marked paid with the M-Pesa receipt and
//! the subscription reactivated. Every transition is idempotent.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{ClientInfo, Permission, Principal},
    crypto,
    error::{ApiResult, AppError},
    services::{
        audit::Entry,
        billing,
        pesatide::{self, StkRequest, Transaction, WebhookEvent},
        sms,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/invoices/{id}/pay", post(pay_invoice))
        .route("/billing/payments/{id}", get(get_payment))
        .route("/payments/pesatide/webhook", post(webhook))
}

#[derive(sqlx::FromRow)]
struct PaymentRow {
    id: Uuid,
    invoice_id: Uuid,
    reference: String,
    provider_id: Option<String>,
    phone: String,
    amount_cents: i64,
    currency: String,
    status: String,
    receipt: Option<String>,
    failure_reason: Option<String>,
    created_at: chrono::DateTime<Utc>,
    completed_at: Option<chrono::DateTime<Utc>>,
}

const PAYMENT_COLUMNS: &str =
    "id, invoice_id, reference, provider_id, phone, amount_cents, currency, status, receipt, failure_reason, created_at, completed_at";

fn payment_json(p: &PaymentRow) -> Value {
    json!({
        "id": p.id, "invoice_id": p.invoice_id, "reference": p.reference, "phone": sms::mask(&p.phone),
        "amount_cents": p.amount_cents, "currency": p.currency, "status": p.status, "receipt": p.receipt,
        "failure_reason": p.failure_reason, "created_at": p.created_at, "completed_at": p.completed_at,
        "final": !matches!(p.status.as_str(), "pending" | "processing"),
    })
}

#[derive(Deserialize)]
struct PayBody {
    phone: String,
}

async fn pay_invoice(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(invoice_id): Path<Uuid>,
    ApiJson(b): ApiJson<PayBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::BillingManage)?;
    let cfg = state.cfg.integrations.pesatide.clone().ok_or_else(|| {
        AppError::unavailable(
            "payments_not_configured",
            "M-Pesa payments are not enabled yet. Use the payment instructions on the invoice.",
        )
    })?;
    state.limiter.check(&format!("pay:{}", p.org_id), 6, 600)?;
    let phone = sms::normalize(&b.phone)?;
    if !phone.starts_with("+254") {
        return Err(AppError::validation("M-Pesa needs a Kenyan Safaricom number"));
    }

    let (number, total, currency, status): (String, i64, String, String) = sqlx::query_as(
        "SELECT number, total_cents, currency, status FROM invoices WHERE id = $1 AND organization_id = $2",
    )
    .bind(invoice_id)
    .bind(p.org_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Invoice"))?;
    if status != "open" {
        return Err(AppError::conflict("invoice_not_open", "This invoice is not open for payment"));
    }
    if currency != "KES" {
        return Err(AppError::conflict(
            "unsupported_currency",
            "M-Pesa payments are only available for invoices in KES",
        ));
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM payments WHERE invoice_id = $1 AND status IN ('pending', 'processing')
                        AND created_at > now() - interval '3 minutes')",
    )
    .bind(invoice_id)
    .fetch_one(&state.db)
    .await?
    {
        return Err(AppError::conflict(
            "payment_in_progress",
            "A payment prompt was just sent. Approve it on your phone or wait a few minutes.",
        ));
    }

    // Whole shillings: M-Pesa does not accept cents, so round up.
    let amount_kes = (total + 99) / 100;
    let reference = format!("MV-{}-{}", number, crypto::random_lower_alnum(6)).to_uppercase();
    let row: PaymentRow = sqlx::query_as(&format!(
        "INSERT INTO payments (organization_id, invoice_id, reference, phone, amount_cents, currency, initiated_by)
         VALUES ($1, $2, $3, $4, $5, 'KES', $6) RETURNING {PAYMENT_COLUMNS}"
    ))
    .bind(p.org_id)
    .bind(invoice_id)
    .bind(&reference)
    .bind(&phone)
    .bind(amount_kes * 100)
    .bind(p.user_id())
    .fetch_one(&state.db)
    .await?;

    let description = format!("{} invoice {number}", state.cfg.product_name);
    let pushed = pesatide::stk_push(
        &cfg,
        &StkRequest {
            reference: &reference,
            amount_kes,
            phone_e164: &phone,
            account_reference: &number,
            description: &description,
        },
    )
    .await;
    let tx = match pushed {
        Ok(tx) => tx,
        Err(e) => {
            tracing::warn!(error = %e, payment = %row.id, "STK push failed");
            sqlx::query("UPDATE payments SET status = 'failed', failure_reason = $2, updated_at = now(), completed_at = now() WHERE id = $1")
                .bind(row.id)
                .bind("The payment prompt could not be sent")
                .execute(&state.db)
                .await?;
            return Err(AppError::unavailable(
                "payment_failed",
                "The M-Pesa prompt could not be sent. Check the number and try again.",
            ));
        }
    };
    let row = apply(&state, row.id, &tx).await?;
    Entry::by(&p, &client, "payment.initiated")
        .target("invoice", invoice_id)
        .details(
            json!({ "number": number, "amount_kes": amount_kes, "reference": reference, "phone": sms::mask(&phone) }),
        )
        .record(&state.db)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(payment_json(&row))))
}

async fn get_payment(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::BillingRead)?;
    let row: PaymentRow =
        sqlx::query_as(&format!("SELECT {PAYMENT_COLUMNS} FROM payments WHERE id = $1 AND organization_id = $2"))
            .bind(id)
            .bind(p.org_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::not_found("Payment"))?;
    let pending = matches!(row.status.as_str(), "pending" | "processing");
    if let (true, Some(cfg), Some(provider_id)) =
        (pending, &state.cfg.integrations.pesatide, row.provider_id.as_deref())
    {
        // Reconcile at most every few seconds per payment while the person waits.
        if state.limiter.check(&format!("pay-status:{id}"), 1, 4).is_ok() {
            match pesatide::fetch(cfg, provider_id).await {
                Ok(tx) => return Ok(Json(payment_json(&apply(&state, row.id, &tx).await?))),
                Err(e) => tracing::warn!(error = %e, payment = %id, "payment status query failed"),
            }
        }
        // Prompts expire after about a minute; give up after ten.
        if row.created_at < Utc::now() - chrono::Duration::minutes(10) {
            let expired = Transaction {
                provider_id: provider_id.to_string(),
                status: "expired",
                receipt: None,
                reason: Some("The payment prompt expired".into()),
            };
            return Ok(Json(payment_json(&apply(&state, row.id, &expired).await?)));
        }
    }
    Ok(Json(payment_json(&row)))
}

async fn webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult<StatusCode> {
    let Some(cfg) = &state.cfg.integrations.pesatide else {
        return Err(AppError::not_found("Webhook"));
    };
    let signature = headers.get("x-pesatide-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !pesatide::verify_signature(&cfg.webhook_secret, &body, signature) {
        tracing::warn!("rejected PesaTide webhook with an invalid signature");
        return Err(AppError::new(StatusCode::UNAUTHORIZED, "invalid_signature", "Invalid webhook signature"));
    }
    let event: WebhookEvent =
        serde_json::from_slice(&body).map_err(|_| AppError::validation("Malformed webhook body"))?;
    let Some(tx) = event.transaction() else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM payments WHERE provider_id = $1 OR reference = $2 LIMIT 1")
            .bind(&tx.provider_id)
            .bind(event.reference().unwrap_or(""))
            .fetch_optional(&state.db)
            .await?;
    match id {
        Some(id) => {
            apply(&state, id, &tx).await?;
            tracing::info!(event = %event.event, payment = %id, status = tx.status, "PesaTide webhook applied");
        }
        None => {
            tracing::warn!(event = %event.event, provider_id = %tx.provider_id, "PesaTide webhook for an unknown payment")
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Record a gateway result. Final states are never overwritten; success marks
/// the invoice paid and lifts a past-due subscription.
async fn apply(state: &AppState, id: Uuid, tx: &Transaction) -> ApiResult<PaymentRow> {
    let mut db = state.db.begin().await?;
    let row: PaymentRow = sqlx::query_as(&format!(
        "UPDATE payments SET
            provider_id = COALESCE(provider_id, $2),
            status = CASE WHEN status IN ('pending', 'processing') THEN $3 ELSE status END,
            receipt = COALESCE($4, receipt),
            failure_reason = CASE WHEN status IN ('pending', 'processing') AND $3 IN ('failed', 'cancelled', 'expired') THEN $5 ELSE failure_reason END,
            completed_at = CASE WHEN status IN ('pending', 'processing') AND $3 NOT IN ('pending', 'processing') THEN now() ELSE completed_at END,
            updated_at = now()
         WHERE id = $1 RETURNING {PAYMENT_COLUMNS}"
    ))
    .bind(id)
    .bind(&tx.provider_id)
    .bind(tx.status)
    .bind(&tx.receipt)
    .bind(&tx.reason)
    .fetch_one(&mut *db)
    .await?;

    let mut newly_paid = false;
    if row.status == "succeeded" {
        let paid: Option<(Uuid, String)> = sqlx::query_as(
            "UPDATE invoices SET status = 'paid', paid_at = now(), payment_method = 'mpesa', payment_reference = $2
             WHERE id = $1 AND status = 'open' RETURNING organization_id, number",
        )
        .bind(row.invoice_id)
        .bind(row.receipt.as_deref().unwrap_or(&row.reference))
        .fetch_optional(&mut *db)
        .await?;
        if let Some((org, number)) = paid {
            newly_paid = true;
            sqlx::query(
                "INSERT INTO audit_logs (organization_id, actor_type, action, target_type, target_id, details)
                 VALUES ($1, 'system', 'invoice.paid', 'invoice', $2, $3)",
            )
            .bind(org)
            .bind(row.invoice_id.to_string())
            .bind(json!({ "number": number, "method": "mpesa", "receipt": row.receipt, "payment": row.id }))
            .execute(&mut *db)
            .await?;
        }
    }
    db.commit().await?;
    if newly_paid {
        billing::update_payment_status(state).await.map_err(AppError::Internal)?;
    }
    Ok(row)
}
