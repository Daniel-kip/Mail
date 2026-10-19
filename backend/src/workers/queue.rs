//! Outbox job claiming, completion and retry scheduling.
//!
//! * Claims use `FOR UPDATE SKIP LOCKED`, so any number of worker processes
//!   can run concurrently without double-processing.
//! * A claimed job holds a lease (`locked_until`). If the worker crashes, the
//!   lease expires and another worker reclaims the job (crash recovery).
//! * Candidates are ordered by their rank *within their tenant* first, so a
//!   tenant with a large backlog cannot starve the others (guide 08).

use std::time::Duration;

use rand::Rng;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub const LEASE_SECS: i64 = 300;

#[derive(Debug, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub organization_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub webhook_delivery_id: Option<Uuid>,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
}

impl Job {
    pub fn is_last_attempt(&self) -> bool {
        self.attempts >= self.max_attempts
    }
}

pub async fn claim(db: &PgPool, kinds: &[&str], worker_id: &str, limit: i64) -> sqlx::Result<Vec<Job>> {
    let kinds: Vec<String> = kinds.iter().map(|k| k.to_string()).collect();
    sqlx::query_as(
        "WITH ready AS (
             SELECT id, run_at, row_number() OVER (PARTITION BY organization_id ORDER BY run_at, id) AS rn
             FROM outbox_jobs
             WHERE kind = ANY($1)
               AND ((status = 'pending' AND run_at <= now()) OR (status = 'processing' AND locked_until < now()))
             ORDER BY run_at
             LIMIT 1000
         ), picked AS (
             SELECT j.id FROM outbox_jobs j JOIN ready r ON r.id = j.id
             WHERE (j.status = 'pending' AND j.run_at <= now()) OR (j.status = 'processing' AND j.locked_until < now())
             ORDER BY r.rn, r.run_at
             LIMIT $2
             FOR UPDATE OF j SKIP LOCKED
         )
         UPDATE outbox_jobs j
         SET status = 'processing', locked_by = $3, locked_until = now() + make_interval(secs => $4),
             attempts = j.attempts + 1, updated_at = now()
         FROM picked WHERE j.id = picked.id
         RETURNING j.id, j.kind, j.organization_id, j.message_id, j.webhook_delivery_id, j.payload, j.attempts, j.max_attempts",
    )
    .bind(&kinds)
    .bind(limit)
    .bind(worker_id)
    .bind(LEASE_SECS as f64)
    .fetch_all(db)
    .await
}

pub async fn complete(db: &PgPool, job: &Job) -> sqlx::Result<()> {
    sqlx::query("UPDATE outbox_jobs SET status = 'done', locked_by = NULL, locked_until = NULL, last_error = NULL, updated_at = now() WHERE id = $1")
        .bind(job.id)
        .execute(db)
        .await?;
    Ok(())
}

/// Reschedule, or mark dead when attempts are exhausted. Returns true if dead.
pub async fn retry_or_dead(db: &PgPool, job: &Job, delay: Duration, error: &str) -> sqlx::Result<bool> {
    let dead = job.is_last_attempt();
    sqlx::query(
        "UPDATE outbox_jobs SET status = $2, run_at = now() + make_interval(secs => $3), locked_by = NULL,
                locked_until = NULL, last_error = $4, updated_at = now() WHERE id = $1",
    )
    .bind(job.id)
    .bind(if dead { "dead" } else { "pending" })
    .bind(delay.as_secs_f64())
    .bind(truncate(error, 1000))
    .execute(db)
    .await?;
    Ok(dead)
}

pub async fn dead(db: &PgPool, job: &Job, error: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE outbox_jobs SET status = 'dead', locked_by = NULL, locked_until = NULL, last_error = $2, updated_at = now() WHERE id = $1")
        .bind(job.id)
        .bind(truncate(error, 1000))
        .execute(db)
        .await?;
    Ok(())
}

/// Exponential backoff with ±20% jitter: base·2^(attempt-1), capped.
pub fn backoff(attempt: i32, base_secs: u64, cap_secs: u64) -> Duration {
    let exp = base_secs.saturating_mul(1u64 << (attempt.clamp(1, 20) - 1) as u32).min(cap_secs);
    let jitter = rand::thread_rng().gen_range(0.8..1.2);
    Duration::from_secs_f64((exp as f64 * jitter).max(1.0))
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        let a1 = backoff(1, 30, 3600).as_secs_f64();
        let a4 = backoff(4, 30, 3600).as_secs_f64();
        let a20 = backoff(20, 30, 3600).as_secs_f64();
        assert!((24.0..=36.0).contains(&a1));
        assert!((192.0..=288.0).contains(&a4));
        assert!(a20 <= 3600.0 * 1.2);
    }

    #[test]
    fn truncation_is_utf8_safe() {
        assert_eq!(truncate("héllo", 2), "h…");
        assert_eq!(truncate("abc", 10), "abc");
    }
}
