//! Password hashing (Argon2id) and password policy.
//!
//! Parameters follow the OWASP minimum for Argon2id (m=19 MiB, t=2, p=1).
//! p=1 is also required for compatibility with Dovecot, which verifies
//! mailbox passwords through libsodium (`{ARGON2ID}` scheme).
//!
//! Hashing is CPU- and memory-bound, so it always runs on the blocking pool.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use rand::rngs::OsRng;

use crate::error::{ApiResult, AppError};

pub const DOVECOT_PREFIX: &str = "{ARGON2ID}";
pub const MIN_LEN: usize = 12;
pub const MAX_LEN: usize = 256;

fn hasher() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, Some(32)).expect("static Argon2 params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

fn hash_sync(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(hasher()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?
        .to_string())
}

fn verify_sync(password: &str, stored: &str) -> bool {
    let phc = stored.strip_prefix(DOVECOT_PREFIX).unwrap_or(stored);
    match PasswordHash::new(phc) {
        Ok(parsed) => hasher().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// PHC string for console users.
pub async fn hash(password: &str) -> ApiResult<String> {
    let pw = password.to_owned();
    tokio::task::spawn_blocking(move || hash_sync(&pw)).await.map_err(AppError::internal)?.map_err(AppError::Internal)
}

/// Dovecot-compatible hash for mailbox credentials.
pub async fn hash_for_mailbox(password: &str) -> ApiResult<String> {
    Ok(format!("{DOVECOT_PREFIX}{}", hash(password).await?))
}

pub async fn verify(password: &str, stored: &str) -> bool {
    let pw = password.to_owned();
    let stored = stored.to_owned();
    tokio::task::spawn_blocking(move || verify_sync(&pw, &stored)).await.unwrap_or(false)
}

/// A fixed hash used to equalize timing when the account does not exist,
/// so response time does not reveal which emails are registered.
pub async fn verify_dummy(password: &str) {
    static DUMMY: once_cell::sync::Lazy<String> =
        once_cell::sync::Lazy::new(|| hash_sync("dummy-password-for-timing").expect("hash"));
    let _ = verify(password, &DUMMY).await;
}

/// Length-based policy per NIST SP 800-63B: no composition rules, but a
/// minimum length, a maximum (to bound hashing cost), and a few obvious
/// rejections.
pub fn check_policy(password: &str, identity: &str) -> ApiResult<()> {
    let len = password.chars().count();
    if len < MIN_LEN {
        return Err(AppError::validation(format!("Password must be at least {MIN_LEN} characters")));
    }
    if len > MAX_LEN {
        return Err(AppError::validation(format!("Password must be at most {MAX_LEN} characters")));
    }
    let lower = password.to_lowercase();
    let first = lower.chars().next().unwrap_or_default();
    if lower.chars().all(|c| c == first) {
        return Err(AppError::validation("Password must not be a single repeated character"));
    }
    let local = identity.split('@').next().unwrap_or("").to_lowercase();
    if local.len() >= 4 && lower.contains(&local) {
        return Err(AppError::validation("Password must not contain your username or email name"));
    }
    const COMMON: &[&str] = &[
        "password1234",
        "123456789012",
        "qwertyuiop12",
        "passwordpassword",
        "iloveyou1234",
        "administrator",
        "welcome12345",
        "changeme1234",
        "letmein12345",
        "1q2w3e4r5t6y",
    ];
    if COMMON.iter().any(|c| lower == *c) {
        return Err(AppError::validation("Password is too common"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_including_dovecot_prefix() {
        let h = hash_sync("correct horse battery staple").unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify_sync("correct horse battery staple", &h));
        assert!(!verify_sync("wrong", &h));
        let dovecot = format!("{DOVECOT_PREFIX}{h}");
        assert!(verify_sync("correct horse battery staple", &dovecot));
        assert!(!verify_sync("x", "not a hash"));
    }

    #[test]
    fn policy() {
        assert!(check_policy("short", "a@b.c").is_err());
        assert!(check_policy("aaaaaaaaaaaaaa", "a@b.c").is_err());
        assert!(check_policy("my-jonathan-pass", "jonathan@x.io").is_err());
        assert!(check_policy("a long and decent passphrase", "jo@x.io").is_ok());
    }
}
