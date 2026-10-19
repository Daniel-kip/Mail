//! Transactional message API.
//!
//! `POST /v1/messages` accepts direct content or a template, one or many
//! recipients (one message per recipient), and an optional
//! `Idempotency-Key` header. A 202 means *accepted for processing*, not
//! delivered (guide 01 and 03 status semantics).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::common::{empty_as_none, page_limit, ApiJson, ApiQuery, Cursor};
use crate::{
    auth::{Permission, Principal},
    crypto,
    error::{ApiResult, AppError},
    mail::mime::{self, Addr, OutAttachment, OutgoingEmail},
    services::{
        sending::{self, NewMessage, ReplyMode},
        templates::{self as tpl, TemplateSource},
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/messages", get(list_messages).post(send))
        .route("/messages/export.csv", get(export_csv))
        .route("/messages/{id}", get(get_message))
        .route("/messages/{id}/cancel", axum::routing::post(cancel_message))
        .route("/unsubscribe/{token}", get(view_unsubscribe).post(unsubscribe))
}

// ── request model ────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(untagged)]
enum AddressInput {
    Text(String),
    Object { email: String, name: Option<String> },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(AddressInput),
    Many(Vec<AddressInput>),
}

#[derive(Deserialize)]
struct AttachmentInput {
    filename: String,
    content: String,
    content_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendRequest {
    from: AddressInput,
    to: OneOrMany,
    subject: Option<String>,
    html: Option<String>,
    text: Option<String>,
    template: Option<String>,
    template_version: Option<i32>,
    #[serde(default)]
    variables: Map<String, Value>,
    reply_mode: Option<String>,
    reply_to: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    metadata: Map<String, Value>,
    #[serde(default)]
    attachments: Vec<AttachmentInput>,
    #[serde(default)]
    headers: Map<String, Value>,
    /// Add RFC 8058 one-click unsubscribe headers (required by Gmail and
    /// Yahoo for bulk/marketing mail; recipients who use it are suppressed).
    #[serde(default)]
    unsubscribe: bool,
    /// RFC 3339 time to send at (1 minute to 30 days ahead).
    send_at: Option<DateTime<Utc>>,
}

/// "Display Name <user@example.com>" or "user@example.com".
fn split_mailbox(s: &str) -> (Option<String>, String) {
    let s = s.trim();
    if let (Some(lt), true) = (s.rfind('<'), s.ends_with('>')) {
        let name = s[..lt].trim().trim_matches('"').trim();
        let addr = s[lt + 1..s.len() - 1].trim();
        return ((!name.is_empty()).then(|| name.to_string()), addr.to_string());
    }
    (None, s.to_string())
}

fn address(field: &str, input: &AddressInput) -> ApiResult<Addr> {
    let (name, email) = match input {
        AddressInput::Text(s) => split_mailbox(s),
        AddressInput::Object { email, name } => (name.clone(), email.clone()),
    };
    let parsed = validation::parse_email(field, &email)?;
    if let Some(n) = &name {
        validation::header_safe(&format!("{field} name"), n, 200)?;
    }
    Ok(Addr::named(name, parsed.as_string()))
}

static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9_.:-]{1,64}$").unwrap());
static HEADER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^X-[A-Za-z0-9-]{1,60}$").unwrap());
static CONTENT_TYPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9!#$&^_.+-]{0,63}/[a-zA-Z0-9][a-zA-Z0-9!#$&^_.+-]{0,63}$").unwrap());

/// File types commonly abused for malware delivery; mainstream providers
/// reject them too.
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "com", "bat", "cmd", "scr", "pif", "vbs", "vbe", "js", "jse", "wsf", "wsh", "msi", "msp", "jar", "ps1",
    "hta", "cpl", "lnk", "reg", "dll", "iso", "img",
];

