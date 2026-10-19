//! Typed configuration loaded from the environment.
//!
//! Development has convenient defaults. In production every security-relevant
//! value must be set explicitly and startup fails otherwise (guide 02:
//! "fail startup if required production configuration is missing").

use std::{net::SocketAddr, str::FromStr};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    /// Plain TCP. Only acceptable on a private network or to a local sink.
    None,
    StartTls,
    Implicit,
}

impl FromStr for TlsMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "none" | "off" | "false" => Ok(Self::None),
            "starttls" => Ok(Self::StartTls),
            "tls" | "implicit" => Ok(Self::Implicit),
            other => bail!("invalid TLS mode {other:?} (expected none|starttls|tls)"),
        }
    }
}

#[derive(Clone)]
pub struct SmtpRelayConfig {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
    pub username: Option<String>,
    pub password: Option<String>,
    pub pool_size: u32,
}

#[derive(Clone)]
pub struct DoveadmConfig {
    /// e.g. http://dovecot:8080 (private network only)
    pub url: String,
    pub api_key: String,
}

#[derive(Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
    /// Dovecot master user used for webmail after the platform has verified
    /// the mailbox password itself (login as `address*master`).
    pub master_user: String,
    pub master_password: String,
}

/// Internal ManageSieve endpoint (Dovecot Pigeonhole) used to install
/// out-of-office scripts on behalf of mailbox users via the master user.
#[derive(Clone)]
pub struct SieveConfig {
    pub host: String,
    pub port: u16,
}

/// PesaTide (Kenyan M-Pesa gateway) credentials. Payments are disabled
/// when the API key is not configured.
#[derive(Clone)]
pub struct PesatideConfig {
    pub api_url: String,
    pub api_key: String,
    pub api_secret: String,
    pub webhook_secret: String,
    /// Optional collection channel (`routeId`); without it the gateway
    /// settles on its platform paybill and credits the API key's wallet.
    pub route_id: Option<String>,
}

/// Africa's Talking SMS credentials for phone verification codes.
#[derive(Clone)]
pub struct SmsConfig {
    pub api_url: String,
    pub username: String,
    pub api_key: String,
    pub sender_id: Option<String>,
}

/// Google OAuth client for "Continue with Google".
#[derive(Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Clone, Default)]
pub struct Integrations {
    pub pesatide: Option<PesatideConfig>,
    pub sms: Option<SmsConfig>,
    pub google: Option<GoogleConfig>,
}

#[derive(Clone)]
pub struct BillingConfig {
    /// ISO 4217 code of all plan prices and invoices.
    pub currency: String,
    /// Tax added to invoices, in basis points (1600 = 16% VAT).
    pub tax_rate_bps: i32,
    pub tax_label: String,
    pub due_days: i64,
    /// Days after the due date before a subscription becomes past due.
    pub grace_days: i64,
    pub issuer: String,
    pub payment_instructions: String,
}

#[derive(Clone)]
pub struct Config {
    pub env: Environment,
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub run_migrations_on_start: bool,
    pub data_encryption_key: [u8; 32],
    pub session_ttl_hours: i64,
    pub cookie_secure: bool,
    pub allow_signup: bool,
    pub trust_proxy_headers: bool,
    pub cors_origins: Vec<String>,

    // Mail identity of the platform
    pub mail_hostname: String,
    /// Public, client-facing endpoints. These are kept separate from private
    /// API-to-mailstore and outbound relay settings.
    pub email_client_imap_host: String,
    pub email_client_imap_port: u16,
    pub email_client_imap_tls: TlsMode,
    pub email_client_smtp_host: String,
    pub email_client_smtp_port: u16,
    pub email_client_smtp_tls: TlsMode,
    /// Optional second submission endpoint (e.g. 587/STARTTLS when the
    /// primary is 465/implicit TLS as RFC 8314 recommends).
    pub email_client_smtp_alt: Option<(u16, TlsMode)>,
    pub spf_include: String,
    /// Platform host serving Mozilla autoconfig documents.
    pub autoconfig_host: String,
    pub return_path_domain: String,
    pub message_id_domain: String,
    pub system_from_address: String,
    pub system_from_name: String,
    pub dkim_selector_prefix: String,

