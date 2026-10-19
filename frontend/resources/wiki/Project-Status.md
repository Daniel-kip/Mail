**Project status** maps the codebase to the implementation blueprint (documents 00–09) and records the verification performed for each release.

How this codebase maps to the implementation blueprint (documents 00–09).
"Definition of done" per blueprint 09 means code, migrations, tests,
configuration, monitoring, rollback notes and operator docs.

## Phase 0: decisions and environment

| Item | Status |
|------|--------|
| Decision log | Done: [Decision log](Decision-Log.md) (D-001 … D-020) plus a per-deployment checklist |
| Project layout, secrets policy, release checks | Done: `.env.example`, secrets only via environment; one-command local start and test scripts; release checklist in OPERATIONS §4 |
| Staging/production separation | Documented. Host provisioning is per deployment |
| Host, port 25, PTR confirmation | **Operator action.** Checklist in OPERATIONS §1 |

## Phase 1: application foundation

| Item | Where |
|------|-------|
| Axum service, typed config with production validation, health and readiness | `config.rs`, `api/health.rs` |
| Structured logs (JSON), request ids in every error body | `http.rs`, `error.rs` |
| Versioned SQL migrations; separate migration and runtime roles | `migrations/`, `infra/postgres/init/01-roles.sh` |
| Users, organizations, memberships, roles, invitations | `api/auth.rs`, `api/organizations.rs` |
| Sessions (HttpOnly cookie, CSRF), lockout, password reset, email verification, TOTP MFA | `auth/`, `api/auth.rs`, `services/totp.rs` |
| API keys (hashed, scoped, expiring, revocable, rotatable) | `auth/api_key.rs`, `api/api_keys.rs` |
| Audit log | `services/audit.rs`, `api/usage.rs` |
| Tenant-isolation tests | `tests/api.rs` |

## Phase 2: domains and templates

| Item | Where |
|------|-------|
| Domain registration, ownership TXT, DKIM key generation, live DNS checks with observations | `api/domains.rs`, `services/dns.rs`, `services/dkim.rs` |
| Sender authorization (verified domain owned by the tenant) | `services/sending.rs` |
| Versioned templates, variable schema, strict rendering, sanitization, preview, rollback | `api/templates.rs`, `services/templates.rs`, `services/sanitize.rs` |
| Quotas, rate limits, suppressions | `services/sending.rs`, `ratelimit.rs`, `api/suppressions.rs` |
| Development SMTP sink | Mailpit in `infra/docker-compose.dev.yml` |

## Phase 3: outbound delivery

| Item | Where |
|------|-------|
| Postfix without open relay; authenticated private relay | `infra/postfix/` |
| Transactional outbox, lease-based workers, crash recovery, tenant fairness | `workers/queue.rs` |
| Idempotent message API, per-recipient status | `api/messages.rs` |
| Envelope sender (VERP), Reply-To policy, bounce correlation and events | `mail/bounce.rs`, `api/internal.rs` |
| Signed webhooks with retries and SSRF protection | `services/webhooks.rs`, `workers/webhook.rs` |
| SPF/DKIM/DMARC alignment, TLS, reverse DNS | DKIM in app; DNS and TLS are **operator actions** (OPERATIONS §2–3, §5) |

## Phase 4: mailboxes

| Item | Where |
|------|-------|
| Inbound Postfix with SQL recipient validation, Rspamd milter | `infra/postfix/`, `infra/rspamd/` |
| Dovecot SQL auth (Argon2id), LMTP, quotas enforced at SMTP, special-use folders | `infra/dovecot/` |
| Mailbox provisioning, quotas, disablement, aliases (tenant-internal) | `api/mailboxes.rs`, migration 0003 views |
| IMAPS and authenticated submission (587/465) with sender-login checks | `infra/postfix/master.cf.template`, `infra/dovecot/` |
| External receiving and restore tests | **Operator action** on the production host (OPERATIONS §5, §7) |

## Phase 5: console and webmail

