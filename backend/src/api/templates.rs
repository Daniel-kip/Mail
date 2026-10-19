//! Versioned email templates.
//!
//! Saving content always creates a new immutable version; `active_version`
//! selects which one is used for sends, so rollback is just re-activating
//! an older version. Each sent message records the version it used.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{ClientInfo, Permission, Principal},
    error::{on_unique_violation, ApiResult, AppError},
    services::{
        audit::Entry,
        sanitize,
        templates::{self, TemplateSource, VariableSpec},
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/templates", get(list_templates).post(create_template))
        .route("/templates/preview", post(preview_unsaved))
        .route("/templates/{id}", get(get_template).put(save_version).patch(update_meta))
        .route("/templates/{id}/versions", get(list_versions))
        .route("/templates/{id}/versions/{version}", get(get_version))
        .route("/templates/{id}/versions/{version}/activate", post(activate_version))
        .route("/templates/{id}/preview", post(preview_saved))
}

#[derive(sqlx::FromRow)]
pub struct TemplateRow {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: String,
    pub active_version: i32,
    pub latest_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct VersionRow {
    pub version: i32,
    pub subject: String,
    pub html_body: Option<String>,
    pub text_body: Option<String>,
    pub variables: Value,
    pub created_by_email: Option<String>,
    pub created_at: DateTime<Utc>,
}

const SELECT_TEMPLATE: &str = "SELECT id, name, slug::text AS slug, description, status, active_version, latest_version, created_at, updated_at FROM templates";

/// Find a template by UUID or slug within the tenant.
pub async fn find(state: &AppState, org_id: Uuid, id_or_slug: &str) -> ApiResult<TemplateRow> {
    let row: Option<TemplateRow> = match Uuid::parse_str(id_or_slug) {
        Ok(id) => {
            sqlx::query_as(&format!("{SELECT_TEMPLATE} WHERE id = $1 AND organization_id = $2"))
                .bind(id)
                .bind(org_id)
                .fetch_optional(&state.db)
                .await?
        }
        Err(_) => {
            sqlx::query_as(&format!("{SELECT_TEMPLATE} WHERE slug = $1 AND organization_id = $2"))
                .bind(id_or_slug)
                .bind(org_id)
                .fetch_optional(&state.db)
                .await?
        }
    };
    row.ok_or_else(|| AppError::not_found("Template"))
}

pub async fn version(state: &AppState, org_id: Uuid, template_id: Uuid, version: i32) -> ApiResult<VersionRow> {
    sqlx::query_as(
        "SELECT v.version, v.subject, v.html_body, v.text_body, v.variables, u.email::text AS created_by_email, v.created_at
         FROM template_versions v LEFT JOIN users u ON u.id = v.created_by
         WHERE v.template_id = $1 AND v.organization_id = $2 AND v.version = $3",
    )
    .bind(template_id)
    .bind(org_id)
    .bind(version)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Template version"))
}

fn template_json(t: &TemplateRow) -> Value {
    json!({
        "id": t.id, "name": t.name, "slug": t.slug, "description": t.description, "status": t.status,
        "active_version": t.active_version, "latest_version": t.latest_version,
        "created_at": t.created_at, "updated_at": t.updated_at,
    })
}

fn version_json(v: &VersionRow) -> Value {
    json!({
        "version": v.version, "subject": v.subject, "html_body": v.html_body, "text_body": v.text_body,
        "variables": v.variables, "created_by": v.created_by_email, "created_at": v.created_at,
    })
}

async fn list_templates(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    let rows: Vec<TemplateRow> =
        sqlx::query_as(&format!("{SELECT_TEMPLATE} WHERE organization_id = $1 ORDER BY status, name"))
            .bind(p.org_id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(json!({ "data": rows.iter().map(template_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct ContentBody {
    subject: String,
    html_body: Option<String>,
    text_body: Option<String>,
    #[serde(default)]
    variables: Vec<VariableSpec>,
}

impl ContentBody {
    fn normalized(&self) -> (Option<String>, Option<String>) {
        let clean = |s: &Option<String>| s.as_ref().map(|v| v.to_string()).filter(|v| !v.trim().is_empty());
        (clean(&self.html_body), clean(&self.text_body))
    }

    /// Validate syntax, declared variables, and that a sample render works.
    fn validate(&self) -> ApiResult<Vec<String>> {
        validation::header_safe("subject", &self.subject, 998)?;
        if self.subject.trim().is_empty() {
            return Err(AppError::validation("subject is required"));
        }
        templates::validate_specs(&self.variables)?;
        let (html, text) = self.normalized();
        let used = templates::validate(&TemplateSource {
            subject: &self.subject,
            html: html.as_deref(),
            text: text.as_deref(),
        })?;
        Ok(used)
    }
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    slug: Option<String>,
    description: Option<String>,
    #[serde(flatten)]
    content: ContentBody,
}

async fn create_template(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::TemplatesWrite)?;
    let name = validation::non_empty("name", &body.name, 120)?;
    let slug = match body.slug.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => validation::slug("slug", s)?,
        None => validation::slug("slug", &validation::slugify(&name))?,
    };
    let undeclared = undeclared(&body.content)?;
    let (html, text) = body.content.normalized();

    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO templates (organization_id, name, slug, description) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(p.org_id)
    .bind(&name)
    .bind(&slug)
    .bind(body.description.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| on_unique_violation(e, "template_slug_taken", "A template with this slug already exists"))?;
    insert_version(
        &mut tx,
        p.org_id,
        id,
        1,
        &body.content.subject,
        html.as_deref(),
        text.as_deref(),
        &body.content.variables,
        p.user_id(),
    )
    .await?;
    Entry::by(&p, &client, "template.created")
        .target("template", id)
        .details(json!({ "slug": slug }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let t = find(&state, p.org_id, &id.to_string()).await?;
    let v = version(&state, p.org_id, id, 1).await?;
    let mut out = template_json(&t);
    out["content"] = version_json(&v);
    out["warnings"] = json!(undeclared);
    Ok((StatusCode::CREATED, Json(out)))
}

/// Variables used in the template but not declared (informational).
fn undeclared(content: &ContentBody) -> ApiResult<Vec<String>> {
    let used = content.validate()?;
    Ok(used
        .into_iter()
        .filter(|u| !content.variables.iter().any(|v| &v.name == u))
        .map(|u| format!("Variable '{u}' is used but not declared; sends will fail unless it is provided"))
        .collect())
}

#[allow(clippy::too_many_arguments)]
async fn insert_version(
    tx: &mut sqlx::PgConnection,
    org_id: Uuid,
    template_id: Uuid,
    version: i32,
    subject: &str,
    html: Option<&str>,
    text: Option<&str>,
    variables: &[VariableSpec],
    user: Option<Uuid>,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO template_versions (template_id, organization_id, version, subject, html_body, text_body, variables, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(template_id)
    .bind(org_id)
    .bind(version)
    .bind(subject.trim())
    .bind(html)
    .bind(text)
    .bind(serde_json::to_value(variables).map_err(AppError::internal)?)
    .bind(user)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

async fn get_template(State(state): State<AppState>, p: Principal, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    let t = find(&state, p.org_id, &id).await?;
    let v = version(&state, p.org_id, t.id, t.active_version).await?;
    let mut out = template_json(&t);
    out["content"] = version_json(&v);
    Ok(Json(out))
}

#[derive(Deserialize)]
struct SaveVersionBody {
    #[serde(flatten)]
    content: ContentBody,
    #[serde(default = "yes")]
    activate: bool,
}

fn yes() -> bool {
    true
}

async fn save_version(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<SaveVersionBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesWrite)?;
    let undeclared = undeclared(&body.content)?;
    let (html, text) = body.content.normalized();
    let t = find(&state, p.org_id, &id).await?;

    let mut tx = state.db.begin().await?;
    // Row lock serializes concurrent saves so version numbers never collide.
    let next: i32 = sqlx::query_scalar(
        "UPDATE templates SET latest_version = latest_version + 1, updated_at = now()
         WHERE id = $1 AND organization_id = $2 RETURNING latest_version",
    )
    .bind(t.id)
    .bind(p.org_id)
    .fetch_one(&mut *tx)
    .await?;
    insert_version(
        &mut tx,
        p.org_id,
        t.id,
        next,
        &body.content.subject,
        html.as_deref(),
        text.as_deref(),
        &body.content.variables,
        p.user_id(),
    )
    .await?;
    if body.activate {
        sqlx::query("UPDATE templates SET active_version = $2 WHERE id = $1")
            .bind(t.id)
            .bind(next)
            .execute(&mut *tx)
            .await?;
    }
    Entry::by(&p, &client, "template.version_created")
        .target("template", t.id)
        .details(json!({ "version": next, "activated": body.activate }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    let t = find(&state, p.org_id, &t.id.to_string()).await?;
    let v = version(&state, p.org_id, t.id, next).await?;
    let mut out = template_json(&t);
    out["content"] = version_json(&v);
    out["warnings"] = json!(undeclared);
    Ok(Json(out))
}

#[derive(Deserialize)]
struct MetaBody {
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
}

async fn update_meta(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<MetaBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesWrite)?;
    let t = find(&state, p.org_id, &id).await?;
    let name = match &body.name {
        Some(n) => validation::non_empty("name", n, 120)?,
        None => t.name.clone(),
    };
    let status = match body.status.as_deref() {
        None => t.status.clone(),
        Some(s @ ("active" | "archived")) => s.to_string(),
        Some(_) => return Err(AppError::validation("status must be 'active' or 'archived'")),
    };
    let description =
        body.description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).or(t.description.clone());
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE templates SET name = $2, description = $3, status = $4, updated_at = now() WHERE id = $1")
        .bind(t.id)
        .bind(&name)
        .bind(&description)
        .bind(&status)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "template.updated")
        .target("template", t.id)
        .details(json!({ "status": status }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    let t = find(&state, p.org_id, &t.id.to_string()).await?;
    Ok(Json(template_json(&t)))
}

async fn list_versions(State(state): State<AppState>, p: Principal, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    let t = find(&state, p.org_id, &id).await?;
    let rows: Vec<(i32, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT v.version, v.subject, u.email::text, v.created_at
         FROM template_versions v LEFT JOIN users u ON u.id = v.created_by
         WHERE v.template_id = $1 AND v.organization_id = $2 ORDER BY v.version DESC",
    )
    .bind(t.id)
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;
    let data: Vec<Value> = rows
        .into_iter()
        .map(|(v, s, by, at)| json!({ "version": v, "subject": s, "created_by": by, "created_at": at, "active": v == t.active_version }))
        .collect();
    Ok(Json(json!({ "data": data })))
}

async fn get_version(
    State(state): State<AppState>,
    p: Principal,
    Path((id, v)): Path<(String, i32)>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    let t = find(&state, p.org_id, &id).await?;
    let row = version(&state, p.org_id, t.id, v).await?;
    let mut out = version_json(&row);
    out["active"] = json!(v == t.active_version);
    Ok(Json(out))
}

async fn activate_version(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path((id, v)): Path<(String, i32)>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesWrite)?;
    let t = find(&state, p.org_id, &id).await?;
    version(&state, p.org_id, t.id, v).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE templates SET active_version = $2, updated_at = now() WHERE id = $1")
        .bind(t.id)
        .bind(v)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "template.version_activated")
        .target("template", t.id)
        .details(json!({ "from": t.active_version, "to": v }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    let t = find(&state, p.org_id, &t.id.to_string()).await?;
    Ok(Json(template_json(&t)))
}

fn preview_response(rendered: templates::Rendered) -> Value {
    json!({
        "subject": rendered.subject,
        "html": rendered.html.as_deref().map(sanitize::wrap_document),
        "text": rendered.text,
    })
}

#[derive(Deserialize)]
struct PreviewUnsavedBody {
    #[serde(flatten)]
    content: ContentBody,
    #[serde(default)]
    data: Map<String, Value>,
}

/// Render editor content that has not been saved yet (live preview).
async fn preview_unsaved(
    State(state): State<AppState>,
    p: Principal,
    ApiJson(body): ApiJson<PreviewUnsavedBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    state.limiter.check(&format!("preview:{}", p.org_id), 300, 60)?;
    body.content.validate()?;
    let (html, text) = body.content.normalized();
    let vars = templates::resolve_variables(&body.content.variables, &body.data, true)?;
    let rendered = templates::render(
        &TemplateSource { subject: &body.content.subject, html: html.as_deref(), text: text.as_deref() },
        &vars,
    )?;
    Ok(Json(preview_response(rendered)))
}

#[derive(Deserialize)]
struct PreviewSavedBody {
    version: Option<i32>,
    #[serde(default)]
    data: Map<String, Value>,
}

async fn preview_saved(
    State(state): State<AppState>,
    p: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<PreviewSavedBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::TemplatesRead)?;
    let t = find(&state, p.org_id, &id).await?;
    let v = version(&state, p.org_id, t.id, body.version.unwrap_or(t.active_version)).await?;
    let specs = templates::parse_specs(&v.variables);
    let vars = templates::resolve_variables(&specs, &body.data, true)?;
    let rendered = templates::render(
        &TemplateSource { subject: &v.subject, html: v.html_body.as_deref(), text: v.text_body.as_deref() },
        &vars,
    )?;
    Ok(Json(preview_response(rendered)))
}
