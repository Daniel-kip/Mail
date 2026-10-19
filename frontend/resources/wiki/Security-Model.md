The **security model** describes Mailvora's threat model, its controls and the release gates that must pass before public launch.

## Threat model (blueprint 07)

Assets: private mail, sender identity and reputation, tenant isolation,
service availability.

| Threat | Primary controls | Where |
|--------|------------------|-------|
| API key theft | Keys shown once, stored as SHA-256, scoped, expirable, revocable and rotatable; `last_used_at`/IP shown; per-key rate limit | `auth/api_key.rs`, `api/api_keys.rs` |
| Credential stuffing / brute force | Argon2id; per-IP and per-account limits; lockout after 10 failures; generic errors with equalized timing; TOTP MFA | `api/auth.rs`, `password.rs`, `services/totp.rs` |
| Session theft / CSRF | 256-bit opaque cookie (`HttpOnly`, `SameSite=Lax`, `__Host-` + `Secure` in production); server-side CSRF token on every write; rotation on MFA; revocation on password change/reset/disable | `auth/session.rs`, `auth/mod.rs` |
| Tenant-ID tampering / IDOR | Tenant derived from the credential only; every query scoped; composite FKs; integration tests per resource | `auth/mod.rs`, migrations, `tests/api.rs` |
| Open relay | Port 25 has no AUTH and uses `reject_unauth_destination`; submission requires TLS + SASL + `reject_sender_login_mismatch`; internal port 10587 accepts only the relay identity via `check_sasl_access` and is never published | `infra/postfix/*` |
| Sender spoofing by tenants | Sending requires a verified domain owned by the tenant (ownership TXT); a domain can be verified by only one tenant (unique index) | `services/sending.rs`, migration 0002 |
| Header / CRLF injection | All header-bound values validated (`header_safe`); MIME built by lettre; template subjects must render to one line | `validation.rs`, `services/templates.rs` |
| HTML / script injection | Outbound HTML sanitized after rendering (ammonia); inbound mail sanitized, shown in a sandboxed iframe with its own CSP, remote content blocked by default; console CSP `script-src 'self'`; DOM helper never parses API data as HTML | `services/sanitize.rs`, `frontend/public/assets/js/lib/dom.js` |
| Malicious attachments | Executable types rejected on send; downloads always `application/octet-stream` + `attachment` disposition + `nosniff`; filenames sanitized | `api/messages.rs`, `api/webmail.rs`, `mail/mime.rs` |
| SSRF via webhooks | https only; credentials and private hosts rejected; resolver refuses private/loopback/link-local/CGNAT addresses at connect time (defeats DNS rebinding); redirects disabled; 10 s timeout | `services/webhooks.rs` |
| Webhook forgery / replay | `Mailvora-Signature: t=…,v1=HMAC-SHA256(t.body)`; receivers check the timestamp tolerance; `Mailvora-Event-Id` for de-duplication | `services/webhooks.rs` |
| Forged bounces | VERP tag is an HMAC of the message id; only envelope recipients of that message can be suppressed; internal endpoint needs a bearer token and is blocked at Nginx | `mail/bounce.rs`, `api/internal.rs` |
| Queue flooding / storage exhaustion | Per-plan daily/monthly quotas reserved under an advisory lock; recipient, size and attachment limits; body limits per route; mailbox quotas enforced at SMTP (`552`); round-robin claiming across tenants | `services/sending.rs`, Dovecot quota-status |
| Secrets at rest | DKIM keys, webhook secrets and TOTP seeds sealed with AES-256-GCM (purpose-bound AAD) under `DATA_ENCRYPTION_KEY`; session/invite/reset tokens and API keys stored as hashes | `crypto.rs` |
| Secret leakage in logs | `Config` has a redacting `Debug`; no bodies, passwords, keys or variables are logged; errors return generic messages plus a request id | `config.rs`, `error.rs` |
| Lingering mail access after disablement | doveadm auth-cache flush + session kick on disable/delete/password change/suspension; 1-minute auth cache bound | `services/doveadm.rs` |
| Privileged misuse | Operator console requires `is_platform_admin` and (in production) MFA; every operator action is audited | `auth/mod.rs` (`PlatformAdmin`), `api/admin.rs` |

## Release gates (all must pass before public launch)

Status as of this build. **Automated** gates are covered by `cargo test`
(integration suite). **Operational** gates must be performed and signed off on
the production host.

| # | Gate | How it is verified | Status |
|---|------|--------------------|--------|
| 1 | No unauthenticated open relay | `infra/scripts/mail_tier_e2e.py` against the real Postfix config (MX, submission and relay ports); repeat from an external network on the production host (OPERATIONS §5) | Verified locally; repeat externally |
| 2 | Tenant isolation | `tenant_isolation_across_resources`, `roles_limit_what_members_can_do` | Automated |
| 3 | Domain verification and sender authorization enforced server-side | `sending_rules_idempotency_and_injection`, `tenant_isolation_across_resources` | Automated |
| 4 | Revoked keys stop working | `api_keys_scopes_revocation_and_rotation` | Automated |
| 5 | Secrets do not leak to logs | Redacting `Debug`; review logs from a staging smoke test with `RUST_LOG=debug` | Operational: review once per release |
| 6 | Privileged MFA and session/CSRF controls | `session_requires_csrf_for_writes`; production `PlatformAdmin` requires MFA | Automated + config |
| 7 | HTML / attachment handling | Sanitizer unit tests, template tests, blocked extensions; `mailbox_e2e.py` confirms script/`javascript:` removal, remote-image blocking and forced attachment downloads on real received mail | Automated + verified |
| 8 | Encrypted backups restored successfully | Quarterly drill (OPERATIONS §7) | Operational |
| 9 | Monitoring and emergency suspension operational | `mailboxes_feed_mail_server_views_and_suspension_stops_everything`; immediate Dovecot revocation (D-022) verified by `mailbox_e2e.py`; metrics endpoint scraped and alerts firing in staging | Automated + operational |

Do **not** launch publicly if any critical test fails, external relay is
possible, a restore has never been tested, or abuse-response ownership is
undefined (blueprint 08).

## Hardening checklist for the host

- Unattended security upgrades for the OS. Rebuild images monthly to pick up
  Debian security updates for Postfix, Dovecot and Rspamd.
- Run `cargo audit` before each release. Block the release on unpatched advisories.
- SSH keys only, no root login, and admin access restricted by IP or VPN.
- Docker: no privileged containers. The API runs as a non-root user (uid
  10001).
- Publish `abuse@` and `postmaster@` for the platform domain and route both to
  the on-call owner.

## Known limitations (tracked)

- Rate limits are per process (D-010). Use a shared store before scaling out.
- No PostgreSQL RLS (D-017). Isolation relies on application scoping, which
  the tests cover.
- The `rsa` crate's private-key operations are not constant-time
  (RUSTSEC-2023-0071). DKIM signing happens server-side on our own messages,
  so remote timing observation is impractical. Revisit when `rsa` 0.10 is
  stable.
- Attachment malware scanning (ClamAV) is not bundled. Add it as an Rspamd
  antivirus module if general-purpose mailboxes are offered to untrusted
  senders.

## See also

- [Security readiness](Security-Readiness.md)
- [Sealed links](Sealed-Links.md)
- [Mobile setup privacy](Mobile-Setup-Privacy.md)
- [Operations](Operations.md)
