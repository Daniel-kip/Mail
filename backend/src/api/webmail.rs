//! Webmail API for hosted mailbox users.
//!
//! Separate identity from the console: mailbox users sign in with their
//! mailbox address and password, get their own cookie and CSRF token, and
//! can only reach their own mailbox. Mail is read from Dovecot over IMAP;
//! sending goes through the same outbox pipeline as the API (signed with the
//! domain's DKIM key) and a copy is appended to the Sent folder.

use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, StatusCode},
    response::{AppendHeaders, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use mail_parser::{MessageParser, MimeHeaders};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::{ApiJson, ApiQuery};
use crate::{
    auth::{
        session::{self, SessionKind, SessionOwner},
        ClientInfo, MailboxSession,
    },
    config::ImapConfig,
    error::{ApiResult, AppError},
    mail::mime::{self, Addr, OutAttachment, OutgoingEmail},
    password,
    services::{
        audit::Entry,
        imap::{self, ImapSession},
        sanitize,
        sending::{self, NewMessage, ReplyMode},
    },
    state::AppState,
    validation,
};

const MAX_FAILED_LOGINS: i32 = 10;

pub fn router(state: &AppState) -> Router<AppState> {
    let upload_limit = state.cfg.max_attachment_bytes + 2 * 1024 * 1024;
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/session", get(current))
        .route("/password", post(change_password))
        .route("/folders", get(folders))
        .route("/folders/{folder}/messages", get(list_messages))
        .route("/folders/{folder}/messages/{uid}", get(read_message))
        .route("/folders/{folder}/messages/{uid}/attachments/{index}", get(download_attachment))
        .route("/folders/{folder}/flags", post(set_flags))
        .route("/folders/{folder}/move", post(move_messages))
        .route("/folders/{folder}/delete", post(delete_messages))
        .route("/send", post(send).layer(DefaultBodyLimit::max(upload_limit)))
        .route("/drafts", post(save_draft).layer(DefaultBodyLimit::max(upload_limit)))
        .merge(super::mail_setup::router())
        .merge(super::webmail_account::router())
}

fn imap_cfg(state: &AppState) -> ApiResult<&ImapConfig> {
    state.cfg.imap.as_ref().ok_or_else(|| {
        AppError::unavailable("mail_storage_unavailable", "Mailbox storage is not configured on this platform")
    })
}

async fn open(state: &AppState, s: &MailboxSession) -> ApiResult<ImapSession> {
    imap::connect(imap_cfg(state)?, &s.address).await.map_err(|e| {
        tracing::error!(mailbox = %s.mailbox_id, error = %e, "IMAP connection failed");
        AppError::unavailable(
            "mail_storage_unavailable",
            "Mailbox storage is temporarily unavailable. Try again shortly.",
        )
    })
}

fn storage_err(e: anyhow::Error) -> AppError {
    tracing::warn!(error = %e, "IMAP operation failed");
    let text = e.to_string();
    if text.contains("NONEXISTENT") || text.to_ascii_lowercase().contains("mailbox doesn't exist") {
        return AppError::not_found("Folder");
    }
    AppError::unavailable("mail_storage_error", "The mailbox operation failed. Try again shortly.")
}

fn check_folder(folder: &str) -> ApiResult<()> {
    imap::check_folder_name(folder).map_err(AppError::validation)
}

async fn finish(mut session: ImapSession) {
    let _ = session.logout().await;
}

// ── authentication ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginBody {
    address: String,
    password: String,
}

#[derive(sqlx::FromRow)]
struct LoginMailbox {
    id: Uuid,
    address: String,
    organization_id: Uuid,
    password_hash: String,
    failed_login_count: i32,
    locked_until: Option<DateTime<Utc>>,
    live: bool,
}