pub fn check_attachment_name(filename: &str) -> ApiResult<String> {
    let clean = mime::sanitize_filename(filename);
    let ext = clean.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if clean.contains('.') && BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::validation(format!("Attachments of type .{ext} are not allowed")));
    }
    Ok(clean)
}

struct Content {
    subject: String,
    html: Option<String>,
    text: Option<String>,
    template: Option<(Uuid, i32)>,
}

async fn resolve_content(state: &AppState, p: &Principal, req: &SendRequest) -> ApiResult<Content> {
    if let Some(t) = &req.template {
        if req.subject.is_some() || req.html.is_some() || req.text.is_some() {
            return Err(AppError::validation("Provide either 'template' or 'subject'/'html'/'text', not both"));
        }
        if !p.can(Permission::TemplatesRead) && !p.can(Permission::MessagesSend) {
            return Err(AppError::forbidden("Template access denied"));
        }
        let template = super::templates::find(state, p.org_id, t).await?;
        if template.status != "active" {
            return Err(AppError::conflict("template_archived", "This template is archived"));
        }
        let version_no = req.template_version.unwrap_or(template.active_version);
        let v = super::templates::version(state, p.org_id, template.id, version_no).await?;
        let specs = tpl::parse_specs(&v.variables);
        let vars = tpl::resolve_variables(&specs, &req.variables, false)?;
        let r = tpl::render(
            &TemplateSource { subject: &v.subject, html: v.html_body.as_deref(), text: v.text_body.as_deref() },
            &vars,
        )?;
        return Ok(Content {
            subject: r.subject,
            html: r.html,
            text: r.text,
            template: Some((template.id, version_no)),
        });
    }

    let subject = req
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation("'subject' is required when no template is used"))?;
    validation::header_safe("subject", subject, 998)?;
    let html = req.html.as_deref().filter(|h| !h.trim().is_empty()).map(crate::services::sanitize::outbound);
    let text = req.text.clone().filter(|t| !t.trim().is_empty());
    if html.is_none() && text.is_none() {
        return Err(AppError::validation("Provide 'html', 'text' or both (or use a template)"));
    }
    Ok(Content { subject: subject.to_string(), html, text, template: None })
}

async fn resolve_reply(state: &AppState, org_id: Uuid, req: &SendRequest) -> ApiResult<(ReplyMode, Option<Addr>)> {
    let mode = match req.reply_mode.as_deref() {
        Some(m) => ReplyMode::parse(m)
            .ok_or_else(|| AppError::validation("reply_mode must be no_reply, reply_to_mailbox or reply_to_user"))?,
        None if req.reply_to.is_some() => ReplyMode::ReplyToUser,
        None => ReplyMode::NoReply,
    };
    let reply_to =
        req.reply_to.as_deref().map(|r| address("reply_to", &AddressInput::Text(r.to_string()))).transpose()?;
    match (mode, &reply_to) {
        (ReplyMode::NoReply, Some(_)) => {
            Err(AppError::validation("reply_to must be omitted when reply_mode is no_reply"))
        }
        (ReplyMode::NoReply, None) => Ok((mode, None)),
        (_, None) => Err(AppError::validation("reply_to is required for this reply_mode")),
        (ReplyMode::ReplyToMailbox, Some(rt)) => {
            // Must be a live hosted mailbox or alias of this tenant.
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM mailboxes WHERE organization_id = $1 AND address = $2 AND status = 'active')
                     OR EXISTS (SELECT 1 FROM aliases WHERE organization_id = $1 AND address = $2 AND status = 'active')",
            )
            .bind(org_id)
            .bind(&rt.email)
            .fetch_one(&state.db)
            .await?;
            if !ok {
                return Err(AppError::validation(format!(
                    "{} is not an active mailbox or alias in this organization",
                    rt.email
                )));
            }
            Ok((mode, reply_to))
        }
        (ReplyMode::ReplyToUser, Some(_)) => Ok((mode, reply_to)),
    }
}

