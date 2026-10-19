//! Return-path (VERP) addresses and bounce/complaint parsing.
//!
//! Every message is sent with a unique envelope sender
//! `b+<message-id>-<tag>@<return-path-domain>`, where `tag` is an HMAC of the
//! id. Postfix pipes mail for that domain to `/internal/v1/bounces` together
//! with the envelope recipient. The HMAC means a forged DSN cannot mark
//! arbitrary messages as bounced, and correlation never trusts headers
//! inside the (attacker-controllable) DSN body (guide 03).

use mail_parser::{MessageParser, MimeHeaders};
use uuid::Uuid;

use crate::crypto;

const TAG_BYTES: usize = 5;

fn tag(key: &[u8], id: Uuid) -> String {
    hex::encode(&crypto::hmac_sha256(key, id.as_bytes())[..TAG_BYTES])
}

/// Key for VERP tags, derived from the platform data key.
pub fn verp_key(data_key: &[u8; 32]) -> Vec<u8> {
    crypto::hmac_sha256(data_key, b"mailvora/verp/v1")
}

pub fn verp_address(key: &[u8], message_id: Uuid, return_path_domain: &str) -> String {
    format!("b+{}-{}@{}", message_id.simple(), tag(key, message_id), return_path_domain)
}

/// Recover and authenticate the message id from a VERP recipient.
pub fn parse_verp(key: &[u8], address: &str) -> Option<Uuid> {
    let local = address.trim().trim_matches(['<', '>']).split('@').next()?;
    let rest = local.strip_prefix("b+")?;
    let (id_hex, given_tag) = rest.split_once('-')?;
    let id = Uuid::parse_str(id_hex).ok()?;
    let expected = tag(key, id);
    crypto::constant_time_eq(expected.as_bytes(), given_tag.to_ascii_lowercase().as_bytes()).then_some(id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackKind {
    /// 5.x.x: permanent failure; the address should be suppressed.
    HardBounce,
    /// 4.x.x or "delayed": Postfix is still retrying.
    Delayed,
    /// Abuse report (ARF, RFC 5965).
    Complaint,
    /// Positive DSN ("delivered"/"relayed"/"expanded").
    Delivered,
}

#[derive(Debug, Clone)]
pub struct Feedback {
    pub kind: FeedbackKind,
    pub recipient: Option<String>,
    pub status: Option<String>,
    pub diagnostic: Option<String>,
    pub remote_mta: Option<String>,
}

/// Parse a DSN (RFC 3464) or ARF report. Returns one entry per recipient
/// block; unknown or malformed input yields an empty list.
pub fn parse_feedback(raw: &[u8]) -> Vec<Feedback> {
    let Some(message) = MessageParser::default().parse(raw) else {
        return vec![];
    };
    let mut out = Vec::new();
    for part in message.parts.iter() {
        let Some(ct) = part.content_type() else { continue };
        let ctype = format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or_default()).to_ascii_lowercase();
        let body = String::from_utf8_lossy(part.contents()).to_string();
        match ctype.as_str() {
            "message/delivery-status" | "message/global-delivery-status" => out.extend(parse_dsn_fields(&body)),
            "message/feedback-report" => {
                let fields = fields_of(&body);
                let feedback_type = field(&fields, "feedback-type").unwrap_or_default().to_ascii_lowercase();
                if feedback_type == "abuse" || feedback_type.is_empty() {
                    out.push(Feedback {
                        kind: FeedbackKind::Complaint,
                        recipient: field(&fields, "original-rcpt-to").map(strip_addr_type),
                        status: None,
                        diagnostic: Some(format!(
                            "feedback-type: {}",
                            if feedback_type.is_empty() { "abuse" } else { &feedback_type }
                        )),
                        remote_mta: field(&fields, "reporting-mta").map(strip_addr_type),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn fields_of(block: &str) -> Vec<(String, String)> {
    // Header-style fields with folded continuation lines.
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in block.lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !fields.is_empty() {
            let last = fields.last_mut().unwrap();
            last.1.push(' ');
            last.1.push_str(line.trim());
        } else if let Some((k, v)) = line.split_once(':') {
            fields.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    fields
}

fn field(fields: &[(String, String)], name: &str) -> Option<String> {
    fields.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// "rfc822; user@example.com" → "user@example.com"
fn strip_addr_type(v: String) -> String {
    v.split_once(';').map(|(_, a)| a.trim().to_string()).unwrap_or(v)
}

fn parse_dsn_fields(body: &str) -> Vec<Feedback> {
    let normalized = body.replace("\r\n", "\n");
    let mut blocks = normalized.split("\n\n").map(str::trim).filter(|b| !b.is_empty());
    let per_message = blocks.next().map(fields_of).unwrap_or_default();
    let remote_default = field(&per_message, "reporting-mta").map(strip_addr_type);
    blocks
        .filter_map(|b| {
            let f = fields_of(b);
            let action = field(&f, "action")?.to_ascii_lowercase();
            let status = field(&f, "status");
            let kind = match action.as_str() {
                "failed" => {
                    // A "failed" action with a 4.x.x status is a give-up after
                    // retries; still permanent from our point of view.
                    FeedbackKind::HardBounce
                }
                "delayed" => FeedbackKind::Delayed,
                "delivered" | "relayed" | "expanded" => FeedbackKind::Delivered,
                _ => return None,
            };
            Some(Feedback {
                kind,
                recipient: field(&f, "final-recipient")
                    .or_else(|| field(&f, "original-recipient"))
                    .map(strip_addr_type),
                status,
                diagnostic: field(&f, "diagnostic-code").map(strip_addr_type),
                remote_mta: field(&f, "remote-mta").map(strip_addr_type).or_else(|| remote_default.clone()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verp_roundtrip_and_forgery() {
        let key = verp_key(&[3u8; 32]);
        let id = Uuid::now_v7();
        let addr = verp_address(&key, id, "bounces.example.net");
        assert!(addr.split('@').next().unwrap().len() <= 64);
        assert_eq!(parse_verp(&key, &addr), Some(id));
        assert_eq!(parse_verp(&key, &format!("<{addr}>")), Some(id));
        let forged = format!("b+{}-0000000000@bounces.example.net", id.simple());
        assert_eq!(parse_verp(&key, &forged), None);
        assert_eq!(parse_verp(&verp_key(&[4u8; 32]), &addr), None);
    }

    const DSN: &str = "From: MAILER-DAEMON@mx.example.net\r\nTo: b+x@bounces.example.net\r\nSubject: Undelivered Mail Returned to Sender\r\nMIME-Version: 1.0\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=\"B\"\r\n\r\n--B\r\nContent-Type: text/plain\r\n\r\nThis is the mail system.\r\n\r\n--B\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; mx.example.net\r\nArrival-Date: Sat, 11 Oct 2026 10:00:00 +0000\r\n\r\nFinal-Recipient: rfc822; nobody@example.org\r\nOriginal-Recipient: rfc822;nobody@example.org\r\nAction: failed\r\nStatus: 5.1.1\r\nRemote-MTA: dns; mx.example.org\r\nDiagnostic-Code: smtp; 550 5.1.1 <nobody@example.org>: Recipient address\r\n rejected: User unknown\r\n\r\n--B--\r\n";

    #[test]
    fn parses_hard_bounce() {
        let f = parse_feedback(DSN.as_bytes());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, FeedbackKind::HardBounce);
        assert_eq!(f[0].recipient.as_deref(), Some("nobody@example.org"));
        assert_eq!(f[0].status.as_deref(), Some("5.1.1"));
        assert!(f[0].diagnostic.as_deref().unwrap().contains("User unknown"));
        assert_eq!(f[0].remote_mta.as_deref(), Some("mx.example.org"));
    }

    #[test]
    fn garbage_is_ignored() {
        assert!(parse_feedback(b"not an email").is_empty());
    }
}
