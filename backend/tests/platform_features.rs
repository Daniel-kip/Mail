//! Integration tests for account types, subscriptions, the ICT-support role,
//! mailbox lifecycle, API key restrictions, scheduling/unsubscribe, client
//! auto-configuration and the mail-app onboarding API.
//!
//! Run with TEST_DATABASE_URL set (see tests/api.rs).

mod common;

use common::{Session, TestApp};
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

async fn register_as(app: &TestApp, email: &str, account_type: &str) -> (StatusCode, Value, Option<Session>) {
    let client = app.client();
    let res = client
        .post(format!("{}/v1/auth/register", app.base))
        .json(&json!({ "full_name": "Test Person", "email": email, "password": "correct horse battery staple",
                       "account_type": account_type }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    let session = (status == StatusCode::CREATED).then(|| Session {
        base: app.base.clone(),
        client,
        csrf: body["csrf_token"].as_str().unwrap().to_string(),
        org_id: body["organization"]["id"].as_str().unwrap().to_string(),
        user_id: body["user"]["id"].as_str().unwrap().to_string(),
    });
    (status, body, session)
}

/// Verified domain with hosted mailboxes enabled.
async fn hosting_domain(app: &TestApp, s: &Session, domain: &str) -> String {
    let id = app.verified_domain(s, domain).await;
    let (st, d) = s.patch(&format!("/v1/domains/{id}"), json!({ "mail_hosting_enabled": true })).await;
    assert_eq!(st, StatusCode::OK, "{d}");
    id
}

async fn add_member(app: &TestApp, org: &Session, member: &Session, role: &str) {
    sqlx::query("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1::uuid, $2::uuid, $3)")
        .bind(&org.org_id)
        .bind(&member.user_id)
        .bind(role)
        .execute(&app.db)
        .await
        .unwrap();
    let (st, _) = member.post("/v1/auth/switch-organization", json!({ "organization_id": org.org_id })).await;
    assert_eq!(st, StatusCode::OK);
}

fn token_from(url: &str) -> String {
    url.split("token=").nth(1).expect("link has a token").to_string()
}

#[tokio::test]
async fn individuals_relay_only_and_institutions_host_mailboxes() {
    let app = app!();
    let plans: Value = reqwest::get(format!("{}/v1/plans", app.base)).await.unwrap().json().await.unwrap();
    let audiences: Vec<&str> =
        plans["data"].as_array().unwrap().iter().map(|p| p["audience"].as_str().unwrap()).collect();
    assert!(audiences.contains(&"individual") && audiences.contains(&"institution"));
    assert!(plans["data"].as_array().unwrap().iter().all(|p| p["is_public"] == true));

    let (st, body, dev) = register_as(&app, "dev@example.com", "individual").await;
    assert_eq!(st, StatusCode::CREATED, "{body}");
    assert_eq!(body["organization"]["account_type"], "individual");
    assert_eq!(body["organization"]["plan"], "developer");
    let dev = dev.unwrap();
    app.verify_email(&dev).await;
    let domain = app.verified_domain(&dev, "invoices.example").await;
    let (st, err) = dev.patch(&format!("/v1/domains/{domain}"), json!({ "mail_hosting_enabled": true })).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{err}");
    assert_eq!(err["error"]["code"], "institution_required");
    let (st, err) = dev.post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "x" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "institution_required");
    // Transactional relaying works.
    let key = app.api_key(&dev, "live", &["messages:send"]).await;
    let (st, sent) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "billing@invoices.example", "to": "client@example.org", "subject": "Invoice 42", "text": "Attached." })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{sent}");
    // Individual plans cap team size at one member.
    let (st, err) =
        dev.post("/v1/members/invitations", json!({ "email": "friend@example.com", "role": "developer" })).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS, "{err}");
    assert_eq!(err["error"]["details"]["limit"], "members");

    // Converting to an institution unlocks hosting.
    let (st, conv) = dev.post("/v1/organization/convert", json!({})).await;
    assert_eq!(st, StatusCode::OK, "{conv}");
    let (st, _) = dev.patch(&format!("/v1/domains/{domain}"), json!({ "mail_hosting_enabled": true })).await;
    assert_eq!(st, StatusCode::OK);
    let (_, org) = dev.get("/v1/organization").await;
    assert_eq!(org["account_type"], "institution");
    assert_eq!(org["plan"]["id"], "free");

    let (st, _, _) = register_as(&app, "bad@example.com", "company").await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    app.cleanup().await;
}

