//! Organization settings, membership and invitations.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::common::ApiJson;
use crate::{
    auth::{ClientInfo, ConsoleSession, Permission, Principal, Role},
    crypto,
    error::{ApiResult, AppError},
    services::{audit::Entry, system_mail},
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/organization", get(get_organization).patch(update_organization))
        .route("/organization/convert", post(convert_to_institution))
        .route("/organization/support", get(get_support).put(update_support))
        .route("/organizations", post(create_organization))
        .route("/members", get(list_members))
        .route("/members/{user_id}", patch(change_role).delete(remove_member))
        .route("/members/invitations", post(invite))
        .route("/members/invitations/{id}", delete(revoke_invitation))
        .route("/invitations/{token}", get(view_invitation))
        .route("/invitations/{token}/accept", post(accept_invitation))
}

#[derive(sqlx::FromRow)]
struct OrgRow {
    id: Uuid,
    name: String,
    slug: String,
    status: String,
    suspended_reason: Option<String>,
    plan_id: String,
    plan_name: String,
    daily_send_limit: i32,
    monthly_send_limit: i32,
    max_domains: i32,
    max_mailboxes: i32,
    default_mailbox_quota_bytes: i64,
    max_mailbox_quota_bytes: i64,
    max_recipients_per_request: i32,
    api_requests_per_minute: i32,
    max_members: i32,
    max_api_keys: i32,
    max_webhooks: i32,
    audience: String,
    account_type: String,
    created_at: DateTime<Utc>,
}

async fn get_organization(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    let o: OrgRow = sqlx::query_as(
        "SELECT o.id, o.name, o.slug::text AS slug, o.status, o.suspended_reason, o.plan_id, p.name AS plan_name,
                p.daily_send_limit, p.monthly_send_limit, p.max_domains, p.max_mailboxes,
                p.default_mailbox_quota_bytes, p.max_mailbox_quota_bytes, p.max_recipients_per_request,
                p.api_requests_per_minute, p.max_members, p.max_api_keys, p.max_webhooks, p.audience,
                o.account_type, o.created_at
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({
        "id": o.id, "name": o.name, "slug": o.slug, "status": o.status, "account_type": o.account_type,
        "suspended_reason": o.suspended_reason, "created_at": o.created_at,
        "plan": {
            "id": o.plan_id, "name": o.plan_name, "audience": o.audience,
            "daily_send_limit": o.daily_send_limit, "monthly_send_limit": o.monthly_send_limit,
            "max_domains": o.max_domains, "max_mailboxes": o.max_mailboxes,
            "default_mailbox_quota_bytes": o.default_mailbox_quota_bytes,
            "max_mailbox_quota_bytes": o.max_mailbox_quota_bytes,
            "max_recipients_per_request": o.max_recipients_per_request,
            "api_requests_per_minute": o.api_requests_per_minute,
            "max_members": o.max_members, "max_api_keys": o.max_api_keys, "max_webhooks": o.max_webhooks,
        }
    })))
}

