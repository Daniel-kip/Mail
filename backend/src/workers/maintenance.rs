//! Periodic housekeeping: expired credentials, idempotency records, content
//! retention, finished jobs and in-memory limiter state.

use std::time::Duration;

use tokio::sync::watch;

use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(300);

pub async fn run(state: AppState, mut shutdown: watch::Receiver<bool>) {
    // Small initial delay so startup is not slowed by cleanup queries.
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(20)) => {}
        _ = shutdown.changed() => return,
    }
    loop {
        if let Err(e) = tick(&state).await {
            tracing::error!(error = %e, "maintenance pass failed");
        }
        tokio::select! {
            _ = tokio::time::sleep(INTERVAL) => {}
            _ = shutdown.changed() => return,
        }
    }
}

pub async fn tick(state: &AppState) -> anyhow::Result<()> {
    let db = &state.db;
    let sessions = sqlx::query(
        "DELETE FROM sessions WHERE expires_at < now() - interval '7 days' OR revoked_at < now() - interval '7 days'",
    )
    .execute(db)
    .await?
    .rows_affected();
    sqlx::query("DELETE FROM user_tokens WHERE expires_at < now() - interval '7 days'").execute(db).await?;
    sqlx::query("DELETE FROM invitations WHERE expires_at < now() - interval '30 days' AND accepted_at IS NULL")
        .execute(db)
        .await?;
    let idem = sqlx::query("DELETE FROM idempotency_keys WHERE created_at < now() - interval '24 hours'")
        .execute(db)
        .await?
        .rows_affected();

    // Content retention: drop bodies of messages in a final state after the
    // configured window; metadata and events remain for reporting.
    let purged = sqlx::query(
        "UPDATE message_contents c SET raw_mime = NULL, html_body = NULL, text_body = NULL, purged_at = now()
         FROM (SELECT m.id FROM messages m JOIN message_contents mc ON mc.message_id = m.id
               WHERE mc.purged_at IS NULL
                 AND m.created_at < now() - make_interval(days => $1)
                 AND m.status IN ('accepted_by_mta','delivered','bounced','failed','suppressed','sandboxed','cancelled')
               LIMIT 5000) old
         WHERE c.message_id = old.id",
    )
    .bind(state.cfg.content_retention_days.clamp(1, 3650) as i32)
    .execute(db)
    .await?
    .rows_affected();

    let jobs = sqlx::query(
        "DELETE FROM outbox_jobs WHERE (status = 'done' AND updated_at < now() - interval '7 days')
                                    OR (status = 'dead' AND updated_at < now() - interval '30 days')",
    )
    .execute(db)
    .await?
    .rows_affected();

    // Mailbox lifecycle and onboarding data: expired links, stale connection
    // tests, and setup analytics older than 13 months (privacy by default).
    sqlx::query("DELETE FROM mailbox_tokens WHERE expires_at < now() - interval '7 days'").execute(db).await?;
    sqlx::query(
        "UPDATE mailbox_connection_checks SET status = 'expired' WHERE status = 'pending' AND expires_at < now()",
    )
    .execute(db)
    .await?;
    sqlx::query("DELETE FROM mailbox_setup_events WHERE created_at < now() - interval '13 months'").execute(db).await?;

    // Subscriptions: renewals, trials ending, overdue invoices.
    let renewed = match crate::services::billing::renew_due(state).await {
        Ok(n) => n,
        Err(e) => {
            tracing::error!(error = %e, "subscription renewal failed");
            0
        }
    };
    if let Err(e) = crate::services::billing::update_payment_status(state).await {
        tracing::error!(error = %e, "payment status update failed");
    }

    state.limiter.prune();
    tracing::debug!(
        sessions,
        idem,
        purged,
        jobs,
        renewed,
        limiter_keys = state.limiter.len(),
        "maintenance pass complete"
    );
    Ok(())
}
