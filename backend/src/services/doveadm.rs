//! Immediate access revocation in Dovecot through its doveadm HTTP API.
//!
//! The mail views drop a mailbox as soon as it is disabled, deleted or its
//! tenant suspended, but Dovecot caches successful authentications and
//! keeps already-open IMAP sessions alive. After such a change commits we
//! flush the auth cache for the affected users and kick their sessions, so
//! the change takes effect immediately (guide 04: disablement; guide 07:
//! revocation).
//!
//! Best effort: failures are logged and never undo the committed change;
//! Dovecot's short auth-cache TTL bounds the window if the call fails.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};

use crate::state::AppState;

/// EX_NOUSER from doveadm kick: the user simply had no open sessions.
const EX_NOUSER: i64 = 68;

pub async fn revoke_access(state: &AppState, addresses: Vec<String>) {
    let Some(cfg) = state.cfg.doveadm.as_ref() else {
        return;
    };
    if addresses.is_empty() {
        return;
    }
    let commands = json!([
        ["authCacheFlush", { "user": addresses }, "flush"],
        ["kick", { "mask": addresses }, "kick"],
    ]);
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "doveadm client setup failed");
            return;
        }
    };
    let result = client
        .post(format!("{}/doveadm/v1", cfg.url.trim_end_matches('/')))
        .header("Authorization", format!("X-Dovecot-API {}", B64.encode(&cfg.api_key)))
        .json(&commands)
        .send()
        .await;
    match result {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            for entry in body.as_array().into_iter().flatten() {
                let kind = entry.get(0).and_then(Value::as_str).unwrap_or("");
                let code = entry.get(1).and_then(|v| v.get("exitCode")).and_then(Value::as_i64);
                if kind == "error" && code != Some(EX_NOUSER) {
                    tracing::warn!(response = %entry, "doveadm command failed");
                }
            }
            tracing::info!(users = addresses.len(), "mail access revoked in Dovecot");
        }
        Ok(resp) => tracing::warn!(status = %resp.status(), "doveadm API rejected the request"),
        Err(e) => tracing::warn!(error = %e, "doveadm API unreachable; auth cache TTL bounds the delay"),
    }
}

/// All mailbox addresses (including tombstoned originals) of a tenant.
pub async fn tenant_addresses(state: &AppState, org_id: uuid::Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT COALESCE(deleted_address, address)::text FROM mailboxes WHERE organization_id = $1")
        .bind(org_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
}
