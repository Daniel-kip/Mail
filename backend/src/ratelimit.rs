//! In-process fixed-window rate limiting.
//!
//! Used for per-API-key request rates and per-IP limits on authentication
//! endpoints (credential stuffing, reset abuse). State is per process: in a
//! multi-instance deployment put a shared limiter (e.g. Redis) behind this
//! same interface — see frontend/resources/wiki/Decision-Log.md (D-010).

use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;

use crate::error::AppError;

#[derive(Default)]
pub struct RateLimiter {
    /// key → (window length secs, window index, hits in window)
    windows: DashMap<String, (u64, u64, u32)>,
}

impl RateLimiter {
    /// Count one hit against `key`; fail when more than `limit` hits occur in
    /// the current `window_secs` window.
    pub fn check(&self, key: &str, limit: u32, window_secs: u64) -> Result<(), AppError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let window = now / window_secs;
        let mut entry = self.windows.entry(key.to_string()).or_insert((window_secs, window, 0));
        if entry.1 != window || entry.0 != window_secs {
            *entry = (window_secs, window, 0);
        }
        entry.2 += 1;
        if entry.2 > limit {
            let retry_after = window_secs - (now % window_secs);
            return Err(AppError::rate_limited(format!("Too many requests. Retry in {retry_after}s."))
                .with_details(serde_json::json!({ "retry_after_seconds": retry_after, "limit": limit })));
        }
        Ok(())
    }

    /// Drop windows that ended long ago. Called periodically by maintenance.
    pub fn prune(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.windows.retain(|_, (secs, window, _)| (*window + 1) * *secs > now);
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_limit() {
        let rl = RateLimiter::default();
        for _ in 0..3 {
            rl.check("k", 3, 60).unwrap();
        }
        assert!(rl.check("k", 3, 60).is_err());
        assert!(rl.check("other", 3, 60).is_ok());
    }
}