fn attachments(state: &AppState, req: &SendRequest) -> ApiResult<Vec<OutAttachment>> {
    if req.attachments.len() > 10 {
        return Err(AppError::validation("At most 10 attachments per message"));
    }
    let mut total = 0usize;
    let mut out = Vec::with_capacity(req.attachments.len());
    for a in &req.attachments {
        let filename = check_attachment_name(&a.filename)?;
        let data = B64
            .decode(a.content.as_bytes())
            .map_err(|_| AppError::validation(format!("Attachment {filename:?}: content must be standard base64")))?;
        total += data.len();
        if total > state.cfg.max_attachment_bytes {
            return Err(AppError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "attachments_too_large",
                format!("Attachments exceed {} MB in total", state.cfg.max_attachment_bytes / 1024 / 1024),
            ));
        }
        let content_type = a.content_type.clone().unwrap_or_else(|| "application/octet-stream".into());
        if !CONTENT_TYPE_RE.is_match(&content_type) {
            return Err(AppError::validation(format!("Attachment {filename:?}: invalid content_type")));
        }
        out.push(OutAttachment { filename, content_type, data });
    }
    Ok(out)
}

fn extra_headers(req: &SendRequest) -> ApiResult<Vec<(String, String)>> {
    if req.headers.len() > 10 {
        return Err(AppError::validation("At most 10 custom headers"));
    }
    req.headers
        .iter()
        .map(|(k, v)| {
            if !HEADER_RE.is_match(k) || k.to_ascii_lowercase().starts_with("x-mailvora-") {
                return Err(AppError::validation(format!(
                    "Header {k:?} is not allowed; custom headers must start with 'X-'"
                )));
            }
            let value = v.as_str().ok_or_else(|| AppError::validation(format!("Header {k:?} must be a string")))?;
            validation::header_safe(&format!("header {k}"), value, 900)?;
            Ok((k.clone(), value.to_string()))
        })
        .collect()
}

// ── POST /v1/messages ────────────────────────────────────────────────────

