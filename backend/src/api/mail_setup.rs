//! "Connect my work email to my phone": the mailbox user's setup API.
//!
//! All endpoints are authenticated by the webmail session, which is bound to
//! exactly one mailbox, so the mailbox is always derived from the credential
//! (tenant isolation by construction). No endpoint accepts a hostname, port,
//! URL or mailbox id: readiness probes only reach the public endpoints the
//! operator configured (no SSRF surface).
//!
//! Three levels of assurance are kept distinct, as the UX requires:
//! 1. *Readiness*: the platform can resolve and reach its own endpoints and
//!    their certificates validate. Says nothing about the phone.
//! 2. *Instructions completed*: the employee finished a guide (analytics).
//! 3. *Connection verified*: proven by the employee from the phone:
//!    - incoming: a short code is placed in the inbox (IMAP APPEND) and the
//!      employee types the code they see in their phone's mail app;
//!    - outgoing: the employee sends a message from the phone to their own
//!      address with a code in the subject; the platform finds it in the
//!      inbox, proving authenticated submission through the mail server.
//!
//! The mailbox password is never requested, stored, logged, embedded in a
//! QR code, a link or the Apple configuration profile.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::MailboxSession,
    config::TlsMode,
    error::{ApiResult, AppError},
    mail::mime::{self, Addr, OutgoingEmail},
    services::{
        imap,
        mail_client::{self, ClientSettings},
        system_mail::escape_html,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_info))
        .route("/setup/readiness", get(setup_readiness))
        .route("/setup/events", post(record_event))
        .route("/setup/verify/incoming", post(start_incoming))
        .route("/setup/verify/incoming/confirm", post(confirm_incoming))
        .route("/setup/verify/outgoing", post(start_outgoing))
        .route("/setup/verify/outgoing/check", post(check_outgoing))
        .route("/setup/apple-profile", get(apple_profile))
}

const DEVICES: &[&str] = &["android", "ios", "computer", "other"];
const CLIENTS: &[&str] = &["gmail", "apple_mail", "outlook", "other", "webmail"];

#[derive(sqlx::FromRow)]
struct Context {
    mailbox_status: String,
    domain_status: String,
    hosting_enabled: bool,
    org_status: String,
    org_name: String,
}

async fn context(state: &AppState, s: &MailboxSession) -> ApiResult<Context> {
    Ok(sqlx::query_as(
        "SELECT m.status AS mailbox_status, d.status AS domain_status, d.mail_hosting_enabled AS hosting_enabled,
                o.status AS org_status, o.name AS org_name
         FROM mailboxes m JOIN domains d ON d.id = m.domain_id JOIN organizations o ON o.id = m.organization_id
         WHERE m.id = $1 AND m.organization_id = $2",
    )
    .bind(s.mailbox_id)
    .bind(s.organization_id)
    .fetch_one(&state.db)
    .await?)
}

async fn verification_status(state: &AppState, mailbox_id: Uuid) -> ApiResult<Value> {
    let rows: Vec<(String, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        "SELECT direction, max(verified_at) FROM mailbox_connection_checks
         WHERE mailbox_id = $1 AND status = 'verified' GROUP BY direction",
    )
    .bind(mailbox_id)
    .fetch_all(&state.db)
    .await?;
    let get = |d: &str| rows.iter().find(|(dir, _)| dir == d).and_then(|(_, t)| *t);
    let (incoming, outgoing) = (get("incoming"), get("outgoing"));
    Ok(json!({
        "incoming_verified_at": incoming,
        "outgoing_verified_at": outgoing,
        "state": match (incoming.is_some(), outgoing.is_some()) {
            (true, true) => "verified",
            (false, false) => "not_verified",
            _ => "partially_verified",
        },
    }))
}

