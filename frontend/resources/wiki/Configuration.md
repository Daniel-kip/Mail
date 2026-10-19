**Configuration** lists the environment variables that control Mailvora. The API and workers read `backend/stack.env` in production (copied from `stack.env.example`) or `backend/.env` in development; the web application reads `frontend/.env` in development and real environment variables in production.

In production both services validate their settings at start-up and refuse to run with unsafe values, such as plain-HTTP public addresses, insecure cookies, debug output, or development shortcuts like `DEV_SKIP_DNS_VERIFICATION`.

Keep `stack.env` private (`chmod 600`): it holds every secret of the platform. Generate secrets with `openssl rand -base64 33` or `mailvora gen-key`.

## Environment

| Variable | Example | Meaning |
|---|---|---|
| `APP_ENV` | `production` | `development` or `production` |
| `PUBLIC_BASE_URL` | `https://app.example.com` | **Required.** The web application; used in every emailed link |
| `PUBLIC_API_URL` | `https://api.example.com` | **Required.** API address shown to developers |
| `COOKIE_SECURE` | `true` | Secure session cookies (forced on in production) |
| `ALLOW_SIGNUP` | `true` | `false` allows only invited users |
| `LOG_FORMAT` / `RUST_LOG` | `json` / `mailvora=info` | Log format and level |

## Secrets

All required.

| Variable | Meaning |
|---|---|
| `DATA_ENCRYPTION_KEY` | 32 random bytes, base64. Encrypts DKIM keys and webhook secrets. **Back it up offline**: losing it loses them |
| `INTERNAL_API_TOKEN` | At least 32 characters; shared with Postfix for bounce ingestion |
| `POSTGRES_PASSWORD` | Database owner (migrations) |
| `MV_APP_PASSWORD` | Runtime database role used by the API and workers |
| `MV_MAILSERVER_PASSWORD` | Read-only database role used by Postfix and Dovecot |
| `RELAY_PASSWORD` | API → Postfix authenticated relay |
| `IMAP_MASTER_PASSWORD` | API → Dovecot master user for webmail (at least 24 characters) |
| `DOVEADM_API_KEY` | API → Dovecot doveadm HTTP API (immediate access revocation) |
| `RSPAMD_CONTROLLER_PASSWORD` | Rspamd web controller |

## Mail identity

| Variable | Example | Meaning |
|---|---|---|
| `PLATFORM_DOMAIN` | `example.com` | The platform's own domain |
| `MAIL_HOSTNAME` | `mail.example.com` | A and PTR record of the mail host; MX target for hosted domains |
| `SPF_INCLUDE` | `_spf.example.com` | TXT `v=spf1 ip4:<sending-ip> -all`; customers include it |
| `RETURN_PATH_DOMAIN` | `bounces.example.com` | Envelope sender domain (VERP bounces) |
| `MESSAGE_ID_DOMAIN` | `example.com` | Right-hand side of generated Message-IDs |
| `SYSTEM_FROM_ADDRESS` / `SYSTEM_FROM_NAME` | `no-reply@example.com` / `Mailvora` | Sender of verification, reset and invitation mail |
| `DKIM_SELECTOR_PREFIX` | `mv` | Selectors look like `mv202610` |
| `RELAY_USER` / `IMAP_MASTER_USER` | `relay@platform.internal` / `webmail` | Internal service identities |

## Limits

| Variable | Default | Meaning |
|---|---|---|
| `MAX_MESSAGE_BYTES` | `10485760` | Largest message accepted by the API |
| `MAX_ATTACHMENT_BYTES` | `8388608` | Largest single attachment |
| `CONTENT_RETENTION_DAYS` | `30` | Message bodies are purged after this many days |
| `SESSION_TTL_HOURS` | `12` | Console session lifetime |
| `WORKER_CONCURRENCY` | `8` | Parallel delivery workers |
| `DATABASE_MAX_CONNECTIONS` | `20` | Connection pool size |
| `WEBHOOK_ALLOW_PRIVATE_TARGETS` | `false` | Must stay `false` in production (SSRF guard) |

