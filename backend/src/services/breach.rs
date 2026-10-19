//! Breached-password screening (NIST SP 800-63B §5.1.1.2) through the
//! Pwned Passwords k-anonymity range API: only the first five hex characters
//! of the password's SHA-1 leave the server, and response padding hides
//! which suffix was of interest. Network failures fail open so an outage of
//! the service never blocks sign-up or password changes.

use std::time::Duration;

use sha1::{Digest, Sha1};

use crate::{
    config::Config,
    error::{ApiResult, AppError},
};

const RANGE_API: &str = "https://api.pwnedpasswords.com/range/";

/// How many times the password appears in known breaches, or `None` when
/// the check could not run.
pub async fn breach_count(password: &str) -> Option<u64> {
    let digest = hex::encode_upper(Sha1::digest(password.as_bytes()));
    let (prefix, suffix) = digest.split_at(5);
    let body = reqwest::Client::new()
        .get(format!("{RANGE_API}{prefix}"))
        .timeout(Duration::from_secs(3))
        .header("Add-Padding", "true")
        .header("User-Agent", concat!("Mailvora/", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    Some(count_in_range(&body, suffix))
}

fn count_in_range(body: &str, suffix: &str) -> u64 {
    body.lines()
        .filter_map(|l| l.trim().split_once(':'))
        .find(|(s, _)| s.eq_ignore_ascii_case(suffix))
        .and_then(|(_, n)| n.trim().parse().ok())
        .unwrap_or(0)
}

/// Reject a password that appears in breach corpora (when enabled).
pub async fn ensure_not_breached(cfg: &Config, password: &str) -> ApiResult<()> {
    if !cfg.password_breach_check {
        return Ok(());
    }
    match breach_count(password).await {
        Some(n) if n > 0 => Err(AppError::validation(format!(
            "This password has appeared in {} known data breach{}. Choose a different one; a short sentence works well.",
            if n > 1000 { "over a thousand".to_string() } else { n.to_string() },
            if n == 1 { "" } else { "es" }
        ))),
        Some(_) => Ok(()),
        None => {
            tracing::warn!("breached-password check unavailable; allowing the password");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_padded_range_responses() {
        let body = "0018A45C4D1DEF81644B54AB7F969B88D65:1\r\n00D4F6E8FA6EECAD2A3AA415EEC418D38EC:0\r\n1E4C9B93F3F0682250B6CF8331B7EE68FD8:3861493\r\n";
        assert_eq!(count_in_range(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 3_861_493);
        assert_eq!(count_in_range(body, "00D4F6E8FA6EECAD2A3AA415EEC418D38EC"), 0);
        assert_eq!(count_in_range(body, "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), 0);
    }
}
