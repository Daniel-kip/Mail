**VPS deployment** installs Mailvora on a single Linux server with Docker Compose. No git is required: the code travels as an archive.

One Linux VPS runs everything with Docker Compose: PHP web app, Rust API and
workers, PostgreSQL, Postfix, Dovecot (IMAP + Sieve), Rspamd and Redis, with
host Nginx terminating TLS. Code travels as a plain archive.

## 1. Server

- Ubuntu 24.04 LTS or Debian 12, 2 vCPU, 4 GB RAM, 40 GB SSD minimum (more
  disk for mailboxes). A static IPv4 with configurable **reverse DNS** and
  **outbound port 25 allowed** (confirm with the provider before buying).
- Point DNS: `app`, `api`, `autoconfig`, `mail` → the VPS IP. Set PTR to
  `mail.example.com`. See [DNS and TLS checklist](DNS-and-TLS-Checklist.md).

```sh
sudo apt update && sudo apt -y upgrade
sudo apt -y install docker.io docker-compose-v2 nginx certbot python3-certbot-nginx ufw unattended-upgrades
sudo ufw allow OpenSSH && sudo ufw allow 25,80,443,465,587,993/tcp && sudo ufw enable
sudo adduser --disabled-password mailvora && sudo usermod -aG docker mailvora
```

## 2. Copy the code (from your Windows machine)

Create an archive of the project folder **without** build outputs and local
secrets, then upload it:

```powershell
# in the folder that contains backend\ and frontend\
tar --exclude=backend/target --exclude=backend/.env --exclude=backend/demo-credentials.txt `
    --exclude=frontend/.env --exclude=frontend/storage/sessions/* --exclude=*.log --exclude=__pycache__ `
    -czf mailvora.tar.gz backend frontend
scp mailvora.tar.gz mailvora@<VPS-IP>:/home/mailvora/
```

On the server:

```sh
sudo mkdir -p /opt/mailvora && sudo chown mailvora: /opt/mailvora
cd /opt/mailvora && tar -xzf ~/mailvora.tar.gz
```

Updates later: upload a new archive, extract it over a fresh folder
(`/opt/mailvora-2026-11-01`), copy `backend/stack.env` across, then run step 5
from the new folder. Keep the previous folder for a quick rollback.

## 3. Configure

```sh
cd /opt/mailvora/backend
cp stack.env.example stack.env && chmod 600 stack.env
openssl rand -base64 32     # run once per secret below
nano stack.env
```

Fill every REQUIRED value: `PUBLIC_BASE_URL=https://app.example.com`,
`PUBLIC_API_URL=https://api.example.com`, all passwords/keys,
`DATA_ENCRYPTION_KEY` (32 bytes base64; **back it up offline**: without it
DKIM keys and webhook secrets cannot be decrypted), mail identity
(`PLATFORM_DOMAIN`, `MAIL_HOSTNAME`, `RETURN_PATH_DOMAIN`, ...), the
`EMAIL_CLIENT_*` settings (993/465/587), billing and branding.

## 4. Certificates

```sh
sudo certbot certonly --nginx -d app.example.com -d api.example.com -d autoconfig.example.com
sudo certbot certonly --standalone -d mail.example.com --pre-hook "systemctl stop nginx" --post-hook "systemctl start nginx"
```

Copy the mail certificate into the Postfix/Dovecot volumes after the first
`up` (step 5), and on every renewal via a deploy hook:

```sh
sudo tee /etc/letsencrypt/renewal-hooks/deploy/mailvora.sh >/dev/null <<'EOF'
#!/bin/sh
for svc in postfix dovecot; do
  docker compose -f /opt/mailvora/backend/docker-compose.yml --env-file /opt/mailvora/backend/stack.env cp /etc/letsencrypt/live/mail.example.com/fullchain.pem $svc:/etc/ssl/mail/fullchain.pem
  docker compose -f /opt/mailvora/backend/docker-compose.yml --env-file /opt/mailvora/backend/stack.env cp /etc/letsencrypt/live/mail.example.com/privkey.pem $svc:/etc/ssl/mail/privkey.pem
  docker compose -f /opt/mailvora/backend/docker-compose.yml --env-file /opt/mailvora/backend/stack.env restart $svc
done
EOF
sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/mailvora.sh
```

## 5. Start

```sh
cd /opt/mailvora/backend
docker compose --env-file stack.env up -d --build
docker compose --env-file stack.env ps                       # all healthy
sudo /etc/letsencrypt/renewal-hooks/deploy/mailvora.sh      # install the mail certificate
docker compose --env-file stack.env exec api mailvora promote-admin you@example.com   # after you register
```

Install the Nginx site: copy `infra/nginx/mailvora.conf` to
`/etc/nginx/sites-available/mailvora`, replace the example hostnames, link it
into `sites-enabled`, `sudo nginx -t && sudo systemctl reload nginx`.

The containers restart automatically (`restart: unless-stopped`), and Docker
starts at boot (`systemctl is-enabled docker`).

## 6. Verify

- `https://app.example.com/healthz` returns `{"status":"ok","web":"ok","api":"ok"}`.
- Register at `https://app.example.com/register`, confirm the email, promote
  yourself (step 5), open `/admin`: Overview must show the relay and mail
  storage as configured.
- Work through [Release checklist](Release-Checklist.md).

## 7. Operate

### Host hardening

```sh
sudo sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/; s/^#\?PermitRootLogin .*/PermitRootLogin no/' /etc/ssh/sshd_config
sudo systemctl reload ssh
sudo dpkg-reconfigure -plow unattended-upgrades
```

### Backups (daily, encrypted, off-site)

```sh
sudo install -m 600 /dev/null /root/.mailvora-backup-key && openssl rand -base64 48 | sudo tee /root/.mailvora-backup-key >/dev/null
echo '30 2 * * * root BACKUP_PASSPHRASE_FILE=/root/.mailvora-backup-key /opt/mailvora/backend/infra/scripts/backup.sh /var/backups/mailvora' | sudo tee /etc/cron.d/mailvora-backup
```

Copy `/var/backups/mailvora` to another provider (rclone/rsync with an
append-only target) and store the passphrase offline: without it the
archives cannot be read.

### Restore (drill this quarterly on a scratch VPS)

```sh
gpg --batch --pinentry-mode loopback --passphrase-file /root/.mailvora-backup-key -d mailvora-<stamp>.tar.gpg | tar -x -C /tmp/restore
sha256sum -c /tmp/restore/SHA256SUMS
docker compose --env-file stack.env up -d postgres dovecot
docker compose exec -T postgres pg_restore -U mailvora -d mailvora --clean --if-exists < /tmp/restore/database.dump
docker compose run --rm --no-deps -T -v /tmp/restore:/in --entrypoint sh dovecot -c 'tar -C /var/vmail -xzf /in/vmail.tar.gz'
docker compose --env-file stack.env up -d
```

### Payments, SMS and Google

- **PesaTide:** create a live API key, set `PESATIDE_*` in `stack.env`, and
  bind the webhook URL `https://api.example.com/v1/payments/pesatide/webhook`
  to that key. A test invoice of KES 1 proves the flow end to end.
- **Africa's Talking:** set `AT_USERNAME`, `AT_API_KEY` and, once approved, a
  sender ID. Without credentials production disables phone verification and
  falls back to email confirmation.
- **Google:** create an OAuth client (Web application) with the redirect URI
  `https://app.example.com/auth/google/callback`; set `GOOGLE_CLIENT_ID` and
  `GOOGLE_CLIENT_SECRET`.
- **Logs:** `docker compose logs -f api frontend postfix dovecot`. Errors in
  the web app show a reference code; search for it in the API logs.
- **Billing:** renewals and overdue handling run automatically in the API
  worker; record payments in `/admin/billing`.
- See [Operations](Operations.md) for queue, abuse and incident handling.

## See also

- [Configuration](Configuration.md)
- [DNS and TLS checklist](DNS-and-TLS-Checklist.md)
- [Operations](Operations.md)
- [Release checklist](Release-Checklist.md)
