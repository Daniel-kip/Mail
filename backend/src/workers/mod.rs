//! Background workers.
//!
//! Workers wake on Postgres `NOTIFY outbox_jobs` (sent by a trigger on
//! insert) and also poll every few seconds, so a missed notification only
//! delays work. Several worker tasks run per process and several processes
//! may run side by side; the queue's row locking keeps them independent.

pub mod maintenance;
pub mod queue;
pub mod send;
pub mod webhook;

use std::{sync::Arc, time::Duration};

use sqlx::postgres::PgListener;
use tokio::sync::{watch, Notify};

use crate::{mail::smtp::SmtpRelay, services::webhooks, state::AppState};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH: i64 = 10;

pub async fn run(state: AppState, shutdown: watch::Receiver<bool>) {
    let relay = match SmtpRelay::new(&state.cfg.smtp, &state.cfg.mail_hostname) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "cannot configure SMTP relay; workers not started");
            return;
        }
    };
    let http = match webhooks::http_client(&state.cfg) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "cannot build webhook HTTP client; workers not started");
            return;
        }
    };

    let mail_wake = Arc::new(Notify::new());
    let hook_wake = Arc::new(Notify::new());
    let mut tasks = Vec::new();

    tasks.push(tokio::spawn(listen(state.clone(), mail_wake.clone(), hook_wake.clone(), shutdown.clone())));

    let senders = state.cfg.worker_concurrency.max(1);
    for i in 0..senders {
        let (state, relay, wake, shutdown) = (state.clone(), relay.clone(), mail_wake.clone(), shutdown.clone());
        let worker_id = format!("{}-mail-{i}", hostname());
        tasks.push(tokio::spawn(async move {
            worker_loop(&state, &worker_id, &["send_message", "system_email"], wake, shutdown, |job| {
                let (state, relay) = (state.clone(), relay.clone());
                async move {
                    match job.kind.as_str() {
                        "send_message" => send::send_message(&state, &relay, &job).await,
                        _ => send::system_email(&state, &relay, &job).await,
                    }
                }
            })
            .await
        }));
    }
    for i in 0..(senders / 2).max(1) {
        let (state, http, wake, shutdown) = (state.clone(), http.clone(), hook_wake.clone(), shutdown.clone());
        let worker_id = format!("{}-hook-{i}", hostname());
        tasks.push(tokio::spawn(async move {
            worker_loop(&state, &worker_id, &["deliver_webhook"], wake, shutdown, |job| {
                let (state, http) = (state.clone(), http.clone());
                async move { webhook::deliver(&state, &http, &job).await }
            })
            .await
        }));
    }
    tasks.push(tokio::spawn(maintenance::run(state.clone(), shutdown.clone())));

    tracing::info!(mail_workers = senders, "background workers started");
    for t in tasks {
        let _ = t.await;
    }
    tracing::info!("background workers stopped");
}

async fn worker_loop<F, Fut>(
    state: &AppState,
    worker_id: &str,
    kinds: &[&str],
    wake: Arc<Notify>,
    mut shutdown: watch::Receiver<bool>,
    handler: F,
) where
    F: Fn(queue::Job) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    loop {
        if *shutdown.borrow() {
            return;
        }
        let jobs = match queue::claim(&state.db, kinds, worker_id, BATCH).await {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::error!(error = %e, worker = worker_id, "failed to claim jobs");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };
        let claimed = jobs.len();
        for job in jobs {
            let (id, kind) = (job.id, job.kind.clone());
            if let Err(e) = handler(job).await {
                // Infrastructure failure: the lease will expire and the job
                // will be retried by whichever worker claims it next.
                tracing::error!(job = id, kind = %kind, error = %e, "job handler failed");
            }
            if *shutdown.borrow() {
                return;
            }
        }
        if claimed == 0 {
            tokio::select! {
                _ = wake.notified() => {}
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = shutdown.changed() => return,
            }
        }
    }
}

async fn listen(state: AppState, mail: Arc<Notify>, hooks: Arc<Notify>, mut shutdown: watch::Receiver<bool>) {
    loop {
        let mut listener = match PgListener::connect_with(&state.db).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "LISTEN connection failed; relying on polling");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };
        if let Err(e) = listener.listen("outbox_jobs").await {
            tracing::warn!(error = %e, "LISTEN failed; relying on polling");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }
        loop {
            tokio::select! {
                n = listener.recv() => match n {
                    Ok(n) if n.payload() == "deliver_webhook" => hooks.notify_one(),
                    Ok(_) => mail.notify_one(),
                    Err(e) => {
                        tracing::warn!(error = %e, "LISTEN connection lost; reconnecting");
                        break;
                    }
                },
                _ = shutdown.changed() => return,
            }
        }
    }
}

/// Process every job that is ready right now, then return how many ran.
/// Used by integration tests and for operational one-off drains.
pub async fn drain(state: &AppState) -> anyhow::Result<usize> {
    let relay = SmtpRelay::new(&state.cfg.smtp, &state.cfg.mail_hostname)?;
    let http = webhooks::http_client(&state.cfg)?;
    let mut total = 0;
    loop {
        let jobs =
            queue::claim(&state.db, &["send_message", "system_email", "deliver_webhook"], "drain", BATCH).await?;
        if jobs.is_empty() {
            return Ok(total);
        }
        for job in jobs {
            total += 1;
            match job.kind.as_str() {
                "send_message" => send::send_message(state, &relay, &job).await?,
                "system_email" => send::system_email(state, &relay, &job).await?,
                _ => webhook::deliver(state, &http, &job).await?,
            }
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "worker".into())
}
