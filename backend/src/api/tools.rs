//! Public deliverability tools (no authentication; rate-limited per IP).
//!
//! * `GET  /v1/tools/dns?name=&type=`  DNS lookup (A, AAAA, MX, TXT, CNAME, NS, SOA, CAA, SRV, PTR)
//! * `GET  /v1/tools/domain?domain=`   MX, SPF, DMARC, DKIM, MTA-STS, TLS-RPT, BIMI, rDNS, blocklists
//! * `POST /v1/tools/headers`          analyze pasted message headers (no network access)

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::common::ApiJson;
use crate::{
    auth::ClientInfo,
    error::{ApiResult, AppError},
    services::mailtools::{analyze_headers, clean_domain, LOOKUP_TYPES},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tools/dns", get(dns_lookup))
        .route("/tools/domain", get(domain_report))
        .route("/tools/headers", post(headers))
        .route("/tools/blocklist", get(blocklist))
        .route("/tools/ports", get(ports))
}

#[derive(Deserialize)]
struct TargetQuery {
    target: String,
}

async fn blocklist(
    State(state): State<AppState>,
    client: ClientInfo,
    Query(q): Query<TargetQuery>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("tools-blocklist:{}", client.ip_key()), 12, 60)?;
    let target = match q.target.trim().parse::<std::net::IpAddr>() {
        Ok(ip) => ip.to_string(),
        Err(_) => clean_domain(&q.target).ok_or_else(|| AppError::validation("Enter a public IP address or domain"))?,
    };
    Ok(Json(state.tools.blocklist_report(&target).await))
}

async fn ports(
    State(state): State<AppState>,
    client: ClientInfo,
    Query(q): Query<TargetQuery>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("tools-ports:{}", client.ip_key()), 6, 60)?;
    let target = match q.target.trim().parse::<std::net::IpAddr>() {
        Ok(ip) => ip.to_string(),
        Err(_) => {
            clean_domain(&q.target).ok_or_else(|| AppError::validation("Enter a public hostname or IP address"))?
        }
    };
    state.tools.port_report(&target).await.map(Json).map_err(AppError::validation)
}

#[derive(Deserialize)]
struct DnsQuery {
    name: String,
    #[serde(rename = "type", default = "default_type")]
    rtype: String,
}

fn default_type() -> String {
    "A".into()
}

async fn dns_lookup(
    State(state): State<AppState>,
    client: ClientInfo,
    Query(q): Query<DnsQuery>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("tools-dns:{}", client.ip_key()), 60, 60)?;
    let rtype = q.rtype.trim().to_ascii_uppercase();
    if !LOOKUP_TYPES.contains(&rtype.as_str()) {
        return Err(AppError::validation(format!("type must be one of {}", LOOKUP_TYPES.join(", "))));
    }
    let name = if rtype == "PTR" {
        q.name
            .trim()
            .parse::<std::net::IpAddr>()
            .map_err(|_| AppError::validation("PTR lookups need an IP address"))?
            .to_string()
    } else {
        clean_domain(&q.name).ok_or_else(|| AppError::validation("Enter a public domain name such as example.com"))?
    };
    let started = std::time::Instant::now();
    match state.tools.lookup(&name, &rtype).await {
        Ok(answers) => Ok(Json(json!({
            "name": name, "type": rtype, "answers": answers,
            "elapsed_ms": started.elapsed().as_millis() as u64, "resolver": "1.1.1.1 (Cloudflare)",
        }))),
        Err(e) => Err(AppError::unavailable("dns_lookup_failed", format!("The lookup failed: {e}"))),
    }
}

#[derive(Deserialize)]
struct DomainQuery {
    domain: String,
}

async fn domain_report(
    State(state): State<AppState>,
    client: ClientInfo,
    Query(q): Query<DomainQuery>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("tools-domain:{}", client.ip_key()), 12, 60)?;
    let domain = clean_domain(&q.domain)
        .ok_or_else(|| AppError::validation("Enter a public domain name such as example.com"))?;
    Ok(Json(state.tools.domain_report(&domain, &state.tools_http).await))
}

#[derive(Deserialize)]
struct HeadersBody {
    headers: String,
}

async fn headers(
    State(state): State<AppState>,
    client: ClientInfo,
    ApiJson(b): ApiJson<HeadersBody>,
) -> ApiResult<Json<Value>> {
    state.limiter.check(&format!("tools-headers:{}", client.ip_key()), 30, 60)?;
    if b.headers.trim().is_empty() || b.headers.len() > 128 * 1024 {
        return Err(AppError::validation("Paste the message headers (up to 128 KB)"));
    }
    Ok(Json(analyze_headers(&b.headers)))
}
