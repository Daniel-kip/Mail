//! Public connection settings advertised to email apps (Gmail, Apple Mail,
//! Outlook, Thunderbird...).
//!
//! Resolution order for a mailbox: a per-domain profile (an institution with
//! its own branded endpoint and certificate), then the platform default
//! profile, then the validated environment configuration. These settings
//! are public by nature: they never include credentials, internal service
//! addresses or private-network hosts, and they are validated the same way
//! at startup and when an operator edits them.

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    config::{is_valid_mail_hostname, Config, TlsMode},
    error::{ApiResult, AppError},
    state::AppState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSettings {
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: TlsMode,
    pub smtp_alt: Option<(u16, TlsMode)>,
    /// "domain", "platform" or "environment".
    pub source: &'static str,
    pub version: i32,
}

pub fn security_label(t: TlsMode) -> &'static str {
    match t {
        TlsMode::Implicit => "SSL/TLS",
        TlsMode::StartTls => "STARTTLS",
        TlsMode::None => "None",
    }
}

pub fn security_code(t: TlsMode) -> &'static str {
    match t {
        TlsMode::Implicit => "tls",
        TlsMode::StartTls => "starttls",
        TlsMode::None => "none",
    }
}

fn parse_security(s: &str) -> Option<TlsMode> {
    match s {
        "tls" => Some(TlsMode::Implicit),
        "starttls" => Some(TlsMode::StartTls),
        _ => None,
    }
}

impl ClientSettings {
    pub fn from_config(cfg: &Config) -> Self {
        ClientSettings {
            imap_host: cfg.email_client_imap_host.clone(),
            imap_port: cfg.email_client_imap_port,
            smtp_host: cfg.email_client_smtp_host.clone(),
            smtp_port: cfg.email_client_smtp_port,
            smtp_tls: cfg.email_client_smtp_tls,
            smtp_alt: cfg.email_client_smtp_alt,
            source: "environment",
            version: 0,
        }
    }

    /// The JSON shape shared by the setup guide, the console and support.
    pub fn to_json(&self, username: Option<&str>) -> Value {
        json!({
            "username": username,
            "imap": {
                "protocol": "IMAP", "host": self.imap_host, "port": self.imap_port,
                "security": security_label(TlsMode::Implicit), "security_code": "tls",
                "authentication": "Normal password",
            },
            "smtp": {
                "protocol": "SMTP", "host": self.smtp_host, "port": self.smtp_port,
                "security": security_label(self.smtp_tls), "security_code": security_code(self.smtp_tls),
                "authentication": "Normal password", "requires_authentication": true,
            },
            "smtp_alternative": self.smtp_alt.map(|(port, tls)| json!({
                "port": port, "security": security_label(tls), "security_code": security_code(tls),
            })),
            "source": self.source,
            "version": self.version,
        })
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "imap_host": self.imap_host, "imap_port": self.imap_port, "imap_security": "tls",
            "smtp_host": self.smtp_host, "smtp_port": self.smtp_port, "smtp_security": security_code(self.smtp_tls),
            "smtp_alt_port": self.smtp_alt.map(|a| a.0), "smtp_alt_security": self.smtp_alt.map(|a| security_code(a.1)),
        })
    }
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    domain_id: Option<Uuid>,
    imap_host: String,
    imap_port: i32,
    smtp_host: String,
    smtp_port: i32,
    smtp_security: String,
    smtp_alt_port: Option<i32>,
    smtp_alt_security: Option<String>,
    version: i32,
}

impl ProfileRow {
    fn into_settings(self) -> Option<ClientSettings> {
        let alt = match (self.smtp_alt_port, self.smtp_alt_security.as_deref().and_then(parse_security)) {
            (Some(p), Some(t)) => Some((u16::try_from(p).ok()?, t)),
            _ => None,
        };
        Some(ClientSettings {
            imap_host: self.imap_host,
            imap_port: u16::try_from(self.imap_port).ok()?,
            smtp_host: self.smtp_host,
            smtp_port: u16::try_from(self.smtp_port).ok()?,
            smtp_tls: parse_security(&self.smtp_security)?,
            smtp_alt: alt,
            source: if self.domain_id.is_some() { "domain" } else { "platform" },
            version: self.version,
        })
    }
}

const PROFILE_COLUMNS: &str =
    "domain_id, imap_host, imap_port, smtp_host, smtp_port, smtp_security, smtp_alt_port, smtp_alt_security, version";

/// Effective settings for a mailbox domain (or the platform default).
pub async fn effective(state: &AppState, domain_id: Option<Uuid>) -> ApiResult<ClientSettings> {
    let row: Option<ProfileRow> = sqlx::query_as(&format!(
        "SELECT {PROFILE_COLUMNS} FROM mail_client_profiles
         WHERE domain_id = $1 OR domain_id IS NULL
         ORDER BY domain_id IS NULL LIMIT 1"
    ))
    .bind(domain_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.and_then(ProfileRow::into_settings).unwrap_or_else(|| ClientSettings::from_config(&state.cfg)))
}

