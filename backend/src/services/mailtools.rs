//! Public email-deliverability tools: DNS lookups, a domain health report
//! (MX, SPF with RFC 7208 lookup counting, DMARC, DKIM selector discovery,
//! MTA-STS, TLS-RPT, BIMI, reverse DNS, DNS blocklists) and a message-header
//! analyzer.
//!
//! Only DNS is queried, except the MTA-STS policy file, which is fetched from
//! `https://mta-sts.<domain>` through the SSRF-guarded client used for
//! webhooks (public addresses only, no redirects, small body cap).

use std::{collections::BTreeMap, net::IpAddr, str::FromStr, time::Duration};

use futures::future::join_all;
use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts},
    error::ResolveErrorKind,
    proto::rr::{RData, RecordType},
    TokioAsyncResolver,
};
use serde::Serialize;
use serde_json::{json, Value};

/// Record types the lookup tool answers.
pub const LOOKUP_TYPES: &[&str] = &["A", "AAAA", "MX", "TXT", "CNAME", "NS", "SOA", "CAA", "SRV", "PTR"];

/// Selectors used by common providers, checked when discovering DKIM keys.
const DKIM_SELECTORS: &[&str] = &[
    "default",
    "google",
    "selector1",
    "selector2",
    "k1",
    "k2",
    "k3",
    "s1",
    "s2",
    "mail",
    "dkim",
    "smtp",
    "mx",
    "zoho",
    "mailjet",
    "mandrill",
    "pm",
    "sendgrid",
    "amazonses",
    "everlytickey1",
    "fm1",
    "fm2",
    "fm3",
];

/// DNS blocklists that answer queries from public resolvers. Spamhaus is
/// excluded because it refuses queries relayed by large public resolvers.
const BLOCKLISTS: &[(&str, &str)] = &[
    ("bl.spamcop.net", "SpamCop"),
    ("b.barracudacentral.org", "Barracuda"),
    ("psbl.surriel.com", "PSBL"),
    ("dnsbl-1.uceprotect.net", "UCEPROTECT L1"),
    ("bl.mailspike.net", "Mailspike"),
];

pub struct MailTools {
    resolver: TokioAsyncResolver,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub level: &'static str, // pass | info | warn | fail
    pub message: String,
}

fn finding(level: &'static str, message: impl Into<String>) -> Finding {
    Finding { level, message: message.into() }
}