    pub smtp: SmtpRelayConfig,
    pub imap: Option<ImapConfig>,
    pub doveadm: Option<DoveadmConfig>,
    pub sieve: Option<SieveConfig>,
    pub billing: BillingConfig,
    pub integrations: Integrations,
    /// Screen new passwords against Pwned Passwords (k-anonymity).
    pub password_breach_check: bool,
    /// Product name shown in emails, profiles and autoconfig documents.
    pub product_name: String,

    pub internal_api_token: Option<String>,
    pub webhook_allow_private_targets: bool,
    /// Development only: treat DNS checks as passing (refused in production).
    pub dev_skip_dns_verification: bool,

    pub worker_enabled: bool,
    pub worker_concurrency: usize,
    pub max_message_bytes: usize,
    pub max_attachment_bytes: usize,
    pub content_retention_days: i64,
}

impl std::fmt::Debug for Config {
    // Hand-written so secrets can never reach logs through `{:?}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("env", &self.env)
            .field("bind", &self.bind)
            .field("public_base_url", &self.public_base_url)
            .field("smtp_host", &self.smtp.host)
            .field("smtp_port", &self.smtp.port)
            .field("imap_enabled", &self.imap.is_some())
            .field("doveadm_enabled", &self.doveadm.is_some())
            .field("worker_enabled", &self.worker_enabled)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn is_production(&self) -> bool {
        self.env == Environment::Production
    }

    pub fn from_env() -> Result<Self> {
        let env = match var_or("APP_ENV", "development").to_ascii_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            "development" | "dev" | "test" => Environment::Development,
            other => bail!("APP_ENV must be development or production, got {other:?}"),
        };
        let prod = env == Environment::Production;

        let data_encryption_key = {
            let raw = std::env::var("DATA_ENCRYPTION_KEY").context(
                "DATA_ENCRYPTION_KEY is required (32 random bytes, base64). Generate one with `mailvora gen-key`",
            )?;
            let bytes = B64.decode(raw.trim()).context("DATA_ENCRYPTION_KEY must be base64")?;
            <[u8; 32]>::try_from(bytes.as_slice())
                .map_err(|_| anyhow::anyhow!("DATA_ENCRYPTION_KEY must decode to exactly 32 bytes"))?
        };

        let public_base_url = var_or("PUBLIC_BASE_URL", "http://localhost:8088").trim_end_matches('/').to_string();

        let smtp = SmtpRelayConfig {
            host: var_or("SMTP_RELAY_HOST", "127.0.0.1"),
            port: parse_or("SMTP_RELAY_PORT", 1025)?,
            tls: var_or("SMTP_RELAY_TLS", "none").parse()?,
            username: opt_var("SMTP_RELAY_USERNAME"),
            password: opt_var("SMTP_RELAY_PASSWORD"),
            pool_size: parse_or("SMTP_RELAY_POOL_SIZE", 4)?,
        };

        let imap = match opt_var("IMAP_HOST") {
            Some(host) => Some(ImapConfig {
                host,
                port: parse_or("IMAP_PORT", 143)?,
                tls: var_or("IMAP_TLS", "none").parse()?,
                master_user: opt_var("IMAP_MASTER_USER")
                    .context("IMAP_MASTER_USER is required when IMAP_HOST is set")?,
                master_password: opt_var("IMAP_MASTER_PASSWORD")
                    .context("IMAP_MASTER_PASSWORD is required when IMAP_HOST is set")?,
            }),
            None => None,
        };

        let platform_domain = var_or("PLATFORM_DOMAIN", "mailvora.localhost");

