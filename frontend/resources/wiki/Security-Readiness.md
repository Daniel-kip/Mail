**Security readiness** maps the production acceptance criteria of the security roadmap to the controls in the codebase and the evidence for each.

The roadmap draws on OWASP ASVS 5.0, the OWASP API Security Top 10, NIST SP 800-63-4 and CISA Secure by Design. Items marked *Operator* can only be proven on the production VPS; they are listed in the [Release checklist](Release-Checklist.md).

## Acceptance checklist

| # | Criterion | Control in Mailvora | Evidence |
|---|---|---|---|
| **Identity** | | | |
| 1 | Passwords never stored in plaintext or fast hashes | Argon2id (m=19 MiB, t=2, p=1) for console and mailbox passwords; Dovecot verifies the same `{ARGON2ID}` hash | `src/password.rs`; D-009 |
| 2 | Privileged users require MFA | Platform operators are refused in production until TOTP is enrolled; owners without 2FA are flagged on `/admin/security` | `src/auth/mod.rs` (`PlatformAdmin`), `GET /v1/admin/security` |
| 3 | Recovery tokens single-use, expiring, stored securely | Reset 1 h, activation 7 d, phone codes 10 min; stored as SHA-256 hashes; consumed on use; superseded by newer tokens; purged daily | `services/mailbox_tokens.rs`, `api/auth.rs` (`phone_codes`), integration test `activation_links_are_single_use…` |
| 4 | Reset and suspension revoke sessions and tokens | Password change/reset revokes other sessions and kicks IMAP via doveadm; tenant suspension revokes everything | D-022; integration tests |
| — | Breached and weak passwords refused | Length ≥ 12, no identity substrings, common-password list, Pwned Passwords k-anonymity check (fail-open) | `services/breach.rs`, `PASSWORD_BREACH_CHECK` |
| — | Users told about security changes | Email alert on password change/reset, 2FA on/off, Google account linked | `system_mail::security_alert` |
| **Authorization** | | | |
| 5 | Changing an ID never exposes another user's data | Every query is scoped by the authenticated organization or mailbox; composite foreign keys | IDOR integration tests per resource (`tests/api.rs`) |
| 6 | No cross-tenant access through APIs, search, exports, attachments | Tenant derived from the session, never from input; exports and attachment downloads use the same scoping | `tests/api.rs`, `tests/platform_features.rs` |
| 7 | Admins cannot grant themselves higher privileges | Only owners change owner/admin roles; nobody changes their own role; last owner protected | `api/organizations.rs` (`change_role`) |
| 8 | Unauthorized operations fail safely and are audited | Deny by default; failures return stable error codes; sign-in and 2FA failures are audited | `audit_logs`, `/admin/security` |
| **Web and API** | | | |
| 9 | HTTPS enforced; cookies Secure, HttpOnly, SameSite | HSTS; `__Host-` session cookie in production; production refuses insecure config | `src/Session/NativeSession.php`, `config/app.php`, `src/config.rs` |
| 10 | CSRF protection on cookie-authenticated changes | Synchronizer tokens in the PHP app; double-submit header check in the API | `VerifyCsrfToken`, `auth::check_csrf` |
| 11 | XSS, SQL injection, mass assignment, unsafe file access tested | Output escaping, strict CSP without inline scripts, sanitized mail HTML in a sandboxed iframe with its own CSP, parameterized SQL only, explicit request DTOs, attachment type blocklist | `SecurityHeaders`, `MailController::body`, PHP feature tests |
| 12 | Rate and size limits resist exhaustion | Per-IP and per-account limits on sign-in, codes, tools and payments; body limits; quotas | `ratelimit.rs`, nginx `limit_req` |
| — | No SSRF | Webhooks, MTA-STS fetches and the port checker resolve and refuse private/reserved addresses; mail-client settings never come from user input | `services/webhooks.rs` (`GuardedResolver`) |
| **Email** | | | |
| 13 | Submission cannot be used as an open relay | Postfix: `permit_sasl_authenticated, reject_unauth_destination`; platform relay accepts only the relay identity; auto-reply listener only from the private network | `infra/postfix/*`, `mail_tier_e2e.py` |
| 14 | SPF, DKIM, DMARC verified against real flows | Per-domain DKIM keys; verification requires ownership + DKIM; DKIM verified by an independent implementation | `tests/verify_dkim.py`, D-004 |
| 15 | Mailbox protocol authorization matches webmail | IMAP/SMTP authenticate against the same active-mailbox passdb; disabling revokes both immediately | `mailbox_e2e.py` |
| 16 | Compromised-account sending limits | Plan quotas, per-mailbox webmail send limit (60/h), automatic suppression of bounces and complaints | `api/webmail.rs`, `services/sending.rs` |
| **Infrastructure and operations** | | | |
| 17 | Only required ports are public | Compose publishes 25, 465, 587, 993 and the loopback-only app ports; firewall rules in the VPS guide | `docker-compose.yml`, [VPS deployment](VPS-Deployment.md) (Operator) |
| 18 | Production secrets outside source and the web root | `stack.env` (chmod 600) and container environment; PHP serves only `public/`; Apache denies everything else | `docker/apache-vhost.conf` |
| 19 | Security events retained without secrets or message bodies | Audit log stores actor, action, target, IP and metadata only; passwords, tokens and content never logged | `services/audit.rs` |
| 20 | Isolated backups with a demonstrated restore | Encrypted (AES-256 GPG) archive of database, mailboxes and config; off-site copy; quarterly restore drill | `infra/scripts/backup.sh` (Operator: drill) |
| 21 | Critical/high pen-test findings resolved | Pre-launch penetration test | Operator |

## Container and host hardening

- Containers: `no-new-privileges`; the API and web containers drop all
  Linux capabilities (the web container keeps only what Apache needs to bind
  :80 and drop to `www-data`); bounded JSON logs.
- The API runs as an unprivileged user; migrations use a separate owner role;
  the mail servers use a read-only database role.
- Host (VPS guide): SSH keys only, no root login, `ufw` allowing 22/25/80/443/465/587/993,
  unattended security upgrades.

## Disclosure and privacy

- `/.well-known/security.txt` (RFC 9116) publishes the security contact.
- The privacy notice lists every processor (PesaTide, Africa's Talking,
  Google sign-in, Pwned Passwords) and the right to complain to Kenya's ODPC.

## Deliberately deferred (Phase 4 in the roadmap)

- Passkeys/WebAuthn as a phishing-resistant factor (TOTP is in place).
- PostgreSQL row-level security as a second isolation layer (D-017).
- Second-person approval for the most sensitive operator actions.

## See also

- [Security model](Security-Model.md)
- [Sealed links](Sealed-Links.md)
- [Release checklist](Release-Checklist.md)