#[tokio::test]
async fn subscriptions_trial_upgrade_invoice_and_scheduled_downgrade() {
    let app = app!();
    let s = app.register("bursar@school.example").await;
    let (st, billing) = s.get("/v1/billing").await;
    assert_eq!(st, StatusCode::OK, "{billing}");
    assert_eq!(billing["subscription"]["plan"]["id"], "free");
    assert!(billing["invoices"].as_array().unwrap().is_empty());

    // Individual plans are not offered to institutions.
    let (st, _) = s.post("/v1/billing/subscription", json!({ "plan_id": "developer_plus" })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // Enterprise is operator-assigned.
    let (st, err) = s.post("/v1/billing/subscription", json!({ "plan_id": "enterprise" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "plan_not_self_service");

    // The retired per-mailbox tier is no longer sold.
    let (st, _) = s.post("/v1/billing/subscription", json!({ "plan_id": "starter" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // First paid plan starts a one-time trial (no invoice yet).
    let (st, r) = s.post("/v1/billing/subscription", json!({ "plan_id": "pro" })).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["outcome"], "trial_started");
    assert!(r["invoice"].is_null());
    let (_, b) = s.get("/v1/billing").await;
    assert_eq!(b["subscription"]["status"], "trialing");
    assert_eq!(b["subscription"]["plan"]["id"], "pro");
    assert_eq!(b["subscription"]["plan"]["name"], "Premium");

    // Trial ends: renewal issues the first invoice (flat KES 1,200 + 16% VAT in tests).
    sqlx::query("UPDATE subscriptions SET current_period_start = now() - interval '14 days', trial_ends_at = now() - interval '1 second', current_period_end = now() - interval '1 second' WHERE organization_id = $1::uuid")
        .bind(&s.org_id)
        .execute(&app.db)
        .await
        .unwrap();
    let renewed = mailvora::services::billing::renew_due(&app.state).await.unwrap();
    assert_eq!(renewed, 1);
    // A second pass is idempotent.
    assert_eq!(mailvora::services::billing::renew_due(&app.state).await.unwrap(), 0);
    let (_, b) = s.get("/v1/billing").await;
    assert_eq!(b["subscription"]["status"], "active");
    let invoices = b["invoices"].as_array().unwrap();
    assert_eq!(invoices.len(), 1, "{b}");
    assert_eq!(invoices[0]["subtotal_cents"], 120000);
    assert_eq!(invoices[0]["tax_cents"], 19200);
    assert_eq!(invoices[0]["total_cents"], 139200);
    let invoice_id = invoices[0]["id"].as_str().unwrap().to_string();
    let (st, inv) = s.get(&format!("/v1/billing/invoices/{invoice_id}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(inv["lines"][0]["amount_cents"], 120000);
    assert_eq!(inv["currency"], "KES");
    // The billing contact got an email (queued as a system email).
    let mails: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_jobs WHERE kind = 'system_email' AND payload->>'subject' LIKE 'Invoice%'",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(mails, 1);

    // Downgrades are scheduled for the end of the paid period.
    let (st, r) = s.post("/v1/billing/subscription", json!({ "plan_id": "free" })).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["outcome"], "scheduled");
    let (_, b) = s.get("/v1/billing").await;
    assert_eq!(b["subscription"]["plan"]["id"], "pro");
    assert_eq!(b["subscription"]["pending_plan"]["id"], "free");

    // Another tenant cannot read the invoice.
    let other = app.register("other@school.example").await;
    let (st, _) = other.get(&format!("/v1/billing/invoices/{invoice_id}")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // The operator records the payment.
    sqlx::query("UPDATE users SET is_platform_admin = true WHERE id = $1::uuid")
        .bind(&other.user_id)
        .execute(&app.db)
        .await
        .unwrap();
    let (st, r) = other
        .post(
            &format!("/v1/admin/invoices/{invoice_id}/mark-paid"),
            json!({ "method": "M-Pesa", "reference": "QK12ABC345" }),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (_, inv) = s.get(&format!("/v1/billing/invoices/{invoice_id}")).await;
    assert_eq!(inv["status"], "paid");
    assert_eq!(inv["payment_reference"], "QK12ABC345");
    app.cleanup().await;
}

#[tokio::test]
async fn support_staff_assist_employees_without_touching_passwords() {
    let app = app!();
    let owner = app.register("ict-head@uni.example").await;
    app.verify_email(&owner).await;
    let domain = hosting_domain(&app, &owner, "uni.example").await;
    let (st, mbx) = owner
        .post(
            "/v1/mailboxes",
            json!({ "domain_id": domain, "local_part": "lecturer", "display_name": "A Lecturer",
                                      "setup_method": "password", "password": "a very long initial passphrase",
                                      "recovery_email": "lecturer.home@gmail.com", "department": "Physics" }),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{mbx}");
    let id = mbx["id"].as_str().unwrap().to_string();
    assert_eq!(mbx["department"], "Physics");

    let support = app.register("helpdesk@uni.example").await;
    add_member(&app, &owner, &support, "support").await;
    // Support sees masked recovery addresses only.
    let (st, seen) = support.get(&format!("/v1/mailboxes/{id}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(seen["recovery_email"], "l*****@gmail.com");
    // Allowed: email a reset link to the employee, unlock.
    let (st, r) = support.post(&format!("/v1/mailboxes/{id}/send-reset"), json!({})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["reset"]["method"], "email");
    assert!(r["reset"]["url"].is_null(), "support never receives the link");
    let (st, _) = support.post(&format!("/v1/mailboxes/{id}/unlock"), json!({})).await;
    assert_eq!(st, StatusCode::OK);
    // Forbidden: handover links, setting passwords, provisioning, changing the recovery address.
    let (st, _) = support.post(&format!("/v1/mailboxes/{id}/send-reset"), json!({ "method": "link" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = support.post(&format!("/v1/mailboxes/{id}/reset-password"), json!({})).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = support.post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "x2" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) =
        support.patch(&format!("/v1/mailboxes/{id}"), json!({ "recovery_email": "attacker@example.org" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = support.post("/v1/api-keys", json!({ "name": "x" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // Every assist action is audited.
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_logs WHERE organization_id = $1::uuid AND actor_id = $2::uuid ORDER BY id",
    )
    .bind(&owner.org_id)
    .bind(&support.user_id)
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(actions, vec!["mailbox.reset_link_sent", "mailbox.unlocked"]);
    app.cleanup().await;
}

#[tokio::test]
async fn activation_links_are_single_use_and_pending_mailboxes_cannot_sign_in() {
    let app = app!();
    let owner = app.register("it@college.example").await;
    app.verify_email(&owner).await;
    let domain = hosting_domain(&app, &owner, "college.example").await;
    let (st, err) = owner
        .post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "nurse", "setup_method": "invite" }))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "invite needs a personal address: {err}");
    let (st, m) = owner
        .post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "nurse", "display_name": "Nurse Wanjiku", "setup_method": "link" }))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{m}");
    assert_eq!(m["status"], "pending");
    assert!(m["password"].is_null());
    let token = token_from(m["activation"]["url"].as_str().unwrap());

    // Not usable before activation, and not visible to the mail server.
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_mailboxes WHERE address = 'nurse@college.example'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(live, 0);
    let c = app.client();
    let res = c
        .post(format!("{}/v1/webmail/login", app.base))
        .json(&json!({ "address": "nurse@college.example", "password": "!pending-activation" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let view: Value =
        c.get(format!("{}/v1/webmail/activation?token={token}", app.base)).send().await.unwrap().json().await.unwrap();
    assert_eq!(view["address"], "nurse@college.example");
    assert_eq!(view["purpose"], "activate");
    let weak = c
        .post(format!("{}/v1/webmail/activate", app.base))
        .json(&json!({ "token": token, "password": "short" }))
        .send()
        .await
        .unwrap();
    // A policy failure rolls back the transaction, so the link stays usable.
    assert_eq!(weak.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ok = c
        .post(format!("{}/v1/webmail/activate", app.base))
        .json(&json!({ "token": token, "password": "my own chosen passphrase" }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK, "{}", ok.text().await.unwrap_or_default());
    let again = c
        .post(format!("{}/v1/webmail/activate", app.base))
        .json(&json!({ "token": token, "password": "another passphrase here" }))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::BAD_REQUEST, "links work exactly once");
    let res = c
        .post(format!("{}/v1/webmail/login", app.base))
        .json(&json!({ "address": "nurse@college.example", "password": "my own chosen passphrase" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Self-service forgot-password never reveals whether a mailbox exists.
    for address in ["nurse@college.example", "nobody@college.example"] {
        let r = app
            .client()
            .post(format!("{}/v1/webmail/password/forgot", app.base))
            .json(&json!({ "address": address }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::ACCEPTED);
    }
    app.cleanup().await;
}

#[tokio::test]
async fn api_keys_honour_ip_and_sender_domain_restrictions() {
    let app = app!();
    let s = app.register("devops@shop.example").await;
    app.verify_email(&s).await;
    let shop = app.verified_domain(&s, "shop.example").await;
    let _ = app.verified_domain(&s, "news.shop-mail.example").await;
    let (st, k) = s
        .post("/v1/api-keys", json!({ "name": "orders", "scopes": ["messages:send"], "allowed_ips": ["10.0.0.0/8"],
                                     "allowed_sender_domains": ["shop.example"], "description": "Order confirmations" }))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{k}");
    let key = k["key"].as_str().unwrap().to_string();
    let id = k["id"].as_str().unwrap().to_string();
    let body = |from: &str| json!({ "from": from, "to": "buyer@example.org", "subject": "Order", "text": "Thanks" });

    let (st, err) = app.with_key(&key, Method::POST, "/v1/messages", Some(body("orders@shop.example"))).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "api_key_ip_not_allowed");

    let (st, k2) = s.patch(&format!("/v1/api-keys/{id}"), json!({ "allowed_ips": ["127.0.0.1"] })).await;
    assert_eq!(st, StatusCode::OK, "{k2}");
    assert_eq!(k2["allowed_ips"][0], "127.0.0.1/32");
    let (st, err) = app.with_key(&key, Method::POST, "/v1/messages", Some(body("promo@news.shop-mail.example"))).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(err["error"]["code"], "sender_domain_not_allowed");
    let (st, ok) = app.with_key(&key, Method::POST, "/v1/messages", Some(body("orders@shop.example"))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{ok}");

    // Restrictions survive rotation; unknown domains and /0 ranges are rejected.
    let (st, rotated) = s.post(&format!("/v1/api-keys/{id}/rotate"), json!({})).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(rotated["allowed_sender_domains"][0], "shop.example");
    let (st, _) = s.post("/v1/api-keys", json!({ "name": "x", "allowed_sender_domains": ["not-mine.example"] })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = s.post("/v1/api-keys", json!({ "name": "x", "allowed_ips": ["0.0.0.0/0"] })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let _ = shop;
    app.cleanup().await;
}

#[tokio::test]
async fn scheduled_messages_cancel_and_one_click_unsubscribe() {
    let app = app!();
    let s = app.register("news@club.example").await;
    app.verify_email(&s).await;
    app.verified_domain(&s, "club.example").await;
    let key = app.api_key(&s, "live", &["messages:send", "messages:read"]).await;
    let at = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let (st, r) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "news@club.example", "to": "member@example.org", "subject": "Later", "text": "Hi", "send_at": at })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{r}");
    assert_eq!(r["messages"][0]["status"], "scheduled");
    let mid = r["messages"][0]["id"].as_str().unwrap().to_string();
    let (st, _) = app.with_key(&key, Method::POST, &format!("/v1/messages/{mid}/cancel"), Some(json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    let (_, m) = app.with_key(&key, Method::GET, &format!("/v1/messages/{mid}"), None).await;
    assert_eq!(m["status"], "cancelled");
    let (st, _) = app.with_key(&key, Method::POST, &format!("/v1/messages/{mid}/cancel"), Some(json!({}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
    let (st, _) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "news@club.example", "to": "m@example.org", "subject": "x", "text": "x", "send_at": "2001-01-01T00:00:00Z" })))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // List-Unsubscribe headers are signed into the stored MIME.
    let (st, r) = app
        .with_key(&key, Method::POST, "/v1/messages", Some(json!({ "from": "news@club.example", "to": "reader@example.org", "subject": "Newsletter", "text": "News", "unsubscribe": true })))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let id: uuid::Uuid = r["messages"][0]["id"].as_str().unwrap().parse().unwrap();
    let raw: Vec<u8> = sqlx::query_scalar("SELECT raw_mime FROM message_contents WHERE message_id = $1")
        .bind(id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let raw = String::from_utf8_lossy(&raw);
    assert!(raw.contains("List-Unsubscribe-Post: List-Unsubscribe=One-Click"));
    let unfolded: String = raw.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    assert!(unfolded.contains("list-unsubscribe:list-unsubscribe-post"), "both headers are DKIM-signed");

    let token = mailvora::services::unsubscribe::token(&app.state.cfg, id);
    let view: Value = reqwest::get(format!("{}/v1/unsubscribe/{token}", app.base)).await.unwrap().json().await.unwrap();
    assert_eq!(view["unsubscribed"], false);
    let res = reqwest::Client::new().post(format!("{}/v1/unsubscribe/{token}", app.base)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let reason: String = sqlx::query_scalar("SELECT reason FROM suppressions WHERE email = 'reader@example.org'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(reason, "unsubscribe");
    let bad =
        reqwest::Client::new().post(format!("{}/v1/unsubscribe/AAAA{}", app.base, &token[4..])).send().await.unwrap();
    assert_eq!(bad.status(), StatusCode::NOT_FOUND, "tampered tokens are rejected");

    // CSV export carries the rows.
    let res = reqwest::Client::new()
        .get(format!("{}/v1/messages/export.csv", app.base))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers()["content-type"].to_str().unwrap().starts_with("text/csv"));
    assert!(res.text().await.unwrap().contains("reader@example.org"));
    app.cleanup().await;
}

#[tokio::test]
async fn client_settings_profiles_autoconfig_and_onboarding_api() {
    let app = app!();
    let s = app.register("it@hospital.example").await;
    app.verify_email(&s).await;
    let domain = hosting_domain(&app, &s, "hospital.example").await;
    let (st, m) = s
        .post("/v1/mailboxes", json!({ "domain_id": domain, "local_part": "dr.otieno", "setup_method": "password", "password": "doctors passphrase 2026" }))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{m}");

    // Autoconfig answers only for hosted domains.
    let x = reqwest::get(format!(
        "{}/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress=dr.otieno@hospital.example",
        app.base
    ))
    .await
    .unwrap();
    assert_eq!(x.status(), StatusCode::OK);
    let xml = x.text().await.unwrap();
    assert!(xml.contains("<hostname>mail.mailvora.test</hostname>") && xml.contains("<port>993</port>"));
    let x = reqwest::get(format!("{}/mail/config-v1.1.xml?emailaddress=a@unknown.example", app.base)).await.unwrap();
    assert_eq!(x.status(), StatusCode::NOT_FOUND);
    let ad = reqwest::Client::new()
        .post(format!("{}/autodiscover/autodiscover.xml", app.base))
        .body("<Autodiscover><Request><EMailAddress>dr.otieno@hospital.example</EMailAddress></Request></Autodiscover>")
        .send()
        .await
        .unwrap();
    assert_eq!(ad.status(), StatusCode::OK);
    assert!(ad.text().await.unwrap().contains("<Type>IMAP</Type>"));

    // Only operators manage profiles; changes are versioned and audited.
    let (st, _) = s.put_json("/v1/admin/mail-client-settings", json!({})).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    sqlx::query("UPDATE users SET is_platform_admin = true WHERE id = $1::uuid")
        .bind(&s.user_id)
        .execute(&app.db)
        .await
        .unwrap();
    let profile = json!({ "imap_host": "imap.platform.example", "imap_port": 993, "smtp_host": "smtp.platform.example",
                          "smtp_port": 465, "smtp_security": "tls", "smtp_alt_port": 587, "smtp_alt_security": "starttls",
                          "change_note": "Dedicated hostnames" });
    let (st, r) = s.put_json("/v1/admin/mail-client-settings", profile.clone()).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["version"], 1);
    let (_, r) = s.put_json("/v1/admin/mail-client-settings", profile).await;
    assert_eq!(r["version"], 2);
    let (st, err) = s.put_json("/v1/admin/mail-client-settings", json!({ "imap_host": "imap.platform.example", "imap_port": 993, "smtp_host": "x.example", "smtp_port": 25, "smtp_security": "none" })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "plaintext submission refused: {err}");
    let (st, r) = s
        .put_json(&format!("/v1/admin/mail-client-settings/domains/{domain}"), json!({ "imap_host": "mail.hospital.example", "imap_port": 993, "smtp_host": "mail.hospital.example", "smtp_port": 465, "smtp_security": "tls" }))
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (_, all) = s.get("/v1/admin/mail-client-settings").await;
    assert_eq!(all["default"]["imap"]["host"], "imap.platform.example");
    assert_eq!(all["overrides"].as_array().unwrap().len(), 1);
    assert_eq!(all["history"].as_array().unwrap().len(), 3);

    // The mailbox user sees the domain's branded endpoint.
    let c = app.client();
    let login: Value = c
        .post(format!("{}/v1/webmail/login", app.base))
        .json(&json!({ "address": "dr.otieno@hospital.example", "password": "doctors passphrase 2026" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let csrf = login["csrf_token"].as_str().unwrap().to_string();
    let setup: Value = c.get(format!("{}/v1/webmail/setup", app.base)).send().await.unwrap().json().await.unwrap();
    assert_eq!(setup["settings"]["imap"]["host"], "mail.hospital.example");
    assert_eq!(setup["settings"]["source"], "domain");
    assert_eq!(setup["verification"]["state"], "not_verified");

    // Analytics accept only known, content-free events.
    for (body, expected) in [
        (json!({ "event": "client_selected", "device": "android", "client": "gmail" }), StatusCode::ACCEPTED),
        (json!({ "event": "help_viewed", "client": "gmail", "detail": "certificate_warning" }), StatusCode::ACCEPTED),
        (json!({ "event": "connection_verified" }), StatusCode::UNPROCESSABLE_ENTITY),
        (json!({ "event": "help_viewed", "detail": "my password is hunter2" }), StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let r = c
            .post(format!("{}/v1/webmail/setup/events", app.base))
            .header("x-csrf-token", &csrf)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), expected, "{body}");
    }
    // Outgoing check issues a code; without mail storage the check reports unavailable (no false success).
    let r: Value = c
        .post(format!("{}/v1/webmail/setup/verify/outgoing", app.base))
        .header("x-csrf-token", &csrf)
        .json(&json!({ "client": "gmail" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["code"].as_str().unwrap().len(), 8);
    assert_eq!(r["send_to"], "dr.otieno@hospital.example");
    let r = c
        .post(format!("{}/v1/webmail/setup/verify/outgoing/check", app.base))
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    let r = c.get(format!("{}/v1/webmail/setup/apple-profile", app.base)).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["content-type"], "application/x-apple-aspen-config");
    let profile = r.text().await.unwrap();
    assert!(profile.contains("mail.hospital.example") && !profile.contains("doctors passphrase"));

    // The organization's onboarding report aggregates the events.
    let (st, report) = s.get("/v1/mailboxes/onboarding").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(report["by_client"][0]["client"], "gmail");
    assert_eq!(report["help_topics"][0]["topic"], "certificate_warning");
    app.cleanup().await;
}
