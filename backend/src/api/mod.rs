//! HTTP API (`/v1`).
//!
//! Console users and API keys share the tenant-scoped endpoints; each
//! handler checks one permission through `Principal::require`. Webmail and
//! platform-operator endpoints use their own extractors.

pub mod admin;
pub mod api_keys;
pub mod auth;
pub mod autoconfig;
pub mod billing;
pub mod common;
pub mod domains;
pub mod health;
pub mod internal;
pub mod mail_setup;
pub mod mailboxes;
pub mod messages;
pub mod organizations;
pub mod payments;
pub mod suppressions;
pub mod templates;
pub mod tools;
pub mod usage;
pub mod webhooks;
pub mod webmail;
pub mod webmail_account;

use axum::{extract::DefaultBodyLimit, Router};

use crate::state::AppState;

pub fn router(state: &AppState) -> Router<AppState> {
    // Base64 attachments inflate payloads by ~4/3; allow headroom over the
    // attachment limit for the send endpoint only.
    let send_limit = state.cfg.max_attachment_bytes * 4 / 3 + 1024 * 1024;
    Router::new()
        .nest("/auth", auth::router())
        .nest("/webmail", webmail::router(state))
        .nest("/admin", admin::router().merge(billing::admin_router()))
        .merge(billing::router())
        .merge(payments::router())
        .merge(tools::router())
        .merge(messages::router().layer(DefaultBodyLimit::max(send_limit)))
        .merge(organizations::router())
        .merge(api_keys::router())
        .merge(domains::router())
        .merge(templates::router())
        .merge(suppressions::router())
        .merge(webhooks::router())
        .merge(mailboxes::router())
        .merge(usage::router())
        .fallback(crate::http::api_not_found)
}
