//! Sending/receiving domains: registration, DNS instructions, verification.

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
    auth::{Actor, ClientInfo, Permission, Principal},
    crypto,
    error::{on_unique_violation, ApiResult, AppError},
    services::{
        audit::Entry,
        dkim,
        dns::{DomainCheck, ExpectedRecords, RecordCheck, OWNERSHIP_HOST_PREFIX, OWNERSHIP_VALUE_PREFIX},
        events,
        mail_client::{self, ClientSettings},
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/domains", get(list_domains).post(create_domain))
        .route("/domains/{id}", get(get_domain).patch(update_domain).delete(delete_domain))
        .route("/domains/{id}/dns-records", get(dns_records))
        .route("/domains/{id}/verification-check", post(verification_check))
}

#[derive(sqlx::FromRow)]
pub struct DomainRow {
    pub id: Uuid,
    pub domain_name: String,
    pub status: String,
    pub verification_token: String,
    pub verified_at: Option<DateTime<Utc>>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub check_count: i32,
    pub last_check_result: Option<Value>,
    pub spf_ok: bool,
    pub dkim_ok: bool,
    pub dmarc_ok: bool,
    pub mx_ok: bool,
    pub dkim_selector: String,
    pub dkim_public_key: String,
    pub mail_hosting_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub mailbox_count: i64,
}

const SELECT_DOMAIN: &str = "
    SELECT d.id, d.domain_name::text AS domain_name, d.status, d.verification_token, d.verified_at,
           d.last_checked_at, d.check_count, d.last_check_result, d.spf_ok, d.dkim_ok, d.dmarc_ok, d.mx_ok,
           d.dkim_selector, d.dkim_public_key, d.mail_hosting_enabled, d.created_at,
           (SELECT count(*) FROM mailboxes m WHERE m.domain_id = d.id AND m.status <> 'deleted') AS mailbox_count
    FROM domains d";

async fn load(state: &AppState, org_id: Uuid, id: Uuid) -> ApiResult<DomainRow> {
    sqlx::query_as(&format!("{SELECT_DOMAIN} WHERE d.id = $1 AND d.organization_id = $2"))
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Domain"))
}

fn record(kind: &str, host: &str, value: String, purpose: &str, required: bool, ok: Option<bool>) -> Value {
    json!({
        "type": kind, "host": host, "value": value, "purpose": purpose,
        "required": required, "status": match ok { Some(true) => "verified", Some(false) => "missing", None => "unchecked" },
    })
}

pub fn dns_records_for(state: &AppState, d: &DomainRow, client: &ClientSettings) -> Vec<Value> {
    let checked = d.last_checked_at.is_some();
    let ok = |v: bool| checked.then_some(v);
    let ownership_ok = d
        .last_check_result
        .as_ref()
        .and_then(|r| r.pointer("/ownership/ok"))
        .and_then(Value::as_bool)
        .unwrap_or(d.status == "verified");
    let spf_mechanism = if state.cfg.spf_include.contains(':') {
        state.cfg.spf_include.clone()
    } else {
        format!("include:{}", state.cfg.spf_include)
    };
    let mut records = vec![
        record(
            "TXT",
            &format!("{OWNERSHIP_HOST_PREFIX}.{}", d.domain_name),
            format!("{OWNERSHIP_VALUE_PREFIX}{}", d.verification_token),
            "Proves you control this domain",
            true,
            ok(ownership_ok),
        ),
        record(
            "TXT",
            &format!("{}._domainkey.{}", d.dkim_selector, d.domain_name),
            dkim::dns_record_value(&d.dkim_public_key),
            "DKIM public key; lets receivers verify our signature on your mail",
            true,
            ok(d.dkim_ok),
        ),
        record(
            "TXT",
            &d.domain_name,
            format!("v=spf1 {spf_mechanism} ~all"),
            "SPF; merge into your existing SPF record if you already have one",
            false,
            ok(d.spf_ok),
        ),
        record(
            "TXT",
            &format!("_dmarc.{}", d.domain_name),
            format!("v=DMARC1; p=none; rua=mailto:dmarc-reports@{}", d.domain_name),
            "DMARC policy; start with p=none and tighten after monitoring reports",
            false,
            ok(d.dmarc_ok),
        ),
        record(
            "MX",
            &d.domain_name,
            format!("10 {}", state.cfg.mail_hostname),
            "Routes inbound mail to hosted mailboxes (needed only for mailboxes)",
            d.mail_hosting_enabled,
            ok(d.mx_ok),
        ),
    ];
    if d.mail_hosting_enabled {
        records.extend(client_discovery_records(state, d, client));
    }
    records
}

