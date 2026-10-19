//! Command-line entry point.
//!
//! ```text
//! mailvora serve                 API + background workers (default)
//! mailvora api                   API only (WORKER_ENABLED is ignored)
//! mailvora worker                background workers only
//! mailvora migrate               apply database migrations and exit
//! mailvora promote-admin EMAIL   grant platform-operator rights to a user
//! mailvora gen-key               print a new DATA_ENCRYPTION_KEY
//! mailvora healthcheck           exit 0 if the local API answers /healthz
//! ```

use std::net::SocketAddr;

use anyhow::{bail, Context};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use mailvora::{config::Config, crypto, db, http, state::AppState, workers};
use tokio::sync::watch;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(PartialEq)]
enum Mode {
    Serve,
    Api,
    Worker,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("serve");

    if command == "gen-key" {
        println!("{}", B64.encode(crypto::random_bytes::<32>()));
        return Ok(());
    }
    if command == "healthcheck" {
        return healthcheck().await;
    }
    if matches!(command, "-h" | "--help" | "help") {
        println!("usage: mailvora [serve|api|worker|migrate|promote-admin EMAIL|gen-key]");
        return Ok(());
    }

    // .env is a development convenience; production injects real env vars.
    let _ = dotenvy::dotenv();
    init_tracing();

    let cfg = Config::from_env()?;

    match command {
        "migrate" => {
            let pool = db::connect(&migration_url(), 2).await?;
            db::migrate(&pool).await?;
            tracing::info!("migrations applied");
            Ok(())
        }
        "promote-admin" => {
            let email = args.get(1).context("usage: mailvora promote-admin EMAIL")?;
            let pool = db::connect(&cfg.database_url, 2).await?;
            let n = sqlx::query("UPDATE users SET is_platform_admin = true, updated_at = now() WHERE email = $1")
                .bind(email)
                .execute(&pool)
                .await?
                .rows_affected();
            if n == 0 {
                bail!("no user with email {email}");
            }
            println!("{email} is now a platform administrator");
            Ok(())
        }
        "serve" => run(cfg, Mode::Serve).await,
        "api" => run(cfg, Mode::Api).await,
        "worker" => run(cfg, Mode::Worker).await,
        other => bail!("unknown command {other:?}; try --help"),
    }
}

/// Migrations may use a more privileged role than the runtime (guide 06).
fn migration_url() -> String {
    std::env::var("MIGRATION_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")).unwrap_or_default()
}

async fn run(cfg: Config, mode: Mode) -> anyhow::Result<()> {
    tracing::info!(config = ?cfg, "starting Mailvora");

    if cfg.run_migrations_on_start {
        let pool = db::connect(&migration_url(), 2).await?;
        db::migrate(&pool).await?;
        pool.close().await;
        tracing::info!("migrations applied");
    }

    let pool = db::connect(&cfg.database_url, cfg.database_max_connections).await?;
    let state = AppState::new(pool, cfg)?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let run_workers = mode == Mode::Worker || (mode == Mode::Serve && state.cfg.worker_enabled);
    let worker_handle = run_workers.then(|| tokio::spawn(workers::run(state.clone(), shutdown_rx.clone())));

    if mode == Mode::Worker {
        shutdown_signal().await;
    } else {
        let bind = state.cfg.bind;
        let app = http::router(state.clone());
        let listener = tokio::net::TcpListener::bind(bind).await.with_context(|| format!("cannot bind {bind}"))?;
        tracing::info!("listening on http://{bind}");
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    }

    // Let workers finish their current job; leases cover anything cut short.
    let _ = shutdown_tx.send(true);
    if let Some(h) = worker_handle {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(25), h).await;
    }
    state.db.close().await;
    tracing::info!("shutdown complete");
    Ok(())
}

/// Container health probe without extra tooling in the image.
async fn healthcheck() -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:8088".into());
    let port = bind.rsplit(':').next().unwrap_or("8088");
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")),
    )
    .await??;
    stream
        .write_all(
            b"GET /healthz HTTP/1.0
Host: localhost

",
        )
        .await?;
    let mut buf = vec![0u8; 64];
    let n = tokio::time::timeout(std::time::Duration::from_secs(3), stream.read(&mut buf)).await??;
    let head = String::from_utf8_lossy(&buf[..n]);
    if head.starts_with("HTTP/1.0 200") || head.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        bail!("unhealthy: {}", head.lines().next().unwrap_or(""))
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "mailvora=info,tower_http=info".into());
    let json = std::env::var("LOG_FORMAT").map(|v| v == "json").unwrap_or(false);
    let registry = tracing_subscriber::registry().with(filter);
    if json {
        registry.with(tracing_subscriber::fmt::layer().json().flatten_event(true)).init();
    } else {
        registry.with(tracing_subscriber::fmt::layer().compact()).init();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