| Item | Where |
|------|-------|
| PHP frontend and same-origin bridge; console: overview, domains/DNS, mailboxes, API keys, templates, messages and events, suppressions, webhooks, members, settings/MFA/sessions, audit log, API quickstart | `frontend/src/`, `frontend/public/assets/js/console/` |
| Operator console: platform health, tenant suspension, plan changes, dead-job requeue | `console/pages/admin.js`, `api/admin.rs` |
| Webmail: folders, search, reading (sanitized, sandboxed, remote content blocked), attachments, compose/reply/forward, drafts, keyboard shortcuts, password change | `frontend/public/assets/js/webmail/app.js`, `api/webmail.rs` |
| Accessible, responsive states | Focus management, keyboard navigation, `aria-*`, mobile layouts in `app.css` |

## Phase 6: production hardening

| Item | Status |
|------|--------|
| Security release gates | See [Security model](Security-Model.md): automated gates in the test suite; operational gates listed |
| Monitoring | `/internal/v1/metrics` (Prometheus) plus the alert table in OPERATIONS §6 |
| Encrypted backups and restore drills | Procedure in OPERATIONS §7. **Operator action** |
| Abuse handling and policies | Suspension tooling built; publishing abuse, privacy and retention policies is an **operator action** |
| Small authorized pilot, gradual volume increase | **Operator action** after go-live checks |

## Phase 7: products, onboarding and PHP frontend (October 2026)

- Account types (individual developer / institution), plans with trials,
  prorated upgrades, invoices with tax and payment instructions, operator
  receivables (D-027).
- Mailbox lifecycle: invitations, activation links and QR handover, single-use
  reset links, unlock, recovery email, departments, CSV import, welcome
  sheets, phone setup report; ICT support role (D-028).
- API keys restricted by IP and sender domain; scheduled sends with cancel;
  one-click unsubscribe (D-030); CSV exports for messages and audit.
- Mail-client settings profiles (platform + per-domain, versioned),
  Thunderbird autoconfig, Outlook Autodiscover, Apple profiles, optional
  RFC 6186 records (D-026).
- Guided phone onboarding with readiness checks and real incoming/outgoing
  connection tests (D-032); webmail out-of-office via Sieve (D-031),
  contacts, directory, signatures, recipient suggestions.
- Server-rendered PHP 8.2+ frontend, no runtime dependencies: public site,
  console, operator console, webmail, setup wizard (D-029).

Verified 14 October 2026 on Windows 11 with Docker Desktop: cargo fmt,
clippy (`-D warnings`) and the Rust unit + integration suites; 49 PHP
unit/feature tests; API smoke and real-Dovecot mailbox end-to-end scripts;
the seeding script (exercises activation link, invitation, support role,
trial); a 13-step headless-Chrome journey with no JavaScript or CSP errors;
out-of-office installed and removed through ManageSieve on the real Dovecot.
Not tested: physical phones (the local certificate is self-signed), delivery
to Gmail/Outlook, Postfix mail tier in this round.

## Phase 8: payments, verification, tools and security hardening (October 2026)

- Brand: Mailvora logo refined into transparent PNG, vector mark, favicons,
  PWA icons and social preview; blue design system across every page (replaced by the emerald theme in Phase 9).
- Landing: hero with the owner's global-network video (pause control,
  reduced-motion aware) and an animated send simulation section.
- Accounts: username + phone sign-up with SMS codes; sign-in by email,
  username or phone; Continue with Google (OIDC + PKCE) (D-034).
- Billing: flat KES pricing; M-Pesa STK push through PesaTide with signed
  webhooks and status reconciliation (D-033).
- Tools hub: domain health score, DNS lookup, record generator, header
  analyzer, blocklist and port checker (D-036).
- Security: breached-password screening, security alert emails, operator
  security dashboard, hardened containers, encrypted backups, security.txt,
  readiness mapping in [Security readiness](Security-Readiness.md) (D-035, D-037).

## Phase 9: Mailvora identity, sealed links and wiki (October 2026)

- Renamed EmailHoster to Mailvora across code, infrastructure and data (D-039).
- Sealed, session-bound `/app?ref=…` addresses for every signed-in page (D-038).
- Refined emerald theme replacing the Phase 8 blue; endless, control-free hero video;
  "Powered by TECHCORE SOLUTIONS" in the site footer (D-040).