/// Optional records that help email apps find the mailbox settings by
/// themselves (RFC 6186 SRV; autoconfig/autodiscover host names). Most apps
/// also discover settings through the MX record, so none of these is
/// required; they are never part of verification.
fn client_discovery_records(state: &AppState, d: &DomainRow, c: &ClientSettings) -> Vec<Value> {
    let optional = |kind: &str, host: String, value: String, purpose: &str| {
        let mut r = record(kind, &host, value, purpose, false, None);
        r["status"] = json!("optional");
        r["group"] = json!("client_setup");
        r
    };
    let mut out = vec![optional(
        "SRV",
        format!("_imaps._tcp.{}", d.domain_name),
        format!("0 1 {} {}.", c.imap_port, c.imap_host),
        "Mail apps: incoming mail (IMAP over TLS) for automatic setup",
    )];
    let submission = |port: u16, tls: crate::config::TlsMode| {
        let (service, label) = if mail_client::security_code(tls) == "tls" {
            ("_submissions", "SMTP over TLS")
        } else {
            ("_submission", "SMTP with STARTTLS")
        };
        optional(
            "SRV",
            format!("{service}._tcp.{}", d.domain_name),
            format!("0 1 {port} {}.", c.smtp_host),
            &format!("Mail apps: outgoing mail ({label}) for automatic setup"),
        )
    };
    out.push(submission(c.smtp_port, c.smtp_tls));
    if let Some((port, tls)) = c.smtp_alt {
        out.push(submission(port, tls));
    }
    out.push(optional(
        "SRV",
        format!("_imap._tcp.{}", d.domain_name),
        "0 0 0 .".into(),
        "Tells apps that unencrypted IMAP is not offered (RFC 6186)",
    ));
    out.push(optional(
        "SRV",
        format!("_pop3._tcp.{}", d.domain_name),
        "0 0 0 .".into(),
        "Tells apps that POP3 is not offered (RFC 6186)",
    ));
    out.push(optional(
        "CNAME",
        format!("autoconfig.{}", d.domain_name),
        format!("{}.", state.cfg.autoconfig_host),
        "Thunderbird-style automatic setup (only if your web certificate covers this name)",
    ));
    out
}

async fn domain_json(state: &AppState, d: &DomainRow) -> ApiResult<Value> {
    let client = mail_client::effective(state, Some(d.id)).await?;
    Ok(json!({
        "id": d.id, "domain": d.domain_name, "status": d.status,
        "sending_enabled": d.status == "verified",
        "mail_hosting_enabled": d.mail_hosting_enabled,
        "verified_at": d.verified_at, "last_checked_at": d.last_checked_at, "check_count": d.check_count,
        "checks": { "spf": d.spf_ok, "dkim": d.dkim_ok, "dmarc": d.dmarc_ok, "mx": d.mx_ok },
        "dkim_selector": d.dkim_selector,
        "mailbox_count": d.mailbox_count,
        "created_at": d.created_at,
        "dns_records": dns_records_for(state, d, &client),
        "last_check": d.last_check_result,
        "client_settings": client.to_json(None),
    }))
}

async fn list_domains(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsRead)?;
    let rows: Vec<DomainRow> =
        sqlx::query_as(&format!("{SELECT_DOMAIN} WHERE d.organization_id = $1 ORDER BY d.domain_name"))
            .bind(p.org_id)
            .fetch_all(&state.db)
            .await?;
    let data: Vec<Value> = rows
        .iter()
        .map(|d| {
            json!({
                "id": d.id, "domain": d.domain_name, "status": d.status, "sending_enabled": d.status == "verified",
                "mail_hosting_enabled": d.mail_hosting_enabled, "verified_at": d.verified_at,
                "last_checked_at": d.last_checked_at,
                "checks": { "spf": d.spf_ok, "dkim": d.dkim_ok, "dmarc": d.dmarc_ok, "mx": d.mx_ok },
                "mailbox_count": d.mailbox_count, "created_at": d.created_at,
            })
        })
        .collect();
    Ok(Json(json!({ "data": data })))
}