/// Turn a personal (individual) workspace into an institution. The plan
/// moves to the free institutional plan; existing keys, domains and
/// templates are kept. One-way: institutions cannot become individuals,
/// because hosted mailboxes would be orphaned.
async fn convert_to_institution(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
) -> ApiResult<Json<Value>> {
    p.require(Permission::OrganizationManage)?;
    if p.role() != Some(Role::Owner) {
        return Err(AppError::forbidden("Only an owner can change the account type"));
    }
    if p.is_institution() {
        return Err(AppError::conflict("already_institution", "This workspace is already an institution"));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE organizations SET account_type = 'institution', plan_id = 'free', updated_at = now() WHERE id = $1",
    )
    .bind(p.org_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE subscriptions SET plan_id = 'free', status = 'active', pending_plan_id = NULL, cancel_at_period_end = false,
                current_period_start = now(), current_period_end = now() + interval '1 month', updated_at = now()
         WHERE organization_id = $1",
    )
    .bind(p.org_id)
    .execute(&mut *tx)
    .await?;
    Entry::by(&p, &client, "organization.converted")
        .target("organization", p.org_id)
        .details(json!({ "from": "individual", "to": "institution", "plan": "free" }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": p.org_id, "account_type": "institution", "plan": "free" })))
}

// ── employee support contact (shown in mailbox setup and help pages) ─────

#[derive(sqlx::FromRow)]
struct SupportRow {
    support_email: Option<String>,
    support_phone: Option<String>,
    support_url: Option<String>,
    support_hours: Option<String>,
    setup_help_text: Option<String>,
}

pub async fn support_settings(state: &AppState, org_id: Uuid) -> ApiResult<Value> {
    let r: SupportRow = sqlx::query_as(
        "SELECT support_email::text AS support_email, support_phone, support_url, support_hours, setup_help_text
         FROM organizations WHERE id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.db)
    .await?;
    Ok(json!({
        "email": r.support_email, "phone": r.support_phone, "url": r.support_url,
        "hours": r.support_hours, "setup_help_text": r.setup_help_text,
    }))
}

async fn get_support(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::MailboxesRead)?;
    Ok(Json(support_settings(&state, p.org_id).await?))
}

#[derive(Deserialize)]
struct SupportBody {
    email: Option<String>,
    phone: Option<String>,
    url: Option<String>,
    hours: Option<String>,
    setup_help_text: Option<String>,
}

fn opt_text(field: &str, v: Option<String>, max: usize) -> ApiResult<Option<String>> {
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) if s.chars().count() > max => {
            Err(AppError::validation(format!("{field} must be at most {max} characters")))
        }
        Some(s) if s.chars().any(|c| c.is_control() && c != '\n' && c != '\r') => {
            Err(AppError::validation(format!("{field} contains control characters")))
        }
        Some(s) => Ok(Some(s)),
    }
}

async fn update_support(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<SupportBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::OrganizationManage)?;
    let email = match body.email.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(e) => Some(validation::parse_email("email", e)?.as_string().to_lowercase()),
        None => None,
    };
    let url = opt_text("url", body.url, 500)?;
    if let Some(u) = &url {
        if !(u.starts_with("https://") || u.starts_with("http://")) || u.contains(char::is_whitespace) {
            return Err(AppError::validation("url must be an http(s) link"));
        }
    }
    let phone = opt_text("phone", body.phone, 40)?;
    let hours = opt_text("hours", body.hours, 200)?;
    let help = opt_text("setup_help_text", body.setup_help_text, 2000)?;
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE organizations SET support_email = $2, support_phone = $3, support_url = $4, support_hours = $5,
                setup_help_text = $6, updated_at = now() WHERE id = $1",
    )
    .bind(p.org_id)
    .bind(&email)
    .bind(&phone)
    .bind(&url)
    .bind(&hours)
    .bind(&help)
    .execute(&mut *tx)
    .await?;
    Entry::by(&p, &client, "organization.support_updated").target("organization", p.org_id).record(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(support_settings(&state, p.org_id).await?))
}

#[derive(Deserialize)]
struct UpdateOrgBody {
    name: String,
}

async fn update_organization(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<UpdateOrgBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::OrganizationManage)?;
    let name = validation::non_empty("name", &body.name, 200)?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE organizations SET name = $2, updated_at = now() WHERE id = $1")
        .bind(p.org_id)
        .bind(&name)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "organization.updated")
        .target("organization", p.org_id)
        .details(json!({ "name": name }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": p.org_id, "name": name })))
}

#[derive(Deserialize)]
struct CreateOrgBody {
    name: String,
    account_type: Option<String>,
}

async fn create_organization(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    ApiJson(body): ApiJson<CreateOrgBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name = validation::non_empty("name", &body.name, 200)?;
    let account_type = super::auth::parse_account_type(body.account_type.as_deref())?;
    let owned: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE user_id = $1 AND role = 'owner'")
        .bind(s.user_id)
        .fetch_one(&state.db)
        .await?;
    if owned >= 10 {
        return Err(AppError::conflict("organization_limit", "You can own at most 10 organizations"));
    }
    let mut tx = state.db.begin().await?;
    let (id, slug) = super::auth::create_tenant(&mut tx, &name, account_type, s.user_id).await?;
    sqlx::query("UPDATE sessions SET active_organization_id = $2 WHERE id = $1")
        .bind(s.session_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    Entry::user(Some(id), s.user_id, &s.email, &client, "organization.created")
        .target("organization", id)
        .details(json!({ "account_type": account_type }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "name": name, "slug": slug, "role": "owner", "account_type": account_type })),
    ))
}