async fn setup_info(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    let c = context(&state, &s).await?;
    let settings = mail_client::effective(&state, Some(s.domain_id)).await?;
    let domain = s.address.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    Ok(Json(json!({
        "mailbox": { "address": s.address, "display_name": s.display_name, "status": c.mailbox_status,
                     "quota_bytes": s.quota_bytes, "organization": c.org_name, "domain": domain },
        "domain": { "status": c.domain_status, "mail_hosting_enabled": c.hosting_enabled },
        "organization_status": c.org_status,
        "settings": settings.to_json(Some(&s.address)),
        "webmail_url": format!("{}/mail", state.cfg.public_base_url),
        "support": super::organizations::support_settings(&state, s.organization_id).await?,
        "verification": verification_status(&state, s.mailbox_id).await?,
        "automation": {
            "account_import": false,
            "apple_profile": true,
            "thunderbird_autoconfig": true,
            "note": "Email apps decide whether they can import settings. This guide never transfers or displays your password.",
        },
    })))
}

// ── readiness probes ─────────────────────────────────────────────────────

async fn smtp_reply(reader: &mut BufReader<TcpStream>) -> anyhow::Result<u16> {
    let mut expected = None;
    for _ in 0..32 {
        let mut line = String::new();
        let n = tokio::time::timeout(std::time::Duration::from_secs(3), reader.read_line(&mut line)).await??;
        if n == 0 || line.len() > 1024 || line.len() < 4 {
            anyhow::bail!("invalid SMTP response");
        }
        let code: u16 = line.get(..3).ok_or_else(|| anyhow::anyhow!("invalid SMTP response"))?.parse()?;
        if let Some(want) = expected {
            if code != want {
                anyhow::bail!("inconsistent SMTP response");
            }
        } else {
            expected = Some(code);
        }
        if line.as_bytes().get(3) != Some(&b'-') {
            return Ok(code);
        }
    }
    anyhow::bail!("SMTP response has too many lines");
}

