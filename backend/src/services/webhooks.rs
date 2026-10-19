//! Outbound webhook HTTP client with SSRF protection, and request signing.
//!
//! * Target URLs must be https (http only when private targets are allowed
//!   in development), without embedded credentials.
//! * DNS answers are filtered at connect time: loopback, private, link-local,
//!   CGNAT, multicast and other special-purpose addresses are refused. Doing
//!   this in the resolver (not just at URL validation) defeats DNS rebinding.
//! * Redirects are not followed.
//! * Each request carries `Mailvora-Signature: t=<unix>,v1=<hex HMAC-SHA256>` over
//!   `"<t>.<raw body>"`; receivers should reject stale timestamps (replay).

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::{
    config::Config,
    crypto,
    error::{ApiResult, AppError},
};

pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => forbidden_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return forbidden_v4(v4);
            }
            let seg = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (seg[0] & 0xffc0) == 0xfe80 // link local fe80::/10
                || (seg[0] == 0x2001 && seg[1] == 0x0db8) // documentation
                || (seg[0] == 0x64 && seg[1] == 0xff9b) // NAT64 64:ff9b::/32
        }
    }
}

fn forbidden_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || o[0] == 0
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18.0.0/15 benchmarking
        || o[0] >= 240 // reserved
}

pub fn validate_url(cfg: &Config, raw: &str) -> ApiResult<String> {
    let url = url::Url::parse(raw.trim()).map_err(|_| AppError::validation("url is not a valid URL"))?;
    match url.scheme() {
        "https" => {}
        "http" if cfg.webhook_allow_private_targets => {}
        _ => return Err(AppError::validation("Webhook URLs must use https://")),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::validation("Webhook URLs must not contain credentials"));
    }
    let host = url.host_str().ok_or_else(|| AppError::validation("Webhook URL needs a host"))?;
    if !cfg.webhook_allow_private_targets {
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
            if is_forbidden_ip(ip) {
                return Err(AppError::validation("Webhook URL points to a private or reserved address"));
            }
        }
        if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") || host.ends_with(".internal") {
            return Err(AppError::validation("Webhook URL points to a private host"));
        }
    }
    Ok(url.to_string())
}

struct GuardedResolver;

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let allowed: Vec<SocketAddr> = addrs.into_iter().filter(|a| !is_forbidden_ip(a.ip())).collect();
            if allowed.is_empty() {
                return Err(format!("{host} resolves only to private or reserved addresses").into());
            }
            Ok(Box::new(allowed.into_iter()) as Addrs)
        })
    }
}

pub fn http_client(cfg: &Config) -> anyhow::Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Mailvora-Webhooks/", env!("CARGO_PKG_VERSION")));
    if !cfg.webhook_allow_private_targets {
        b = b.dns_resolver(Arc::new(GuardedResolver));
    }
    Ok(b.build()?)
}

pub fn new_secret() -> String {
    format!("whsec_{}", crypto::random_token(32))
}

pub fn signature_header(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(body);
    format!("t={timestamp},v1={}", hex::encode(crypto::hmac_sha256(secret.as_bytes(), &signed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(is_forbidden_ip(ip.parse().unwrap()), "{ip} must be forbidden");
        }
        for ip in ["93.184.216.34", "1.1.1.1", "2606:4700:4700::1111"] {
            assert!(!is_forbidden_ip(ip.parse().unwrap()), "{ip} must be allowed");
        }
    }

    #[test]
    fn signature_is_stable() {
        let s = signature_header("whsec_test", 1700000000, b"{\"a\":1}");
        assert!(s.starts_with("t=1700000000,v1="));
        assert_eq!(s, signature_header("whsec_test", 1700000000, b"{\"a\":1}"));
        assert_ne!(s, signature_header("whsec_other", 1700000000, b"{\"a\":1}"));
    }
}