// ── members ──────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct MemberRow {
    user_id: Uuid,
    email: String,
    full_name: String,
    role: String,
    mfa_enabled: bool,
    last_login_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct InvitationRow {
    id: Uuid,
    email: String,
    role: String,
    invited_by_email: Option<String>,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

async fn list_members(State(state): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    p.require(Permission::MembersRead)?;
    let members: Vec<MemberRow> = sqlx::query_as(
        "SELECT u.id AS user_id, u.email::text AS email, u.full_name, m.role, u.mfa_enabled, u.last_login_at, m.created_at
         FROM memberships m JOIN users u ON u.id = m.user_id
         WHERE m.organization_id = $1
         ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'developer' THEN 2 WHEN 'support' THEN 3 ELSE 4 END, u.full_name",
    )
    .bind(p.org_id)
    .fetch_all(&state.db)
    .await?;

    let invitations: Vec<Value> = if p.can(Permission::MembersManage) {
        let rows: Vec<InvitationRow> = sqlx::query_as(
            "SELECT i.id, i.email::text AS email, i.role, u.email::text AS invited_by_email, i.expires_at, i.created_at
             FROM invitations i LEFT JOIN users u ON u.id = i.invited_by
             WHERE i.organization_id = $1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > now()
             ORDER BY i.created_at DESC",
        )
        .bind(p.org_id)
        .fetch_all(&state.db)
        .await?;
        rows.into_iter()
            .map(|r| json!({ "id": r.id, "email": r.email, "role": r.role, "invited_by": r.invited_by_email, "expires_at": r.expires_at, "created_at": r.created_at }))
            .collect()
    } else {
        vec![]
    };

    Ok(Json(json!({
        "data": members.into_iter().map(|m| json!({
            "user_id": m.user_id, "email": m.email, "full_name": m.full_name, "role": m.role,
            "mfa_enabled": m.mfa_enabled, "last_login_at": m.last_login_at, "joined_at": m.created_at,
            "is_you": Some(m.user_id) == p.user_id(),
        })).collect::<Vec<_>>(),
        "invitations": invitations,
    })))
}

/// Only owners may grant or remove the owner and admin roles.
fn can_assign(actor: Role, target_role: Role) -> bool {
    match target_role {
        Role::Owner | Role::Admin => actor == Role::Owner,
        Role::Developer | Role::Support | Role::Viewer => actor >= Role::Admin,
    }
}

#[derive(Deserialize)]
struct InviteBody {
    email: String,
    role: String,
}

async fn invite(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    ApiJson(body): ApiJson<InviteBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    p.require(Permission::MembersManage)?;
    let actor_role = p.role().ok_or_else(|| AppError::forbidden("Only members can invite"))?;
    let role = Role::parse(&body.role)
        .filter(|r| *r != Role::Owner)
        .ok_or_else(|| AppError::validation("role must be admin, developer, support or viewer"))?;
    if !can_assign(actor_role, role) {
        return Err(AppError::forbidden("Only owners can invite administrators"));
    }
    let email = validation::parse_email("email", &body.email)?.as_string().to_lowercase();
    state.limiter.check(&format!("invite:{}", p.org_id), 50, 86400)?;

    // Seats: current members plus open invitations (excluding a replaced one).
    let (seats, max_members): (i64, i32) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM memberships WHERE organization_id = $1)
              + (SELECT count(*) FROM invitations WHERE organization_id = $1 AND accepted_at IS NULL
                   AND revoked_at IS NULL AND expires_at > now() AND email <> $2),
                p.max_members
         FROM organizations o JOIN plans p ON p.id = o.plan_id WHERE o.id = $1",
    )
    .bind(p.org_id)
    .bind(&email)
    .fetch_one(&state.db)
    .await?;
    if seats >= max_members as i64 {
        return Err(AppError::quota_exceeded(format!(
            "Your plan allows {max_members} team member(s) including pending invitations. Upgrade the plan to invite more."
        ))
        .with_details(json!({ "limit": "members", "max": max_members })));
    }

    let already_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.organization_id = $1 AND u.email = $2)",
    )
    .bind(p.org_id)
    .bind(&email)
    .fetch_one(&state.db)
    .await?;
    if already_member {
        return Err(AppError::conflict("already_member", "That person is already a member"));
    }

    let token = crypto::random_token(32);
    let mut tx = state.db.begin().await?;
    // Replace any older open invitation for the same address.
    sqlx::query("UPDATE invitations SET revoked_at = now() WHERE organization_id = $1 AND email = $2 AND accepted_at IS NULL AND revoked_at IS NULL")
        .bind(p.org_id)
        .bind(&email)
        .execute(&mut *tx)
        .await?;
    let (id, expires_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO invitations (organization_id, email, role, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, expires_at",
    )
    .bind(p.org_id)
    .bind(&email)
    .bind(role.as_str())
    .bind(crypto::sha256(&token))
    .bind(p.user_id())
    .bind(Utc::now() + Duration::days(7))
    .fetch_one(&mut *tx)
    .await?;

    let (org_name, inviter): (String, String) =
        sqlx::query_as("SELECT o.name, u.full_name FROM organizations o, users u WHERE o.id = $1 AND u.id = $2")
            .bind(p.org_id)
            .bind(p.user_id())
            .fetch_one(&mut *tx)
            .await?;
    let link = format!("{}/invitations/{token}", state.cfg.public_base_url);
    system_mail::enqueue(
        &mut *tx,
        system_mail::invitation(&state.cfg.system_from_name, &email, &org_name, &inviter, role.as_str(), &link),
    )
    .await?;
    Entry::by(&p, &client, "member.invited")
        .target("invitation", id)
        .details(json!({ "email": email, "role": role.as_str() }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "email": email, "role": role.as_str(), "expires_at": expires_at })),
    ))
}