async fn send(
    State(state): State<AppState>,
    p: Principal,
    headers: HeaderMap,
    ApiJson(body): ApiJson<Value>,
) -> ApiResult<Response> {
    p.require(Permission::MessagesSend)?;

    let idempotency_key = headers
        .get("idempotency-key")
        .map(|v| v.to_str().map(str::trim).map(str::to_string))
        .transpose()
        .map_err(|_| AppError::validation("Idempotency-Key must be ASCII"))?
        .filter(|k| !k.is_empty());
    if let Some(k) = &idempotency_key {
        if k.len() > 255 {
            return Err(AppError::validation("Idempotency-Key must be at most 255 characters"));
        }
    }
    // serde_json maps are ordered, so this is a canonical hash of the body.
    let request_hash = crypto::sha256(serde_json::to_vec(&body).map_err(AppError::internal)?);

    if let Some(key) = &idempotency_key {
        if let Some(replay) = replay_if_completed(&state, p.org_id, key, &request_hash).await? {
            return Ok(replay);
        }
    }

    let req: SendRequest = serde_json::from_value(body).map_err(|e| AppError::validation(e.to_string()))?;

    // ── validation (no database writes yet) ──
    let from = address("from", &req.from)?;
    let from_domain = from.email.rsplit_once('@').map(|(_, d)| d.to_lowercase()).unwrap_or_default();
    p.check_sender_domain(&from_domain)?;
    let domain = sending::sending_domain(&state, p.org_id, &from_domain).await?;
    if let Some(at) = req.send_at {
        let now = Utc::now();
        if at < now + chrono::Duration::seconds(60) || at > now + chrono::Duration::days(30) {
            return Err(AppError::validation("send_at must be between 1 minute and 30 days from now"));
        }
    }

    let mut recipients: Vec<Addr> = Vec::new();
    let inputs: Vec<&AddressInput> = match &req.to {
        OneOrMany::One(a) => vec![a],
        OneOrMany::Many(v) => v.iter().collect(),
    };
    for (i, input) in inputs.iter().enumerate() {
        let a = address(&format!("to[{i}]"), input)?;
        if !recipients.iter().any(|r| r.email.eq_ignore_ascii_case(&a.email)) {
            recipients.push(a);
        }
    }
    if recipients.is_empty() {
        return Err(AppError::validation("At least one recipient is required"));
    }
    let max_recipients: i32 = sqlx::query_scalar(
        "SELECT p.max_recipients_per_request FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    if recipients.len() > max_recipients as usize {
        return Err(AppError::validation(format!("At most {max_recipients} recipients per request on your plan")));
    }

    if req.tags.len() > 10 || req.tags.iter().any(|t| !TAG_RE.is_match(t)) {
        return Err(AppError::validation("Up to 10 tags; each 1–64 chars of letters, digits, '_', '.', ':', '-'"));
    }
    let metadata = Value::Object(req.metadata.clone());
    if req.metadata.len() > 20 || metadata.to_string().len() > 4096 {
        return Err(AppError::validation("metadata may have at most 20 keys and 4 KB"));
    }

    let (reply_mode, reply_to) = resolve_reply(&state, p.org_id, &req).await?;
    let content = resolve_content(&state, &p, &req).await?;
    let files = attachments(&state, &req)?;
    let custom_headers = extra_headers(&req)?;

    // ── build one signed MIME message per recipient ──
    let batch_id = Uuid::now_v7();
    struct Built {
        id: Uuid,
        smtp_id: String,
        to: Addr,
        envelope_from: String,
        raw: Vec<u8>,
    }
    let mut built = Vec::with_capacity(recipients.len());
    for to in &recipients {
        let (id, smtp_id) = sending::new_ids(&state);
        let mut hdrs = custom_headers.clone();
        hdrs.push(("X-Mailvora-Message-ID".into(), id.to_string()));
        if req.unsubscribe {
            hdrs.push(("List-Unsubscribe".into(), format!("<{}>", crate::services::unsubscribe::url(&state.cfg, id))));
            hdrs.push(("List-Unsubscribe-Post".into(), "List-Unsubscribe=One-Click".into()));
        }
        let email = OutgoingEmail {
            from: Some(from.clone()),
            to: vec![to.clone()],
            reply_to: reply_to.clone(),
            subject: content.subject.clone(),
            message_id: smtp_id.clone(),
            text: content.text.clone(),
            html: content.html.as_deref().map(crate::services::sanitize::wrap_document),
            attachments: files.clone(),
            headers: hdrs,
            ..Default::default()
        };
        let message = mime::build(&email).map_err(|e| AppError::validation(format!("Could not build message: {e}")))?;
        let raw = sending::sign(&state, &domain, message)?;
        if raw.len() > state.cfg.max_message_bytes {
            return Err(AppError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "message_too_large",
                format!("Message exceeds {} MB", state.cfg.max_message_bytes / 1024 / 1024),
            ));
        }
        built.push(Built { id, smtp_id, to: to.clone(), envelope_from: sending::envelope_from(&state, id), raw });
    }

    // ── persist atomically ──
    let mut tx = state.db.begin().await?;
    if let Some(key) = &idempotency_key {
        let inserted = sqlx::query("INSERT INTO idempotency_keys (organization_id, key, request_hash) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(p.org_id)
            .bind(key)
            .bind(&request_hash)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if inserted == 0 {
            // A concurrent request with the same key committed first.
            drop(tx);
            return match replay_if_completed(&state, p.org_id, key, &request_hash).await? {
                Some(r) => Ok(r),
                None => Err(AppError::conflict(
                    "idempotency_in_progress",
                    "A request with this Idempotency-Key is still being processed",
                )),
            };
        }
    }

    let all: Vec<String> = built.iter().map(|b| b.to.email.to_lowercase()).collect();
    let suppressed = sending::suppressed_among(&mut tx, p.org_id, &all).await?;
    let to_send = all.iter().filter(|a| !suppressed.contains(a)).count() as i32;
    sending::reserve_quota(&mut tx, p.org_id, to_send).await?;

    let mut results = Vec::with_capacity(built.len());
    for b in &built {
        let rcpt = vec![b.to.email.clone()];
        let is_suppressed = suppressed.contains(&b.to.email.to_lowercase());
        let status = sending::insert(
            &mut tx,
            &NewMessage {
                id: b.id,
                org_id: p.org_id,
                batch_id,
                source: if p.api_key_id().is_some() { "api" } else { "console" },
                api_key_id: p.api_key_id(),
                user_id: p.user_id(),
                mailbox_id: None,
                domain_id: domain.id,
                from_address: &from.email,
                from_name: from.name.as_deref(),
                envelope_from: &b.envelope_from,
                to_display: &b.to.email,
                recipients: &rcpt,
                reply_to: reply_to.as_ref().map(|r| r.email.as_str()),
                reply_mode,
                subject: &content.subject,
                template: content.template,
                sandbox: p.is_sandbox(),
                smtp_message_id: &b.smtp_id,
                tags: &req.tags,
                metadata: &metadata,
                idempotency_key: idempotency_key.as_deref(),
                raw: &b.raw,
                html: content.html.as_deref(),
                text: content.text.as_deref(),
                suppressed: is_suppressed,
                scheduled_at: req.send_at,
                list_unsubscribe: req.unsubscribe,
            },
        )
        .await?;
        results.push(json!({ "id": b.id, "to": b.to.email, "status": status, "smtp_message_id": b.smtp_id }));
    }

    let response = json!({
        "batch_id": batch_id,
        "status": "accepted",
        "sandbox": p.is_sandbox(),
        "scheduled_at": req.send_at,
        "messages": results,
    });
    if let Some(key) = &idempotency_key {
        sqlx::query("UPDATE idempotency_keys SET response_status = 202, response_body = $3 WHERE organization_id = $1 AND key = $2")
            .bind(p.org_id)
            .bind(key)
            .bind(&response)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

async fn replay_if_completed(
    state: &AppState,
    org_id: Uuid,
    key: &str,
    request_hash: &[u8],
) -> ApiResult<Option<Response>> {
    let existing: Option<(Vec<u8>, Option<i32>, Option<Value>)> = sqlx::query_as(
        "SELECT request_hash, response_status, response_body FROM idempotency_keys WHERE organization_id = $1 AND key = $2",
    )
    .bind(org_id)
    .bind(key)
    .fetch_optional(&state.db)
    .await?;
    let Some((hash, status, body)) = existing else { return Ok(None) };
    if !crypto::constant_time_eq(&hash, request_hash) {
        return Err(AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "idempotency_key_reused",
            "This Idempotency-Key was already used with a different request body",
        ));
    }
    match (status, body) {
        (Some(s), Some(b)) => {
            let mut res = (StatusCode::from_u16(s as u16).unwrap_or(StatusCode::ACCEPTED), Json(b)).into_response();
            res.headers_mut().insert(HeaderName::from_static("idempotent-replayed"), HeaderValue::from_static("true"));
            Ok(Some(res))
        }
        _ => Err(AppError::conflict(
            "idempotency_in_progress",
            "A request with this Idempotency-Key is still being processed",
        )),
    }
}

// ── reads ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    q: Option<String>,
    tag: Option<String>,
    batch_id: Option<Uuid>,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: Uuid,
    batch_id: Uuid,
    source: String,
    from_address: String,
    from_name: Option<String>,
    to_address: String,
    reply_to: Option<String>,
    reply_mode: String,
    subject: String,
    status: String,
    sandbox: bool,
    attempts: i32,
    last_error: Option<String>,
    last_smtp_code: Option<i32>,
    tags: Vec<String>,
    metadata: Value,
    template_id: Option<Uuid>,
    template_version: Option<i32>,
    smtp_message_id: String,
    size_bytes: i32,
    accepted_at: Option<DateTime<Utc>>,
    bounced_at: Option<DateTime<Utc>>,
    scheduled_at: Option<DateTime<Utc>>,
    list_unsubscribe: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const MESSAGE_COLUMNS: &str = "id, batch_id, source, from_address::text AS from_address, from_name, to_address, reply_to, reply_mode, subject,
    status, sandbox, attempts, last_error, last_smtp_code, tags, metadata, template_id, template_version, smtp_message_id,
    size_bytes, accepted_at, bounced_at, scheduled_at, list_unsubscribe, created_at, updated_at";

fn message_json(m: &MessageRow) -> Value {
    json!({
        "id": m.id, "batch_id": m.batch_id, "source": m.source,
        "from": m.from_address, "from_name": m.from_name, "to": m.to_address,
        "reply_to": m.reply_to, "reply_mode": m.reply_mode, "subject": m.subject,
        "status": m.status, "sandbox": m.sandbox, "attempts": m.attempts,
        "last_error": m.last_error, "last_smtp_code": m.last_smtp_code,
        "tags": m.tags, "metadata": m.metadata,
        "template_id": m.template_id, "template_version": m.template_version,
        "smtp_message_id": m.smtp_message_id, "size_bytes": m.size_bytes,
        "accepted_at": m.accepted_at, "bounced_at": m.bounced_at,
        "scheduled_at": m.scheduled_at, "list_unsubscribe": m.list_unsubscribe,
        "created_at": m.created_at, "updated_at": m.updated_at,
    })
}

const STATUSES: &[&str] = &[
    "scheduled",
    "cancelled",
    "queued",
    "processing",
    "accepted_by_mta",
    "delivered",
    "deferred",
    "bounced",
    "failed",
    "suppressed",
    "sandboxed",
];

async fn list_messages(
    State(state): State<AppState>,
    p: Principal,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MessagesRead)?;
    let limit = page_limit(q.limit, 50, 200);
    let cursor = Cursor::decode(q.cursor.as_deref())?;
    let status = empty_as_none(q.status);
    if let Some(s) = &status {
        if !STATUSES.contains(&s.as_str()) {
            return Err(AppError::validation(format!("Unknown status {s:?}")));
        }
    }
    let search =
        empty_as_none(q.q).map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let rows: Vec<MessageRow> = sqlx::query_as(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE organization_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR to_address ILIKE $3 OR subject ILIKE $3 OR from_address::text ILIKE $3)
           AND ($4::text IS NULL OR $4 = ANY(tags))
           AND ($5::uuid IS NULL OR batch_id = $5)
           AND ($6::timestamptz IS NULL OR (created_at, id) < ($6, $7))
         ORDER BY created_at DESC, id DESC
         LIMIT $8"
    ))
    .bind(p.org_id)
    .bind(&status)
    .bind(&search)
    .bind(empty_as_none(q.tag))
    .bind(q.batch_id)
    .bind(cursor.as_ref().map(|c| c.t))
    .bind(cursor.as_ref().map(|c| c.id))
    .bind(limit + 1)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let page = &rows[..rows.len().min(limit as usize)];
    let next_cursor = if has_more { page.last().map(|m| Cursor::encode(m.created_at, m.id)) } else { None };
    Ok(Json(json!({ "data": page.iter().map(message_json).collect::<Vec<_>>(), "next_cursor": next_cursor })))
}

