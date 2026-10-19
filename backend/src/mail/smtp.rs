//! Submission to the platform's outbound relay (Postfix in production,
//! Mailpit in development).
//!
//! The worker never talks to remote MX hosts directly: Postfix owns queueing,
//! remote retries, TLS policy and connection management (guide 03). A 2xx
//! from the relay means *the relay accepted responsibility* — status
//! `accepted_by_mta` — not that the recipient's server accepted it.

use std::time::Duration;

use lettre::{
    address::Envelope,
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParameters},
        PoolConfig,
    },
    Address, AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};

use crate::config::{SmtpRelayConfig, TlsMode};

#[derive(Clone)]
pub struct SmtpRelay {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

#[derive(Debug)]
pub struct Accepted {
    /// Postfix queue id parsed from "250 2.0.0 Ok: queued as XXXX", when present.
    pub queue_id: Option<String>,
    pub response: String,
}

#[derive(Debug)]
pub enum SubmitError {
    /// 4xx, timeouts and connection failures: retry with backoff.
    Transient { code: Option<u16>, message: String },
    /// 5xx: retrying cannot succeed.
    Permanent { code: Option<u16>, message: String },
}

impl SubmitError {
    pub fn code(&self) -> Option<u16> {
        match self {
            SubmitError::Transient { code, .. } | SubmitError::Permanent { code, .. } => *code,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            SubmitError::Transient { message, .. } | SubmitError::Permanent { message, .. } => message,
        }
    }
}

impl SmtpRelay {
    pub fn new(cfg: &SmtpRelayConfig, helo_name: &str) -> anyhow::Result<Self> {
        let tls_params = || TlsParameters::new(cfg.host.clone());
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
            .port(cfg.port)
            .hello_name(lettre::transport::smtp::extension::ClientId::Domain(helo_name.to_string()))
            .timeout(Some(Duration::from_secs(30)))
            .pool_config(PoolConfig::new().max_size(cfg.pool_size.max(1)));
        builder = match cfg.tls {
            TlsMode::None => builder.tls(Tls::None),
            TlsMode::StartTls => builder.tls(Tls::Required(tls_params()?)),
            TlsMode::Implicit => builder.tls(Tls::Wrapper(tls_params()?)),
        };
        if let (Some(user), Some(pass)) = (&cfg.username, &cfg.password) {
            builder = builder
                .credentials(Credentials::new(user.clone(), pass.clone()))
                .authentication(vec![Mechanism::Plain, Mechanism::Login]);
        }
        Ok(Self { transport: builder.build() })
    }

    pub async fn submit(
        &self,
        envelope_from: &str,
        recipients: &[String],
        raw: &[u8],
    ) -> Result<Accepted, SubmitError> {
        let permanent = |m: String| SubmitError::Permanent { code: None, message: m };
        let from: Address = envelope_from.parse().map_err(|e| permanent(format!("invalid envelope sender: {e}")))?;
        let to = recipients
            .iter()
            .map(|r| r.parse::<Address>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| permanent(format!("invalid recipient: {e}")))?;
        let envelope = Envelope::new(Some(from), to).map_err(|e| permanent(format!("invalid envelope: {e}")))?;

        match self.transport.send_raw(&envelope, raw).await {
            Ok(resp) => {
                let text: Vec<&str> = resp.message().collect();
                let response = format!("{} {}", resp.code(), text.join(" "));
                Ok(Accepted { queue_id: parse_queue_id(&response), response })
            }
            Err(e) => {
                let code = e.status().and_then(|c| c.to_string().parse::<u16>().ok());
                let message = e.to_string();
                if e.is_permanent() {
                    Err(SubmitError::Permanent { code, message })
                } else {
                    Err(SubmitError::Transient { code, message })
                }
            }
        }
    }

    /// Open a connection and issue NOOP; used by readiness reporting.
    pub async fn test_connection(&self) -> bool {
        self.transport.test_connection().await.unwrap_or(false)
    }
}

pub fn parse_queue_id(response: &str) -> Option<String> {
    let idx = response.find("queued as ")?;
    let id: String = response[idx + 10..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    (!id.is_empty()).then_some(id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn queue_id() {
        assert_eq!(super::parse_queue_id("250 2.0.0 Ok: queued as 4Xk9p21ZbVz3"), Some("4Xk9p21ZbVz3".into()));
        assert_eq!(super::parse_queue_id("250 OK"), None);
    }
}
