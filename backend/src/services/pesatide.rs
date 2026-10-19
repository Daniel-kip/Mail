//! PesaTide M-Pesa gateway: STK push collection, status queries and signed
//! webhooks (<https://pesatide.com/api-guide>).
//!
//! Flow: create a local `payments` row → `POST /payments/stk-push` with an
//! `Idempotency-Key` → the customer approves the prompt on their phone →
//! PesaTide calls our webhook (`X-PesaTide-Signature` = hex HMAC-SHA256 of
//! the raw body). `GET /payments/{id}` reconciles when a webhook is late.
//! `PROCESSING` only means the prompt was accepted; only `SUCCESS` pays.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{config::PesatideConfig, crypto};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub provider_id: String,
    /// Local status: pending | processing | succeeded | failed | cancelled | expired.
    pub status: &'static str,
    pub receipt: Option<String>,
    pub reason: Option<String>,
}

/// Map a PesaTide status to the local payment status.
pub fn local_status(remote: &str) -> &'static str {
    match remote.to_ascii_uppercase().as_str() {
        "SUCCESS" | "SUCCEEDED" | "COMPLETED" => "succeeded",
        "FAILED" => "failed",
        "CANCELLED" | "CANCELED" => "cancelled",
        "EXPIRED" | "TIMEOUT" => "expired",
        "PROCESSING" => "processing",
        _ => "pending",
    }
}

/// `+254712345678` → `254712345678` (the gateway's MSISDN format).
pub fn msisdn(e164: &str) -> String {
    e164.trim_start_matches('+').to_string()
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("Mailvora/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("static reqwest configuration")
}

fn parse_transaction(v: &Value) -> Option<Transaction> {
    let data = if v.get("data").is_some_and(Value::is_object) { &v["data"] } else { v };
    let provider_id = data["transactionRequestId"].as_str().or(data["id"].as_str())?.to_string();
    Some(Transaction {
        provider_id,
        status: local_status(data["status"].as_str().unwrap_or("PENDING")),
        receipt: data["mpesaReceipt"].as_str().map(str::to_string),
        reason: data["resultDesc"].as_str().or(data["reason"].as_str()).map(|s| s.chars().take(300).collect()),
    })
}

fn error_message(status: reqwest::StatusCode, body: &Value) -> String {
    let msg = body["error"]["message"].as_str().or(body["message"].as_str()).or(body["error"].as_str());
    format!("PesaTide returned HTTP {}{}", status.as_u16(), msg.map(|m| format!(": {m}")).unwrap_or_default())
}

pub struct StkRequest<'a> {
    pub reference: &'a str,
    pub amount_kes: i64,
    pub phone_e164: &'a str,
    pub account_reference: &'a str,
    pub description: &'a str,
}

pub async fn stk_push(cfg: &PesatideConfig, req: &StkRequest<'_>) -> anyhow::Result<Transaction> {
    let mut body = json!({
        "reference": req.reference,
        "amount": req.amount_kes,
        "phone": msisdn(req.phone_e164),
        "accountReference": req.account_reference,
        "description": req.description,
    });
    if let Some(route) = &cfg.route_id {
        body["routeId"] = json!(route);
    }
    let res = client()
        .post(format!("{}/payments/stk-push", cfg.api_url))
        .header("x-api-key", &cfg.api_key)
        .header("x-api-secret", &cfg.api_secret)
        .header("Idempotency-Key", req.reference)
        .json(&body)
        .send()
        .await?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        anyhow::bail!(error_message(status, &v));
    }
    parse_transaction(&v).ok_or_else(|| anyhow::anyhow!("PesaTide response had no transactionRequestId"))
}

pub async fn fetch(cfg: &PesatideConfig, provider_id: &str) -> anyhow::Result<Transaction> {
    let res = client()
        .get(format!("{}/payments/{}", cfg.api_url, urlencode(provider_id)))
        .header("x-api-key", &cfg.api_key)
        .header("x-api-secret", &cfg.api_secret)
        .send()
        .await?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        anyhow::bail!(error_message(status, &v));
    }
    parse_transaction(&v).ok_or_else(|| anyhow::anyhow!("PesaTide response had no transactionRequestId"))
}

fn urlencode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// Constant-time check of `X-PesaTide-Signature` against the raw body.
pub fn verify_signature(secret: &str, body: &[u8], header: &str) -> bool {
    let expected = hex::encode(crypto::hmac_sha256(secret.as_bytes(), body));
    let received = header.trim().trim_start_matches("sha256=").to_ascii_lowercase();
    crypto::constant_time_eq(expected.as_bytes(), received.as_bytes())
}

#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub event: String,
    pub data: Value,
}

impl WebhookEvent {
    pub fn transaction(&self) -> Option<Transaction> {
        parse_transaction(&self.data)
    }

    pub fn reference(&self) -> Option<&str> {
        self.data["reference"].as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_status_mapping() {
        let body = br#"{"event":"payment.success","data":{"transactionRequestId":"57c2","status":"SUCCESS","mpesaReceipt":"TGR7ABC123"}}"#;
        let sig = hex::encode(crypto::hmac_sha256(b"whsec", body));
        assert!(verify_signature("whsec", body, &sig));
        assert!(verify_signature("whsec", body, &sig.to_uppercase()));
        assert!(!verify_signature("other", body, &sig));
        let ev: WebhookEvent = serde_json::from_slice(body).unwrap();
        let tx = ev.transaction().unwrap();
        assert_eq!(
            (tx.provider_id.as_str(), tx.status, tx.receipt.as_deref()),
            ("57c2", "succeeded", Some("TGR7ABC123"))
        );
        assert_eq!(local_status("PROCESSING"), "processing");
        assert_eq!(local_status("CANCELLED"), "cancelled");
        assert_eq!(msisdn("+254712345678"), "254712345678");
    }
}
