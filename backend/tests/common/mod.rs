//! Integration test harness.
//!
//! Each test gets a fresh PostgreSQL database (created from
//! `TEST_DATABASE_URL`, which must point at a role allowed to CREATE
//! DATABASE), migrations applied, and the real router served on a random
//! port. Tests are skipped with a notice when `TEST_DATABASE_URL` is unset.

#![allow(dead_code)]

use std::net::SocketAddr;

use mailvora::{config::Config, db, http, state::AppState};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, Connection, PgConnection, PgPool};

pub struct TestApp {
    pub base: String,
    pub state: AppState,
    pub db: PgPool,
    admin_url: String,
    db_name: String,
}

pub async fn spawn() -> Option<TestApp> {
    spawn_with(|_| {}).await
}

pub async fn spawn_with(customize: impl FnOnce(&mut Config)) -> Option<TestApp> {
    let Ok(admin_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set; skipping integration test");
        return None;
    };
    let db_name = format!("mv_test_{}", uuid::Uuid::new_v4().simple());
    let mut admin = PgConnection::connect(&admin_url).await.expect("connect admin db");
    sqlx::query(&format!("CREATE DATABASE {db_name}")).execute(&mut admin).await.expect("create test db");
    admin.close().await.ok();

    let url = replace_db(&admin_url, &db_name);
    let pool = PgPoolOptions::new().max_connections(10).connect(&url).await.expect("connect test db");
    db::migrate(&pool).await.expect("migrate");

    let mut cfg = Config::for_tests(&url);
    customize(&mut cfg);
    let state = AppState::new(pool.clone(), cfg).expect("state");
    let app = http::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    Some(TestApp { base: format!("http://{addr}"), state, db: pool, admin_url, db_name })
}

fn replace_db(url: &str, db: &str) -> String {
    let (prefix, rest) = url.rsplit_once('/').expect("database url has a path");
    let query = rest.split_once('?').map(|(_, q)| format!("?{q}")).unwrap_or_default();
    format!("{prefix}/{db}{query}")
}

impl TestApp {
    pub async fn cleanup(self) {
        self.db.close().await;
        if let Ok(mut admin) = PgConnection::connect(&self.admin_url).await {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", self.db_name))
                .execute(&mut admin)
                .await;
        }
    }

    pub fn client(&self) -> Client {
        Client::builder().cookie_store(true).build().unwrap()
    }

    /// Register a user (and organization) and return a CSRF-aware session.
    pub async fn register(&self, email: &str) -> Session {
        let client = self.client();
        let res = client
            .post(format!("{}/v1/auth/register", self.base))
            .json(&json!({ "full_name": "Test User", "email": email, "password": "correct horse battery staple", "organization_name": format!("Org of {email}") }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED, "register failed: {}", res.text().await.unwrap_or_default());
        let body: Value = res.json().await.unwrap();
        Session {
            base: self.base.clone(),
            client,
            csrf: body["csrf_token"].as_str().unwrap().to_string(),
            org_id: body["organization"]["id"].as_str().unwrap().to_string(),
            user_id: body["user"]["id"].as_str().unwrap().to_string(),
        }
    }

    pub async fn verify_email(&self, s: &Session) {
        sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1::uuid")
            .bind(&s.user_id)
            .execute(&self.db)
            .await
            .unwrap();
    }

    /// Add a domain and pass verification (DEV_SKIP_DNS_VERIFICATION is on in tests).
    pub async fn verified_domain(&self, s: &Session, domain: &str) -> String {
        let (st, d) = s.post("/v1/domains", json!({ "domain": domain })).await;
        assert_eq!(st, StatusCode::CREATED, "{d}");
        let id = d["id"].as_str().unwrap().to_string();
        let (st, d) = s.post(&format!("/v1/domains/{id}/verification-check"), json!({})).await;
        assert_eq!(st, StatusCode::OK, "{d}");
        assert_eq!(d["status"], "verified");
        id
    }

    pub async fn api_key(&self, s: &Session, environment: &str, scopes: &[&str]) -> String {
        let (st, k) =
            s.post("/v1/api-keys", json!({ "name": "test", "environment": environment, "scopes": scopes })).await;
        assert_eq!(st, StatusCode::CREATED, "{k}");
        k["key"].as_str().unwrap().to_string()
    }

    pub async fn with_key(&self, key: &str, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Client::new().request(method, format!("{}{path}", self.base)).bearer_auth(key);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await.unwrap();
        let status = res.status();
        let text = res.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }
}

pub struct Session {
    pub base: String,
    pub client: Client,
    pub csrf: String,
    pub org_id: String,
    pub user_id: String,
}

impl Session {
    pub async fn request(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = self.client.request(method.clone(), format!("{}{path}", self.base));
        if method != Method::GET {
            req = req.header("x-csrf-token", &self.csrf);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await.unwrap();
        let status = res.status();
        let text = res.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }
    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.request(Method::GET, path, None).await
    }
    pub async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.request(Method::POST, path, Some(body)).await
    }
    pub async fn patch(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.request(Method::PATCH, path, Some(body)).await
    }
    pub async fn put_json(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.request(Method::PUT, path, Some(body)).await
    }
    pub async fn delete(&self, path: &str) -> (StatusCode, Value) {
        self.request(Method::DELETE, path, None).await
    }
}