        let cfg = Config {
            env,
            bind: var_or("HTTP_BIND", "127.0.0.1:8088").parse().context("HTTP_BIND must be host:port")?,
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            database_max_connections: parse_or("DATABASE_MAX_CONNECTIONS", 20)?,
            run_migrations_on_start: parse_bool_or("RUN_MIGRATIONS_ON_START", !prod)?,
            data_encryption_key,
            session_ttl_hours: parse_or("SESSION_TTL_HOURS", 12)?,
            cookie_secure: parse_bool_or("COOKIE_SECURE", prod)?,
            allow_signup: parse_bool_or("ALLOW_SIGNUP", true)?,
            trust_proxy_headers: parse_bool_or("TRUST_PROXY_HEADERS", false)?,
            cors_origins: opt_var("CORS_ORIGINS")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default(),

            mail_hostname: var_or("MAIL_HOSTNAME", &format!("mail.{platform_domain}")),
            email_client_imap_host: var_or(
                "EMAIL_CLIENT_IMAP_HOST",
                &var_or("MAIL_HOSTNAME", &format!("mail.{platform_domain}")),
            ),
            email_client_imap_port: parse_or("EMAIL_CLIENT_IMAP_PORT", if prod { 993 } else { 1993 })?,
            email_client_imap_tls: var_or("EMAIL_CLIENT_IMAP_TLS", "tls").parse()?,
            email_client_smtp_host: var_or(
                "EMAIL_CLIENT_SMTP_HOST",
                &var_or("MAIL_HOSTNAME", &format!("mail.{platform_domain}")),
            ),
            // RFC 8314: prefer implicit TLS (465) for submission, keep 587/STARTTLS as the alternative.
            email_client_smtp_port: parse_or("EMAIL_CLIENT_SMTP_PORT", if prod { 465 } else { 2465 })?,
            email_client_smtp_tls: var_or("EMAIL_CLIENT_SMTP_TLS", "tls").parse()?,
            email_client_smtp_alt: match var_or("EMAIL_CLIENT_SMTP_ALT_PORT", if prod { "587" } else { "2587" }).as_str() {
                "" | "0" | "none" | "off" => None,
                port => Some((
                    port.parse().map_err(|e| anyhow::anyhow!("EMAIL_CLIENT_SMTP_ALT_PORT: {e}"))?,
                    var_or("EMAIL_CLIENT_SMTP_ALT_TLS", "starttls").parse()?,
                )),
            },
            spf_include: var_or("SPF_INCLUDE", &format!("_spf.{platform_domain}")),
            autoconfig_host: var_or("AUTOCONFIG_HOST", &format!("autoconfig.{platform_domain}")),
            return_path_domain: var_or("RETURN_PATH_DOMAIN", &format!("bounces.{platform_domain}")),
            message_id_domain: var_or("MESSAGE_ID_DOMAIN", &platform_domain),
            system_from_address: var_or("SYSTEM_FROM_ADDRESS", &format!("no-reply@{platform_domain}")),
            system_from_name: var_or("SYSTEM_FROM_NAME", &var_or("PRODUCT_NAME", "Mailvora")),
            product_name: var_or("PRODUCT_NAME", "Mailvora"),
            dkim_selector_prefix: var_or("DKIM_SELECTOR_PREFIX", "mv"),
            public_base_url,

            smtp,
            imap,
            doveadm: match (opt_var("DOVEADM_API_URL"), opt_var("DOVEADM_API_KEY")) {
                (Some(url), Some(api_key)) => Some(DoveadmConfig { url, api_key }),
                _ => None,
            },
            sieve: match opt_var("MANAGESIEVE_HOST") {
                Some(host) => Some(SieveConfig { host, port: parse_or("MANAGESIEVE_PORT", 4190)? }),
                None => None,
            },
            billing: BillingConfig {
                currency: var_or("BILLING_CURRENCY", "KES").to_ascii_uppercase(),
                tax_rate_bps: parse_or("BILLING_TAX_RATE_BPS", 0)?,
                tax_label: var_or("BILLING_TAX_LABEL", "Tax"),
                due_days: parse_or("BILLING_DUE_DAYS", 14)?,
                grace_days: parse_or("BILLING_GRACE_DAYS", 14)?,
                issuer: var_or("BILLING_ISSUER", &var_or("PRODUCT_NAME", "Mailvora")),
                payment_instructions: var_or(
                    "BILLING_PAYMENT_INSTRUCTIONS",
                    "Pay with M-Pesa from the invoice page: enter your Safaricom number and approve the prompt on your phone.",
                ),
            },
            password_breach_check: parse_bool_or("PASSWORD_BREACH_CHECK", true)?,
            integrations: Integrations {
                pesatide: match (opt_var("PESATIDE_API_KEY"), opt_var("PESATIDE_API_SECRET"), opt_var("PESATIDE_WEBHOOK_SECRET")) {
                    (Some(api_key), Some(api_secret), Some(webhook_secret)) => Some(PesatideConfig {
                        api_url: var_or("PESATIDE_API_URL", "https://pesatide.com/v1").trim_end_matches('/').to_string(),
                        api_key,
                        api_secret,
                        webhook_secret,
                        route_id: opt_var("PESATIDE_ROUTE_ID"),
                    }),
                    (None, None, None) => None,
                    _ => bail!("PESATIDE_API_KEY, PESATIDE_API_SECRET and PESATIDE_WEBHOOK_SECRET must be set together"),
                },
                sms: match (opt_var("AT_USERNAME"), opt_var("AT_API_KEY")) {
                    (Some(username), Some(api_key)) => Some(SmsConfig {
                        api_url: var_or(
                            "AT_API_URL",
                            if username == "sandbox" {
                                "https://api.sandbox.africastalking.com/version1/messaging"
                            } else {
                                "https://api.africastalking.com/version1/messaging"
                            },
                        ),
                        username,
                        api_key,
                        sender_id: opt_var("AT_SENDER_ID"),
                    }),
                    (None, None) => None,
                    _ => bail!("AT_USERNAME and AT_API_KEY must be set together"),
                },
                google: match (opt_var("GOOGLE_CLIENT_ID"), opt_var("GOOGLE_CLIENT_SECRET")) {
                    (Some(client_id), Some(client_secret)) => Some(GoogleConfig { client_id, client_secret }),
                    (None, None) => None,
                    _ => bail!("GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET must be set together"),
                },
            },

            internal_api_token: opt_var("INTERNAL_API_TOKEN"),
            webhook_allow_private_targets: parse_bool_or("WEBHOOK_ALLOW_PRIVATE_TARGETS", !prod)?,
            dev_skip_dns_verification: parse_bool_or("DEV_SKIP_DNS_VERIFICATION", false)?,

            worker_enabled: parse_bool_or("WORKER_ENABLED", true)?,
            worker_concurrency: parse_or("WORKER_CONCURRENCY", 8)?,
            max_message_bytes: parse_or("MAX_MESSAGE_BYTES", 10 * 1024 * 1024)?,
            max_attachment_bytes: parse_or("MAX_ATTACHMENT_BYTES", 8 * 1024 * 1024)?,
            content_retention_days: parse_or("CONTENT_RETENTION_DAYS", 30)?,
        };