async fn login(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(body): ApiJson<LoginBody>,
) -> ApiResult<Response> {
    let address = body.address.trim().to_lowercase();
    state.limiter.check(&format!("webmail-ip:{}", client.ip_key()), 30, 600)?;
    state.limiter.check(&format!("webmail-addr:{address}"), 10, 600)?;

    let m: Option<LoginMailbox> = sqlx::query_as(
        "SELECT m.id, m.address::text AS address, m.organization_id, m.password_hash, m.failed_login_count, m.locked_until,
                EXISTS (SELECT 1 FROM mail_mailboxes mm WHERE mm.address = m.address::text) AS live
         FROM mailboxes m WHERE m.address = $1 AND m.status <> 'deleted'",
    )
    .bind(&address)
    .fetch_optional(&state.db)
    .await?;
    let Some(m) = m else {
        password::verify_dummy(&body.password).await;
        return Err(AppError::invalid_credentials());
    };
    if m.locked_until.is_some_and(|t| t > Utc::now()) {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "account_locked",
            "Too many failed sign-in attempts. Try again in 15 minutes.",
        ));
    }
    if !password::verify(&body.password, &m.password_hash).await {
        let failures = m.failed_login_count + 1;
        let lock = (failures >= MAX_FAILED_LOGINS).then(|| Utc::now() + Duration::minutes(15));
        sqlx::query("UPDATE mailboxes SET failed_login_count = $2, locked_until = $3 WHERE id = $1")
            .bind(m.id)
            .bind(if lock.is_some() { 0 } else { failures })
            .bind(lock)
            .execute(&state.db)
            .await?;
        return Err(AppError::invalid_credentials());
    }
    if !m.live {
        return Err(AppError::forbidden("This mailbox is disabled. Contact your administrator."));
    }
    sqlx::query(
        "UPDATE mailboxes SET failed_login_count = 0, locked_until = NULL, last_login_at = now() WHERE id = $1",
    )
    .bind(m.id)
    .execute(&state.db)
    .await?;
    let s = session::create(
        &state.db,
        &state.cfg,
        SessionKind::Webmail,
        SessionOwner { user_id: None, mailbox_id: Some(m.id), active_organization_id: None, mfa_pending: false },
        client.ip,
        client.user_agent.as_deref(),
    )
    .await?;
    Entry {
        organization_id: Some(m.organization_id),
        actor_type: "mailbox",
        actor_id: Some(m.id),
        actor_label: Some(m.address.clone()),
        action: "mailbox.webmail_login",
        target_type: None,
        target_id: None,
        client: Some(&client),
        details: json!({}),
    }
    .record(&state.db)
    .await?;
    let cookie = session::set_cookie(&state.cfg, SessionKind::Webmail, &s.token, state.cfg.session_ttl_hours * 3600);
    let payload = session_payload(&state, m.id, &s.csrf_token).await?;
    Ok((AppendHeaders([(header::SET_COOKIE, cookie)]), Json(payload)).into_response())
}

async fn session_payload(state: &AppState, mailbox_id: Uuid, csrf: &str) -> ApiResult<Value> {
    let (address, display_name, quota, org_name): (String, Option<String>, i64, String) = sqlx::query_as(
        "SELECT m.address::text, m.display_name, m.quota_bytes, o.name FROM mailboxes m JOIN organizations o ON o.id = m.organization_id WHERE m.id = $1",
    )
    .bind(mailbox_id)
    .fetch_one(&state.db)
    .await?;
    Ok(json!({
        "mailbox": { "id": mailbox_id, "address": address, "display_name": display_name, "quota_bytes": quota, "organization": org_name },
        "csrf_token": csrf,
        "storage_configured": state.cfg.imap.is_some(),
    }))
}