async fn get_message(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::MessagesRead)?;
    let m: MessageRow =
        sqlx::query_as(&format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = $1 AND organization_id = $2"))
            .bind(id)
            .bind(p.org_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::not_found("Message"))?;
    let events: Vec<(String, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT event_type, details, occurred_at FROM message_events WHERE message_id = $1 AND organization_id = $2 ORDER BY occurred_at, id",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    #[derive(sqlx::FromRow)]
    struct ContentRow {
        html_body: Option<String>,
        text_body: Option<String>,
        purged_at: Option<DateTime<Utc>>,
    }
    let content: Option<ContentRow> =
        sqlx::query_as("SELECT html_body, text_body, purged_at FROM message_contents WHERE message_id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let mut out = message_json(&m);
    out["events"] = json!(events
        .into_iter()
        .map(|(t, d, at)| json!({ "type": t, "details": d, "occurred_at": at }))
        .collect::<Vec<_>>());
    out["content"] = match content {
        Some(ContentRow { purged_at: Some(purged), .. }) => json!({ "purged_at": purged }),
        Some(ContentRow { html_body, text_body, purged_at: None }) => {
            json!({ "html": html_body.as_deref().map(crate::services::sanitize::wrap_document), "text": text_body })
        }
        None => Value::Null,
    };
    Ok(Json(out))
}

// ── scheduling ───────────────────────────────────────────────────────────

/// Cancel a scheduled message before it is handed to the mail server. The
/// reserved quota is returned to the day it was counted on.
async fn cancel_message(State(state): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    p.require(Permission::MessagesSend)?;
    let mut tx = state.db.begin().await?;
    let row: Option<(DateTime<Utc>,)> = sqlx::query_as(
        "UPDATE messages SET status = 'cancelled', last_error = 'Cancelled before sending', updated_at = now()
         WHERE id = $1 AND organization_id = $2 AND status = 'scheduled' RETURNING created_at",
    )
    .bind(id)
    .bind(p.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((created_at,)) = row else {
        return Err(AppError::conflict(
            "not_cancellable",
            "Only scheduled messages that have not been sent can be cancelled",
        ));
    };
    sqlx::query("UPDATE outbox_jobs SET status = 'done', last_error = 'cancelled', updated_at = now() WHERE message_id = $1 AND status = 'pending'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE usage_daily SET messages_queued = greatest(messages_queued - 1, 0)
         WHERE organization_id = $1 AND day = ($2 AT TIME ZONE 'UTC')::date",
    )
    .bind(p.org_id)
    .bind(created_at)
    .execute(&mut *tx)
    .await?;
    let (actor_type, _, label) = p.audit_actor();
    crate::services::events::message_event(
        &mut tx,
        p.org_id,
        id,
        "cancelled",
        json!({ "by": label, "actor_type": actor_type }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "cancelled" })))
}

// ── export ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExportQuery {
    status: Option<String>,
    q: Option<String>,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
}

