The **decision log** records every production-affecting choice, why it was made and when to revisit it.

Production-affecting choices, as blueprint 00 requires. Add new entries at the
end. Mark superseded entries instead of editing them.

| ID | Decision | Why | Revisit when |
|----|----------|-----|--------------|
| D-001 | **Modular monolith.** One binary with `serve` / `api` / `worker` modes | Simplest to operate; the queue already supports many workers | API latency or queue age SLOs are missed on one host |
| D-002 | **Transactional outbox in PostgreSQL** (`outbox_jobs`, `FOR UPDATE SKIP LOCKED`, 5-minute leases, LISTEN/NOTIFY wake-ups) | No extra broker; atomic with business writes; crash recovery through lease expiry | Throughput exceeds what one Postgres primary can sustain |
| D-003 | **At-least-once delivery.** A crash between relay acceptance and the status update can resend once; the stable `Message-ID` lets receivers deduplicate | Exactly-once over SMTP is not achievable; this is the industry norm | Never; documented behaviour |
| D-004 | **Verification = ownership TXT + DKIM required; SPF and DMARC recommended** | Our Return-Path is on the platform's bounce domain, so SPF alignment is ours to provide; DKIM (`d=` customer domain) provides DMARC alignment | Custom return-path domains per tenant are added |
| D-005 | **Domain verification token stored in plain text** (the blueprint suggests a hash) | The customer must be able to view it to publish it, and once published it is public DNS data; secrecy adds nothing | n/a |
| D-006 | **The API signs DKIM** with a per-domain RSA-2048 key, sealed with AES-256-GCM; Rspamd signing is disabled | Multi-tenant key management stays in one place; keys can be rotated per domain | Ed25519 DKIM is verified by all major receivers (add a dual signature) |
| D-007 | **Dependencies pinned to the newest patch of the mature major line** (axum 0.8, sqlx 0.8, reqwest 0.12, rand 0.8, …) instead of the newest majors | Well-known APIs and fewer migration risks at launch; patch releases still carry security fixes | Each quarter: run `cargo outdated`/`cargo audit` and plan major upgrades |
| D-008 | **API keys hashed with SHA-256**, not a slow KDF | Keys carry about 238 bits of entropy, so brute force is infeasible; fast verification on every request | n/a |
| D-009 | **Human passwords use Argon2id** m=19 MiB, t=2, p=1 (OWASP minimum); mailbox hashes carry a `{ARGON2ID}` prefix for Dovecot | One verifier for webmail, IMAP and SMTP; p=1 is required by libsodium | Hardware improves; raise m/t (existing hashes still verify) |
| D-010 | **In-process fixed-window rate limiter** | Single node at launch | Before running more than one API instance: move it to Redis |
| D-011 | **Webmail uses a Dovecot master user** after the platform verifies the password itself | Mailbox passwords never need recoverable storage and are never held in the browser | n/a |
| D-012 | **One message row per recipient for API sends** | Independent status, events and suppression per recipient; no recipient lists leak in `To:` | n/a |
| D-013 | **Aliases deliver only to mailboxes of the same tenant**; no external forwarding | Forwarding loops, SPF breakage and abuse risk (blueprint 04) | Demand justifies building SRS and loop protection |
| D-014 | **Test API keys** run the full pipeline but never deliver (`sandboxed`) and count toward quota | Safe integration testing and staging, while abuse stays bounded | n/a |
| D-015 | **Frontend in plain JS modules, served by the API** | No build chain to maintain; same-origin cookies; strict `script-src 'self'` | The UI grows beyond what is maintainable without a framework |
| D-016 | **Content retention.** Message bodies are purged after `CONTENT_RETENTION_DAYS` (default 30); metadata and events are kept | Data minimisation (blueprint 07 privacy) | Legal hold requirements need per-tenant policy |
| D-017 | **PostgreSQL row-level security not enabled** at launch; isolation comes from query scoping, composite FKs and integration tests | Simpler roles; tests cover IDOR on every resource type | Before adding raw-SQL reporting or third-party DB access |
| D-018 | **Storage key = `<domain>/<mailbox uuid>`** | A deleted-then-recreated address never inherits old mail | n/a |
| D-019 | **Inline styles allowed in the console CSP** (`style-src 'unsafe-inline'`); scripts remain `'self'` only | Sandboxed email previews need inline CSS; no script execution path is opened | A style-injection risk is identified |
| D-020 | **`reply_mode` values:** `no_reply`, `reply_to_mailbox`, `reply_to_user`. `application_managed` (inbound routing to webhooks) is deferred | Needs inbound parsing routes; not required for the MVP | Customers need reply-handling APIs |
| D-021 | **Rspamd greylisting disabled** (module and action) | It defers the first message from every new sender by about 5 minutes, which breaks verification codes, password resets and admissions replies. Scoring plus reject at 15 handles spam | Spam volume shows scoring alone is insufficient |
| D-022 | **Immediate mail access revocation through Dovecot's doveadm HTTP API** (auth-cache flush + kick) on mailbox disable/delete/password change, hosting disable and tenant suspension; auth cache TTL 1 min as the fallback bound | Testing showed a disabled mailbox could still log in over IMAP for the cache TTL, and open sessions would continue | n/a |
| D-023 | **DKIM signs From, To, Cc, Subject, Date, Message-ID, Reply-To, MIME-Version, In-Reply-To, References, but not Content-Type** | lettre writes the top-level Content-Type after signing, so signing it produced signatures that failed everywhere (found by verifying with dkimpy). The body hash still covers the MIME structure | lettre signs Content-Type correctly, or signing moves to a raw-message signer |
| D-024 | **Development builds compile dependencies at opt-level 2** | Unoptimized RSA key generation and Argon2 took tens of seconds per request, which made the dev UI unusable and tests flaky | n/a |
| D-025 | **Separate PHP frontend and Rust API.** Supersedes D-015. PHP owns page rendering, browser sessions and a same-origin backend-for-frontend bridge; Rust remains authoritative for business rules, authorization, mail workflows and API-key clients. Production exposes `app.example.com` through PHP and `api.example.com` through the Rust API. | Keeps frontend and backend independently deployable while keeping backend session cookies and credentials out of browser JavaScript; API clients retain a direct HTTPS endpoint | Revisit only if deployment topology or client access requirements change |
| D-026 | **RFC 8314 defaults for mail apps:** IMAP 993 and SMTP 465 implicit TLS first, 587 STARTTLS as the alternative; settings resolved domain override → platform default (operator console, versioned) → environment | Implicit TLS avoids STARTTLS downgrade issues; one validated source feeds guides, handouts, autoconfig, Autodiscover and Apple profiles | A deployment cannot open 465 |
| D-027 | **Two account types and invoice billing.** Individual developers (relay only) and institutions (adds hosted mailboxes); plans have a base price plus a per-mailbox price, 14-day trials, prorated upgrades, period-end downgrades; invoices with configurable tax and payment instructions, payments recorded by operators | Matches how small developers and schools buy; bank transfer and M-Pesa dominate locally; no card processor dependency | A payment gateway (M-Pesa STK push, card) is integrated |
| D-028 | **People choose their own mailbox passwords** through single-use invitation (7 d) or activation (link/QR) tokens; an ICT support role can resend, unlock and send reset links but never set or see passwords | Passwords stop travelling through admins, chats and spreadsheets; support stays possible | n/a |
| D-029 | **Server-rendered PHP frontend (BFF) replaces the browser SPA.** Supersedes the SPA part of D-025. No inline scripts, progressive enhancement only; received mail HTML is served from a dedicated route into a sandboxed iframe with its own CSP | Works on low-end phones and slow links, simpler to secure and test, deployable on any PHP host | The UI needs real-time collaborative features |
| D-030 | **One-click unsubscribe (RFC 8058)** on request; headers are DKIM-signed; using it suppresses the recipient for that tenant | Required by Gmail/Yahoo bulk-sender rules since 2024 | n/a |
| D-031 | **Out-of-office via Sieve vacation**, uploaded with ManageSieve as the webmail master user; Dovecot sends replies through a private Postfix listener (`10026`, internal network only) | Server-side replies work while devices are off; no new credentials | Users need full Sieve filter editing |
| D-032 | **Guided phone onboarding is stateless and data-driven** (`resources/guides/mail-clients.php`): device → app → readiness → steps with "why" and "can't find" → incoming/outgoing connection tests; analytics record only event names, device and app | Guides can be revised when vendor menus change without code changes; real connection proof instead of assumptions | Vendor apps adopt standard autodiscovery reliably |
| D-033 | **Flat Kenyan-shilling pricing and M-Pesa through PesaTide.** Starter KES 120 and Standard KES 200 (apps), Premium KES 1,200 (whole institution), no per-mailbox fees. Invoices are paid by STK push; the HMAC-signed webhook (and status polling) marks them paid; final states are never overwritten | Matches how Kenyan customers pay; no card data; idempotent reconciliation | A second gateway or card payments are needed |
| D-034 | **Account verification by SMS code or Google, not email.** Sign-up takes a username, phone and password; a six-digit code (hashed, 10 min, 5 attempts) verifies the phone through Africa's Talking. "Continue with Google" uses OIDC code flow with PKCE, state and nonce; the API exchanges the code. Email stays a contact for invoices and can be confirmed later | Users are not forced through any mail provider; SMS is the most familiar channel in Kenya; Google covers users who prefer it | SMS costs or deliverability become a problem (offer WhatsApp/passkeys) |
| D-035 | **Breached-password screening** with the Pwned Passwords range API (k-anonymity, padded responses), failing open on outages | NIST SP 800-63B requirement; no plaintext or full hash leaves the server | A local breach corpus is preferred |
| D-036 | **Public deliverability tools** (domain score, DNS lookup, record generator, header analyzer, blocklists, port check), rate-limited per IP; only DNS plus SSRF-guarded HTTPS (MTA-STS) and TCP probes to public addresses | Useful acquisition channel and support aid without exposing internal networks | Abuse shows up in the rate-limit metrics |
| D-037 | **Security alerts and container hardening.** Email alerts on password/2FA/Google-link changes; containers run with no-new-privileges and dropped capabilities; encrypted backups via `backup.sh`; RFC 9116 security.txt | Roadmap Phase 1-3 controls (see [Security readiness](Security-Readiness.md)) | Phase 4: passkeys, row-level security |
| D-038 | **Sealed links for every signed-in page.** Console, operator and webmail addresses become `/app?ref=<token>`, the real path encrypted with AES-256-GCM under a per-session key. Typed record addresses are refused; fixed menu pages redirect to their sealed form. Public, email-link and standard endpoints stay readable | Stops address editing ("directory manipulation") and hides the application's structure; links cannot be shared across sessions. Authorisation remains the real control | Bookmarking specific records becomes a requirement |
| D-039 | **Product renamed from EmailHoster to Mailvora everywhere**: crate and binary `mailvora`, database roles `mv_app`/`mv_mailserver`, API keys `mv_live_`/`mv_test_`, webhook headers `Mailvora-*`, ownership record `_mailvora`, DKIM selector prefix `mv` | One name for customers, operators and code before launch | Never (pre-launch change; no deployed data) |
| D-040 | **Refined emerald theme** on graphite neutrals replaces the blue theme; success green is distinct from the accent; the hero video autoplays muted in an endless loop without controls (reduced-motion users see the poster) | Owner preference; matches the landing video; status colours stay distinguishable | Brand refresh |
| D-041 | **Documentation as a GitHub-style wiki** (`frontend/resources/wiki`: Home, sidebar, one page per topic, "See also" links) instead of `backend/docs`, rendered by the web application: customer guides at `/docs`, the full wiki for operators at `/admin/docs` | Easier to navigate for each audience; one place for all docs | The wiki is published to a hosted wiki |
| D-042 | **System Infrastructure PDF for operators.** Architecture, system flows and security flows are written as print-styled HTML with inline SVG diagrams (`frontend/resources/infrastructure`) and rendered to a vector PDF by headless Chrome; operators download it from `/admin/infrastructure`, which shows previews and the SHA-256 checksum | Vector diagrams stay sharp at any zoom; one source for print and screen; downloads are logged | Reviewers need an editable format |

## Environment decisions to record per deployment

Fill these in for each production deployment (blueprint 00, "decisions to
record before implementation"):

- Hosting provider and region, and its acceptable-use policy confirmed for
  outbound TCP/25.
- Linux distribution and versions: Debian bookworm images, PostgreSQL 16,
  Postfix 3.7, Dovecot 2.3, Rspamd 3.4.
- Static IPv4 and PTR record set to `MAIL_HOSTNAME`. IPv6 stays unpublished
  until its PTR, SPF and reputation are ready.
- Backup RPO/RTO targets (suggested start: RPO 1 h, RTO 4 h) and the off-site
  location.
- Abuse contact (`abuse@`, `postmaster@`) owner and on-call rota.
- Data-protection obligations (e.g. GDPR, FERPA, the Kenya DPA) and the
  retention period for mail and logs.

## See also

- [Project status](Project-Status.md)
- [Research notes](Research-Notes.md)
- [Architecture](Architecture.md)