fn tls_connector() -> anyhow::Result<tokio_rustls::TlsConnector> {
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = std::sync::Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(std::sync::Arc::new(config)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeFailure {
    Dns,
    Connect,
    Certificate,
    Protocol,
}

async fn probe_client_endpoint(host: &str, port: u16, tls: TlsMode, smtp: bool) -> Result<(), ProbeFailure> {
    use tokio_rustls::rustls::pki_types::ServerName;
    let addresses = tokio::time::timeout(std::time::Duration::from_secs(3), tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| ProbeFailure::Dns)?
        .map_err(|_| ProbeFailure::Dns)?;
    let addresses: Vec<_> = addresses.collect();
    let address = *addresses.first().ok_or(ProbeFailure::Dns)?;
    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(3), TcpStream::connect(address))
        .await
        .map_err(|_| ProbeFailure::Connect)?
        .map_err(|_| ProbeFailure::Connect)?;
    let name = ServerName::try_from(host.to_string()).map_err(|_| ProbeFailure::Certificate)?;
    match tls {
        TlsMode::Implicit => {}
        TlsMode::StartTls if smtp => {
            let mut reader = BufReader::new(stream);
            if smtp_reply(&mut reader).await.map_err(|_| ProbeFailure::Protocol)? != 220 {
                return Err(ProbeFailure::Protocol);
            }
            reader
                .get_mut()
                .write_all(b"EHLO setup-check.mailvora.invalid\r\n")
                .await
                .map_err(|_| ProbeFailure::Protocol)?;
            if smtp_reply(&mut reader).await.map_err(|_| ProbeFailure::Protocol)? != 250 {
                return Err(ProbeFailure::Protocol);
            }
            reader.get_mut().write_all(b"STARTTLS\r\n").await.map_err(|_| ProbeFailure::Protocol)?;
            if smtp_reply(&mut reader).await.map_err(|_| ProbeFailure::Protocol)? != 220 {
                return Err(ProbeFailure::Protocol);
            }
            stream = reader.into_inner();
        }
        _ => return Err(ProbeFailure::Protocol),
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(4),
        tls_connector().map_err(|_| ProbeFailure::Certificate)?.connect(name, stream),
    )
    .await
    .map_err(|_| ProbeFailure::Certificate)?
    .map_err(|_| ProbeFailure::Certificate)?;
    Ok(())
}

fn diagnostic(result: Result<(), ProbeFailure>, host: &str, port: u16, service: &str) -> Value {
    let (code, message) = match result {
        Ok(()) => ("ok", format!("The {service} server responds and its security certificate is valid for {host}.")),
        Err(ProbeFailure::Dns) => ("dns", format!("The {service} server name {host} could not be found. Your institution's IT team needs to check its DNS record.")),
        Err(ProbeFailure::Connect) => ("unreachable", format!("The {service} server could not be reached on port {port}. Your institution's IT team needs to check the service and firewall.")),
        Err(ProbeFailure::Certificate) => ("certificate", format!("The secure connection to the {service} server could not be verified. Do not continue and do not accept certificate warnings; ask your IT team to fix the certificate.")),
        Err(ProbeFailure::Protocol) => ("protocol", format!("The {service} server did not complete its secure setup check. Ask your IT team to check the mail app settings.")),
    };
    json!({ "status": if result.is_ok() { "ready" } else { "needs_attention" }, "code": code, "host": host, "port": port, "message": message })
}

async fn setup_readiness(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mail-setup:{}", s.mailbox_id), 8, 60)?;
    let c = context(&state, &s).await?;
    let settings = mail_client::effective(&state, Some(s.domain_id)).await?;

    let (imap_result, smtp_result) = tokio::join!(
        probe_client_endpoint(&settings.imap_host, settings.imap_port, TlsMode::Implicit, false),
        probe_client_endpoint(&settings.smtp_host, settings.smtp_port, settings.smtp_tls, true),
    );
    let quota = if let Some(cfg) = &state.cfg.imap {
        match tokio::time::timeout(std::time::Duration::from_secs(6), imap::quota_usage(cfg, &s.address)).await {
            Ok(Ok(Some((used, limit)))) => {
                let full = used >= limit;
                let nearly = limit > 0 && used * 100 >= limit * 90;
                json!({
                    "status": if full { "full" } else if nearly { "nearly_full" } else { "ready" },
                    "used_bytes": used, "limit_bytes": limit,
                    "message": if full { "Your mailbox is full. New mail cannot arrive until you delete or archive messages." }
                               else if nearly { "Your mailbox is almost full. Delete old messages or ask IT for more space." }
                               else { "Your mailbox has free space." },
                })
            }
            Ok(Ok(None)) => json!({ "status": "unknown", "message": "The mail server did not report storage use." }),
            _ => json!({ "status": "unknown", "message": "Mailbox storage use could not be checked right now." }),
        }
    } else {
        json!({ "status": "unknown", "message": "Mailbox storage is not connected on this platform." })
    };
    let mailbox_ready = c.mailbox_status == "active" && c.org_status == "active";
    let domain_ready = c.domain_status == "verified" && c.hosting_enabled;
    let all_ready =
        mailbox_ready && domain_ready && imap_result.is_ok() && smtp_result.is_ok() && quota["status"] != "full";
    record(&state, &s, "readiness_checked", None, None, Some(if all_ready { "ready" } else { "needs_attention" }))
        .await?;
    Ok(Json(json!({
        "checked_at": Utc::now(),
        "overall": if all_ready { "ready" } else { "needs_attention" },
        "mailbox": { "status": if mailbox_ready { "ready" } else { "needs_attention" },
            "message": if mailbox_ready { "Your mailbox is active and you are allowed to use it." }
                       else { "Your mailbox or organization is not active. Contact your IT team." } },
        "domain": { "status": if domain_ready { "ready" } else { "needs_attention" },
            "message": if domain_ready { "Your institution's email domain is verified and hosted mail is on." }
                       else { "Your institution's email domain is not ready for hosted mail. Contact your IT team." } },
        "imap": diagnostic(imap_result, &settings.imap_host, settings.imap_port, "incoming mail (IMAP)"),
        "smtp": diagnostic(smtp_result, &settings.smtp_host, settings.smtp_port, "outgoing mail (SMTP)"),
        "authentication": { "status": if mailbox_ready { "ready" } else { "needs_attention" },
            "message": "Sign-in to mail apps uses your full email address and your mailbox password." },
        "quota": quota,
        "phone_connection_verified": false,
        "note": "These checks test the mail service from the platform's side. They do not prove that your phone has connected; use the connection test after setup for that.",
    })))
}

// ── analytics ────────────────────────────────────────────────────────────

async fn record(
    state: &AppState,
    s: &MailboxSession,
    event: &str,
    device: Option<&str>,
    client: Option<&str>,
    detail: Option<&str>,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO mailbox_setup_events (organization_id, mailbox_id, event, device, client, detail) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(s.organization_id)
    .bind(s.mailbox_id)
    .bind(event)
    .bind(device)
    .bind(client)
    .bind(detail)
    .execute(&state.db)
    .await?;
    Ok(())
}

#[derive(Deserialize)]
struct EventBody {
    event: String,
    device: Option<String>,
    client: Option<String>,
    detail: Option<String>,
}

const CLIENT_EVENTS: &[&str] = &[
    "guide_started",
    "device_selected",
    "client_selected",
    "step_viewed",
    "guide_completed",
    "help_viewed",
    "test_requested",
];

async fn record_event(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiJson(b): ApiJson<EventBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("mail-setup-events:{}", s.mailbox_id), 120, 600)?;
    if !CLIENT_EVENTS.contains(&b.event.as_str()) {
        return Err(AppError::validation("Unknown setup event"));
    }
    let device = b.device.filter(|d| !d.is_empty());
    if device.as_deref().is_some_and(|d| !DEVICES.contains(&d)) {
        return Err(AppError::validation("Unknown device"));
    }
    let client = b.client.filter(|c| !c.is_empty());
    if client.as_deref().is_some_and(|c| !CLIENTS.contains(&c)) {
        return Err(AppError::validation("Unknown email app"));
    }
    // Free text is limited to a short, safe topic key (e.g. "certificate_warning").
    let detail = b.detail.filter(|d| !d.is_empty());
    if detail
        .as_deref()
        .is_some_and(|d| d.len() > 60 || !d.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'))
    {
        return Err(AppError::validation("detail must be a short topic key"));
    }
    record(&state, &s, &b.event, device.as_deref(), client.as_deref(), detail.as_deref()).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "ok": true }))))
}

