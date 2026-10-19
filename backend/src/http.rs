//! Router assembly and cross-cutting HTTP middleware.

use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, Request},
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{api, error::REQUEST_ID, state::AppState};

pub fn router(state: AppState) -> Router {
    let app = Router::new()
        .merge(api::health::router())
        .merge(api::autoconfig::router())
        .nest("/v1", api::router(&state))
        .nest("/internal/v1", api::internal::router());

    let mut app = app
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn(security_headers))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(60)))
        .layer(CatchPanicLayer::custom(|_| crate::error::AppError::internal("handler panicked").into_response()))
        .layer(TraceLayer::new_for_http().make_span_with(|req: &Request| {
            let request_id = req.extensions().get::<RequestId>().map(|r| r.0.clone()).unwrap_or_default();
            tracing::info_span!("http", method = %req.method(), path = %req.uri().path(), request_id = %request_id)
        }))
        .layer(middleware::from_fn(request_id));

    if !state.cfg.cors_origins.is_empty() {
        let origins: Vec<HeaderValue> = state.cfg.cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
        app = app.layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_credentials(true)
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::PUT, Method::DELETE])
                .allow_headers([
                    header::CONTENT_TYPE,
                    header::AUTHORIZATION,
                    HeaderName::from_static("x-csrf-token"),
                    HeaderName::from_static("idempotency-key"),
                    HeaderName::from_static("x-request-id"),
                ])
                .expose_headers([HeaderName::from_static("x-request-id")])
                .max_age(Duration::from_secs(600)),
        );
    }

    app.with_state(state)
}

#[derive(Clone)]
pub struct RequestId(pub String);

/// Accept a well-formed inbound `X-Request-Id` (for tracing across hops),
/// otherwise generate a time-ordered UUID. Echo it on the response and make
/// it available to error bodies through a task-local.
async fn request_id(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| {
            !v.is_empty() && v.len() <= 64 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map(str::to_string)
        .unwrap_or_else(|| format!("req_{}", uuid::Uuid::now_v7().simple()));
    req.extensions_mut().insert(RequestId(id.clone()));
    let mut res = REQUEST_ID.scope(id.clone(), next.run(req)).await;
    if let Ok(v) = HeaderValue::from_str(&id) {
        res.headers_mut().insert(HeaderName::from_static("x-request-id"), v);
    }
    res
}

async fn security_headers(req: Request, next: Next) -> Response {
    let is_api = {
        let p = req.uri().path();
        p.starts_with("/v1/")
            || p.starts_with("/internal/")
            || p.starts_with("/healthz")
            || p.starts_with("/readyz")
            || p.starts_with("/.well-known/autoconfig/")
            || p.starts_with("/mail/config-v1.1.xml")
            || p.to_ascii_lowercase().starts_with("/autodiscover/")
    };
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let set = |h: &mut axum::http::HeaderMap, name: &'static str, value: &'static str| {
        h.insert(HeaderName::from_static(name), HeaderValue::from_static(value));
    };
    set(h, "x-content-type-options", "nosniff");
    set(h, "referrer-policy", "no-referrer");
    set(h, "x-frame-options", "DENY");
    set(h, "cross-origin-opener-policy", "same-origin");
    set(h, "permissions-policy", "camera=(), microphone=(), geolocation=(), payment=()");
    if is_api {
        set(h, "content-security-policy", "default-src 'none'; frame-ancestors 'none'");
        if !h.contains_key(header::CACHE_CONTROL) {
            set(h, "cache-control", "no-store");
        }
    } else {
        // Scripts only from our origin. Inline *styles* are allowed so that
        // sandboxed email previews (iframe srcdoc, no scripts) render; the
        // HTML they contain is sanitized server-side first.
        set(
            h,
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; \
             font-src 'self'; connect-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'none'; \
             form-action 'self'; frame-ancestors 'none'",
        );
    }
    res
}

/// JSON 404 for unknown API routes (instead of the SPA fallback).
pub async fn api_not_found() -> Response {
    crate::error::AppError::new(StatusCode::NOT_FOUND, "route_not_found", "No such API route").into_response()
}