        validate_email_client(&cfg)?;
        validate_billing(&cfg.billing)?;
        if prod {
            cfg.validate_production()?;
        }
        Ok(cfg)
    }

    fn validate_production(&self) -> Result<()> {
        let mut problems = Vec::new();
        if !self.public_base_url.starts_with("https://") {
            problems.push("PUBLIC_BASE_URL must use https://");
        }
        if !self.cookie_secure {
            problems.push("COOKIE_SECURE must be true");
        }
        match &self.internal_api_token {
            Some(t) if t.len() >= 32 => {}
            _ => problems.push("INTERNAL_API_TOKEN must be set (>= 32 chars)"),
        }
        if self.dev_skip_dns_verification {
            problems.push("DEV_SKIP_DNS_VERIFICATION must be false");
        }
        if self.webhook_allow_private_targets {
            problems.push("WEBHOOK_ALLOW_PRIVATE_TARGETS must be false");
        }
        if self.smtp.username.is_none() || self.smtp.password.is_none() {
            problems.push("SMTP_RELAY_USERNAME and SMTP_RELAY_PASSWORD are required (authenticated submission)");
        }
        if self.mail_hostname.ends_with(".localhost") {
            problems.push("MAIL_HOSTNAME / PLATFORM_DOMAIN must be a real DNS name");
        }
        if let Some(imap) = &self.imap {
            if imap.master_password.len() < 24 {
                problems.push("IMAP_MASTER_PASSWORD must be >= 24 chars");
            }
        }
        for (name, host) in [
            ("EMAIL_CLIENT_IMAP_HOST", self.email_client_imap_host.as_str()),
            ("EMAIL_CLIENT_SMTP_HOST", self.email_client_smtp_host.as_str()),
        ] {
            if !valid_mail_hostname(host) {
                problems.push(match name {
                    "EMAIL_CLIENT_IMAP_HOST" => "EMAIL_CLIENT_IMAP_HOST must be a public DNS hostname",
                    _ => "EMAIL_CLIENT_SMTP_HOST must be a public DNS hostname",
                });
            }
        }
        if self.email_client_imap_tls != TlsMode::Implicit {
            problems.push("EMAIL_CLIENT_IMAP_TLS must be tls (implicit TLS on the advertised endpoint)");
        }
        if self.email_client_smtp_tls != TlsMode::StartTls && self.email_client_smtp_tls != TlsMode::Implicit {
            problems.push("EMAIL_CLIENT_SMTP_TLS must be starttls or tls");
        }
        if problems.is_empty() {
            Ok(())
        } else {
            bail!("refusing to start in production:\n  - {}", problems.join("\n  - "))
        }
    }
}

