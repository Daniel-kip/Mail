**Mailvora** is a Rust control plane (REST API and background workers) behind a server-rendered PHP web application, with mail transport and storage delegated to Postfix, Dovecot and Rspamd. This page describes the components, the outbound and inbound mail flows, tenancy and the data model.

## Principles (blueprint 01)

1. **Mature mail software for protocols.** Postfix handles SMTP, Dovecot IMAP,
   LMTP, SASL and quota, and Rspamd filtering. The Rust service is the control
   plane only.
2. **The server is the security boundary.** Every tenant-owned operation is
   authorized in Rust. The browser receives no secrets and its permission
   checks only affect what the UI shows.
3. **Persist before acknowledging.** A message row and its outbox job commit
   in one transaction before the API returns `202`.
4. **Modular monolith.** One binary runs the API and the workers (`serve`). The
   same binary can run them separately (`api`, `worker`) when load warrants it.

## Components

| Component | Responsibility |
|-----------|----------------|
| Nginx | TLS, HSTS, request/rate limits, blocks `/internal/` |
| `mailvora serve` | Rust REST API (`/v1`), background workers and health endpoints |
| PHP frontend | Page shell, browser session, CSRF checks and same-origin `/v1` bridge to Rust |
| PostgreSQL 16 | All metadata; read-only `mail_*` views for the mail tier |
| Postfix | MX (25), client submission (587/465), private platform relay (10587), bounce pipe |
| Dovecot | IMAP(S), LMTP delivery, SASL for Postfix, quota policy, webmail master user |
| Rspamd + Redis | Spam/authentication scoring via milter, Bayes storage |

### Backend modules (`backend/src`)

| Module | Purpose |
|--------|---------|
| `config` | Typed environment config; production validation refuses unsafe settings |
| `auth` | Sessions (cookie + CSRF), API keys, `Principal`, roles and scopes, webmail sessions |
| `api/*` | One file per resource; every handler calls `Principal::require(permission)` |
| `services/sending` | Shared send pipeline: quota reservation, suppressions, DKIM signing, atomic insert |
| `services/events` | Message timeline and webhook fan-out, inside the caller's transaction |
| `services/{dns,dkim,templates,sanitize,imap,webhooks,totp,audit,system_mail}` | Single-purpose services |
| `mail/{mime,smtp,bounce}` | MIME building, relay submission with 4xx/5xx classification, VERP and DSN parsing |
| `workers` | Outbox claiming (`SKIP LOCKED` + leases), send/webhook/system-mail handlers, maintenance |

## Outbound flow

1. The client calls `POST /v1/messages` with `Authorization: Bearer mv_live_…`.
2. The `Principal` extractor finds the key by its public prefix, compares
   SHA-256 in constant time, checks revocation, expiry and tenant status, and
   applies the per-key rate limit.
3. The handler validates the sender domain (verified and owned by this
   tenant), recipients, reply policy, template variables, attachments and
   headers.
4. It builds and DKIM-signs one MIME message per recipient. The envelope
   sender is a VERP address `b+<id>-<hmac>@<return-path-domain>`.
5. In one transaction it inserts the idempotency record, takes an advisory
   lock and reserves quota, checks suppressions, and writes the messages,
   contents, `queued` events, webhook deliveries and outbox jobs. Then it
   commits and returns `202`.
6. A trigger runs `NOTIFY outbox_jobs` to wake a worker, which claims jobs
   round-robin across tenants.
7. The worker submits to Postfix :10587 as the relay identity. A 2xx sets
   `accepted_by_mta`, a 4xx sets `deferred` and retries with backoff, and a
   5xx sets `failed`.
8. Postfix delivers to remote MX hosts and retries there itself. A failure
   produces a DSN to the VERP address. Postfix pipes it to
   `/internal/v1/bounces`, which authenticates the HMAC tag, marks the message
   `bounced`, and suppresses the address.

