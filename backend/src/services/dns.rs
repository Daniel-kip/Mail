//! Live DNS checks for customer domains.
//!
//! Verification policy (frontend/resources/wiki/Decision-Log.md D-004):
//! * **Ownership** – `_mailvora.<domain> TXT "mailvora-verification=<token>"`.
//!   Required. Proves the tenant controls the zone.
//! * **DKIM** – `<selector>._domainkey.<domain> TXT "v=DKIM1; k=rsa; p=<key>"`.
//!   Required: every message is DKIM-signed with this key, and DKIM is what
//!   gives DMARC alignment for the customer's From domain.
//! * **SPF** – recommended. Our envelope sender (Return-Path) lives on the
//!   platform's bounce domain, whose SPF we control; the customer record
//!   only matters for receivers that evaluate it directly.
//! * **DMARC** – recommended (start with p=none, tighten later).
//! * **MX** – required only for hosted mailboxes (inbound mail).

use std::time::Duration;

use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts},
    error::ResolveErrorKind,
    TokioAsyncResolver,
};
use serde::Serialize;

pub const OWNERSHIP_HOST_PREFIX: &str = "_mailvora";
pub const OWNERSHIP_VALUE_PREFIX: &str = "mailvora-verification=";

pub struct DnsChecker {
    resolver: TokioAsyncResolver,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordCheck {
    pub ok: bool,
    /// What we found, verbatim, so users can debug their DNS.
    pub observed: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainCheck {
    pub ownership: RecordCheck,
    pub dkim: RecordCheck,
    pub spf: RecordCheck,
    pub dmarc: RecordCheck,
    pub mx: RecordCheck,
    pub checked_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub development_override: bool,
}

impl DomainCheck {
    /// Sending is authorized once ownership and DKIM are in place.
    pub fn verified(&self) -> bool {
        self.ownership.ok && self.dkim.ok
    }
}

pub struct ExpectedRecords<'a> {
    pub domain: &'a str,
    pub verification_token: &'a str,
    pub dkim_selector: &'a str,
    pub dkim_public_key: &'a str,
    pub spf_include: &'a str,
    pub mail_hostname: &'a str,
}

impl DnsChecker {
    pub fn new() -> anyhow::Result<Self> {
        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_secs(4);
        opts.attempts = 2;
        // Keep caching short so a user who just published a record can recheck.
        opts.positive_max_ttl = Some(Duration::from_secs(30));
        opts.negative_max_ttl = Some(Duration::from_secs(5));
        Ok(Self { resolver: TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), opts) })
    }

    async fn txt(&self, name: &str) -> Result<Vec<String>, String> {
        match self.resolver.txt_lookup(name).await {
            Ok(records) => Ok(records
                .iter()
                // A TXT record may be split into several character-strings.
                .map(|r| r.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect::<String>())
                .collect()),
            Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => Ok(vec![]),
            Err(e) => Err(format!("DNS lookup failed: {e}")),
        }
    }

    async fn mx(&self, name: &str) -> Result<Vec<String>, String> {
        match self.resolver.mx_lookup(name).await {
            Ok(records) => Ok(records
                .iter()
                .map(|r| format!("{} {}", r.preference(), r.exchange().to_ascii().trim_end_matches('.').to_lowercase()))
                .collect()),
            Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => Ok(vec![]),
            Err(e) => Err(format!("DNS lookup failed: {e}")),
        }
    }

    pub async fn check(&self, exp: &ExpectedRecords<'_>) -> DomainCheck {
        let ownership_host = format!("{OWNERSHIP_HOST_PREFIX}.{}", exp.domain);
        let dkim_host = format!("{}._domainkey.{}", exp.dkim_selector, exp.domain);
        let dmarc_host = format!("_dmarc.{}", exp.domain);

        let (own, root, dkim, dmarc, mx) = tokio::join!(
            self.txt(&ownership_host),
            self.txt(exp.domain),
            self.txt(&dkim_host),
            self.txt(&dmarc_host),
            self.mx(exp.domain),
        );

        DomainCheck {
            ownership: evaluate_ownership(own, exp.verification_token),
            dkim: evaluate_dkim(dkim, exp.dkim_public_key),
            spf: evaluate_spf(root, exp.spf_include),
            dmarc: evaluate_dmarc(dmarc),
            mx: evaluate_mx(mx, exp.mail_hostname),
            checked_at: chrono::Utc::now(),
            development_override: false,
        }
    }
}

fn lookup_failed(err: String) -> RecordCheck {
    RecordCheck { ok: false, observed: vec![], message: err }
}

pub fn evaluate_ownership(found: Result<Vec<String>, String>, token: &str) -> RecordCheck {
    let found = match found {
        Ok(f) => f,
        Err(e) => return lookup_failed(e),
    };
    let expected = format!("{OWNERSHIP_VALUE_PREFIX}{token}");
    let ok = found.iter().any(|v| v.trim() == expected);
    let message = if ok {
        "Ownership record found".to_string()
    } else if found.is_empty() {
        "No ownership TXT record found yet. DNS changes can take a few minutes to propagate.".to_string()
    } else {
        "A TXT record exists but does not contain the expected verification value".to_string()
    };
    RecordCheck { ok, observed: found, message }
}