/// Effective settings for a hosted domain by name (autoconfig lookups).
pub async fn effective_for_domain_name(state: &AppState, domain: &str) -> ApiResult<Option<ClientSettings>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT d.id FROM domains d JOIN mail_domains md ON md.domain = d.domain_name::text WHERE d.domain_name = $1",
    )
    .bind(domain)
    .fetch_optional(&state.db)
    .await?;
    match id {
        Some(id) => Ok(Some(effective(state, Some(id)).await?)),
        None => Ok(None),
    }
}

pub async fn platform_default(state: &AppState) -> ApiResult<ClientSettings> {
    let row: Option<ProfileRow> =
        sqlx::query_as(&format!("SELECT {PROFILE_COLUMNS} FROM mail_client_profiles WHERE domain_id IS NULL"))
            .fetch_optional(&state.db)
            .await?;
    Ok(row.and_then(ProfileRow::into_settings).unwrap_or_else(|| ClientSettings::from_config(&state.cfg)))
}

/// Operator input for a profile, validated before it is stored.
#[derive(Debug, Deserialize)]
pub struct ProfileInput {
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: String,
    pub smtp_alt_port: Option<u16>,
    pub smtp_alt_security: Option<String>,
    pub change_note: Option<String>,
}

pub struct ValidProfile {
    pub imap_host: String,
    pub imap_port: i32,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_security: &'static str,
    pub smtp_alt_port: Option<i32>,
    pub smtp_alt_security: Option<&'static str>,
    pub change_note: Option<String>,
}

impl ProfileInput {
    pub fn validate(self) -> ApiResult<ValidProfile> {
        let imap_host = self.imap_host.trim().trim_end_matches('.').to_ascii_lowercase();
        let smtp_host = self.smtp_host.trim().trim_end_matches('.').to_ascii_lowercase();
        for (field, host) in [("imap_host", &imap_host), ("smtp_host", &smtp_host)] {
            if !is_valid_mail_hostname(host) {
                return Err(AppError::validation(format!(
                    "{field} must be a public DNS hostname without scheme or port (for example mail.example.edu)"
                )));
            }
            if host.ends_with(".internal") || host.ends_with(".local") || host.ends_with(".lan") {
                return Err(AppError::validation(format!(
                    "{field} must be reachable from the internet, not a private name"
                )));
            }
        }
        if self.imap_port == 0 || self.smtp_port == 0 {
            return Err(AppError::validation("Ports must be between 1 and 65535"));
        }
        let smtp_security = match self.smtp_security.as_str() {
            "tls" => "tls",
            "starttls" => "starttls",
            _ => {
                return Err(AppError::validation(
                    "smtp_security must be 'tls' or 'starttls'; unencrypted submission is not allowed",
                ))
            }
        };
        let (smtp_alt_port, smtp_alt_security) =
            match (self.smtp_alt_port.filter(|p| *p != 0), self.smtp_alt_security.as_deref()) {
                (None, _) => (None, None),
                (Some(p), Some(s @ ("tls" | "starttls"))) => {
                    if p == self.smtp_port {
                        return Err(AppError::validation(
                            "The alternative submission port must differ from the primary port",
                        ));
                    }
                    (Some(p as i32), Some(if s == "tls" { "tls" } else { "starttls" }))
                }
                (Some(_), _) => return Err(AppError::validation("smtp_alt_security must be 'tls' or 'starttls'")),
            };
        let change_note = self.change_note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        if change_note.as_ref().is_some_and(|n| n.chars().count() > 500) {
            return Err(AppError::validation("change_note must be at most 500 characters"));
        }
        Ok(ValidProfile {
            imap_host,
            imap_port: self.imap_port as i32,
            smtp_host,
            smtp_port: self.smtp_port as i32,
            smtp_security,
            smtp_alt_port,
            smtp_alt_security,
            change_note,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProfileInput {
        ProfileInput {
            imap_host: "Mail.Example.EDU".into(),
            imap_port: 993,
            smtp_host: "mail.example.edu".into(),
            smtp_port: 465,
            smtp_security: "tls".into(),
            smtp_alt_port: Some(587),
            smtp_alt_security: Some("starttls".into()),
            change_note: Some(" New certificate ".into()),
        }
    }

    #[test]
    fn validates_and_normalizes_profiles() {
        let p = input().validate().unwrap();
        assert_eq!(p.imap_host, "mail.example.edu");
        assert_eq!(p.smtp_alt_port, Some(587));
        assert_eq!(p.change_note.as_deref(), Some("New certificate"));
    }

    #[test]
    fn rejects_plaintext_private_and_malformed_values() {
        let mut i = input();
        i.smtp_security = "none".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.imap_host = "https://mail.example.edu".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.smtp_host = "postfix.internal".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.smtp_alt_port = Some(465);
        assert!(i.validate().is_err());
        let mut i = input();
        i.imap_port = 0;
        assert!(i.validate().is_err());
    }

    #[test]
    fn json_never_carries_secrets() {
        let cfg = Config::for_tests("postgres://localhost/x");
        let v = ClientSettings::from_config(&cfg).to_json(Some("a@example.edu"));
        let s = v.to_string();
        assert!(!s.contains("password\":"), "{s}");
        assert_eq!(v["imap"]["port"], 993);
        assert_eq!(v["smtp"]["security"], "SSL/TLS");
        assert_eq!(v["smtp_alternative"]["port"], 587);
    }
}