- Documentation reorganised into this wiki (D-041).

## Verification performed

All of the following were run against this codebase on a local stack:
PostgreSQL 16, Mailpit, and the real Postfix 3.7, Dovecot 2.3.19 and
Rspamd images from `infra/`.

| Layer | How | Result |
|-------|-----|--------|
| Unit tests | `cargo test --lib` (validation, crypto, sanitizer, templates, DKIM, DSN parsing, VERP, TOTP, SSRF ranges, QR roles…) | 40 passed |
| Integration (API + Postgres + worker) | `cargo test --test api` with `TEST_DATABASE_URL`: tenant isolation, CSRF, roles, key scopes/revocation/rotation, idempotency, header injection, templates, suppressions, quotas, worker → relay, bounce ingestion, SSRF, mailbox views, suspension | 12 passed |
| Lint / format | `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | clean |
| End-to-end HTTP | `infra/scripts/smoke.sh`: register → verification mail via outbox → domain → key → idempotent send → relay | all passed |
| DKIM interoperability | `tests/verify_dkim.py` (dkimpy) on text, alternative and mixed messages, plus a message relayed end to end | pass (after fixing D-023) |
| Browser UI | `frontend/tests/ui_smoke.py` (headless Chrome via DevTools): registration, email confirmation, domain verify, API key shown once, template live preview, compose + status updates, overview chart, all console pages, MFA QR, sign-out/deep-link sign-in; fails on any JS error | 11/11, no JS errors |
| Frontend modules | All 26 ES modules imported in headless Chrome | all parse and link |
| QR encoder | Python port compared bit for bit with `segno` for versions 1/8/13/32 × all 8 masks | identical |
| Hosted mailboxes + webmail | `infra/scripts/mailbox_e2e.py` against real Dovecot: provisioning, LMTP delivery, IMAP login (SQL passdb, Argon2id), special-use folders, sanitized reading, cid inlining, remote-image blocking, attachments, search, reply → relay + Sent copy + \Answered, drafts, move/delete/Trash, quota, isolation, immediate disable over webmail and IMAP | 36/36 |
| Mail tier | `infra/scripts/mail_tier_e2e.py` against real Postfix + Dovecot + Rspamd: no open relay, unknown recipients rejected in SMTP, delivery via LMTP, alias expansion, Rspamd headers, submission requires STARTTLS + AUTH, sender-login mismatch refused, relay port accepts only the platform identity | 16/16 |
| Bounce cycle | API send through the real Postfix relay → null-MX destination → real DSN → VERP → pipe → `/internal/v1/bounces` | `queued → accepted → bounced`, hard-bounce suppression, later send `suppressed` |

Defects found and fixed during verification: lettre DKIM signing of
Content-Type (D-023), SPA routes returning 404 status, Dovecot credential
directory permissions, the Dovecot auth cache delaying disablement (D-022),
the bounce pipe's URL encoding and environment, Rspamd greylisting (D-021),
drafts without recipients, IMAP special-use attribute parsing, and several
responsive-layout and CSS-specificity issues.

**Not verifiable locally (operator actions on the production host):**
reverse DNS, external open-relay test from another network, delivery to
Gmail/Outlook with header inspection, TLS certificates, backup restore drill.

## Deferred (tracked, not in the MVP)

- `application_managed` reply mode and inbound-to-webhook routing (D-020).
- External forwarding with SRS (D-013).
- Shared (Redis) rate limiter for multi-instance API (D-010).
- Object storage for message bodies; per-tenant retention and legal hold.
- Attachment antivirus (ClamAV via Rspamd), Rspamd learning from Junk moves (imapsieve).
- Payment gateway integration (invoices and manual payment recording exist; M-Pesa STK/card capture is not integrated).
- Server-sent events for live delivery updates (polling is used now, as blueprint 05 allows).

## See also

- [Decision log](Decision-Log.md)
- [Release checklist](Release-Checklist.md)