fn validate_email_client(cfg: &Config) -> Result<()> {
    if !valid_mail_hostname(&cfg.email_client_imap_host) || !valid_mail_hostname(&cfg.email_client_smtp_host) {
        bail!("EMAIL_CLIENT_IMAP_HOST and EMAIL_CLIENT_SMTP_HOST must be DNS hostnames (without scheme or port)");
    }
    if cfg.email_client_imap_port == 0 || cfg.email_client_smtp_port == 0 {
        bail!("EMAIL_CLIENT_IMAP_PORT and EMAIL_CLIENT_SMTP_PORT must be between 1 and 65535");
    }
    if cfg.email_client_imap_tls != TlsMode::Implicit {
        bail!("EMAIL_CLIENT_IMAP_TLS must be tls (implicit TLS)");
    }
    if !matches!(cfg.email_client_smtp_tls, TlsMode::StartTls | TlsMode::Implicit) {
        bail!("EMAIL_CLIENT_SMTP_TLS must be starttls or tls; unencrypted submission is not supported");
    }
    if let Some((port, tls)) = cfg.email_client_smtp_alt {
        if port == 0 || port == cfg.email_client_smtp_port {
            bail!("EMAIL_CLIENT_SMTP_ALT_PORT must differ from EMAIL_CLIENT_SMTP_PORT");
        }
        if !matches!(tls, TlsMode::StartTls | TlsMode::Implicit) {
            bail!("EMAIL_CLIENT_SMTP_ALT_TLS must be starttls or tls");
        }
    }
    Ok(())
}

fn validate_billing(b: &BillingConfig) -> Result<()> {
    if b.currency.len() != 3 || !b.currency.bytes().all(|c| c.is_ascii_uppercase()) {
        bail!("BILLING_CURRENCY must be a 3-letter ISO 4217 code");
    }
    if !(0..=5000).contains(&b.tax_rate_bps) {
        bail!("BILLING_TAX_RATE_BPS must be between 0 and 5000");
    }
    if !(1..=90).contains(&b.due_days) || !(0..=90).contains(&b.grace_days) {
        bail!("BILLING_DUE_DAYS must be 1-90 and BILLING_GRACE_DAYS 0-90");
    }
    Ok(())
}

/// Public so the operator API validates stored client profiles the same way.
pub fn is_valid_mail_hostname(host: &str) -> bool {
    valid_mail_hostname(host)
}