// ── connection verification ──────────────────────────────────────────────

/// 8 characters from an unambiguous alphabet (no 0/O, 1/I/L).
fn new_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let bytes = crate::crypto::random_bytes::<8>();
    bytes.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect()
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StartBody {
    client: Option<String>,
}

fn client_of(b: &StartBody) -> ApiResult<Option<String>> {
    match b.client.as_deref().filter(|c| !c.is_empty()) {
        Some(c) if CLIENTS.contains(&c) => Ok(Some(c.to_string())),
        Some(_) => Err(AppError::validation("Unknown email app")),
        None => Ok(None),
    }
}

async fn start_incoming(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiJson(b): ApiJson<StartBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("mail-verify:{}", s.mailbox_id), 10, 3600)?;
    let client = client_of(&b)?;
    let cfg = state.cfg.imap.as_ref().ok_or_else(|| {
        AppError::unavailable("mail_storage_unavailable", "Mailbox storage is not connected on this platform")
    })?;
    let code = new_code();
    let expires_at = Utc::now() + Duration::minutes(30);
    let product = &state.cfg.product_name;
    let text = format!(
        "This message checks that your email app can receive mail.\n\nYour code: {code}\n\nEnter this code on the {product} setup page. \
         It expires in 30 minutes. You can delete this message afterwards.\n\nIf you did not start a connection test, you can ignore this message."
    );
    let html = format!(
        "<p style=\"font-family:Arial,sans-serif;font-size:15px;color:#1f2d2a\">This message checks that your email app can receive mail.</p>\
         <p style=\"font-family:Arial,sans-serif;font-size:13px;color:#5b6b66;margin:18px 0 6px\">Your code</p>\
         <p style=\"font-family:Consolas,monospace;font-size:28px;letter-spacing:4px;font-weight:bold;color:#183b36;margin:0 0 18px\">{code}</p>\
         <p style=\"font-family:Arial,sans-serif;font-size:14px;color:#1f2d2a\">Enter this code on the {} setup page. It expires in 30 minutes.</p>",
        escape_html(product)
    );
    let (id, smtp_id) = crate::services::sending::new_ids(&state);
    let message = mime::build(&OutgoingEmail {
        from: Some(Addr::named(Some(format!("{product} setup check")), state.cfg.system_from_address.clone())),
        to: vec![Addr::named(s.display_name.clone(), s.address.clone())],
        subject: format!("Your {product} connection code: {code}"),
        message_id: smtp_id,
        text: Some(text),
        html: Some(html),
        headers: vec![("X-Mailvora-Setup-Check".into(), id.to_string()), ("Auto-Submitted".into(), "auto-generated".into())],
        ..Default::default()
    })
    .map_err(AppError::Internal)?;
    let raw = message.formatted();
    let mut session = imap::connect(cfg, &s.address).await.map_err(|e| {
        tracing::error!(mailbox = %s.mailbox_id, error = %e, "IMAP connection failed for setup check");
        AppError::unavailable(
            "mail_storage_unavailable",
            "Mailbox storage is temporarily unavailable. Try again shortly.",
        )
    })?;
    let appended = imap::append(&mut session, "INBOX", "", &raw).await;
    let _ = session.logout().await;
    appended.map_err(|e| {
        tracing::warn!(mailbox = %s.mailbox_id, error = %e, "setup check append failed");
        AppError::unavailable("mail_storage_error", "The test message could not be delivered. Try again shortly.")
    })?;
    sqlx::query(
        "UPDATE mailbox_connection_checks SET status = 'expired' WHERE mailbox_id = $1 AND direction = 'incoming' AND status = 'pending'",
    )
    .bind(s.mailbox_id)
    .execute(&state.db)
    .await?;
    let check_id: Uuid = sqlx::query_scalar(
        "INSERT INTO mailbox_connection_checks (organization_id, mailbox_id, direction, code, client, expires_at)
         VALUES ($1, $2, 'incoming', $3, $4, $5) RETURNING id",
    )
    .bind(s.organization_id)
    .bind(s.mailbox_id)
    .bind(&code)
    .bind(&client)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;
    record(&state, &s, "test_requested", None, client.as_deref(), Some("incoming")).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": check_id, "direction": "incoming", "expires_at": expires_at,
            "subject_hint": format!("Your {product} connection code"),
            "instructions": "Open the inbox of your work account in the email app on your phone. Find the message titled \"Your connection code\" and type the code here.",
        })),
    ))
}

