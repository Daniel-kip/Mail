The **operations runbook** is for the engineers who deploy and run Mailvora in production. It covers host prerequisites, DNS, TLS, deployment, monitoring, backups and incident handling.

## 1. Host prerequisites (blueprint 06)

Confirm every item **before** building anything. Self-hosting mail is not
possible on a host that blocks it.

- [ ] Static public IPv4. The provider allows **inbound and outbound TCP/25**.
      Many VPS providers block 25 by default; get written confirmation.
- [ ] Reverse DNS (PTR) for the IP set to `MAIL_HOSTNAME`, with an A record
      `MAIL_HOSTNAME` pointing back to the IP (forward-confirmed rDNS).
- [ ] Firewall open only for 25, 80, 443, 465, 587, 993, and SSH from an admin
      network. Never expose 5432, 10587, 143, 12345, 10993, 11334 or 6379.
- [ ] Time sync (chrony or systemd-timesyncd) enabled. TOTP and TLS depend on it.
- [ ] Disk sized for mailboxes, the Postgres queue and backups. Monitor inodes
      too, since maildir creates one file per message.
- [ ] Separate staging and production hosts, databases and secrets.

## 2. DNS records

Replace `example.com`, the IP, and the generated values.

### Platform records (published once by the operator)

| Name | Type | Value | Why |
|------|------|-------|-----|
| `mail.example.com` | A | `<public-ip>` | Mail host (`MAIL_HOSTNAME`) |
| PTR of `<public-ip>` | PTR | `mail.example.com` | Set at the hosting provider |
| `app.example.com` | A | `<public-ip>` | PHP console and webmail (`PUBLIC_BASE_URL`) |
| `api.example.com` | A | `<public-ip>` | Rust API for API-key clients; TLS required |
| `_spf.example.com` | TXT | `v=spf1 ip4:<sending-ip> -all` | Target of tenants' `include:` (`SPF_INCLUDE`) |
| `bounces.example.com` | MX | `10 mail.example.com` | Return-path domain receives DSNs |
| `bounces.example.com` | TXT | `v=spf1 ip4:<sending-ip> -all` | SPF for our envelope sender |
| `example.com` | TXT `_dmarc` | `v=DMARC1; p=quarantine; rua=mailto:dmarc@example.com` | Policy for the platform domain |

Register the platform domain (`SYSTEM_FROM_ADDRESS` domain) as a tenant domain
in the operator's organization and verify it. System email (verification,
password reset, invitations) is then DKIM-signed with its key.

### Tenant records (shown per domain in the console)

| Name | Type | Example value | Required |
|------|------|---------------|----------|
| `_mailvora.school.edu` | TXT | `mailvora-verification=<token>` | yes (ownership) |
| `eh202610._domainkey.school.edu` | TXT | `v=DKIM1; k=rsa; p=<public key>` | yes (signing) |
| `school.edu` | TXT | `v=spf1 include:_spf.example.com ~all` (merged with existing SPF) | recommended |
| `_dmarc.school.edu` | TXT | `v=DMARC1; p=none; rua=mailto:…` | recommended |
| `school.edu` | MX | `10 mail.example.com` | for hosted mailboxes |

Never publish two SPF records for one name; the console warns when it sees
duplicates. Tighten DMARC from `p=none` to `quarantine` and then `reject` after
reviewing aggregate reports.

## 3. TLS

- Web: certbot (`--webroot /var/www/certbot`) for `app.example.com`, with the
  Nginx config in `infra/nginx/mailvora.conf`. Renewal runs from a systemd
  timer.
- Mail: issue a certificate for `mail.example.com` and bind-mount the live
  directory into both mail containers as `/etc/ssl/mail` (`fullchain.pem`,
  `privkey.pem`), replacing the `postfix-certs`/`dovecot-certs` volumes. Run
  `docker compose restart postfix dovecot` in a certbot deploy hook. Without a
  mounted certificate the containers generate a self-signed one; that is only
  acceptable in development.
- Alert at 21 days before expiry (§6).

## 4. Deployment