fn opt_var(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn var_or(name: &str, default: &str) -> String {
    opt_var(name).unwrap_or_else(|| default.to_string())
}

fn parse_or<T: FromStr>(name: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    match opt_var(name) {
        Some(v) => v.parse().map_err(|e| anyhow::anyhow!("{name}: {e}")),
        None => Ok(default),
    }
}

fn parse_bool_or(name: &str, default: bool) -> Result<bool> {
    match opt_var(name) {
        None => Ok(default),
        Some(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => bail!("{name} must be a boolean, got {v:?}"),
        },
    }
}

fn valid_mail_hostname(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && host.rsplit('.').next().is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
}

impl Config {
    /// Configuration for integration tests and local tooling: development
    /// mode, a fixed data key and the given database. Not for production.
    #[doc(hidden)]
    pub fn for_tests(database_url: &str) -> Self {
        Config {
            env: Environment::Development,
            bind: "127.0.0.1:0".parse().unwrap(),
            public_base_url: "http://localhost:8088".into(),
            database_url: database_url.into(),
            database_max_connections: 10,
            run_migrations_on_start: false,
            data_encryption_key: [42u8; 32],
            session_ttl_hours: 12,
            cookie_secure: false,
            allow_signup: true,
            trust_proxy_headers: false,
            cors_origins: vec![],
            mail_hostname: "mail.mailvora.test".into(),
            email_client_imap_host: "mail.mailvora.test".into(),
            email_client_imap_port: 993,
            email_client_imap_tls: TlsMode::Implicit,
            email_client_smtp_host: "mail.mailvora.test".into(),
            email_client_smtp_port: 465,
            email_client_smtp_tls: TlsMode::Implicit,
            email_client_smtp_alt: Some((587, TlsMode::StartTls)),
            spf_include: "_spf.mailvora.test".into(),
            autoconfig_host: "autoconfig.mailvora.test".into(),
            return_path_domain: "bounces.mailvora.test".into(),
            message_id_domain: "mailvora.test".into(),
            system_from_address: "no-reply@mailvora.test".into(),
            system_from_name: "Mailvora".into(),
            dkim_selector_prefix: "mv".into(),
            smtp: SmtpRelayConfig {
                host: std::env::var("TEST_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
                port: std::env::var("TEST_SMTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(1025),
                tls: TlsMode::None,
                username: None,
                password: None,
                pool_size: 2,
            },
            imap: None,
            doveadm: None,
            sieve: None,
            billing: BillingConfig {
                currency: "KES".into(),
                tax_rate_bps: 1600,
                tax_label: "VAT".into(),
                due_days: 14,
                grace_days: 14,
                issuer: "Mailvora Test".into(),
                payment_instructions: "Test instructions".into(),
            },
            integrations: Integrations::default(),
            password_breach_check: false,
            product_name: "Mailvora".into(),
            internal_api_token: Some("test-internal-token-0123456789abcdef".into()),
            webhook_allow_private_targets: false,
            dev_skip_dns_verification: true,
            worker_enabled: false,
            worker_concurrency: 1,
            max_message_bytes: 10 * 1024 * 1024,
            max_attachment_bytes: 8 * 1024 * 1024,
            content_retention_days: 30,
        }
    }
}

#[cfg(test)]
mod email_client_config_tests {
    use super::{valid_mail_hostname, validate_email_client, Config, TlsMode};

    #[test]
    fn validates_advertised_mail_hostnames() {
        assert!(valid_mail_hostname("mail.example.edu"));
        assert!(valid_mail_hostname("imap.eu.example.co"));
        assert!(!valid_mail_hostname("http://mail.example.edu"));
        assert!(!valid_mail_hostname("mail.example.edu:993"));
        assert!(!valid_mail_hostname("mail..example.edu"));
        assert!(!valid_mail_hostname("127.0.0.1"));
    }

    #[test]
    fn rejects_plaintext_or_invalid_client_settings() {
        let mut cfg = Config::for_tests("postgres://localhost/mailvora");
        assert!(validate_email_client(&cfg).is_ok());
        cfg.email_client_imap_tls = TlsMode::None;
        assert!(validate_email_client(&cfg).is_err());
        cfg.email_client_imap_tls = TlsMode::Implicit;
        cfg.email_client_smtp_tls = TlsMode::None;
        assert!(validate_email_client(&cfg).is_err());
    }
}