#[derive(Deserialize)]
struct ConfirmBody {
    code: String,
}

async fn confirm_incoming(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiJson(b): ApiJson<ConfirmBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mail-verify-confirm:{}", s.mailbox_id), 20, 600)?;
    let code: String =
        b.code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_ascii_uppercase();
    let pending: Option<(Uuid, String, i32, Option<String>)> = sqlx::query_as(
        "SELECT id, code, attempts, client FROM mailbox_connection_checks
         WHERE mailbox_id = $1 AND direction = 'incoming' AND status = 'pending' AND expires_at > now()
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(s.mailbox_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, expected, attempts, client)) = pending else {
        return Err(AppError::conflict("no_active_test", "There is no active test. Start a new connection test."));
    };
    if attempts >= 5 {
        sqlx::query("UPDATE mailbox_connection_checks SET status = 'expired' WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
        return Err(AppError::conflict("too_many_attempts", "Too many incorrect codes. Start a new connection test."));
    }
    if !crate::crypto::constant_time_eq(code.as_bytes(), expected.as_bytes()) {
        sqlx::query("UPDATE mailbox_connection_checks SET attempts = attempts + 1 WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
        record(&state, &s, "connection_failed", None, client.as_deref(), Some("incoming_wrong_code")).await?;
        return Err(AppError::validation(
            "That code does not match. Check the newest message titled \"connection code\" and try again.",
        ));
    }
    sqlx::query("UPDATE mailbox_connection_checks SET status = 'verified', verified_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    record(&state, &s, "connection_verified", None, client.as_deref(), Some("incoming")).await?;
    Ok(Json(json!({
        "verified": true, "direction": "incoming",
        "message": "Your email app is receiving work mail.",
        "verification": verification_status(&state, s.mailbox_id).await?,
    })))
}

async fn start_outgoing(
    State(state): State<AppState>,
    s: MailboxSession,
    ApiJson(b): ApiJson<StartBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("mail-verify:{}", s.mailbox_id), 10, 3600)?;
    let client = client_of(&b)?;
    let code = new_code();
    let expires_at = Utc::now() + Duration::minutes(30);
    sqlx::query(
        "UPDATE mailbox_connection_checks SET status = 'expired' WHERE mailbox_id = $1 AND direction = 'outgoing' AND status = 'pending'",
    )
    .bind(s.mailbox_id)
    .execute(&state.db)
    .await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO mailbox_connection_checks (organization_id, mailbox_id, direction, code, client, expires_at)
         VALUES ($1, $2, 'outgoing', $3, $4, $5) RETURNING id",
    )
    .bind(s.organization_id)
    .bind(s.mailbox_id)
    .bind(&code)
    .bind(&client)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;
    record(&state, &s, "test_requested", None, client.as_deref(), Some("outgoing")).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id, "direction": "outgoing", "code": code, "send_to": s.address,
            "subject": format!("Setup check {code}"), "expires_at": expires_at,
            "instructions": "In the email app on your phone, write a new message from your work account to your own work address. Use the subject shown here (the body can be empty), send it, then come back and press Check.",
        })),
    ))
}