#[derive(Deserialize)]
struct CreateDomainBody {
    domain: String,
}

async fn create_domain(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateDomainBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::DomainsWrite)?;
    if let Actor::User { email_verified: false, .. } = p.actor {
        return Err(AppError::forbidden("Confirm your email address before adding domains")
            .with_details(json!({ "reason": "email_unverified" })));
    }
    let name = validation::normalize_domain(&body.domain)?;
    if name == state.cfg.return_path_domain || name.ends_with(&format!(".{}", state.cfg.return_path_domain)) {
        return Err(AppError::validation("This domain is reserved by the platform"));
    }

    let (count, limit): (i64, i32) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM domains WHERE organization_id = $1), p.max_domains
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if count >= limit as i64 {
        return Err(AppError::quota_exceeded(format!("Your plan allows {limit} domains")));
    }
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM domains WHERE domain_name = $1 AND status = 'verified' AND organization_id <> $2)",
    )
    .bind(&name)
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if claimed {
        return Err(AppError::conflict("domain_claimed", "This domain is already verified by another organization"));
    }

    let keys = dkim::generate().await?;
    let selector = format!("{}{}", state.cfg.dkim_selector_prefix, Utc::now().format("%Y%m"));
    let token = crypto::random_lower_alnum(32);

    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO domains (organization_id, domain_name, verification_token, dkim_selector, dkim_private_key_enc, dkim_public_key)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(p.org_id)
    .bind(&name)
    .bind(&token)
    .bind(&selector)
    .bind(dkim::seal(&state.secrets, &keys.private_pem))
    .bind(&keys.public_b64)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| on_unique_violation(e, "domain_exists", "This domain is already added to your organization"))?;
    Entry::by(&p, &client, "domain.created")
        .target("domain", id)
        .details(json!({ "domain": name }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let d = load(&state, p.org_id, id).await?;
    Ok((StatusCode::CREATED, Json(domain_json(&state, &d).await?)))
}

async fn get_domain(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsRead)?;
    let d = load(&state, p.org_id, id).await?;
    Ok(Json(domain_json(&state, &d).await?))
}

async fn dns_records(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsRead)?;
    let d = load(&state, p.org_id, id).await?;
    let client = mail_client::effective(&state, Some(d.id)).await?;
    Ok(Json(json!({ "domain": d.domain_name, "records": dns_records_for(&state, &d, &client) })))
}

fn passing(message: &str) -> RecordCheck {
    RecordCheck { ok: true, observed: vec![], message: message.to_string() }
}

async fn verification_check(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsWrite)?;
    let d = load(&state, p.org_id, id).await?;
    state.limiter.check(&format!("domain-check:{id}"), 10, 600)?;

    let check: DomainCheck = if state.cfg.dev_skip_dns_verification && !state.cfg.is_production() {
        tracing::warn!(domain = %d.domain_name, "DEV_SKIP_DNS_VERIFICATION: DNS checks bypassed");
        let msg = "Development override: DNS not queried";
        DomainCheck {
            ownership: passing(msg),
            dkim: passing(msg),
            spf: passing(msg),
            dmarc: passing(msg),
            mx: passing(msg),
            checked_at: Utc::now(),
            development_override: true,
        }
    } else {
        state
            .dns
            .check(&ExpectedRecords {
                domain: &d.domain_name,
                verification_token: &d.verification_token,
                dkim_selector: &d.dkim_selector,
                dkim_public_key: &d.dkim_public_key,
                spf_include: &state.cfg.spf_include,
                mail_hostname: &state.cfg.mail_hostname,
            })
            .await
    };

    let verified_now = check.verified();
    let new_status = if verified_now {
        "verified"
    } else if d.status == "verified" {
        "failed"
    } else {
        "pending"
    };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE domains SET status = $3, verified_at = CASE WHEN $3 = 'verified' THEN COALESCE(verified_at, now()) ELSE verified_at END,
                last_checked_at = now(), check_count = check_count + 1, last_check_result = $4,
                spf_ok = $5, dkim_ok = $6, dmarc_ok = $7, mx_ok = $8, updated_at = now()
         WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(p.org_id)
    .bind(new_status)
    .bind(serde_json::to_value(&check).map_err(AppError::internal)?)
    .bind(check.spf.ok)
    .bind(check.dkim.ok)
    .bind(check.dmarc.ok)
    .bind(check.mx.ok)
    .execute(&mut *tx)
    .await
    .map_err(|e| on_unique_violation(e, "domain_claimed", "This domain was verified by another organization"))?;

    Entry::by(&p, &client, "domain.verification_checked")
        .target("domain", id)
        .details(json!({ "result": new_status, "ownership": check.ownership.ok, "dkim": check.dkim.ok, "spf": check.spf.ok, "dmarc": check.dmarc.ok, "mx": check.mx.ok, "development_override": check.development_override }))
        .record(&mut *tx)
        .await?;
    if verified_now && d.status != "verified" {
        events::emit(&mut tx, p.org_id, "domain.verified", json!({ "domain_id": id, "domain": d.domain_name })).await?;
    } else if !verified_now && d.status == "verified" {
        events::emit(
            &mut tx,
            p.org_id,
            "domain.verification_failed",
            json!({ "domain_id": id, "domain": d.domain_name, "reason": "required DNS records no longer found" }),
        )
        .await?;
    }
    tx.commit().await?;

    let d = load(&state, p.org_id, id).await?;
    let mut out = domain_json(&state, &d).await?;
    out["check"] = serde_json::to_value(&check).map_err(AppError::internal)?;
    Ok(Json(out))
}

