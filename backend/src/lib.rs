//! Mailvora — multi-tenant transactional email API and hosted mailboxes.
//!
//! The Rust service is the control plane (tenants, identities, domains, keys,
//! templates, policies, quotas, logs, webmail API). Mail transport and
//! storage are delegated to Postfix, Dovecot and Rspamd (guide 01).

pub mod api;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod error;
pub mod http;
pub mod mail;
pub mod password;
pub mod ratelimit;
pub mod services;
pub mod state;
pub mod validation;
pub mod workers;

pub use state::AppState;