async fn logout(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Response> {
    session::revoke(&state.db, s.session_id).await?;
    let cookie = session::clear_cookie(&state.cfg, SessionKind::Webmail);
    Ok((AppendHeaders([(header::SET_COOKIE, cookie)]), Json(json!({ "ok": true }))).into_response())
}

async fn current(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    let mut payload = session_payload(&state, s.mailbox_id, &s.csrf_token).await?;
    if let Some(cfg) = &state.cfg.imap {
        if let Ok(Some((used, limit))) = imap::quota_usage(cfg, &s.address).await {
            payload["quota"] = json!({ "used_bytes": used, "limit_bytes": limit });
        }
    }
    Ok(Json(payload))
}

#[derive(Deserialize)]
struct PasswordBody {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    client: ClientInfo,
    s: MailboxSession,
    ApiJson(body): ApiJson<PasswordBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("webmail-pw:{}", s.mailbox_id), 10, 3600)?;
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM mailboxes WHERE id = $1")
        .bind(s.mailbox_id)
        .fetch_one(&state.db)
        .await?;
    if !password::verify(&body.current_password, &hash).await {
        return Err(AppError::new(StatusCode::FORBIDDEN, "invalid_current_password", "Current password is incorrect"));
    }
    password::check_policy(&body.new_password, &s.address)?;
    crate::services::breach::ensure_not_breached(&state.cfg, &body.new_password).await?;
    let new_hash = password::hash_for_mailbox(&body.new_password).await?;
    sqlx::query(
        "UPDATE mailboxes SET password_hash = $2, password_changed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(s.mailbox_id)
    .bind(new_hash)
    .execute(&state.db)
    .await?;
    let revoked = session::revoke_all_for_mailbox(&state.db, s.mailbox_id, Some(s.session_id)).await?;
    // The old password must stop working in mail apps right away.
    crate::services::doveadm::revoke_access(&state, vec![s.address.clone()]).await;
    Entry {
        organization_id: Some(s.organization_id),
        actor_type: "mailbox",
        actor_id: Some(s.mailbox_id),
        actor_label: Some(s.address.clone()),
        action: "mailbox.password_changed",
        target_type: Some("mailbox"),
        target_id: Some(s.mailbox_id.to_string()),
        client: Some(&client),
        details: json!({ "other_sessions_revoked": revoked }),
    }
    .record(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true, "note": "IMAP/SMTP clients must be updated with the new password." })))
}

// ── reading ──────────────────────────────────────────────────────────────

async fn folders(State(state): State<AppState>, s: MailboxSession) -> ApiResult<Json<Value>> {
    let mut session = open(&state, &s).await?;
    let result = imap::list_folders(&mut session).await;
    finish(session).await;
    Ok(Json(json!({ "data": result.map_err(storage_err)? })))
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<u32>,
    per_page: Option<u32>,
    q: Option<String>,
}

async fn list_messages(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(folder): Path<String>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<Json<Value>> {
    check_folder(&folder)?;
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).clamp(10, 100);
    let mut session = open(&state, &s).await?;
    let result = imap::list_messages(&mut session, &folder, page, per_page, q.q.as_deref()).await;
    finish(session).await;
    let (total, messages) = result.map_err(storage_err)?;
    Ok(Json(json!({ "data": messages, "page": page, "per_page": per_page, "total": total })))
}

fn addr_list(list: Option<&mail_parser::Address>) -> Vec<Value> {
    list.map(|l| l.iter().map(|a| json!({ "name": a.name(), "email": a.address() })).collect()).unwrap_or_default()
}

#[derive(Deserialize)]
struct ReadQuery {
    remote: Option<bool>,
}