/// RFC 4180 field with spreadsheet formula injection neutralized (OWASP
/// "CSV injection"): values starting with = + - @ tab or CR get a quote.
pub fn csv_field(v: &str) -> String {
    let guarded = if v.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{v}") } else { v.to_string() };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

async fn export_csv(
    State(state): State<AppState>,
    p: Principal,
    ApiQuery(q): ApiQuery<ExportQuery>,
) -> ApiResult<Response> {
    p.require(Permission::MessagesRead)?;
    state.limiter.check(&format!("export:{}", p.org_id), 20, 3600)?;
    let status = empty_as_none(q.status);
    if let Some(s) = &status {
        if !STATUSES.contains(&s.as_str()) {
            return Err(AppError::validation(format!("Unknown status {s:?}")));
        }
    }
    let search =
        empty_as_none(q.q).map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let rows: Vec<MessageRow> = sqlx::query_as(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE organization_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR to_address ILIKE $3 OR subject ILIKE $3 OR from_address::text ILIKE $3)
           AND ($4::date IS NULL OR created_at >= $4::date)
           AND ($5::date IS NULL OR created_at < $5::date + 1)
         ORDER BY created_at DESC, id DESC LIMIT 50000"
    ))
    .bind(p.org_id)
    .bind(&status)
    .bind(&search)
    .bind(q.from)
    .bind(q.to)
    .fetch_all(&state.db)
    .await?;
    let mut out = String::from(
        "id,created_at,status,source,from,to,subject,tags,attempts,last_smtp_code,last_error,accepted_at,scheduled_at\r\n",
    );
    for m in &rows {
        let fields = [
            m.id.to_string(),
            m.created_at.to_rfc3339(),
            m.status.clone(),
            m.source.clone(),
            m.from_address.clone(),
            m.to_address.clone(),
            m.subject.clone(),
            m.tags.join(" "),
            m.attempts.to_string(),
            m.last_smtp_code.map(|c| c.to_string()).unwrap_or_default(),
            m.last_error.clone().unwrap_or_default(),
            m.accepted_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
            m.scheduled_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
        ];
        out.push_str(&fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    let filename = format!("messages-{}.csv", Utc::now().format("%Y%m%d-%H%M"));
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        out,
    )
        .into_response())
}