## Mail app settings

The server settings shown to employees' phones. Hosts must present publicly trusted certificates. Operators can override them later in the operator console, per domain too.

| Variable | Default | Meaning |
|---|---|---|
| `EMAIL_CLIENT_IMAP_HOST` / `_PORT` / `_TLS` | `mail.example.com` / `993` / `tls` | Incoming mail (implicit TLS, RFC 8314) |
| `EMAIL_CLIENT_SMTP_HOST` / `_PORT` / `_TLS` | `mail.example.com` / `465` / `tls` | Outgoing mail |
| `EMAIL_CLIENT_SMTP_ALT_PORT` / `_ALT_TLS` | `587` / `starttls` | Alternative outgoing port |
| `AUTOCONFIG_HOST` | `autoconfig.example.com` | Thunderbird autoconfig and Outlook Autodiscover |

## Branding and billing

| Variable | Example | Meaning |
|---|---|---|
| `PRODUCT_NAME` | `Mailvora` | Product name in pages and emails |
| `SUPPORT_EMAIL` | `support@example.com` | Shown in the help widget, footers and security.txt |
| `COMPANY_NAME` / `COMPANY_ADDRESS` | `Mailvora Ltd` / `Nairobi, Kenya` | Legal footer and invoices |
| `APP_TIMEZONE` | `Africa/Nairobi` | Display time zone |
| `BILLING_CURRENCY` | `KES` | ISO 4217 code; M-Pesa requires KES |
| `BILLING_TAX_RATE_BPS` / `BILLING_TAX_LABEL` | `0` / `VAT` | 1600 adds 16% VAT on top; 0 charges prices as listed |
| `BILLING_DUE_DAYS` / `BILLING_GRACE_DAYS` | `14` / `14` | Days to pay, and grace days after the due date |
| `BILLING_ISSUER` | `Mailvora Ltd, PIN P000000000X` | Issuer line on invoices |
| `BILLING_PAYMENT_INSTRUCTIONS` | | Shown on invoices |

## Integrations

Each group is optional, but its members must be set together.

| Variable | Meaning |
|---|---|
| `PESATIDE_API_URL`, `PESATIDE_API_KEY`, `PESATIDE_API_SECRET`, `PESATIDE_WEBHOOK_SECRET`, `PESATIDE_ROUTE_ID` | M-Pesa payments; see [Billing and payments](Billing-and-Payments.md) |
| `AT_USERNAME`, `AT_API_KEY`, `AT_SENDER_ID` | Africa's Talking SMS verification codes. Without them, production disables phone verification and falls back to email confirmation |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | "Continue with Google"; redirect URI `https://app.example.com/auth/google/callback` |
| `PASSWORD_BREACH_CHECK` | `true` screens new passwords against Pwned Passwords (k-anonymity) |
| `APPLE_PROFILE_SIGNING_CERT`, `_KEY`, `_CHAIN` | Optional signing of Apple `.mobileconfig` profiles |

## Web application

| Variable | Default | Meaning |
|---|---|---|
| `APP_URL` | | Public https address of the web application |
| `API_URL` | `http://127.0.0.1:8088` | Private address of the API |
| `PUBLIC_API_URL` | `API_URL` | API address shown to developers |
| `API_CONNECT_TIMEOUT` / `API_TIMEOUT` / `API_VERIFY_TLS` | `5` / `40` / `true` | API client behaviour |
| `SESSION_NAME` / `SESSION_SECURE` / `SESSION_IDLE_MINUTES` | `mv_session` / production / `120` | Session cookie |
| `TRUST_PROXY` | production | Trust `X-Real-IP` and `X-Forwarded-Proto` from your own reverse proxy |
| `SEALED_LINKS` | `true` | [Sealed links](Sealed-Links.md); always on in production |
| `SIGNUP_ENABLED` | `true` | Show the sign-up form |
| `SPF_INCLUDE` | | Offered by the DNS record generator |

## See also

- [VPS deployment](VPS-Deployment.md)
- [Quick start](Quick-Start.md)
- [Security model](Security-Model.md)