async fn read_message(
    State(state): State<AppState>,
    s: MailboxSession,
    Path((folder, uid)): Path<(String, u32)>,
    ApiQuery(q): ApiQuery<ReadQuery>,
) -> ApiResult<Json<Value>> {
    check_folder(&folder)?;
    let mut session = open(&state, &s).await?;
    let raw = imap::fetch_raw(&mut session, &folder, uid).await;
    let raw = match raw {
        Ok(Some(r)) => r,
        Ok(None) => {
            finish(session).await;
            return Err(AppError::not_found("Message"));
        }
        Err(e) => {
            finish(session).await;
            return Err(storage_err(e));
        }
    };
    let _ = imap::store_flags(&mut session, &folder, &[uid], true, "\\Seen").await;
    finish(session).await;

    let msg = MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| AppError::unavailable("unparseable_message", "This message could not be parsed"))?;

    // Inline (cid:) images become data: URIs; everything else is listed as an attachment.
    let mut inline: HashMap<String, String> = HashMap::new();
    let mut attachments = Vec::new();
    for (index, part) in msg.attachments().enumerate() {
        let ct = part
            .content_type()
            .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream")))
            .unwrap_or_else(|| "application/octet-stream".into())
            .to_ascii_lowercase();
        let data = part.contents();
        if let Some(cid) = part.content_id() {
            if ct.starts_with("image/") && data.len() <= 2 * 1024 * 1024 && !matches!(ct.as_str(), "image/svg+xml") {
                inline
                    .insert(cid.trim_matches(['<', '>']).to_string(), format!("data:{ct};base64,{}", B64.encode(data)));
                continue;
            }
        }
        attachments.push(json!({
            "index": index,
            "filename": mime::sanitize_filename(part.attachment_name().unwrap_or("attachment")),
            "content_type": ct,
            "size": data.len(),
        }));
    }

    let allow_remote = q.remote.unwrap_or(false);
    let (html, has_remote) = match msg.body_html(0) {
        Some(h) => {
            let r = sanitize::inbound(&h, &inline, allow_remote);
            (r.document, r.has_remote_content)
        }
        None => {
            let text = msg.body_text(0).map(|t| t.to_string()).unwrap_or_default();
            let r = sanitize::inbound(&sanitize::text_to_html(&text), &inline, false);
            (r.document, false)
        }
    };

    let header_text = |name: &'static str| msg.header_raw(name).map(|v| v.trim().to_string());
    Ok(Json(json!({
        "uid": uid,
        "folder": folder,
        "subject": msg.subject().unwrap_or("(no subject)"),
        "from": addr_list(msg.from()),
        "to": addr_list(msg.to()),
        "cc": addr_list(msg.cc()),
        "reply_to": addr_list(msg.reply_to()),
        "date": msg.date().map(|d| d.to_rfc3339()),
        "message_id": msg.message_id(),
        "references": header_text("References"),
        "html_document": html,
        "text": msg.body_text(0).map(|t| t.to_string()),
        "has_remote_content": has_remote,
        "remote_content_loaded": allow_remote,
        "attachments": attachments,
        "authentication_results": header_text("Authentication-Results"),
        "spam_flag": header_text("X-Spam").or_else(|| header_text("X-Spam-Flag")),
    })))
}

async fn download_attachment(
    State(state): State<AppState>,
    s: MailboxSession,
    Path((folder, uid, index)): Path<(String, u32, usize)>,
) -> ApiResult<Response> {
    check_folder(&folder)?;
    let mut session = open(&state, &s).await?;
    let raw = imap::fetch_raw(&mut session, &folder, uid).await;
    finish(session).await;
    let raw = raw.map_err(storage_err)?.ok_or_else(|| AppError::not_found("Message"))?;
    let msg = MessageParser::default().parse(&raw).ok_or_else(|| AppError::not_found("Attachment"))?;
    let part = msg.attachments().nth(index).ok_or_else(|| AppError::not_found("Attachment"))?;
    let filename = mime::sanitize_filename(part.attachment_name().unwrap_or("attachment"));
    let encoded = percent_encoding::utf8_percent_encode(&filename, percent_encoding::NON_ALPHANUMERIC).to_string();
    let ascii: String =
        filename.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_ ".contains(c) { c } else { '_' }).collect();
    // Always served as a download with a neutral type so browsers never
    // render attacker-controlled content on our origin.
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        Body::from(part.contents().to_vec()),
    )
        .into_response())
}

// ── organizing ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FlagsBody {
    uids: Vec<u32>,
    seen: Option<bool>,
    flagged: Option<bool>,
}