// ── one-click unsubscribe (public, RFC 8058) ─────────────────────────────

#[derive(sqlx::FromRow)]
struct UnsubTarget {
    organization_id: Uuid,
    to_address: String,
    organization: String,
    from_address: String,
}

async fn unsub_target(state: &AppState, token: &str) -> ApiResult<(Uuid, UnsubTarget)> {
    let id = crate::services::unsubscribe::verify(&state.cfg, token)
        .ok_or_else(|| AppError::not_found("Unsubscribe link"))?;
    let t: UnsubTarget = sqlx::query_as(
        "SELECT m.organization_id, m.to_address, o.name AS organization, m.from_address::text AS from_address
         FROM messages m JOIN organizations o ON o.id = m.organization_id WHERE m.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Unsubscribe link"))?;
    Ok((id, t))
}

fn mask(e: &str) -> String {
    super::mailboxes::mask_email(e)
}

async fn view_unsubscribe(
    State(state): State<AppState>,
    client: crate::auth::ClientInfo,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("unsub-view:{}", client.ip_key()), 120, 3600)?;
    let (_, t) = unsub_target(&state, &token).await?;
    let already: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM suppressions WHERE organization_id = $1 AND email = $2)")
            .bind(t.organization_id)
            .bind(&t.to_address)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(
        json!({ "organization": t.organization, "sender": t.from_address, "email": mask(&t.to_address), "unsubscribed": already }),
    ))
}