The production Compose stack builds the PHP frontend from `../frontend` and
publishes it only on `127.0.0.1:8080`; it reaches Rust at `http://api:8088`
inside the private Compose network. Rust is bound to `127.0.0.1:8088` on the
host. Configure host Nginx from `infra/nginx/mailvora.conf`: the app
hostname goes to PHP (including the browser's same-origin `/v1` bridge), while
the API hostname goes to Rust for scoped API-key clients. Do not point browser
JavaScript at the API hostname. Keep the PHP port loopback-only or restrict it
to trusted reverse proxies; the frontend forwards Nginx's overwritten
`X-Real-IP` to Rust for per-IP controls.

```bash
# copy the project folder to the server, e.g. with scp or rsync
rsync -a mailvora/ server:/opt/mailvora/ && ssh server
cd /opt/mailvora
cp .env.example .env && chmod 600 .env     # fill every REQUIRED value
docker compose build
docker compose up -d postgres
docker compose run --rm api migrate        # explicit, once per release
docker compose up -d
docker compose exec api mailvora promote-admin ops@example.com
```

- `DOVEADM_API_KEY` lets the API revoke mail access immediately (auth-cache flush and session kick) when a mailbox is disabled or a tenant suspended. Without it, revocation waits for Dovecot's 1-minute auth cache.
- Store secrets in your vault and template `.env` from it. Back up
  `DATA_ENCRYPTION_KEY` separately from database backups: without it, DKIM
  keys, webhook secrets and TOTP seeds cannot be decrypted.
- The API role (`mv_app`) has DML only. Migrations run as the owner role
  (`MIGRATION_DATABASE_URL`). Set `RUN_MIGRATIONS_ON_START=false` once you run
  migrations as a pipeline step.
- On `docker compose up -d` with a new image, the API stops accepting
  connections, workers finish their current job (25 s grace), and unfinished
  leases are reclaimed after 5 minutes.
- **Rollback:** redeploy the previous image tag. Migrations are additive.
  Never roll back a migration that dropped data; restore from backup instead.

### Release checklist

1. Checks pass locally: `run-tests.cmd` (or `cargo fmt --check`, `cargo clippy -D warnings` and
   `cargo test` with a database), plus `cargo audit`.
2. Back up the database before the migration (§7).
3. Deploy to staging, then run the smoke test: register, verify a domain,
   send a message with a test key, check webhooks, and do a webmail login.
4. Deploy to production and watch §6 metrics for 30 minutes.

## 5. Mail tier checks after every config change

Run the automated suite first (from a host that can reach the ports):
`python infra/scripts/mail_tier_e2e.py --mx mail.example.com:25 --submission mail.example.com:587 --relay <private-ip>:10587 ...`
It checks that relay is refused, unknown recipients are rejected during
SMTP, delivery reaches Dovecot, submission requires STARTTLS + AUTH with
sender-login enforcement, and the relay port accepts only the platform
identity. Then confirm from an **external network** by hand:

```bash
# Open relay test, run FROM AN EXTERNAL HOST. Must fail with "Relay access denied".
swaks --server mail.example.com --from a@external.test --to b@another-external.test

# Unknown recipient must be rejected during SMTP (no backscatter):
swaks --server mail.example.com --to doesnotexist@hosted-domain.example

# Submission requires TLS + auth, and the sender must match the login:
swaks --server mail.example.com:587 --tls --auth-user user@hosted.example --from other@hosted.example ...

# Internal relay port must not be reachable from outside:
nc -vz mail.example.com 10587     # expect: connection refused / filtered
```

Then send to a Gmail and an Outlook account and inspect the headers:
`dkim=pass header.d=<tenant domain>`, `spf=pass` for the bounce domain, and
`dmarc=pass`.

## 6. Monitoring

Scrape `GET /internal/v1/metrics` (bearer `INTERNAL_API_TOKEN`) from the
private network. Alert on:

| Signal | Source | Alert when |
|--------|--------|-----------|
| Queue backlog | `mv_outbox_jobs{status="pending"}` | > 1000 for 10 min |
| Queue age | `mv_outbox_oldest_pending_seconds` | > 300 s |
| Dead jobs | `mv_outbox_jobs{status="dead"}` | increases |
| Bounce rate | `mv_messages_last_hour{status="bounced"}` ÷ total | > 5 % (abuse or list problem) |
| Webhook failures | `mv_webhook_deliveries_last_hour{status="failed"}` | sustained |
| Postfix queue | `postqueue -p \| tail -1` (node exporter textfile) | > 500 or age > 1 h |
| API health | `GET /readyz` | non-200 for 1 min |
| Disk and inodes | node exporter | > 80 % |
| Certificate expiry | blackbox exporter on 443/465/993 | < 21 days |
| Backup age | backup job metric | > RPO |

Logs are JSON when `LOG_FORMAT=json` and include `request_id`. Secrets,
passwords, message bodies and template variables are never logged.

## 7. Backups and recovery

| What | How | Frequency |
|------|-----|-----------|
| PostgreSQL | `pg_dump -Fc` (plus WAL archiving for point-in-time recovery if RPO < 24 h) | hourly / continuous |
| Mailboxes | `docker run --rm -v mailvora_vmail:/v ... tar` or `doveadm backup` to an off-site target | nightly |
| Config and secrets | `.env` in the vault, the repository, `/etc/letsencrypt` | on change |
| Postfix queue | `postfix-spool` volume snapshot | nightly |

Encrypt backups (e.g. `restic` or `age`) and keep an off-site copy. Run
restore drills every quarter:

1. **Database only:** restore into staging and run `mailvora migrate`;
   `/readyz` must be OK.
2. **Mailbox only:** restore one maildir and confirm messages appear over IMAP.
3. **Full service:** fresh host, then restore all of the above; record the
   time taken against RTO.

**Preventing duplicate sends after a restore:** messages restored in
`queued` or `processing` status may already have been delivered. Before
starting workers on a restored database, run
`UPDATE messages SET status='failed', last_error='restored from backup; verify before resending' WHERE status IN ('queued','processing') AND created_at < '<backup time>';`
and delete their pending `send_message` jobs.

## 8. Data lifecycle

- Message bodies are purged after `CONTENT_RETENTION_DAYS`. Metadata and
  events are kept.
- **Deleted mailboxes:** access stops immediately and the address is freed.
  After the retention window, purge the maildir with
  `docker compose exec dovecot rm -rf /var/vmail/<storage_key>`. The
  `storage_key` comes from `SELECT storage_key, deleted_address FROM mailboxes WHERE status='deleted'`.
  Then delete the row.
- Junk and Trash auto-expunge after 30 days (Dovecot `autoexpunge`).

## 9. Incident runbooks (blueprint 08)

**Credential or API key compromise**
1. In the console, revoke the key or ask the tenant to. Operators can suspend
   the tenant from the operator console.
2. Rotate any related secrets: webhook secret, mailbox passwords (reset).
3. Review the audit log (`api_key.*`, `user.login*`) and the messages sent by
   that key (`GET /v1/messages`, filter by time).
4. Notify the tenant, and record the incident.

**Spam outbreak from a tenant**
1. Operator console → **Suspend**. This halts queued mail immediately and
   rejects new sends, inbound mail and logins.
2. Purge already-relayed mail from Postfix:
   `docker compose exec postfix postqueue -p`, then
   `postsuper -d <queue id>` (or `postsuper -d ALL deferred` if needed).
3. Find the vector: a compromised key, a mailbox password, or a template abused
   through user input.
4. Check blocklists (Spamhaus, Barracuda), request delisting after cleanup,
   and reduce the tenant's plan limits before reinstating.

**Disk full**
1. Stop intake by suspending the largest-growing tenant if one is the cause,
   or `docker compose stop postfix` (remote MTAs will queue and retry for
   days).
2. Expand the volume, then confirm `postfix check` and Dovecot logs are clean.
3. Restart and verify with a controlled test message.

**Mail outage (relay or Dovecot down)**
1. API sends keep queuing safely, and the worker marks attempts `deferred`
   with backoff.
2. Restore dependencies in order: Postgres → Dovecot → Rspamd → Postfix → API.
3. Requeue dead jobs from the operator console if they exceeded their
   retries, then verify with a test message.

**Lost DATA_ENCRYPTION_KEY**
Domains must re-verify with new DKIM keys (delete and re-add them), webhook
secrets must be rotated, and users must re-enroll MFA. Prevent this by keeping
the key in two separate vaults.

## See also

- [VPS deployment](VPS-Deployment.md)
- [Security model](Security-Model.md)
- [Troubleshooting](Troubleshooting.md)
- [DNS and TLS checklist](DNS-and-TLS-Checklist.md)