async fn set_flags(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(folder): Path<String>,
    ApiJson(body): ApiJson<FlagsBody>,
) -> ApiResult<Json<Value>> {
    check_folder(&folder)?;
    if body.uids.is_empty() || body.uids.len() > 500 {
        return Err(AppError::validation("Provide 1 to 500 uids"));
    }
    let mut session = open(&state, &s).await?;
    let mut result = Ok(());
    if let Some(seen) = body.seen {
        result = imap::store_flags(&mut session, &folder, &body.uids, seen, "\\Seen").await;
    }
    if let (Ok(()), Some(flagged)) = (&result, body.flagged) {
        result = imap::store_flags(&mut session, &folder, &body.uids, flagged, "\\Flagged").await;
    }
    finish(session).await;
    result.map_err(storage_err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MoveBody {
    uids: Vec<u32>,
    destination: String,
}

async fn move_messages(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(folder): Path<String>,
    ApiJson(body): ApiJson<MoveBody>,
) -> ApiResult<Json<Value>> {
    check_folder(&folder)?;
    check_folder(&body.destination)?;
    if body.uids.is_empty() || body.uids.len() > 500 {
        return Err(AppError::validation("Provide 1 to 500 uids"));
    }
    let mut session = open(&state, &s).await?;
    let result = imap::move_messages(&mut session, &folder, &body.uids, &body.destination).await;
    finish(session).await;
    result.map_err(storage_err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct DeleteBody {
    uids: Vec<u32>,
}

/// Move to Trash; deleting from Trash (or Junk) removes permanently.
async fn delete_messages(
    State(state): State<AppState>,
    s: MailboxSession,
    Path(folder): Path<String>,
    ApiJson(body): ApiJson<DeleteBody>,
) -> ApiResult<Json<Value>> {
    check_folder(&folder)?;
    if body.uids.is_empty() || body.uids.len() > 500 {
        return Err(AppError::validation("Provide 1 to 500 uids"));
    }
    let mut session = open(&state, &s).await?;
    let result: anyhow::Result<&'static str> = async {
        let folders = imap::list_folders(&mut session).await?;
        let trash = imap::find_role(&folders, "trash").map(|f| f.name.clone());
        let current_role = folders.iter().find(|f| f.name == folder).and_then(|f| f.role);
        match trash {
            Some(t) if !matches!(current_role, Some("trash") | Some("junk")) => {
                imap::move_messages(&mut session, &folder, &body.uids, &t).await?;
                Ok("moved_to_trash")
            }
            _ => {
                imap::expunge(&mut session, &folder, &body.uids).await?;
                Ok("deleted")
            }
        }
    }
    .await;
    finish(session).await;
    Ok(Json(json!({ "ok": true, "result": result.map_err(storage_err)? })))
}

// ── composing ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ComposePayload {
    #[serde(default)]
    to: Vec<String>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    bcc: Vec<String>,
    #[serde(default)]
    subject: String,
    text: Option<String>,
    html: Option<String>,
    in_reply_to: Option<String>,
    references: Option<String>,
    /// Draft to remove after a successful send.
    draft_uid: Option<u32>,
}

struct Composed {
    payload: ComposePayload,
    attachments: Vec<OutAttachment>,
}

async fn read_compose(state: &AppState, mut multipart: Multipart) -> ApiResult<Composed> {
    let mut payload: Option<ComposePayload> = None;
    let mut attachments = Vec::new();
    let mut total = 0usize;
    while let Some(field) =
        multipart.next_field().await.map_err(|e| AppError::validation(format!("Invalid upload: {e}")))?
    {
        match field.name() {
            Some("payload") => {
                let text = field.text().await.map_err(|e| AppError::validation(format!("Invalid payload: {e}")))?;
                payload = Some(
                    serde_json::from_str(&text).map_err(|e| AppError::validation(format!("Invalid payload: {e}")))?,
                );
            }
            Some("attachment") => {
                if attachments.len() >= 20 {
                    return Err(AppError::validation("At most 20 attachments"));
                }
                let filename = super::messages::check_attachment_name(field.file_name().unwrap_or("attachment"))?;
                let content_type = field
                    .content_type()
                    .filter(|c| c.len() < 128 && c.contains('/') && !c.contains(['\r', '\n', ';']))
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let data = field.bytes().await.map_err(|e| AppError::validation(format!("Upload failed: {e}")))?;
                total += data.len();
                if total > state.cfg.max_attachment_bytes {
                    return Err(AppError::new(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "attachments_too_large",
                        format!("Attachments exceed {} MB in total", state.cfg.max_attachment_bytes / 1024 / 1024),
                    ));
                }
                attachments.push(OutAttachment { filename, content_type, data: data.to_vec() });
            }
            _ => {}
        }
    }
    let payload = payload.ok_or_else(|| AppError::validation("Missing 'payload' part"))?;
    Ok(Composed { payload, attachments })
}

fn parse_list(field: &str, list: &[String]) -> ApiResult<Vec<Addr>> {
    let mut out: Vec<Addr> = Vec::new();
    for raw in list.iter().flat_map(|s| s.split([',', ';'])).map(str::trim).filter(|s| !s.is_empty()) {
        let (name, email) = match (raw.rfind('<'), raw.ends_with('>')) {
            (Some(lt), true) => {
                let n = raw[..lt].trim().trim_matches('"').trim();
                ((!n.is_empty()).then(|| n.to_string()), raw[lt + 1..raw.len() - 1].trim().to_string())
            }
            _ => (None, raw.to_string()),
        };
        let parsed = validation::parse_email(field, &email)?;
        if let Some(n) = &name {
            validation::header_safe(field, n, 200)?;
        }
        if !out.iter().any(|a| a.email.eq_ignore_ascii_case(&parsed.as_string())) {
            out.push(Addr::named(name, parsed.as_string()));
        }
    }
    Ok(out)
}

struct BuiltMime {
    email: OutgoingEmail,
    to: Vec<Addr>,
    cc: Vec<Addr>,
    bcc: Vec<Addr>,
}

fn build_mime(s: &MailboxSession, c: &Composed, message_id: &str, for_draft: bool) -> ApiResult<BuiltMime> {
    let p = &c.payload;
    let to = parse_list("to", &p.to)?;
    let cc = parse_list("cc", &p.cc)?;
    let bcc = parse_list("bcc", &p.bcc)?;
    validation::header_safe("subject", &p.subject, 998)?;
    for (f, v) in [("in_reply_to", &p.in_reply_to), ("references", &p.references)] {
        if let Some(v) = v {
            validation::header_safe(f, v, 4000)?;
        }
    }
    if !for_draft && to.is_empty() && cc.is_empty() && bcc.is_empty() {
        return Err(AppError::validation("Add at least one recipient"));
    }
    let html =
        p.html.as_deref().filter(|h| !h.trim().is_empty()).map(|h| sanitize::wrap_document(&sanitize::outbound(h)));
    let email = OutgoingEmail {
        from: Some(Addr::named(s.display_name.clone(), s.address.clone())),
        to: to.clone(),
        cc: cc.clone(),
        subject: if p.subject.trim().is_empty() { "(no subject)".into() } else { p.subject.trim().to_string() },
        message_id: message_id.to_string(),
        in_reply_to: p.in_reply_to.clone(),
        references: p.references.clone(),
        text: p.text.clone().or_else(|| if html.is_none() { Some(String::new()) } else { None }),
        html,
        attachments: c.attachments.clone(),
        ..Default::default()
    };
    Ok(BuiltMime { email, to, cc, bcc })
}

async fn send(
    State(state): State<AppState>,
    s: MailboxSession,
    multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state.limiter.check(&format!("webmail-send:{}", s.mailbox_id), 60, 3600)?;
    let composed = read_compose(&state, multipart).await?;
    let (id, smtp_id) = sending::new_ids(&state);
    let BuiltMime { email, to, cc, bcc } = build_mime(&s, &composed, &smtp_id, false)?;
    let recipients: Vec<String> = to.iter().chain(cc.iter()).chain(bcc.iter()).map(|a| a.email.clone()).collect();
    if recipients.len() > 100 {
        return Err(AppError::validation("At most 100 recipients per message"));
    }

    let domain_name = s.address.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    let domain = sending::sending_domain(&state, s.organization_id, &domain_name).await?;
    let message = mime::build(&email).map_err(|e| AppError::validation(format!("Could not build message: {e}")))?;
    let raw = sending::sign(&state, &domain, message)?;
    if raw.len() > state.cfg.max_message_bytes {
        return Err(AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "message_too_large", "Message is too large"));
    }
    let envelope_from = sending::envelope_from(&state, id);
    let to_display = to.iter().chain(cc.iter()).map(|a| a.email.clone()).collect::<Vec<_>>().join(", ");

    let mut tx = state.db.begin().await?;
    sending::reserve_quota(&mut tx, s.organization_id, recipients.len() as i32).await?;
    sending::insert(
        &mut tx,
        &NewMessage {
            id,
            org_id: s.organization_id,
            batch_id: id,
            source: "webmail",
            api_key_id: None,
            user_id: None,
            mailbox_id: Some(s.mailbox_id),
            domain_id: domain.id,
            from_address: &s.address,
            from_name: s.display_name.as_deref(),
            envelope_from: &envelope_from,
            to_display: if to_display.is_empty() { "(bcc)" } else { &to_display },
            recipients: &recipients,
            reply_to: Some(&s.address),
            reply_mode: ReplyMode::ReplyToMailbox,
            subject: &email.subject,
            template: None,
            sandbox: false,
            smtp_message_id: &smtp_id,
            tags: &["webmail".to_string()],
            metadata: &json!({}),
            idempotency_key: None,
            raw: &raw,
            html: None,
            text: None,
            suppressed: false,
            scheduled_at: None,
            list_unsubscribe: false,
        },
    )
    .await?;
    tx.commit().await?;

    // Keep a copy in Sent and clean up the draft. Failures here do not undo
    // the send; the user is told the copy could not be saved.
    let mut warnings = Vec::new();
    if let Ok(mut session) = open(&state, &s).await {
        let folders = imap::list_folders(&mut session).await.unwrap_or_default();
        if let Some(sent) = imap::find_role(&folders, "sent") {
            if let Err(e) = imap::append(&mut session, &sent.name, "(\\Seen)", &raw).await {
                tracing::warn!(error = %e, "could not append to Sent");
                warnings.push("The message was sent but a copy could not be saved to Sent.");
            }
        }
        if let (Some(uid), Some(drafts)) = (composed.payload.draft_uid, imap::find_role(&folders, "drafts")) {
            let _ = imap::expunge(&mut session, &drafts.name, &[uid]).await;
        }
        if let Some(irt) = &composed.payload.in_reply_to {
            // Mark the original as answered when it is in the inbox.
            let found = match session.select("INBOX").await {
                Ok(_) => session.uid_search(format!("HEADER Message-ID {}", quote_imap(irt))).await.ok(),
                Err(_) => None,
            };
            if let Some(uids) = found {
                let uids: Vec<u32> = uids.into_iter().collect();
                let _ = imap::store_flags(&mut session, "INBOX", &uids, true, "\\Answered").await;
            }
        }
        finish(session).await;
    }
    Ok((StatusCode::ACCEPTED, Json(json!({ "id": id, "status": "queued", "warnings": warnings }))))
}

fn quote_imap(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(300).collect();
    format!("\"{}\"", cleaned.replace('\\', "\\\\").replace('"', "\\\""))
}

async fn save_draft(State(state): State<AppState>, s: MailboxSession, multipart: Multipart) -> ApiResult<Json<Value>> {
    let composed = read_compose(&state, multipart).await?;
    let (_, smtp_id) = sending::new_ids(&state);
    let BuiltMime { email, .. } = build_mime(&s, &composed, &smtp_id, true)?;
    let raw = mime::build(&email).map_err(|e| AppError::validation(format!("Could not build draft: {e}")))?.formatted();
    let mut session = open(&state, &s).await?;
    let result: anyhow::Result<()> = async {
        let folders = imap::list_folders(&mut session).await?;
        let drafts = imap::find_role(&folders, "drafts").map(|f| f.name.clone()).unwrap_or_else(|| "Drafts".into());
        if let Some(old) = composed.payload.draft_uid {
            imap::expunge(&mut session, &drafts, &[old]).await?;
        }
        imap::append(&mut session, &drafts, "(\\Draft \\Seen)", &raw).await
    }
    .await;
    finish(session).await;
    result.map_err(storage_err)?;
    Ok(Json(json!({ "ok": true })))
}
