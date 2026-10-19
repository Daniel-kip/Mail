//! RFC 6238 TOTP (SHA-1, 6 digits, 30 s), compatible with standard
//! authenticator apps. Accepts ±1 step of clock skew and rejects any step at
//! or before the last accepted one, so a code cannot be replayed.

use totp_rs::{Algorithm, Secret, TOTP};

pub struct Enrollment {
    pub secret_bytes: Vec<u8>,
    pub secret_base32: String,
    pub otpauth_url: String,
}

fn build(secret: Vec<u8>, issuer: &str, account: &str) -> anyhow::Result<TOTP> {
    TOTP::new(Algorithm::SHA1, 6, 1, 30, secret, Some(issuer.replace(':', "")), account.replace(':', ""))
        .map_err(|e| anyhow::anyhow!("TOTP setup failed: {e}"))
}

pub fn enroll(issuer: &str, account: &str) -> anyhow::Result<Enrollment> {
    let secret_bytes = crate::crypto::random_bytes::<20>().to_vec();
    let totp = build(secret_bytes.clone(), issuer, account)?;
    let secret_base32 = Secret::Raw(secret_bytes.clone()).to_encoded().to_string();
    Ok(Enrollment { secret_bytes, secret_base32, otpauth_url: totp.get_url() })
}

/// Returns the matched time step if `code` is valid and newer than `last_step`.
pub fn verify(secret: &[u8], code: &str, last_step: Option<i64>) -> Option<i64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let totp = build(secret.to_vec(), "x", "x").ok()?;
    let now = chrono::Utc::now().timestamp();
    let current = now / 30;
    for step in [current - 1, current, current + 1] {
        if last_step.is_some_and(|last| step <= last) {
            continue;
        }
        let expected = totp.generate((step * 30) as u64);
        if crate::crypto::constant_time_eq(expected.as_bytes(), code.as_bytes()) {
            return Some(step);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_and_replay() {
        let e = enroll("Mailvora", "a@b.c").unwrap();
        assert!(e.otpauth_url.starts_with("otpauth://totp/"));
        let totp = build(e.secret_bytes.clone(), "x", "x").unwrap();
        let code = totp.generate_current().unwrap();
        let step = verify(&e.secret_bytes, &code, None).expect("valid code");
        assert!(verify(&e.secret_bytes, &code, Some(step)).is_none(), "replay must fail");
        assert!(verify(&e.secret_bytes, "000000x", None).is_none());
    }
}
