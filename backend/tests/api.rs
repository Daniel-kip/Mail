//! End-to-end API tests against a real PostgreSQL database.
//!
//! Mapped to guide 08 (functional acceptance matrix and security cases).
//! Run with:
//!   TEST_DATABASE_URL=postgres://mailvora:mailvora_dev@127.0.0.1:5434/mailvora cargo test
//! The worker test additionally needs Mailpit on 127.0.0.1:1025/8025.

mod common;

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

macro_rules! app {
    () => {
        match common::spawn().await {
            Some(a) => a,
            None => return,
        }
    };
}

#[tokio::test]
async fn health_and_readiness() {
    let app = app!();
    let r: Value = reqwest::get(format!("{}/healthz", app.base)).await.unwrap().json().await.unwrap();
    assert_eq!(r["status"], "ok");
    let res = reqwest::get(format!("{}/readyz", app.base)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = reqwest::get(format!("{}/v1/does-not-exist", app.base)).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "route_not_found");
    assert!(body["error"]["request_id"].as_str().unwrap().starts_with("req_"));
    app.cleanup().await;
}

#[tokio::test]
async fn session_requires_csrf_for_writes() {
    let app = app!();
    let s = app.register("csrf@example.com").await;
    // Same cookie, no CSRF header → rejected.
    let res = s
        .client
        .post(format!("{}/v1/domains", app.base))
        .json(&json!({ "domain": "x.example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "csrf_failed");
    // Reads do not need it.
    let (st, _) = s.get("/v1/domains").await;
    assert_eq!(st, StatusCode::OK);
    app.cleanup().await;
}

#[tokio::test]
async fn login_failures_and_lockout() {
    let app = app!();
    app.register("lock@example.com").await;
    let c = app.client();
    for _ in 0..10 {
        let res = c
            .post(format!("{}/v1/auth/login", app.base))
            .json(&json!({ "email": "lock@example.com", "password": "wrong password here" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
    let res = c
        .post(format!("{}/v1/auth/login", app.base))
        .json(&json!({ "email": "lock@example.com", "password": "correct horse battery staple" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS, "account must be locked after repeated failures");
    // Unknown accounts get the same generic error.
    let res = c
        .post(format!("{}/v1/auth/login", app.base))
        .json(&json!({ "email": "nobody@example.com", "password": "whatever whatever" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    app.cleanup().await;
}

#[tokio::test]
async fn tenant_isolation_across_resources() {
    let app = app!();
    let a = app.register("alice@example.com").await;
    let b = app.register("bob@example.com").await;
    app.verify_email(&a).await;
    app.verify_email(&b).await;
    let domain_a = app.verified_domain(&a, "alice-school.example").await;

    // Bob cannot read, verify or delete Alice's domain.
    for (method, path) in [
        (Method::GET, format!("/v1/domains/{domain_a}")),
        (Method::POST, format!("/v1/domains/{domain_a}/verification-check")),
        (Method::DELETE, format!("/v1/domains/{domain_a}")),
    ] {
        let (st, _) = b.request(method, &path, if path.ends_with("check") { Some(json!({})) } else { None }).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{path}");
    }
    let (_, list) = b.get("/v1/domains").await;
    assert_eq!(list["data"].as_array().unwrap().len(), 0);

    // Alice sends a message; Bob's key cannot see it.
    let key_a = app.api_key(&a, "live", &["messages:send", "messages:read"]).await;
    let key_b = app.api_key(&b, "live", &["messages:send", "messages:read"]).await;
    let (st, sent) = app
        .with_key(&key_a, Method::POST, "/v1/messages", Some(json!({ "from": "no-reply@alice-school.example", "to": "x@example.org", "subject": "Hi", "text": "Hello" })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{sent}");
    let mid = sent["messages"][0]["id"].as_str().unwrap();
    let (st, _) = app.with_key(&key_b, Method::GET, &format!("/v1/messages/{mid}"), None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (_, list) = app.with_key(&key_b, Method::GET, "/v1/messages", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 0);

    // Bob cannot send from Alice's verified domain.
    let (st, err) = app
        .with_key(&key_b, Method::POST, "/v1/messages", Some(json!({ "from": "no-reply@alice-school.example", "to": "x@example.org", "subject": "Hi", "text": "Hello" })))
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{err}");
    // A domain verified by one tenant cannot be added by another.
    let (st, _) = b.post("/v1/domains", json!({ "domain": "alice-school.example" })).await;
    assert_eq!(st, StatusCode::CONFLICT);
    app.cleanup().await;
}

#[tokio::test]
async fn api_keys_scopes_revocation_and_rotation() {
    let app = app!();
    let s = app.register("keys@example.com").await;

    // Live keys require a confirmed email; test keys do not.
    let (st, _) = s.post("/v1/api-keys", json!({ "name": "live", "environment": "live" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    app.verify_email(&s).await;
    app.verified_domain(&s, "keys.example").await;

    let read_only = app.api_key(&s, "live", &["messages:read"]).await;
    let (st, err) = app
        .with_key(
            &read_only,
            Method::POST,
            "/v1/messages",
            Some(json!({ "from": "a@keys.example", "to": "b@example.org", "subject": "s", "text": "t" })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "insufficient_permissions");

    // Keys can never manage keys.
    let (st, _) = app.with_key(&read_only, Method::GET, "/v1/api-keys", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Non-grantable scopes are rejected at creation.
    let (st, _) =
        s.post("/v1/api-keys", json!({ "name": "x", "environment": "live", "scopes": ["api_keys:manage"] })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Revocation takes effect immediately.
    let (_, keys) = s.get("/v1/api-keys").await;
    let id = keys["data"][0]["id"].as_str().unwrap().to_string();
    let (st, _) = s.delete(&format!("/v1/api-keys/{id}")).await;
    assert_eq!(st, StatusCode::OK);
    let (st, err) = app.with_key(&read_only, Method::GET, "/v1/messages", None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    assert_eq!(err["error"]["code"], "api_key_revoked");

    // Rotation issues a working key and kills the old one.
    let old = app.api_key(&s, "live", &["messages:read"]).await;
    let (_, keys) = s.get("/v1/api-keys").await;
    let active = keys["data"].as_array().unwrap().iter().find(|k| k["status"] == "active").unwrap();
    let (st, rotated) = s.post(&format!("/v1/api-keys/{}/rotate", active["id"].as_str().unwrap()), json!({})).await;
    assert_eq!(st, StatusCode::CREATED);
    let new = rotated["key"].as_str().unwrap();
    assert_eq!(app.with_key(&old, Method::GET, "/v1/messages", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(app.with_key(new, Method::GET, "/v1/messages", None).await.0, StatusCode::OK);

    // Garbage keys get a generic error.
    assert_eq!(
        app.with_key(
            "mv_live_aaaaaaaaaaaa_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            Method::GET,
            "/v1/messages",
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    app.cleanup().await;
}

#[tokio::test]
async fn sending_rules_idempotency_and_injection() {
    let app = app!();
    let s = app.register("send@example.com").await;
    app.verify_email(&s).await;
    let key = app.api_key(&s, "live", &["messages:send", "messages:read"]).await;

    // Unregistered / unverified domains cannot send.
    let msg = json!({ "from": "no-reply@pending.example", "to": "a@example.org", "subject": "Hello", "text": "Body" });
    let (st, err) = app.with_key(&key, Method::POST, "/v1/messages", Some(msg.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["details"]["reason"], "domain_not_registered");
    s.post("/v1/domains", json!({ "domain": "pending.example" })).await;
    let (st, err) = app.with_key(&key, Method::POST, "/v1/messages", Some(msg.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["details"]["reason"], "domain_not_verified");

    app.verified_domain(&s, "school.example").await;
    let body = json!({ "from": "Registrar <no-reply@school.example>", "to": ["a@example.org", "b@example.org", "A@example.org"], "subject": "Results", "html": "<p>Hi<script>alert(1)</script></p>", "text": "Hi", "tags": ["results"] });

    // Header injection is rejected before anything is stored.
    let mut evil = body.clone();
    evil["subject"] = json!("Hi\r\nBcc: victim@example.net");
    let (st, _) = app.with_key(&key, Method::POST, "/v1/messages", Some(evil)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Idempotent send: duplicates (case-insensitive) collapse; replay returns the same response.
    let client = reqwest::Client::new();
    let send = |b: Value| {
        client
            .post(format!("{}/v1/messages", app.base))
            .bearer_auth(&key)
            .header("idempotency-key", "order-1001")
            .json(&b)
            .send()
    };
    let first = send(body.clone()).await.unwrap();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let first: Value = first.json().await.unwrap();
    assert_eq!(first["messages"].as_array().unwrap().len(), 2, "duplicate recipients must collapse");
    let replay = send(body.clone()).await.unwrap();
    assert_eq!(replay.headers().get("idempotent-replayed").unwrap(), "true");
    let replay: Value = replay.json().await.unwrap();
    assert_eq!(replay, first);
    let mut changed = body.clone();
    changed["subject"] = json!("Different");
    let res = send(changed).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Atomicity: every queued message has exactly one outbox job; HTML was sanitized.
    let (msgs, jobs): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM messages WHERE status = 'queued'), (SELECT count(*) FROM outbox_jobs WHERE kind = 'send_message')",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!((msgs, jobs), (2, 2));
    let mid = first["messages"][0]["id"].as_str().unwrap();
    let (_, detail) = app.with_key(&key, Method::GET, &format!("/v1/messages/{mid}"), None).await;
    assert!(!detail["content"]["html"].as_str().unwrap().contains("<script"));
    assert_eq!(detail["events"][0]["type"], "queued");

    // The stored MIME is DKIM-signed and uses a VERP return path.
    let (raw, envelope): (Vec<u8>, String) = sqlx::query_as(
        "SELECT c.raw_mime, m.envelope_from FROM message_contents c JOIN messages m ON m.id = c.message_id WHERE m.id = $1::uuid",
    )
    .bind(mid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    let raw = String::from_utf8_lossy(&raw);
    assert!(raw.contains("DKIM-Signature:") && raw.contains("d=school.example"), "message must be DKIM signed");
    assert!(envelope.starts_with("b+") && envelope.ends_with("@bounces.mailvora.test"));
    app.cleanup().await;
}

#[tokio::test]
async fn templates_versions_and_variables() {
    let app = app!();
    let s = app.register("tpl@example.com").await;
    app.verify_email(&s).await;
    app.verified_domain(&s, "tpl.example").await;
    let (st, t) = s
        .post(
            "/v1/templates",
            json!({ "name": "Welcome", "subject": "Welcome {{ name }}", "html_body": "<p>Hello {{ name }}</p><a href=\"{{ url }}\">x</a>", "text_body": "Hello {{ name }}",
                    "variables": [{ "name": "name", "required": true, "sample": "Ann" }, { "name": "url", "required": false, "default": "https://example.org" }] }),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{t}");
    assert_eq!(t["slug"], "welcome");
    let key = app.api_key(&s, "live", &["messages:send"]).await;

    let (st, err) = app
        .with_key(
            &key,
            Method::POST,
            "/v1/messages",
            Some(json!({ "from": "a@tpl.example", "to": "b@example.org", "template": "welcome" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err["error"]["code"], "missing_template_variables");

    let (st, sent) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "a@tpl.example", "to": "b@example.org", "template": "welcome", "variables": { "name": "<b>Zed</b>", "url": "javascript:alert(1)" } })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{sent}");
    let html: String =
        sqlx::query_scalar("SELECT html_body FROM message_contents LIMIT 1").fetch_one(&app.db).await.unwrap();
    assert!(html.contains("&lt;b&gt;Zed&lt;/b&gt;"), "variables are escaped: {html}");
    assert!(!html.contains("javascript:"), "dangerous URLs are removed: {html}");

    // New version, then roll back.
    let id = t["id"].as_str().unwrap();
    let (st, v2) = s.request(Method::PUT, &format!("/v1/templates/{id}"), Some(json!({ "subject": "Hi {{ name }}", "text_body": "v2 {{ name }}", "variables": [{ "name": "name", "required": true }] }))).await;
    assert_eq!(st, StatusCode::OK, "{v2}");
    assert_eq!(v2["active_version"], 2);
    let (st, back) = s.post(&format!("/v1/templates/{id}/versions/1/activate"), json!({})).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(back["active_version"], 1);
    // Syntax errors are rejected with a useful message.
    let (st, err) =
        s.post("/v1/templates", json!({ "name": "Broken", "subject": "Hi {{ name", "text_body": "x" })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err["error"]["code"], "template_error");
    app.cleanup().await;
}

#[tokio::test]
async fn suppressions_quotas_and_test_keys() {
    let app = app!();
    let s = app.register("quota@example.com").await;
    app.verify_email(&s).await;
    app.verified_domain(&s, "quota.example").await;
    let key = app.api_key(&s, "live", &["messages:send", "suppressions:write"]).await;

    let (st, _) =
        app.with_key(&key, Method::POST, "/v1/suppressions", Some(json!({ "email": "blocked@example.org" }))).await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, r) = app.with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "a@quota.example", "to": ["blocked@example.org", "ok@example.org"], "subject": "s", "text": "t" }))).await;
    let statuses: Vec<&str> = r["messages"].as_array().unwrap().iter().map(|m| m["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, vec!["suppressed", "queued"]);

    // Test keys run the pipeline but never create delivery jobs.
    let test_key = app.api_key(&s, "test", &["messages:send"]).await;
    let (_, r) = app
        .with_key(
            &test_key,
            Method::POST,
            "/v1/messages",
            Some(json!({ "from": "a@quota.example", "to": "t@example.org", "subject": "s", "text": "t" })),
        )
        .await;
    assert_eq!(r["sandbox"], true);
    assert_eq!(r["messages"][0]["status"], "sandboxed");

    // Daily quota: shrink the plan and exceed it.
    sqlx::query(
        "INSERT INTO plans SELECT 'tiny', 'Tiny', 3, 100, 2, 2, 104857600, 104857600, 10, 600 ON CONFLICT DO NOTHING",
    )
    .execute(&app.db)
    .await
    .unwrap();
    sqlx::query("UPDATE organizations SET plan_id = 'tiny' WHERE id = $1::uuid")
        .bind(&s.org_id)
        .execute(&app.db)
        .await
        .unwrap();
    let (st, err) = app.with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "a@quota.example", "to": ["1@example.org", "2@example.org"], "subject": "s", "text": "t" }))).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS, "{err}");
    assert_eq!(err["error"]["code"], "quota_exceeded");
    app.cleanup().await;
}

#[tokio::test]
async fn worker_delivers_to_relay_and_bounces_are_processed() {
    let app = app!();
    let s = app.register("worker@example.com").await;
    app.verify_email(&s).await;
    app.verified_domain(&s, "worker.example").await;
    let key = app.api_key(&s, "live", &["messages:send", "messages:read"]).await;
    let (st, r) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "Worker Test <no-reply@worker.example>", "to": "inbox@example.org", "subject": "Worker delivery", "text": "Delivered by the outbox worker." })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let mid = r["messages"][0]["id"].as_str().unwrap().to_string();

    // Requires an SMTP sink on TEST_SMTP_HOST:TEST_SMTP_PORT (Mailpit by default).
    if std::net::TcpStream::connect(("127.0.0.1", 1025)).is_err() && std::env::var("TEST_SMTP_HOST").is_err() {
        eprintln!("no SMTP sink on 127.0.0.1:1025; skipping delivery assertions");
        app.cleanup().await;
        return;
    }
    let processed = mailvora::workers::drain(&app.state).await.unwrap();
    assert!(processed >= 1);
    let (_, m) = app.with_key(&key, Method::GET, &format!("/v1/messages/{mid}"), None).await;
    assert_eq!(m["status"], "accepted_by_mta", "{m}");
    let types: Vec<&str> = m["events"].as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(types, vec!["queued", "accepted"]);

    // A DSN posted by Postfix marks the message bounced and suppresses the address.
    let envelope: String = sqlx::query_scalar("SELECT envelope_from FROM messages WHERE id = $1::uuid")
        .bind(&mid)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let dsn = "From: MAILER-DAEMON@mx.example.org\r\nSubject: Undelivered\r\nMIME-Version: 1.0\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=\"B\"\r\n\r\n--B\r\nContent-Type: text/plain\r\n\r\nfailed\r\n\r\n--B\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; mx.example.org\r\n\r\nFinal-Recipient: rfc822; inbox@example.org\r\nAction: failed\r\nStatus: 5.1.1\r\nDiagnostic-Code: smtp; 550 5.1.1 User unknown\r\n\r\n--B--\r\n";
    let client = reqwest::Client::new();
    let unauthorized = client
        .post(format!("{}/internal/v1/bounces", app.base))
        .query(&[("recipient", &envelope)])
        .body(dsn)
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let forged = client
        .post(format!("{}/internal/v1/bounces", app.base))
        .bearer_auth("test-internal-token-0123456789abcdef")
        .query(&[("recipient", format!("b+{}-0000000000@bounces.mailvora.test", mid.replace('-', "")))])
        .body(dsn)
        .send()
        .await
        .unwrap();
    let forged: Value = forged.json().await.unwrap();
    assert_eq!(forged["status"], "ignored", "forged VERP tags must be ignored");
    let res = client
        .post(format!("{}/internal/v1/bounces", app.base))
        .bearer_auth("test-internal-token-0123456789abcdef")
        .query(&[("recipient", &envelope)])
        .body(dsn)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let (_, m) = app.with_key(&key, Method::GET, &format!("/v1/messages/{mid}"), None).await;
    assert_eq!(m["status"], "bounced");
    let suppressed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM suppressions WHERE email = 'inbox@example.org' AND reason = 'hard_bounce')",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(suppressed);
    app.cleanup().await;
}

#[tokio::test]
async fn webhook_targets_are_ssrf_protected_and_events_fan_out() {
    let app = app!();
    let s = app.register("hooks@example.com").await;
    for bad in [
        "http://example.org/hook",
        "https://127.0.0.1/hook",
        "https://169.254.169.254/latest",
        "https://user:pw@example.org/",
        "https://localhost/x",
    ] {
        let (st, _) = s.post("/v1/webhooks", json!({ "url": bad, "events": ["*"] })).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{bad} must be rejected");
    }
    let (st, ep) =
        s.post("/v1/webhooks", json!({ "url": "https://hooks.example.org/email", "events": ["message.queued"] })).await;
    assert_eq!(st, StatusCode::CREATED, "{ep}");
    assert!(ep["secret"].as_str().unwrap().starts_with("whsec_"));

    app.verify_email(&s).await;
    app.verified_domain(&s, "hooks.example").await;
    let key = app.api_key(&s, "live", &["messages:send"]).await;
    app.with_key(
        &key,
        Method::POST,
        "/v1/messages",
        Some(json!({ "from": "a@hooks.example", "to": "b@example.org", "subject": "s", "text": "t" })),
    )
    .await;
    let deliveries: i64 =
        sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE event_type = 'message.queued'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(deliveries, 1, "the queued event is fanned out in the same transaction");
    app.cleanup().await;
}

#[tokio::test]
async fn mailboxes_feed_mail_server_views_and_suspension_stops_everything() {
    let app = app!();
    let s = app.register("mbox@example.com").await;
    app.verify_email(&s).await;
    let domain = app.verified_domain(&s, "campus.example").await;

    let (st, _) = s.post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "jane" })).await;
    assert_eq!(st, StatusCode::CONFLICT, "hosting must be enabled first");
    s.patch(&format!("/v1/domains/{domain}"), json!({ "mail_hosting_enabled": true })).await;
    let (st, m) = s
        .post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "Jane.Doe", "display_name": "Jane Doe" }))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{m}");
    assert_eq!(m["address"], "jane.doe@campus.example");
    let password = m["password"].as_str().unwrap();
    assert_eq!(password.len(), 20);

    let (pw_hash, storage): (String, String) =
        sqlx::query_as("SELECT password, storage_key FROM mail_mailboxes WHERE address = 'jane.doe@campus.example'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(pw_hash.starts_with("{ARGON2ID}$argon2id$"), "Dovecot-compatible hash");
    assert!(storage.starts_with("campus.example/"));
    let domains: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_domains WHERE domain = 'campus.example'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(domains, 1);

    // Alias to the mailbox; aliases may not point outside the tenant.
    let (st, _) = s
        .post(
            "/v1/aliases",
            json!({ "domain_id": domain, "local_part": "info", "destinations": ["jane.doe@campus.example"] }),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _) = s
        .post("/v1/aliases", json!({ "domain_id": domain, "local_part": "fwd", "destinations": ["someone@gmail.com"] }))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let logins: String =
        sqlx::query_scalar("SELECT logins FROM mail_sender_logins WHERE address = 'info@campus.example'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(logins, "jane.doe@campus.example");

    // Webmail login with the generated password.
    let c = app.client();
    let res = c
        .post(format!("{}/v1/webmail/login", app.base))
        .json(&json!({ "address": "Jane.Doe@campus.example", "password": password }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = c.get(format!("{}/v1/webmail/session", app.base)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = c.get(format!("{}/v1/webmail/setup", app.base)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let setup: Value = res.json().await.unwrap();
    assert_eq!(setup["mailbox"]["address"], "jane.doe@campus.example");
    assert_eq!(setup["settings"]["username"], "jane.doe@campus.example");
    assert_eq!(setup["settings"]["imap"]["port"], 993);
    // RFC 8314: implicit TLS submission first, STARTTLS on 587 as the alternative.
    assert_eq!(setup["settings"]["smtp"]["port"], 465);
    assert_eq!(setup["settings"]["smtp"]["security"], "SSL/TLS");
    assert_eq!(setup["settings"]["smtp_alternative"]["port"], 587);
    assert!(setup["password"].is_null(), "setup never returns mailbox credentials");
    assert!(setup["settings"]["imap"]["master_password"].is_null(), "private Dovecot credentials are not exposed");

    let anonymous = app.client().get(format!("{}/v1/webmail/setup", app.base)).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED, "setup requires this mailbox's session");

    // Emergency suspension: keys stop, mail views drop the tenant, webmail ends.
    let key = app.api_key(&s, "live", &["messages:read"]).await;
    sqlx::query("UPDATE users SET is_platform_admin = true WHERE id = $1::uuid")
        .bind(&s.user_id)
        .execute(&app.db)
        .await
        .unwrap();
    let (st, r) =
        s.post(&format!("/v1/admin/organizations/{}/suspend", s.org_id), json!({ "reason": "test suspension" })).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (st, err) = app.with_key(&key, Method::GET, "/v1/messages", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "organization_suspended");
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_mailboxes").fetch_one(&app.db).await.unwrap();
    assert_eq!(live, 0, "suspended tenants disappear from the mail server views");
    let res = c.get(format!("{}/v1/webmail/session", app.base)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = c.get(format!("{}/v1/webmail/setup", app.base)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "suspension revokes mailbox setup access");
    // Console stays readable but writes are blocked.
    let (st, _) = s.get("/v1/domains").await;
    assert_eq!(st, StatusCode::OK);
    let (st, err) = s.post("/v1/domains", json!({ "domain": "new.example" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "organization_suspended");
    app.cleanup().await;
}

#[tokio::test]
async fn roles_limit_what_members_can_do() {
    let app = app!();
    let owner = app.register("owner@example.com").await;
    let viewer = app.register("viewer@example.com").await;
    // Add viewer to owner's org directly, as an accepted invitation would.
    sqlx::query("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1::uuid, $2::uuid, 'viewer')")
        .bind(&owner.org_id)
        .bind(&viewer.user_id)
        .execute(&app.db)
        .await
        .unwrap();
    let (st, _) = viewer.post("/v1/auth/switch-organization", json!({ "organization_id": owner.org_id })).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = viewer.get("/v1/domains").await;
    assert_eq!(st, StatusCode::OK);
    let (st, err) = viewer.post("/v1/domains", json!({ "domain": "v.example" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "insufficient_permissions");
    let (st, _) = viewer.get("/v1/api-keys").await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // The last owner cannot be demoted or removed.
    let (st, _) = owner.delete(&format!("/v1/members/{}", owner.user_id)).await;
    assert_eq!(st, StatusCode::CONFLICT);
    // Switching to an organization you do not belong to fails.
    let (st, _) = owner.post("/v1/auth/switch-organization", json!({ "organization_id": viewer.org_id })).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    app.cleanup().await;
}
