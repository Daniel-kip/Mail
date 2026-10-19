//! One-click unsubscribe (RFC 8058 / RFC 2369).
//!
//! Gmail and Yahoo require bulk and marketing mail to carry
//! `List-Unsubscribe` with an HTTPS URL plus
//! `List-Unsubscribe-Post: List-Unsubscribe=One-Click`, both covered by the
//! DKIM signature. The URL embeds a stateless token:
//! `base64url(message_id[16] || HMAC-SHA256(key, message_id)[..16])`, so it
//! cannot be forged or enumerated and needs no storage.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use uuid::Uuid;

use crate::{config::Config, crypto};

fn key(cfg: &Config) -> Vec<u8> {
    crypto::hmac_sha256(&cfg.data_encryption_key, b"list-unsubscribe-v1")
}

pub fn token(cfg: &Config, message_id: Uuid) -> String {
    let mac = crypto::hmac_sha256(&key(cfg), message_id.as_bytes());
    let mut raw = message_id.as_bytes().to_vec();
    raw.extend_from_slice(&mac[..16]);
    B64URL.encode(raw)
}

pub fn verify(cfg: &Config, token: &str) -> Option<Uuid> {
    let raw = B64URL.decode(token.trim()).ok()?;
    if raw.len() != 32 {
        return None;
    }
    let id = Uuid::from_slice(&raw[..16]).ok()?;
    let mac = crypto::hmac_sha256(&key(cfg), id.as_bytes());
    crypto::constant_time_eq(&mac[..16], &raw[16..]).then_some(id)
}

/// The URL placed in `List-Unsubscribe` (served by the web frontend, which
/// forwards one-click POSTs to `POST /v1/unsubscribe/{token}`).
pub fn url(cfg: &Config, message_id: Uuid) -> String {
    format!("{}/u/{}", cfg.public_base_url, token(cfg, message_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_roundtrip_and_reject_tampering() {
        let cfg = Config::for_tests("postgres://localhost/x");
        let id = Uuid::now_v7();
        let t = token(&cfg, id);
        assert_eq!(verify(&cfg, &t), Some(id));
        let mut bytes = B64URL.decode(&t).unwrap();
        bytes[3] ^= 1;
        assert_eq!(verify(&cfg, &B64URL.encode(&bytes)), None);
        assert_eq!(verify(&cfg, "not-a-token"), None);
        assert!(url(&cfg, id).starts_with("http://localhost:8088/u/"));
    }
}
