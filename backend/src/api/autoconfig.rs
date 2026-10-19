//! Automatic mail-client configuration documents.
//!
//! * Mozilla autoconfig (`config-v1.1.xml`), queried by Thunderbird,
//!   Thunderbird for Android (K-9), FairEmail, Evolution and others at
//!   `https://autoconfig.<domain>/mail/config-v1.1.xml?emailaddress=...` and
//!   `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`. Clients
//!   also try the MX host's parent domain, so one platform-level
//!   `autoconfig.<platform>` host serves every hosted domain.
//! * Microsoft Autodiscover "POX" (`/autodiscover/autodiscover.xml`), used by
//!   Outlook for Windows/Mac for IMAP accounts.
//!
//! Neither Gmail nor every Outlook mobile version reads these documents
//! (they use their own provider databases), so the guided setup remains the
//! universal path. The documents contain only public endpoint names and
//! answer only for verified, hosted domains: hosting status is already
//! public through MX records, and mailbox existence is never revealed.

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use crate::{
    auth::ClientInfo,
    services::{
        mail_client::{self, security_code, ClientSettings},
        system_mail::escape_html,
    },
    state::AppState,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/.well-known/autoconfig/mail/config-v1.1.xml", get(mozilla))
        .route("/mail/config-v1.1.xml", get(mozilla))
        .route("/autodiscover/autodiscover.xml", get(autodiscover).post(autodiscover))
        .route("/Autodiscover/Autodiscover.xml", get(autodiscover).post(autodiscover))
}

#[derive(Deserialize)]
struct MozQuery {
    emailaddress: Option<String>,
}

fn xml_response(status: StatusCode, body: String) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=3600")],
        body,
    )
        .into_response()
}

fn domain_of(address: &str) -> Option<String> {
    let (_, domain) = address.trim().rsplit_once('@')?;
    validation::normalize_domain(domain).ok()
}

fn socket_type(t: crate::config::TlsMode) -> &'static str {
    if security_code(t) == "tls" {
        "SSL"
    } else {
        "STARTTLS"
    }
}

pub fn mozilla_document(product: &str, domain: &str, s: &ClientSettings) -> String {
    let alt = s
        .smtp_alt
        .map(|(port, tls)| {
            format!(
                r#"
    <outgoingServer type="smtp">
      <hostname>{host}</hostname>
      <port>{port}</port>
      <socketType>{socket}</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </outgoingServer>"#,
                host = escape_html(&s.smtp_host),
                socket = socket_type(tls),
            )
        })
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="{domain}">
    <domain>{domain}</domain>
    <displayName>{product}</displayName>
    <displayShortName>{product}</displayShortName>
    <incomingServer type="imap">
      <hostname>{imap_host}</hostname>
      <port>{imap_port}</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>{smtp_host}</hostname>
      <port>{smtp_port}</port>
      <socketType>{smtp_socket}</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </outgoingServer>{alt}
  </emailProvider>
</clientConfig>
"#,
        domain = escape_html(domain),
        product = escape_html(product),
        imap_host = escape_html(&s.imap_host),
        imap_port = s.imap_port,
        smtp_host = escape_html(&s.smtp_host),
        smtp_port = s.smtp_port,
        smtp_socket = socket_type(s.smtp_tls),
    )
}

async fn mozilla(State(state): State<AppState>, client: ClientInfo, Query(q): Query<MozQuery>) -> Response {
    if state.limiter.check(&format!("autoconfig:{}", client.ip_key()), 120, 600).is_err() {
        return xml_response(StatusCode::TOO_MANY_REQUESTS, String::new());
    }
    let Some(domain) = q.emailaddress.as_deref().and_then(domain_of) else {
        return xml_response(StatusCode::BAD_REQUEST, String::new());
    };
    match mail_client::effective_for_domain_name(&state, &domain).await {
        Ok(Some(s)) => xml_response(StatusCode::OK, mozilla_document(&state.cfg.product_name, &domain, &s)),
        Ok(None) => xml_response(StatusCode::NOT_FOUND, String::new()),
        Err(e) => {
            tracing::warn!(error = %e, "autoconfig lookup failed");
            xml_response(StatusCode::SERVICE_UNAVAILABLE, String::new())
        }
    }
}

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<EMailAddress>\s*([^<\s]{3,254})\s*</EMailAddress>").unwrap());

