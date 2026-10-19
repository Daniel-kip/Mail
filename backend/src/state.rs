use std::sync::Arc;

use sqlx::PgPool;

use crate::{
    config::Config,
    crypto::SecretBox,
    ratelimit::RateLimiter,
    services::{dns::DnsChecker, mailtools::MailTools, webhooks},
};

/// Shared, cheaply clonable application state.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub secrets: SecretBox,
    pub limiter: Arc<RateLimiter>,
    pub dns: Arc<DnsChecker>,
    /// Public deliverability tools and their SSRF-guarded HTTP client.
    pub tools: Arc<MailTools>,
    pub tools_http: reqwest::Client,
}

impl AppState {
    pub fn new(db: PgPool, cfg: Config) -> anyhow::Result<Self> {
        let secrets = SecretBox::new(&cfg.data_encryption_key);
        Ok(Self {
            db,
            secrets,
            limiter: Arc::new(RateLimiter::default()),
            dns: Arc::new(DnsChecker::new()?),
            tools: Arc::new(MailTools::new()),
            tools_http: webhooks::http_client(&cfg)?,
            cfg: Arc::new(cfg),
        })
    }
}