async fn unsubscribe(
    State(state): State<AppState>,
    client: crate::auth::ClientInfo,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("unsub:{}", client.ip_key()), 60, 3600)?;
    let (message_id, t) = unsub_target(&state, &token).await?;
    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO suppressions (organization_id, email, reason, source_message_id, details)
         VALUES ($1, $2, 'unsubscribe', $3, 'One-click unsubscribe') ON CONFLICT (organization_id, email) DO NOTHING",
    )
    .bind(t.organization_id)
    .bind(&t.to_address)
    .bind(message_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted > 0 {
        crate::services::events::message_event(
            &mut tx,
            t.organization_id,
            message_id,
            "unsubscribed",
            json!({ "method": "one_click" }),
        )
        .await?;
        sqlx::query(
            "INSERT INTO audit_logs (organization_id, actor_type, action, target_type, target_id, ip, details)
             VALUES ($1, 'system', 'suppression.unsubscribed', 'message', $2, $3, $4)",
        )
        .bind(t.organization_id)
        .bind(message_id.to_string())
        .bind(client.ip_network())
        .bind(json!({ "email": mask(&t.to_address) }))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true, "organization": t.organization, "email": mask(&t.to_address) })))
}

#[cfg(test)]
mod csv_tests {
    use super::csv_field;

    #[test]
    fn csv_fields_are_quoted_and_formula_safe() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_field("+254700000000"), "'+254700000000");
    }
}