async fn check_outgoing(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("mail-verify-check:{}", s.mailbox_id), 30, 600)?;
    let pending: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, code, client FROM mailbox_connection_checks
         WHERE mailbox_id = $1 AND direction = 'outgoing' AND status = 'pending' AND expires_at > now()
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(s.mailbox_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, code, client)) = pending else {
        return Err(AppError::conflict("no_active_test", "There is no active test. Start a new connection test."));
    };
    let cfg = state.cfg.imap.as_ref().ok_or_else(|| {
        AppError::unavailable("mail_storage_unavailable", "Mailbox storage is not connected on this platform")
    })?;
    let mut session = imap::connect(cfg, &s.address).await.map_err(|e| {
        tracing::error!(mailbox = %s.mailbox_id, error = %e, "IMAP connection failed for setup check");
        AppError::unavailable(
            "mail_storage_unavailable",
            "Mailbox storage is temporarily unavailable. Try again shortly.",
        )
    })?;
    let found = imap::list_messages(&mut session, "INBOX", 1, 20, Some(&code)).await;
    let messages = match found {
        Ok((_, m)) => m,
        Err(e) => {
            let _ = session.logout().await;
            tracing::warn!(mailbox = %s.mailbox_id, error = %e, "setup check search failed");
            return Err(AppError::unavailable(
                "mail_storage_error",
                "The inbox could not be searched. Try again shortly.",
            ));
        }
    };
    // The message must come from the mailbox itself and must not be one the
    // platform generated (webmail sends carry our Message-ID domain), so a
    // message composed in webmail cannot stand in for the phone.
    let platform_id_suffix = format!("@{}", state.cfg.message_id_domain);
    let mut ok = false;
    for m in messages.iter().filter(|m| {
        m.subject.to_ascii_uppercase().contains(&code)
            && m.from_address.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(&s.address))
    }) {
        if let Ok(Some(raw)) = imap::fetch_raw(&mut session, "INBOX", m.uid).await {
            let message_id = mail_parser::MessageParser::default()
                .parse(&raw)
                .and_then(|p| p.message_id().map(str::to_string))
                .unwrap_or_default();
            if !message_id.ends_with(&platform_id_suffix) {
                ok = true;
                break;
            }
        }
    }
    let _ = session.logout().await;
    if !ok {
        return Ok(Json(json!({
            "verified": false, "direction": "outgoing",
            "message": "The test message has not arrived yet. Wait a minute and check again. If your phone shows a sending error, open the help for \"Cannot send email\".",
        })));
    }
    sqlx::query("UPDATE mailbox_connection_checks SET status = 'verified', verified_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    record(&state, &s, "connection_verified", None, client.as_deref(), Some("outgoing")).await?;
    Ok(Json(json!({
        "verified": true, "direction": "outgoing",
        "message": "Your email app sent a message through the work mail server.",
        "verification": verification_status(&state, s.mailbox_id).await?,
    })))
}

