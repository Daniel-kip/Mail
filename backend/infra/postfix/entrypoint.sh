#!/bin/sh
# Render configuration from templates and environment, then run Postfix in
# the foreground. Fails fast if a required variable is missing.
set -eu

for v in MAIL_HOSTNAME RETURN_PATH_DOMAIN DOVECOT_HOST RSPAMD_HOST RELAY_USER DB_HOST DB_NAME DB_USER DB_PASSWORD INTERNAL_API_TOKEN; do
  eval "val=\${$v:-}"
  if [ -z "$val" ]; then echo "postfix: $v is required" >&2; exit 1; fi
done

INTERNAL_NETWORK="${INTERNAL_NETWORK:-10.89.0.0/24}"
export INTERNAL_NETWORK
VARS='${MAIL_HOSTNAME} ${RETURN_PATH_DOMAIN} ${DOVECOT_HOST} ${RSPAMD_HOST} ${RELAY_USER} ${DB_HOST} ${DB_NAME} ${DB_USER} ${DB_PASSWORD} ${INTERNAL_NETWORK}'
envsubst "$VARS" < /etc/postfix/templates/main.cf.template > /etc/postfix/main.cf
envsubst "$VARS" < /etc/postfix/templates/master.cf.template > /etc/postfix/master.cf
mkdir -p /etc/postfix/pgsql
for f in /etc/postfix/templates/pgsql/*.cf.template; do
  out="/etc/postfix/pgsql/$(basename "$f" .template)"
  envsubst "$VARS" < "$f" > "$out"
  chown root:postfix "$out"
  chmod 0640 "$out"
done

# Readable only by the pipe(8) user; kept outside /etc/postfix, whose files
# Postfix expects to be root-owned.
mkdir -p /run/mailvora
chown nobody:nogroup /run/mailvora
chmod 0500 /run/mailvora
umask 077
printf '%s' "$INTERNAL_API_TOKEN" > /run/mailvora/internal_api_token
chown nobody:nogroup /run/mailvora/internal_api_token
chmod 0400 /run/mailvora/internal_api_token
printf '%s' "${PLATFORM_API_URL:-http://api:8088}" > /run/mailvora/api_url
chown nobody:nogroup /run/mailvora/api_url
chmod 0400 /run/mailvora/api_url
umask 022

# Development fallback: self-signed certificate if none is mounted.
if [ ! -s /etc/ssl/mail/fullchain.pem ] || [ ! -s /etc/ssl/mail/privkey.pem ]; then
  echo "postfix: no certificate mounted at /etc/ssl/mail, generating a self-signed one (development only)" >&2
  mkdir -p /etc/ssl/mail
  openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=${MAIL_HOSTNAME}" \
    -keyout /etc/ssl/mail/privkey.pem -out /etc/ssl/mail/fullchain.pem >/dev/null 2>&1
fi

# Postfix needs resolv.conf etc. inside its spool for some services.
cp -f /etc/resolv.conf /etc/hosts /etc/services /var/spool/postfix/etc/ 2>/dev/null || true

postfix check
exec postfix start-fg