async fn revoke_invitation(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MembersManage)?;
    let n = sqlx::query("UPDATE invitations SET revoked_at = now() WHERE id = $1 AND organization_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL")
        .bind(id)
        .bind(p.org_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Invitation"));
    }
    Entry::by(&p, &client, "member.invitation_revoked").target("invitation", id).record(&state.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct RoleBody {
    role: String,
}

async fn change_role(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(user_id): Path<Uuid>,
    ApiJson(body): ApiJson<RoleBody>,
) -> ApiResult<Json<Value>> {
    p.require(Permission::MembersManage)?;
    let actor = p.role().ok_or_else(|| AppError::forbidden("Only members can change roles"))?;
    if p.user_id() == Some(user_id) {
        return Err(AppError::forbidden("You cannot change your own role"));
    }
    let new_role = Role::parse(&body.role)
        .ok_or_else(|| AppError::validation("role must be owner, admin, developer, support or viewer"))?;

    let mut tx = state.db.begin().await?;
    // Lock the org's memberships so concurrent changes cannot remove the last owner.
    let current: Option<String> =
        sqlx::query_scalar("SELECT role FROM memberships WHERE organization_id = $1 AND user_id = $2 FOR UPDATE")
            .bind(p.org_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let current = current.and_then(|r| Role::parse(&r)).ok_or_else(|| AppError::not_found("Member"))?;
    if !can_assign(actor, current) || !can_assign(actor, new_role) {
        return Err(AppError::forbidden("Only owners can change owner or admin roles"));
    }
    if current == Role::Owner && new_role != Role::Owner {
        ensure_another_owner(&mut tx, p.org_id, user_id).await?;
    }
    sqlx::query("UPDATE memberships SET role = $3 WHERE organization_id = $1 AND user_id = $2")
        .bind(p.org_id)
        .bind(user_id)
        .bind(new_role.as_str())
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, "member.role_changed")
        .target("user", user_id)
        .details(json!({ "from": current.as_str(), "to": new_role.as_str() }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "user_id": user_id, "role": new_role.as_str() })))
}

async fn ensure_another_owner(tx: &mut sqlx::PgConnection, org_id: Uuid, leaving: Uuid) -> ApiResult<()> {
    let owners: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM memberships WHERE organization_id = $1 AND role = 'owner' FOR UPDATE")
            .bind(org_id)
            .fetch_all(&mut *tx)
            .await?;
    if owners.iter().all(|u| *u == leaving) {
        return Err(AppError::conflict("last_owner", "An organization must keep at least one owner"));
    }
    Ok(())
}