// ── Apple configuration profile ──────────────────────────────────────────

fn xml(s: &str) -> String {
    escape_html(s)
}

/// An unsigned `.mobileconfig` with a single Mail payload. It carries no
/// password: iOS asks for it during installation. The PHP frontend can sign
/// it with the operator's certificate so iOS shows it as verified.
pub fn mobileconfig(
    settings: &ClientSettings,
    address: &str,
    display_name: Option<&str>,
    org: &str,
    product: &str,
    identifier_root: &str,
) -> String {
    // Stable per mailbox, so re-installing replaces the profile instead of
    // adding a second account.
    let stable = |label: &str| {
        let digest = crate::crypto::sha256(format!("{label}:{address}"));
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        uuid::Builder::from_random_bytes(bytes).into_uuid().to_string().to_uppercase()
    };
    let uuid_payload = stable("mail-payload");
    let uuid_profile = stable("mail-profile");
    let smtp_ssl = matches!(settings.smtp_tls, TlsMode::Implicit | TlsMode::StartTls);
    let name = display_name.unwrap_or(address);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>PayloadContent</key>
  <array>
    <dict>
      <key>PayloadType</key><string>com.apple.mail.managed</string>
      <key>PayloadVersion</key><integer>1</integer>
      <key>PayloadIdentifier</key><string>{root}.mail.{uuid_payload}</string>
      <key>PayloadUUID</key><string>{uuid_payload}</string>
      <key>PayloadDisplayName</key><string>{org} work email</string>
      <key>EmailAccountDescription</key><string>{org} work email</string>
      <key>EmailAccountName</key><string>{name}</string>
      <key>EmailAccountType</key><string>EmailTypeIMAP</string>
      <key>EmailAddress</key><string>{address}</string>
      <key>IncomingMailServerHostName</key><string>{imap_host}</string>
      <key>IncomingMailServerPortNumber</key><integer>{imap_port}</integer>
      <key>IncomingMailServerUseSSL</key><true/>
      <key>IncomingMailServerUsername</key><string>{address}</string>
      <key>IncomingMailServerAuthentication</key><string>EmailAuthPassword</string>
      <key>OutgoingMailServerHostName</key><string>{smtp_host}</string>
      <key>OutgoingMailServerPortNumber</key><integer>{smtp_port}</integer>
      <key>OutgoingMailServerUseSSL</key>{smtp_ssl}
      <key>OutgoingMailServerUsername</key><string>{address}</string>
      <key>OutgoingMailServerAuthentication</key><string>EmailAuthPassword</string>
      <key>OutgoingPasswordSameAsIncomingPassword</key><true/>
      <key>PreventMove</key><false/>
      <key>SMIMEEnabled</key><false/>
    </dict>
  </array>
  <key>PayloadDescription</key><string>Adds your {org} work email ({address}) to the Mail app. You will be asked for your mailbox password. The profile contains no password and can be removed at any time in Settings.</string>
  <key>PayloadDisplayName</key><string>{org} work email</string>
  <key>PayloadIdentifier</key><string>{root}.profile.{uuid_profile}</string>
  <key>PayloadOrganization</key><string>{org}</string>
  <key>PayloadRemovalDisallowed</key><false/>
  <key>PayloadType</key><string>Configuration</string>
  <key>PayloadUUID</key><string>{uuid_profile}</string>
  <key>PayloadVersion</key><integer>1</integer>
  <key>ConsentText</key><dict><key>default</key><string>Provided by {product}. Installing adds one email account; it does not manage or monitor your device.</string></dict>
</dict>
</plist>
"#,
        root = xml(identifier_root),
        org = xml(org),
        name = xml(name),
        address = xml(address),
        product = xml(product),
        imap_host = xml(&settings.imap_host),
        imap_port = settings.imap_port,
        smtp_host = xml(&settings.smtp_host),
        smtp_port = settings.smtp_port,
        smtp_ssl = if smtp_ssl { "<true/>" } else { "<false/>" },
    )
}