pub fn autodiscover_document(address: &str, s: &ClientSettings) -> String {
    let smtp_ssl = if security_code(s.smtp_tls) == "tls" { "on" } else { "off" };
    let smtp_encryption = if security_code(s.smtp_tls) == "tls" { "SSL" } else { "TLS" };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/responseschema/2006">
  <Response xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a">
    <Account>
      <AccountType>email</AccountType>
      <Action>settings</Action>
      <Protocol>
        <Type>IMAP</Type>
        <Server>{imap_host}</Server>
        <Port>{imap_port}</Port>
        <LoginName>{address}</LoginName>
        <DomainRequired>off</DomainRequired>
        <SPA>off</SPA>
        <SSL>on</SSL>
        <Encryption>SSL</Encryption>
        <AuthRequired>on</AuthRequired>
      </Protocol>
      <Protocol>
        <Type>SMTP</Type>
        <Server>{smtp_host}</Server>
        <Port>{smtp_port}</Port>
        <LoginName>{address}</LoginName>
        <DomainRequired>off</DomainRequired>
        <SPA>off</SPA>
        <SSL>{smtp_ssl}</SSL>
        <Encryption>{smtp_encryption}</Encryption>
        <AuthRequired>on</AuthRequired>
        <UsePOPAuth>on</UsePOPAuth>
        <SMTPLast>off</SMTPLast>
      </Protocol>
    </Account>
  </Response>
</Autodiscover>
"#,
        imap_host = escape_html(&s.imap_host),
        imap_port = s.imap_port,
        smtp_host = escape_html(&s.smtp_host),
        smtp_port = s.smtp_port,
        address = escape_html(address),
    )
}

async fn autodiscover(State(state): State<AppState>, client: ClientInfo, body: Bytes) -> Response {
    if state.limiter.check(&format!("autodiscover:{}", client.ip_key()), 120, 600).is_err() {
        return xml_response(StatusCode::TOO_MANY_REQUESTS, String::new());
    }
    // Requests are tiny; never parse large bodies.
    if body.len() > 8 * 1024 {
        return xml_response(StatusCode::PAYLOAD_TOO_LARGE, String::new());
    }
    let text = String::from_utf8_lossy(&body);
    let Some(address) = EMAIL_RE.captures(&text).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_lowercase())
    else {
        return xml_response(StatusCode::BAD_REQUEST, String::new());
    };
    let Some(domain) = domain_of(&address) else {
        return xml_response(StatusCode::BAD_REQUEST, String::new());
    };
    match mail_client::effective_for_domain_name(&state, &domain).await {
        Ok(Some(s)) => xml_response(StatusCode::OK, autodiscover_document(&address, &s)),
        Ok(None) => xml_response(StatusCode::NOT_FOUND, String::new()),
        Err(e) => {
            tracing::warn!(error = %e, "autodiscover lookup failed");
            xml_response(StatusCode::SERVICE_UNAVAILABLE, String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn mozilla_document_lists_primary_and_alternative_submission() {
        let cfg = Config::for_tests("postgres://localhost/x");
        let s = ClientSettings::from_config(&cfg);
        let x = mozilla_document("Mailvora", "school.ac.ke", &s);
        assert!(x.contains("<incomingServer type=\"imap\">"));
        assert!(x.contains("<port>993</port>"));
        assert!(x.contains("<port>465</port>"));
        assert!(x.contains("<port>587</port>"));
        assert!(x.contains("<socketType>STARTTLS</socketType>"));
        assert!(x.contains("%EMAILADDRESS%"));
        assert!(!x.to_lowercase().contains("password<"));
    }

    #[test]
    fn autodiscover_extracts_address_safely() {
        let req = "<Autodiscover><Request><EMailAddress> Amina@School.ac.ke </EMailAddress></Request></Autodiscover>";
        let cap = EMAIL_RE.captures(req).unwrap().get(1).unwrap().as_str();
        assert_eq!(cap, "Amina@School.ac.ke");
        let cfg = Config::for_tests("postgres://localhost/x");
        let doc = autodiscover_document("a<b>@x.ke", &ClientSettings::from_config(&cfg));
        assert!(doc.contains("a&lt;b&gt;@x.ke"));
        assert!(doc.contains("<Type>IMAP</Type>"));
    }
}
