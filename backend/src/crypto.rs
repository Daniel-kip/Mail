//! Cryptographic primitives used across the platform.
//!
//! * Random tokens come from the OS CSPRNG.
//! * Secrets that must be recoverable (DKIM private keys, webhook signing
//!   secrets, TOTP seeds) are sealed with AES-256-GCM under
//!   `DATA_ENCRYPTION_KEY`, with a purpose string as associated data so a
//!   ciphertext cannot be swapped between columns.
//! * Secrets that only need verification (session tokens, API keys,
//!   invitation tokens) are stored as SHA-256 digests.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use hmac::{Hmac, Mac};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng, RngCore};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// URL-safe random token with `bytes` bytes of entropy.
pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    B64URL.encode(buf)
}

/// Random `[A-Za-z0-9]` string (≈5.95 bits per character).
pub fn random_alphanumeric(len: usize) -> String {
    OsRng.sample_iter(&Alphanumeric).take(len).map(char::from).collect()
}

/// Lowercase alphanumeric, for identifiers that may land in DNS or emails.
pub fn random_lower_alnum(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    (0..len).map(|_| CHARS[OsRng.gen_range(0..CHARS.len())] as char).collect()
}

pub fn sha256(data: impl AsRef<[u8]>) -> Vec<u8> {
    Sha256::digest(data.as_ref()).to_vec()
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
}

const NONCE_LEN: usize = 12;
const VERSION: u8 = 1;

impl SecretBox {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(key)) }
    }

    /// Output layout: `version(1) || nonce(12) || ciphertext+tag`.
    pub fn seal(&self, purpose: &str, plaintext: &[u8]) -> Vec<u8> {
        let nonce_bytes = random_bytes::<NONCE_LEN>();
        let ct = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: plaintext, aad: purpose.as_bytes() })
            .expect("AES-GCM encryption cannot fail for in-memory buffers");
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ct);
        out
    }

    pub fn open(&self, purpose: &str, sealed: &[u8]) -> anyhow::Result<Vec<u8>> {
        if sealed.len() < 1 + NONCE_LEN + 16 || sealed[0] != VERSION {
            anyhow::bail!("sealed value is malformed");
        }
        let (nonce, ct) = sealed[1..].split_at(NONCE_LEN);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: purpose.as_bytes() })
            .map_err(|_| anyhow::anyhow!("sealed value failed authentication (wrong key or purpose)"))
    }

    pub fn open_string(&self, purpose: &str, sealed: &[u8]) -> anyhow::Result<String> {
        Ok(String::from_utf8(self.open(purpose, sealed)?)?)
    }
}

pub mod purpose {
    pub const DKIM_KEY: &str = "dkim-private-key";
    pub const WEBHOOK_SECRET: &str = "webhook-secret";
    pub const TOTP_SECRET: &str = "totp-secret";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip_and_purpose_binding() {
        let sb = SecretBox::new(&[7u8; 32]);
        let sealed = sb.seal("a", b"hello");
        assert_eq!(sb.open("a", &sealed).unwrap(), b"hello");
        assert!(sb.open("b", &sealed).is_err(), "purpose must be bound as AAD");
        let other = SecretBox::new(&[8u8; 32]);
        assert!(other.open("a", &sealed).is_err());
    }

    #[test]
    fn tokens_are_unique_and_sized() {
        let a = random_token(32);
        let b = random_token(32);
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(random_alphanumeric(40).len(), 40);
    }
}