async fn remove_member(
    State(state): State<AppState>,
    client: ClientInfo,
    p: Principal,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let leaving_self = p.user_id() == Some(user_id);
    if !leaving_self {
        p.require(Permission::MembersManage)?;
    }
    let mut tx = state.db.begin().await?;
    let current: Option<String> =
        sqlx::query_scalar("SELECT role FROM memberships WHERE organization_id = $1 AND user_id = $2 FOR UPDATE")
            .bind(p.org_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let current = current.and_then(|r| Role::parse(&r)).ok_or_else(|| AppError::not_found("Member"))?;
    if !leaving_self {
        let actor = p.role().ok_or_else(|| AppError::forbidden("Only members can remove members"))?;
        if !can_assign(actor, current) {
            return Err(AppError::forbidden("Only owners can remove owners or admins"));
        }
    }
    if current == Role::Owner {
        ensure_another_owner(&mut tx, p.org_id, user_id).await?;
    }
    sqlx::query("DELETE FROM memberships WHERE organization_id = $1 AND user_id = $2")
        .bind(p.org_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // Sessions pointed at this org lose their active organization immediately.
    sqlx::query("UPDATE sessions SET active_organization_id = NULL WHERE user_id = $1 AND active_organization_id = $2")
        .bind(user_id)
        .bind(p.org_id)
        .execute(&mut *tx)
        .await?;
    Entry::by(&p, &client, if leaving_self { "member.left" } else { "member.removed" })
        .target("user", user_id)
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

// ── invitation acceptance ────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct InviteLookup {
    id: Uuid,
    organization_id: Uuid,
    organization_name: String,
    email: String,
    role: String,
    inviter: Option<String>,
    expires_at: DateTime<Utc>,
}

async fn lookup_invitation(state: &AppState, token: &str) -> ApiResult<InviteLookup> {
    sqlx::query_as(
        "SELECT i.id, i.organization_id, o.name AS organization_name, i.email::text AS email, i.role,
                u.full_name AS inviter, i.expires_at
         FROM invitations i JOIN organizations o ON o.id = i.organization_id
         LEFT JOIN users u ON u.id = i.invited_by
         WHERE i.token_hash = $1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > now()",
    )
    .bind(crypto::sha256(token))
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Invitation"))
}

/// Public: lets the invite page show what is being accepted.
async fn view_invitation(
    State(state): State<AppState>,
    client: ClientInfo,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("invite-view:{}", client.ip_key()), 60, 3600)?;
    let i = lookup_invitation(&state, &token).await?;
    Ok(Json(json!({
        "organization_name": i.organization_name, "email": i.email, "role": i.role,
        "invited_by": i.inviter, "expires_at": i.expires_at,
    })))
}

async fn accept_invitation(
    State(state): State<AppState>,
    client: ClientInfo,
    s: ConsoleSession,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    let i = lookup_invitation(&state, &token).await?;
    if !i.email.eq_ignore_ascii_case(&s.email) {
        return Err(AppError::forbidden(format!(
            "This invitation was sent to {}. Sign in with that account to accept it.",
            i.email
        )));
    }
    let mut tx = state.db.begin().await?;
    let claimed = sqlx::query(
        "UPDATE invitations SET accepted_at = now() WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL",
    )
    .bind(i.id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if claimed == 0 {
        return Err(AppError::not_found("Invitation"));
    }
    sqlx::query("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(i.organization_id)
        .bind(s.user_id)
        .bind(&i.role)
        .execute(&mut *tx)
        .await?;
    // Accepting an emailed invitation proves control of the address.
    sqlx::query("UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) WHERE id = $1")
        .bind(s.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sessions SET active_organization_id = $2 WHERE id = $1")
        .bind(s.session_id)
        .bind(i.organization_id)
        .execute(&mut *tx)
        .await?;
    Entry::user(Some(i.organization_id), s.user_id, &s.email, &client, "member.joined")
        .target("invitation", i.id)
        .details(json!({ "role": i.role }))
        .record(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "organization_id": i.organization_id, "role": i.role })))
}