`202` means accepted for processing. `accepted_by_mta` means our relay took
responsibility. Neither means inbox placement (blueprint 03).

## Inbound flow

1. The recipient domain's MX points at `MAIL_HOSTNAME`. Postfix accepts mail
   only for domains in `mail_domains`. Unknown recipients are rejected during
   SMTP via `reject_unlisted_recipient` and `mail_mailboxes` /
   `mail_aliases`, so no backscatter is generated.
2. Dovecot's quota-status policy rejects over-quota recipients at RCPT time
   with `552 5.2.2`.
3. The Rspamd milter adds `Authentication-Results` and spam headers, and
   rejects at score 15 or above.
4. LMTP delivers to Dovecot. The maildir lives at
   `/var/vmail/<domain>/<mailbox-uuid>/Maildir`, keyed by mailbox id so a
   recycled address never inherits old mail.
5. Users read mail in webmail. The API verifies their password, then opens
   IMAP as `address*webmail`, the Dovecot master user. They can also use any
   IMAP client on port 993.

## Tenancy and authorization

- The tenant is **always** derived from the credential: the session's active
  membership (re-read on every request, so role changes apply immediately) or
  the API key's organization. No endpoint accepts a caller-supplied
  organization id for scoping.
- Every query includes `organization_id = $principal`. Composite foreign keys
  such as `(domain_id, organization_id)` make cross-tenant references
  impossible at the schema level.
- Roles are owner, admin, developer and viewer, mapped to permissions in
  `auth/permissions.rs`. API keys carry explicit scopes from a grantable subset.
  Key, member and organization management always require a human session.
- Suspending a tenant blocks API keys and write permissions, halts queued
  mail, ends webmail sessions, and removes the tenant from every `mail_*`
  view. That stops inbound acceptance and IMAP/SMTP logins at once. The API
  also calls Dovecot's doveadm HTTP API to flush cached authentications and
  kick open sessions, so disablement takes effect immediately (D-022).

## Data model (summary)

`plans` → `organizations` ← `memberships` → `users` · `invitations` ·
`sessions` (console and webmail) · `audit_logs` · `domains` (DKIM key sealed
with AES-GCM) · `api_keys` (SHA-256) · `templates` → `template_versions`
(immutable) · `messages` → `message_contents` (purged by retention) and
`message_events` · `idempotency_keys` · `usage_daily` · `suppressions` ·
`webhook_endpoints` → `webhook_deliveries` · `outbox_jobs` · `mailboxes`
(Dovecot-compatible Argon2id) · `aliases`.

Views for the mail tier (role `mv_mailserver`, `SELECT` only):
`mail_domains`, `mail_mailboxes`, `mail_aliases`, `mail_sender_logins`.

## Frontend

The independently deployable frontend is PHP 8.2+ with cURL. PHP renders the
page shell, owns the browser session and exposes a same-origin `/v1` bridge;
the Rust API URL is configured only on the PHP server. The browser never gets
the Rust session cookie or backend URL. In production, host Nginx routes
`app.example.com` to the PHP frontend and `api.example.com` to Rust for scoped
API-key clients. Plain ES modules provide the console and webmail UI. The DOM
helper inserts API data as text, and untrusted email HTML is sanitized and
shown in sandboxed iframes without scripts.

## Scaling path

The design runs on one host and scales in these steps, each taken when metrics
show the need:

1. Split processes: `mailvora api` behind a load balancer, plus N
   `mailvora worker`. The queue is safe for concurrent workers. Replace the
   in-memory rate limiter with a shared store first (D-010).
2. Move inbound and outbound to separate Postfix hosts, and add dedicated
   sending IPs per traffic class.
3. Move message bodies from `message_contents` to object storage, keeping
   references in Postgres.
4. Add PostgreSQL read replicas for the log and analytics endpoints.

## See also

- [Web application](Web-Application.md)
- [Sending API](Sending-API.md)
- [Security model](Security-Model.md)
- [Decision log](Decision-Log.md)