impl MailTools {
    pub fn new() -> Self {
        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_secs(4);
        opts.attempts = 2;
        opts.positive_max_ttl = Some(Duration::from_secs(60));
        opts.negative_max_ttl = Some(Duration::from_secs(10));
        Self { resolver: TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), opts) }
    }

    /// Generic lookup returning presentation-format answers.
    pub async fn lookup(&self, name: &str, rtype: &str) -> Result<Vec<String>, String> {
        let rtype = rtype.to_ascii_uppercase();
        if rtype == "PTR" {
            let ip = IpAddr::from_str(name).map_err(|_| "PTR lookups need an IP address".to_string())?;
            return match self.resolver.reverse_lookup(ip).await {
                Ok(r) => Ok(r.iter().map(|n| n.to_ascii().trim_end_matches('.').to_string()).collect()),
                Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => Ok(vec![]),
                Err(e) => Err(format!("{e}")),
            };
        }
        let rt = RecordType::from_str(&rtype).map_err(|_| format!("Unsupported record type {rtype}"))?;
        match self.resolver.lookup(name, rt).await {
            Ok(answer) => Ok(answer
                .record_iter()
                .filter(|r| r.record_type() == rt)
                .filter_map(|r| r.data().map(present))
                .collect()),
            Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => Ok(vec![]),
            Err(e) => Err(format!("{e}")),
        }
    }

    async fn txt(&self, name: &str) -> Vec<String> {
        self.lookup(name, "TXT").await.unwrap_or_default()
    }

    async fn ips(&self, host: &str) -> Vec<IpAddr> {
        match self.resolver.lookup_ip(host).await {
            Ok(r) => r.iter().collect(),
            Err(_) => vec![],
        }
    }

    /// Full deliverability report for a domain.
    pub async fn domain_report(&self, domain: &str, http: &reqwest::Client) -> Value {
        let tls_rpt_host = format!("_smtp._tls.{domain}");
        let bimi_host = format!("default._bimi.{domain}");
        let (mx, spf, dmarc, dkim, mta_sts, tls_rpt, bimi, caa) = tokio::join!(
            self.mx_report(domain),
            self.spf_report(domain),
            self.dmarc_report(domain),
            self.dkim_report(domain),
            self.mta_sts_report(domain, http),
            self.txt(&tls_rpt_host),
            self.txt(&bimi_host),
            self.lookup(domain, "CAA"),
        );
        let tls_rpt: Vec<String> = tls_rpt.into_iter().filter(|t| t.starts_with("v=TLSRPTv1")).collect();
        let bimi: Vec<String> = bimi.into_iter().filter(|t| t.starts_with("v=BIMI1")).collect();

        let sections = [&mx, &spf, &dmarc, &dkim, &mta_sts];
        let fails = sections.iter().filter(|s| s["status"] == "fail").count();
        let warns = sections.iter().filter(|s| s["status"] == "warn").count();
        let score = (100 - fails as i64 * 20 - warns as i64 * 7 - if tls_rpt.is_empty() { 3 } else { 0 }).clamp(0, 100);
        json!({
            "domain": domain,
            "score": score,
            "grade": match score { 90..=100 => "A", 75..=89 => "B", 60..=74 => "C", 40..=59 => "D", _ => "F" },
            "mx": mx,
            "spf": spf,
            "dmarc": dmarc,
            "dkim": dkim,
            "mta_sts": mta_sts,
            "tls_rpt": { "status": if tls_rpt.is_empty() { "info" } else { "pass" }, "records": tls_rpt,
                         "findings": [if tls_rpt.is_empty() { finding("info", "No TLS-RPT record: you will not receive reports about TLS delivery failures (RFC 8460)") } else { finding("pass", "TLS reporting is enabled") }] },
            "bimi": { "status": if bimi.is_empty() { "info" } else { "pass" }, "records": bimi,
                      "findings": [if bimi.is_empty() { finding("info", "No BIMI record (optional; shows your logo in supporting inboxes once DMARC is enforced)") } else { finding("pass", "BIMI record published") }] },
            "caa": caa.unwrap_or_default(),
            "checked_at": chrono::Utc::now(),
        })
    }

    async fn mx_report(&self, domain: &str) -> Value {
        let raw = match self.resolver.mx_lookup(domain).await {
            Ok(r) => r
                .iter()
                .map(|m| (m.preference(), m.exchange().to_ascii().trim_end_matches('.').to_lowercase()))
                .collect::<Vec<_>>(),
            Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => vec![],
            Err(e) => {
                return json!({ "status": "fail", "records": [], "findings": [finding("fail", format!("MX lookup failed: {e}"))] })
            }
        };
        let mut sorted = raw;
        sorted.sort();
        let mut findings = vec![];
        if sorted.is_empty() {
            findings.push(finding("fail", "No MX records: this domain cannot receive email"));
        }
        if sorted.iter().any(|(_, h)| h.is_empty() || h == ".") {
            findings.push(finding("info", "Null MX (RFC 7505): the domain explicitly accepts no mail"));
        }
        let hosts = join_all(sorted.iter().take(5).map(|(pref, host)| async move {
            let ips = self.ips(host).await;
            let checks = join_all(ips.iter().take(4).map(|ip| self.ip_report(*ip, host))).await;
            json!({ "preference": pref, "host": host, "addresses": checks })
        }))
        .await;
        for h in &hosts {
            let host = h["host"].as_str().unwrap_or_default();
            let addrs = h["addresses"].as_array().cloned().unwrap_or_default();
            if addrs.is_empty() && !host.is_empty() && host != "." {
                findings.push(finding("fail", format!("{host} does not resolve to an IP address")));
            }
            for a in &addrs {
                if a["fcrdns"] == false {
                    findings.push(finding(
                        "warn",
                        format!(
                            "{} ({host}) has no matching reverse DNS (PTR); receivers may distrust it",
                            a["ip"].as_str().unwrap_or("?")
                        ),
                    ));
                }
                for l in a["listed_on"].as_array().into_iter().flatten() {
                    findings.push(finding(
                        "fail",
                        format!("{} is listed on {}", a["ip"].as_str().unwrap_or("?"), l.as_str().unwrap_or("?")),
                    ));
                }
            }
        }
        if findings.is_empty() {
            findings.push(finding(
                "pass",
                format!(
                    "{} mail server{} found, with reverse DNS and no blocklist listings",
                    hosts.len(),
                    if hosts.len() == 1 { "" } else { "s" }
                ),
            ));
        }
        json!({ "status": worst(&findings), "records": hosts, "findings": findings })
    }

    async fn ip_report(&self, ip: IpAddr, host: &str) -> Value {
        let ptr = self.lookup(&ip.to_string(), "PTR").await.unwrap_or_default();
        let mut fcrdns = false;
        for name in ptr.iter().take(2) {
            if self.ips(name).await.contains(&ip) {
                fcrdns = true;
            }
        }
        let listed = match ip {
            IpAddr::V4(v4) => {
                let o = v4.octets();
                let rev = format!("{}.{}.{}.{}", o[3], o[2], o[1], o[0]);
                let results = join_all(BLOCKLISTS.iter().map(|(zone, label)| {
                    let q = format!("{rev}.{zone}");
                    async move {
                        match self.resolver.ipv4_lookup(q).await {
                            // 127.0.0.x answers mean "listed"; 127.255.255.x are resolver refusals.
                            Ok(r) => {
                                r.iter().any(|a| a.0.octets()[0] == 127 && a.0.octets()[1] != 255).then_some(*label)
                            }
                            Err(_) => None,
                        }
                    }
                }))
                .await;
                results.into_iter().flatten().collect::<Vec<_>>()
            }
            IpAddr::V6(_) => vec![],
        };
        json!({ "ip": ip.to_string(), "host": host, "ptr": ptr, "fcrdns": fcrdns, "listed_on": listed })
    }

    async fn spf_report(&self, domain: &str) -> Value {
        let records: Vec<String> =
            self.txt(domain).await.into_iter().filter(|t| t.to_ascii_lowercase().starts_with("v=spf1")).collect();
        let mut findings = vec![];
        if records.is_empty() {
            findings
                .push(finding("fail", "No SPF record: receivers cannot tell which servers may send for this domain"));
            return json!({ "status": "fail", "record": null, "lookups": 0, "findings": findings });
        }
        if records.len() > 1 {
            findings.push(finding("fail", "More than one SPF record: SPF evaluation returns a permanent error (RFC 7208 §4.5). Merge them into one"));
        }
        let record = records[0].clone();
        let lookups = self.count_spf_lookups(&record, 0).await;
        if lookups > 10 {
            findings.push(finding(
                "fail",
                format!("SPF needs {lookups} DNS lookups; the limit is 10 (RFC 7208 §4.6.4), so evaluation fails"),
            ));
        } else if lookups >= 8 {
            findings.push(finding(
                "warn",
                format!("SPF uses {lookups} of the 10 allowed DNS lookups; adding one more provider may break it"),
            ));
        }
        let lower = record.to_ascii_lowercase();
        let all = lower.split_whitespace().find(|t| t.ends_with("all")).unwrap_or("");
        match all {
            "-all" => findings.push(finding("pass", "Ends with -all: unlisted senders are rejected")),
            "~all" => findings.push(finding("pass", "Ends with ~all (soft fail): fine with an enforcing DMARC policy")),
            "?all" => findings.push(finding("warn", "Ends with ?all (neutral): offers no protection")),
            "+all" | "all" => findings.push(finding("fail", "+all lets anyone send as this domain. Use ~all or -all")),
            _ if !lower.contains("redirect=") => {
                findings.push(finding("warn", "No all mechanism: the default result is neutral"))
            }
            _ => {}
        }
        if lower.contains(" ptr") {
            findings.push(finding("warn", "The ptr mechanism is deprecated and slow (RFC 7208 §5.5)"));
        }
        json!({ "status": worst(&findings), "record": record, "lookups": lookups, "findings": findings })
    }

    /// Count DNS-querying terms (include, a, mx, ptr, exists, redirect), recursively.
    fn count_spf_lookups<'a>(
        &'a self,
        record: &'a str,
        depth: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + Send + 'a>> {
        Box::pin(async move {
            if depth > 10 {
                return 0;
            }
            let mut total = 0;
            for term in record.split_whitespace().skip(1) {
                let t = term.trim_start_matches(['+', '-', '~', '?']).to_ascii_lowercase();
                let target = if let Some(d) = t.strip_prefix("include:") {
                    Some(d.to_string())
                } else {
                    t.strip_prefix("redirect=").map(str::to_string)
                };
                if t.starts_with("include:")
                    || t.starts_with("redirect=")
                    || t == "a"
                    || t.starts_with("a:")
                    || t.starts_with("a/")
                    || t == "mx"
                    || t.starts_with("mx:")
                    || t.starts_with("mx/")
                    || t.starts_with("ptr")
                    || t.starts_with("exists:")
                {
                    total += 1;
                }
                if let Some(target) = target {
                    if let Some(sub) =
                        self.txt(&target).await.into_iter().find(|r| r.to_ascii_lowercase().starts_with("v=spf1"))
                    {
                        total += self.count_spf_lookups(&sub, depth + 1).await;
                    }
                }
            }
            total
        })
    }

    async fn dmarc_report(&self, domain: &str) -> Value {
        let records: Vec<String> =
            self.txt(&format!("_dmarc.{domain}")).await.into_iter().filter(|t| t.starts_with("v=DMARC1")).collect();
        let mut findings = vec![];
        let Some(record) = records.first().cloned() else {
            findings.push(finding(
                "fail",
                "No DMARC record: anyone can spoof this domain, and Gmail/Yahoo require DMARC for bulk senders",
            ));
            return json!({ "status": "fail", "record": null, "tags": {}, "findings": findings });
        };
        let tags: BTreeMap<String, String> = record
            .split(';')
            .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())))
            .collect();
        match tags.get("p").map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("reject") => findings.push(finding("pass", "Policy p=reject: spoofed mail is rejected")),
            Some("quarantine") => findings.push(finding("pass", "Policy p=quarantine: spoofed mail goes to spam")),
            Some("none") => findings
                .push(finding("warn", "Policy p=none only monitors. Move to quarantine once reports look clean")),
            _ => findings.push(finding("fail", "Missing or invalid p= tag")),
        }
        if !tags.contains_key("rua") {
            findings.push(finding("warn", "No rua= address: you will not receive aggregate reports"));
        }
        if let Some(pct) = tags.get("pct").and_then(|p| p.parse::<u8>().ok()) {
            if pct < 100 {
                findings.push(finding("info", format!("pct={pct}: the policy applies to only {pct}% of failing mail")));
            }
        }
        json!({ "status": worst(&findings), "record": record, "tags": tags, "findings": findings })
    }

    async fn dkim_report(&self, domain: &str) -> Value {
        let found = join_all(DKIM_SELECTORS.iter().map(|sel| async move {
            let txt = self.txt(&format!("{sel}._domainkey.{domain}")).await;
            txt.into_iter().find(|t| t.contains("p=")).map(|t| (sel.to_string(), t))
        }))
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let mut findings = vec![];
        if found.is_empty() {
            findings.push(finding("warn", "No DKIM key found under common selectors. Check your provider's selector (Mailvora shows yours on the domain page)"));
        }
        let keys: Vec<Value> = found
            .iter()
            .map(|(sel, rec)| {
                let p = rec.split(';').find_map(|kv| kv.trim().strip_prefix("p=")).unwrap_or("").trim();
                let bits = (p.len() * 6).saturating_sub(300); // base64 length of an RSA key ≈ modulus bits / 6 + ASN.1 overhead
                if p.is_empty() {
                    findings.push(finding("info", format!("Selector {sel} is revoked (empty p=)")));
                } else if (rec.contains("k=rsa") || !rec.contains("k=")) && bits < 1000 {
                    findings.push(finding("warn", format!("Selector {sel} uses a short RSA key; use 2048-bit keys")));
                }
                json!({ "selector": sel, "record": rec })
            })
            .collect();
        if findings.is_empty() {
            findings.push(finding(
                "pass",
                format!(
                    "DKIM key{} found: {}",
                    if keys.len() == 1 { "" } else { "s" },
                    found.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        json!({ "status": worst(&findings), "keys": keys, "findings": findings, "selectors_checked": DKIM_SELECTORS.len() })
    }

    async fn mta_sts_report(&self, domain: &str, http: &reqwest::Client) -> Value {
        let txt: Vec<String> =
            self.txt(&format!("_mta-sts.{domain}")).await.into_iter().filter(|t| t.starts_with("v=STSv1")).collect();
        if txt.is_empty() {
            return json!({ "status": "info", "record": null, "policy": null,
                "findings": [finding("info", "No MTA-STS: senders may deliver over unencrypted or unauthenticated TLS (RFC 8461)")] });
        }
        let mut findings = vec![];
        let url = format!("https://mta-sts.{domain}/.well-known/mta-sts.txt");
        let policy = match http.get(&url).timeout(Duration::from_secs(6)).send().await {
            Ok(r) if r.status().is_success() => {
                let text = r.text().await.unwrap_or_default();
                Some(text.chars().take(4096).collect::<String>())
            }
            Ok(r) => {
                findings.push(finding("fail", format!("Policy file returned HTTP {}", r.status().as_u16())));
                None
            }
            Err(e) => {
                findings.push(finding("fail", format!("Policy file could not be fetched over HTTPS: {e}")));
                None
            }
        };
        if let Some(p) = &policy {
            let mode = p.lines().find_map(|l| l.trim().strip_prefix("mode:")).map(|m| m.trim().to_ascii_lowercase());
            match mode.as_deref() {
                Some("enforce") => findings.push(finding("pass", "MTA-STS policy in enforce mode")),
                Some("testing") => {
                    findings.push(finding("warn", "MTA-STS in testing mode: failures are reported, not enforced"))
                }
                _ => findings.push(finding("warn", "MTA-STS policy has no valid mode")),
            }
        }
        json!({ "status": worst(&findings), "record": txt[0], "policy": policy, "findings": findings })
    }
}

impl Default for MailTools {
    fn default() -> Self {
        Self::new()
    }
}

fn worst(findings: &[Finding]) -> &'static str {
    ["fail", "warn", "info", "pass"].into_iter().find(|l| findings.iter().any(|f| f.level == *l)).unwrap_or("pass")
}

fn present(data: &RData) -> String {
    match data {
        RData::TXT(t) => t.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect::<String>(),
        RData::MX(m) => format!("{} {}", m.preference(), m.exchange().to_ascii().trim_end_matches('.')),
        other => other.to_string().trim_end_matches('.').to_string(),
    }
}

/// Validate a hostname for the lookup tools (no scheme, no path, public TLD).
pub fn clean_domain(raw: &str) -> Option<String> {
    let d = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    let d =
        d.strip_prefix("http://").or_else(|| d.strip_prefix("https://")).unwrap_or(&d).split('/').next()?.to_string();
    let d = d.rsplit_once('@').map(|(_, h)| h.to_string()).unwrap_or(d);
    let ok = d.len() <= 253
        && d.contains('.')
        && d.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        && !d.ends_with(".localhost")
        && !d.ends_with(".internal")
        && !d.ends_with(".local");
    ok.then_some(d)
}

/// Parse a Received/Date timestamp. Real servers emit sloppy dates (wrong
/// weekday, comments, missing seconds), so fall back to a weekday-free parse.
fn parse_header_date(raw: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let clean = raw.split(" (").next().unwrap_or(raw).trim();
    if let Ok(d) = chrono::DateTime::parse_from_rfc2822(clean) {
        return Some(d);
    }
    let no_weekday = clean.split_once(',').map(|(_, rest)| rest.trim()).unwrap_or(clean);
    ["%d %b %Y %H:%M:%S %z", "%d %b %Y %H:%M %z"]
        .iter()
        .find_map(|f| chrono::DateTime::parse_from_str(no_weekday, f).ok())
}

/// Analyze pasted message headers: delivery hops with delays and the
/// sender-authentication verdicts receivers recorded.
pub fn analyze_headers(raw: &str) -> Value {
    let mut headers: Vec<(String, String)> = vec![];
    for line in raw.replace("\r\n", "\n").lines() {
        if line.is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            if let Some(last) = headers.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
        } else if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let get = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    let all = |name: &str| {
        headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone()).collect::<Vec<_>>()
    };

    // Received headers are prepended, so reverse them for chronological order.
    let mut hops: Vec<Value> = vec![];
    let mut prev: Option<chrono::DateTime<chrono::FixedOffset>> = None;
    for (i, r) in all("Received").iter().rev().enumerate() {
        let (route, date) = r.rsplit_once(';').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((r.as_str(), ""));
        let when = parse_header_date(date);
        let delay = match (prev, when) {
            (Some(p), Some(w)) => Some((w - p).num_seconds().max(0)),
            _ => None,
        };
        if when.is_some() {
            prev = when;
        }
        let field = |key: &str| {
            route
                .split_whitespace()
                .skip_while(|w| !w.eq_ignore_ascii_case(key))
                .nth(1)
                .map(|s| s.trim_matches(['(', ')', ';']).to_string())
        };
        hops.push(json!({
            "hop": i + 1, "from": field("from"), "by": field("by"), "with": field("with"),
            "date": when.map(|d| d.to_rfc3339()), "delay_seconds": delay,
            "tls": route.contains("TLS") || route.contains("ESMTPS") || route.contains("ESMTPSA"),
        }));
    }
    let auth = all("Authentication-Results").join("; ");
    let verdict = |method: &str| -> Option<String> {
        let lower = auth.to_ascii_lowercase();
        let idx = lower.find(&format!("{method}="))?;
        Some(lower[idx + method.len() + 1..].split(|c: char| !c.is_ascii_alphanumeric()).next()?.to_string())
    };
    let total = hops.iter().filter_map(|h| h["delay_seconds"].as_i64()).sum::<i64>();
    let mut findings = vec![];
    for (m, label) in [("spf", "SPF"), ("dkim", "DKIM"), ("dmarc", "DMARC")] {
        match verdict(m).as_deref() {
            Some("pass") => findings.push(finding("pass", format!("{label} passed"))),
            Some(v) => findings.push(finding("fail", format!("{label} result: {v}"))),
            None => findings.push(finding("info", format!("No {label} result recorded"))),
        }
    }
    if hops.iter().any(|h| h["tls"] == false) && !hops.is_empty() {
        findings.push(finding("warn", "At least one hop did not record TLS"));
    }
    if let Some(slow) = hops.iter().filter_map(|h| h["delay_seconds"].as_i64()).max() {
        if slow > 300 {
            findings.push(finding(
                "warn",
                format!("A hop took {} minutes: likely greylisting or a queue backlog", slow / 60),
            ));
        }
    }
    json!({
        "summary": {
            "from": get("From"), "to": get("To"), "subject": get("Subject"), "date": get("Date"),
            "message_id": get("Message-ID"), "return_path": get("Return-Path"),
            "spam_status": get("X-Spam-Status").or(get("X-Spam-Flag")).or(get("X-Spam")),
            "list_unsubscribe": get("List-Unsubscribe"),
        },
        "authentication": { "spf": verdict("spf"), "dkim": verdict("dkim"), "dmarc": verdict("dmarc"), "raw": if auth.is_empty() { None } else { Some(auth) } },
        "hops": hops,
        "total_delay_seconds": total,
        "header_count": headers.len(),
        "findings": findings,
    })
}

/// Mail-related TCP ports the port checker may probe.
pub const MAIL_PORTS: &[(u16, &str)] = &[
    (25, "SMTP (server to server)"),
    (465, "SMTP submission, implicit TLS"),
    (587, "SMTP submission, STARTTLS"),
    (993, "IMAP, implicit TLS"),
    (143, "IMAP"),
    (995, "POP3, implicit TLS"),
    (110, "POP3"),
    (443, "HTTPS (webmail, autodiscover)"),
];

impl MailTools {
    /// DNSBL check for an IP address, or for every mail server of a domain.
    pub async fn blocklist_report(&self, target: &str) -> Value {
        let mut ips: Vec<(IpAddr, String)> = vec![];
        if let Ok(ip) = IpAddr::from_str(target) {
            ips.push((ip, target.to_string()));
        } else {
            if let Ok(mx) = self.resolver.mx_lookup(target).await {
                for m in mx.iter().take(4) {
                    let host = m.exchange().to_ascii().trim_end_matches('.').to_lowercase();
                    for ip in self.ips(&host).await.into_iter().take(2) {
                        ips.push((ip, host.clone()));
                    }
                }
            }
            for ip in self.ips(target).await.into_iter().take(2) {
                if !ips.iter().any(|(i, _)| *i == ip) {
                    ips.push((ip, target.to_string()));
                }
            }
        }
        let results = join_all(ips.iter().map(|(ip, host)| self.ip_report(*ip, host))).await;
        let listed = results.iter().filter(|r| r["listed_on"].as_array().is_some_and(|a| !a.is_empty())).count();
        json!({
            "target": target,
            "addresses": results,
            "lists_checked": BLOCKLISTS.iter().map(|(zone, label)| json!({ "zone": zone, "name": label })).collect::<Vec<_>>(),
            "listed_count": listed,
            "status": if ips.is_empty() || listed > 0 { "fail" } else { "pass" },
            "checked_at": chrono::Utc::now(),
        })
    }

    /// TCP reachability of the standard mail ports on a public host.
    /// Refuses private and reserved addresses (SSRF protection).
    pub async fn port_report(&self, host: &str) -> Result<Value, String> {
        let ip = match IpAddr::from_str(host) {
            Ok(ip) => ip,
            Err(_) => *self.ips(host).await.first().ok_or_else(|| format!("{host} does not resolve"))?,
        };
        if crate::services::webhooks::is_forbidden_ip(ip) {
            return Err("Only public internet addresses can be checked".into());
        }
        let probes = join_all(MAIL_PORTS.iter().map(|(port, label)| async move {
            let started = std::time::Instant::now();
            let open = matches!(
                tokio::time::timeout(Duration::from_secs(4), tokio::net::TcpStream::connect((ip, *port))).await,
                Ok(Ok(_))
            );
            json!({ "port": port, "service": label, "open": open, "ms": started.elapsed().as_millis() as u64 })
        }))
        .await;
        Ok(json!({ "host": host, "ip": ip.to_string(), "ports": probes, "checked_at": chrono::Utc::now() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_domains() {
        assert_eq!(clean_domain(" https://Example.COM/path ").as_deref(), Some("example.com"));
        assert_eq!(clean_domain("jane@school.ac.ke").as_deref(), Some("school.ac.ke"));
        assert!(clean_domain("localhost").is_none());
        assert!(clean_domain("foo.localhost").is_none());
        assert!(clean_domain("-bad.com").is_none());
    }

    #[test]
    fn analyzes_headers() {
        let raw = "Received: from mx.b.example by mx.c.example with ESMTPS; Mon, 13 Oct 2026 10:00:30 +0000\r\n\
                   Received: from app.a.example by mx.b.example with ESMTPSA; Mon, 13 Oct 2026 10:00:00 +0000\r\n\
                   Authentication-Results: mx.c.example; spf=pass smtp.mailfrom=a.example; dkim=pass header.d=a.example;\r\n dmarc=pass\r\n\
                   From: A <a@a.example>\r\nSubject: Hi\r\n\r\nbody";
        let r = analyze_headers(raw);
        assert_eq!(r["hops"].as_array().unwrap().len(), 2);
        assert_eq!(r["hops"][1]["delay_seconds"], 30);
        assert_eq!(r["authentication"]["dmarc"], "pass");
        assert_eq!(r["summary"]["subject"], "Hi");
        assert_eq!(r["total_delay_seconds"], 30);
    }
}
