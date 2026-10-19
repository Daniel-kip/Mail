//! API key format, generation and parsing.
//!
//! `mv_<live|test>_<id:12 lower-alnum>_<secret:40 alnum>`
//!
//! The `mv_<env>_<id>` prefix is stored and indexed for lookup and is safe
//! to display. The full key is only ever returned once, at creation.

use crate::crypto;

pub struct GeneratedKey {
    pub full: String,
    pub prefix: String,
    pub secret_hash: Vec<u8>,
}

pub fn generate(environment: &str) -> GeneratedKey {
    let prefix = format!("mv_{environment}_{}", crypto::random_lower_alnum(12));
    let full = format!("{prefix}_{}", crypto::random_alphanumeric(40));
    let secret_hash = crypto::sha256(&full);
    GeneratedKey { full, prefix, secret_hash }
}

/// Returns the lookup prefix if `raw` is shaped like one of our keys.
pub fn prefix_of(raw: &str) -> Option<&str> {
    let mut parts = raw.splitn(4, '_');
    let (brand, env, id, secret) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let shape_ok = brand == "mv"
        && (env == "live" || env == "test")
        && id.len() == 12
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && secret.len() == 40
        && secret.bytes().all(|b| b.is_ascii_alphanumeric());
    shape_ok.then(|| &raw[..3 + env.len() + 1 + 12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_parse() {
        let k = generate("live");
        assert!(k.full.starts_with("mv_live_"));
        assert_eq!(prefix_of(&k.full), Some(k.prefix.as_str()));
        assert_eq!(k.secret_hash, crypto::sha256(&k.full));
        assert_eq!(prefix_of("mv_live_short_x"), None);
        assert_eq!(prefix_of("Bearer something"), None);
        let t = generate("test");
        assert!(prefix_of(&t.full).unwrap().starts_with("mv_test_"));
    }
}