fn normalize_key(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

pub fn evaluate_dkim(found: Result<Vec<String>, String>, public_key: &str) -> RecordCheck {
    let found = match found {
        Ok(f) => f,
        Err(e) => return lookup_failed(e),
    };
    let want = normalize_key(public_key);
    let ok = found.iter().any(|v| {
        v.split(';')
            .filter_map(|tag| tag.trim().split_once('='))
            .any(|(k, val)| k.trim() == "p" && normalize_key(val) == want)
    });
    let message = if ok {
        "DKIM public key matches".to_string()
    } else if found.is_empty() {
        "No DKIM record found at the selector host".to_string()
    } else {
        "A DKIM record exists but its public key does not match this domain's signing key".to_string()
    };
    RecordCheck { ok, observed: found, message }
}

pub fn evaluate_spf(found: Result<Vec<String>, String>, include: &str) -> RecordCheck {
    let found = match found {
        Ok(f) => f,
        Err(e) => return lookup_failed(e),
    };
    let spf: Vec<String> = found.into_iter().filter(|v| v.to_ascii_lowercase().starts_with("v=spf1")).collect();
    let mechanism = if include.starts_with("ip4:") || include.starts_with("ip6:") || include.starts_with("a:") {
        include.to_string()
    } else {
        format!("include:{include}")
    };
    let (ok, message) = match spf.len() {
        0 => (false, format!("No SPF record. Recommended: \"v=spf1 {mechanism} ~all\"")),
        1 if spf[0].split_whitespace().any(|t| t.eq_ignore_ascii_case(&mechanism)) => {
            (true, "SPF authorizes the platform".to_string())
        }
        1 => (false, format!("SPF record exists but does not contain '{mechanism}'")),
        _ => (false, "Multiple SPF records found. Receivers treat this as a permanent error (RFC 7208 §4.5); merge them into one.".to_string()),
    };
    RecordCheck { ok, observed: spf, message }
}

pub fn evaluate_dmarc(found: Result<Vec<String>, String>) -> RecordCheck {
    let found = match found {
        Ok(f) => f,
        Err(e) => return lookup_failed(e),
    };
    let dmarc: Vec<String> = found.into_iter().filter(|v| v.trim_start().starts_with("v=DMARC1")).collect();
    let ok = dmarc.len() == 1;
    let message = match dmarc.len() {
        0 => "No DMARC policy. Recommended to start with \"v=DMARC1; p=none; rua=mailto:…\"".to_string(),
        1 => "DMARC policy published".to_string(),
        _ => "Multiple DMARC records found; receivers will ignore them".to_string(),
    };
    RecordCheck { ok, observed: dmarc, message }
}

pub fn evaluate_mx(found: Result<Vec<String>, String>, mail_hostname: &str) -> RecordCheck {
    let found = match found {
        Ok(f) => f,
        Err(e) => return lookup_failed(e),
    };
    let target = mail_hostname.to_lowercase();
    let ok = found.iter().any(|r| r.split_whitespace().nth(1) == Some(target.as_str()));
    let message = if ok {
        "MX points to the platform mail host".to_string()
    } else if found.is_empty() {
        format!("No MX record. Required for hosted mailboxes: \"10 {target}\"")
    } else {
        format!("MX records exist but none point to {target}")
    };
    RecordCheck { ok, observed: found, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership() {
        let r = evaluate_ownership(Ok(vec!["mailvora-verification=abc".into()]), "abc");
        assert!(r.ok);
        assert!(!evaluate_ownership(Ok(vec!["mailvora-verification=zzz".into()]), "abc").ok);
        assert!(!evaluate_ownership(Ok(vec![]), "abc").ok);
        assert!(!evaluate_ownership(Err("timeout".into()), "abc").ok);
    }

    #[test]
    fn dkim_matches_split_and_spaced_keys() {
        let rec = "v=DKIM1; k=rsa; p=MIIB IjAN Bgkq".to_string();
        assert!(evaluate_dkim(Ok(vec![rec]), "MIIBIjANBgkq").ok);
        assert!(!evaluate_dkim(Ok(vec!["v=DKIM1; p=OTHER".into()]), "MIIBIjANBgkq").ok);
    }

    #[test]
    fn spf_rules() {
        let inc = "_spf.example.net";
        assert!(evaluate_spf(Ok(vec!["v=spf1 include:_spf.example.net ~all".into()]), inc).ok);
        assert!(!evaluate_spf(Ok(vec!["v=spf1 -all".into()]), inc).ok);
        let dup = evaluate_spf(Ok(vec!["v=spf1 include:_spf.example.net ~all".into(), "v=spf1 -all".into()]), inc);
        assert!(!dup.ok && dup.message.contains("Multiple"));
        assert!(evaluate_spf(Ok(vec!["v=spf1 ip4:1.2.3.4 -all".into()]), "ip4:1.2.3.4").ok);
    }

    #[test]
    fn mx_rules() {
        assert!(evaluate_mx(Ok(vec!["10 mail.host.net".into()]), "mail.host.net").ok);
        assert!(!evaluate_mx(Ok(vec!["10 aspmx.l.google.com".into()]), "mail.host.net").ok);
    }
}
