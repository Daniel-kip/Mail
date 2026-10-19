//! Liveness and readiness probes.
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{
    error::{ApiResult, AppError},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz)).route("/readyz", get(readyz))
}

/// The process is up and serving HTTP.
async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Dependencies are reachable: database round-trip and migration state.
async fn readyz(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&state.db)
        .await
        .map_err(|_| AppError::unavailable("database_unavailable", "Database is not reachable"))?;
    let expected = crate::db::MIGRATOR.iter().count() as i64;
    if applied < expected {
        return Err(AppError::unavailable("migrations_pending", "Database migrations are pending"));
    }
    Ok(Json(json!({ "status": "ready", "database": "ok", "migrations": applied })))
}
