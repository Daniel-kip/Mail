The **release checklist** lists the automated checks, the manual checks on the VPS and the real-device acceptance tests that every Mailvora release must pass.

## Automated (every release, on the development machine)

| Check | Command | Covers |
|---|---|---|
| Rust format, lint | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | Style, common bugs |
| Infrastructure PDF | `python frontend/scripts/build_infrastructure_pdf.py` (after any change to topology, ports or controls) | Rebuilds the operator PDF, its previews and checksum |
| Rust unit + integration | `backend\run-tests.cmd -RustOnly` | Auth, tenancy/IDOR, billing, activation tokens, support role, API key restrictions, scheduling, unsubscribe, client profiles, autoconfig, onboarding API |
| PHP unit + feature | `php tests/run.php` in `frontend/` (the runner loads extensions) | Every page renders, CSRF, validation, one-time secrets, role gating, webmail sandboxing, setup wizard, admin |
| API end-to-end | `infra/scripts/smoke.py` | Register, verify, domain, key, send, idempotency, relay, DKIM |
| Mailboxes end-to-end | `infra/scripts/mailbox_e2e.py` | Real Dovecot: list, sanitize, attachments, reply, drafts, move, delete, quota, revocation |
| Browser journey | `frontend/tests/ui_smoke.py` | Real Chrome: sign-up to activated mailbox, phone wizard, webmail, every console page, no JS/CSP errors |
| Everything | `backend\run-tests.cmd` (with the stack started) | All of the above |

## Manual on the VPS (before go-live)

- [ ] DNS/TLS checklist complete ([DNS and TLS checklist](DNS-and-TLS-Checklist.md)).
- [ ] `/healthz` ok; `/admin` overview shows relay and storage configured.
- [ ] Mail app settings in `/admin/mail-clients` match the real hostnames
      (993 / 465 / 587) and the readiness check is green for a test mailbox.
- [ ] Send to Gmail and Outlook.com: SPF, DKIM and DMARC pass; not in spam.
- [ ] Receive from Gmail into a hosted mailbox; reply from webmail.
- [ ] Out-of-office: enable in webmail settings, send from an outside
      account, receive exactly one auto-reply.
- [ ] Invitation flow with a real personal address; reset flow; unlock.
- [ ] Apple profile downloads in Safari and installs (signed if configured).
- [ ] Billing: start a trial, issue an invoice (`/admin/billing` → Run
      billing now after moving the period in a test org), record a payment.
- [ ] Backups run and a restore was tested.

## Real-device acceptance (record results honestly)

Run on physical phones against the production hostname. Note app version,
OS version, date, tester, and the result of both connection tests.

| Device / app | Guide | Incoming test | Outgoing test | Notes |
|---|---|---|---|---|
| Android: Gmail | android-gmail | | | |
| Android: Outlook | android-outlook | | | |
| iPhone: Apple Mail (manual) | ios-apple_mail | | | |
| iPhone: Apple Mail (profile) | ios-apple_mail step 1 | | | |
| iPhone: Gmail | ios-gmail | | | |
| iPhone: Outlook | ios-outlook | | | |
| Desktop: Thunderbird (autoconfig) | any-other | | | |

As delivered, **no physical-device test has been performed**: the local
development server uses a self-signed certificate that real phones
correctly refuse. The guides were written against the vendors' current
documentation (sources are listed in each guide).

## See also

- [Quick start](Quick-Start.md)
- [Security readiness](Security-Readiness.md)
- [Project status](Project-Status.md)