async fn apple_profile(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Response> {
    state.limiter.check(&format!("mail-profile:{}", s.mailbox_id), 20, 3600)?;
    let c = context(&state, &s).await?;
    let settings = mail_client::effective(&state, Some(s.domain_id)).await?;
    let root: String = state.cfg.message_id_domain.split('.').rev().collect::<Vec<_>>().join(".");
    let body =
        mobileconfig(&settings, &s.address, s.display_name.as_deref(), &c.org_name, &state.cfg.product_name, &root);
    record(&state, &s, "profile_downloaded", Some("ios"), Some("apple_mail"), None).await?;
    let filename = format!(
        "{}-work-email.mobileconfig",
        s.address.split('@').next().unwrap_or("work").replace(|c: char| !c.is_ascii_alphanumeric(), "-")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-apple-aspen-config".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn diagnostics_are_actionable_without_exposing_probe_errors() {
        let dns = diagnostic(Err(ProbeFailure::Dns), "mail.example.edu", 993, "incoming mail");
        assert_eq!(dns["status"], "needs_attention");
        assert_eq!(dns["code"], "dns");
        assert!(dns["message"].as_str().unwrap().contains("DNS"));
        let tls = diagnostic(Err(ProbeFailure::Certificate), "mail.example.edu", 465, "outgoing mail");
        assert!(tls["message"].as_str().unwrap().contains("do not accept certificate warnings"));
        let ready = diagnostic(Ok(()), "mail.example.edu", 993, "incoming mail");
        assert_eq!(ready["status"], "ready");
    }

    #[test]
    fn codes_are_unambiguous() {
        for _ in 0..200 {
            let c = new_code();
            assert_eq!(c.len(), 8);
            assert!(!c.contains(['0', 'O', '1', 'I', 'L']), "{c}");
        }
    }

    #[test]
    fn apple_profile_has_no_password_and_escapes_values() {
        let cfg = Config::for_tests("postgres://localhost/x");
        let settings = ClientSettings::from_config(&cfg);
        let p = mobileconfig(
            &settings,
            "amina@school.ac.ke",
            Some("Amina <O'Neil>"),
            "Riverside & Co",
            "Mailvora",
            "test.mailvora",
        );
        assert!(p.contains("com.apple.mail.managed"));
        assert!(!p.contains("<key>IncomingPassword</key>"));
        assert!(!p.contains("<key>OutgoingPassword</key>"));
        assert!(p.contains("Riverside &amp; Co"));
        assert!(p.contains("Amina &lt;O&#39;Neil&gt;"));
        assert!(p.contains("<integer>993</integer>"));
        assert!(p.contains("<integer>465</integer>"));
    }
}