#[derive(Deserialize)]
struct UpdateDomainBody {
    mail_hosting_enabled: Option<bool>,
}

async fn update_domain(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<UpdateDomainBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsWrite)?;
    let d = load(&state, p.org_id, id).await?;
    if let Some(enabled) = body.mail_hosting_enabled {
        if enabled {
            p.require_institution()?;
        }
        if enabled && d.status != "verified" {
            return Err(AppError::conflict(
                "domain_not_verified",
                "Verify the domain before enabling hosted mailboxes",
            ));
        }
        let mut tx = state.db.begin().await?;
        sqlx::query(
            "UPDATE domains SET mail_hosting_enabled = $3, updated_at = now() WHERE id = $1 AND organization_id = $2",
        )
        .bind(id)
        .bind(p.org_id)
        .bind(enabled)
        .execute(&mut *tx)
        .await?;
        Entry::by(&p, &client, if enabled { "domain.mail_hosting_enabled" } else { "domain.mail_hosting_disabled" })
            .target("domain", id)
            .record(&mut *tx)
            .await?;
        tx.commit().await?;
        if !enabled && d.mail_hosting_enabled {
            let addresses: Vec<String> =
                sqlx::query_scalar("SELECT address::text FROM mailboxes WHERE domain_id = $1 AND status = 'active'")
                    .bind(id)
                    .fetch_all(&state.db)
                    .await?;
            crate::services::doveadm::revoke_access(&state, addresses).await;
        }
    }
    let d = load(&state, p.org_id, id).await?;
    Ok(Json(domain_json(&state, &d).await?))
}

async fn delete_domain(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::DomainsWrite)?;
    let d = load(&state, p.org_id, id).await?;
    let in_use: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM mailboxes WHERE domain_id = $1 AND status <> 'deleted'),
                (SELECT count(*) FROM aliases WHERE domain_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if in_use.0 > 0 || in_use.1 > 0 {
        return Err(AppError::conflict(
            "domain_in_use",
            format!("Remove this domain's {} mailboxes and {} aliases first", in_use.0, in_use.1),
        ));
    }
    let mut tx = state.db.begin().await?;
    // Deleted mailboxes keep their row for audit; detach them by removing
    // their records once the retention window has passed (see runbook).
    sqlx::query("DELETE FROM mailboxes WHERE domain_id = $1 AND status = 'deleted'").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM domains WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(p.org_id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "domain.deleted")
        .target("domain", id)
        .details(json!({ "domain": d.domain_name }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "deleted": true })))
}

/// Helper for other modules: resolve a verified sending domain by name.
pub async fn verified_domain_id(state: &AppState, org_id: Uuid, domain: &str) -> ApiResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM domains WHERE organization_id = $1 AND domain_name = $2 AND status = 'verified'",
    )
    .bind(org_id)
    .bind(domain)
    .fetch_optional(&state.db)
    .await?)
}
