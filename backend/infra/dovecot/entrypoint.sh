#!/bin/sh
# Render configuration, create credential files from the environment, run Dovecot.
set -eu

for v in MAIL_HOSTNAME DB_HOST DB_NAME DB_USER DB_PASSWORD TRUSTED_NETWORK IMAP_MASTER_USER IMAP_MASTER_PASSWORD RELAY_USER RELAY_PASSWORD DOVEADM_API_KEY; do
  eval "val=\${$v:-}"
  if [ -z "$val" ]; then echo "dovecot: $v is required" >&2; exit 1; fi
done

SUBMISSION_HOST="${SUBMISSION_HOST:-postfix:10026}"
export SUBMISSION_HOST
VARS='${MAIL_HOSTNAME} ${DB_HOST} ${DB_NAME} ${DB_USER} ${DB_PASSWORD} ${TRUSTED_NETWORK} ${DOVEADM_API_KEY} ${SUBMISSION_HOST}'
envsubst "$VARS" < /etc/dovecot/templates/dovecot.conf.template > /etc/dovecot/dovecot.conf
chmod 0640 /etc/dovecot/dovecot.conf
chown root:dovecot /etc/dovecot/dovecot.conf
envsubst "$VARS" < /etc/dovecot/templates/dovecot-sql.conf.ext.template > /etc/dovecot/dovecot-sql.conf.ext
chmod 0640 /etc/dovecot/dovecot-sql.conf.ext
chown root:dovecot /etc/dovecot/dovecot-sql.conf.ext

# Credential files hold Argon2id hashes, never the plaintext secrets.
mkdir -p /etc/dovecot/private
chown root:dovecot /etc/dovecot/private
chmod 0750 /etc/dovecot/private
umask 077
printf '%s:%s\n' "$IMAP_MASTER_USER" "$(doveadm pw -s ARGON2ID -p "$IMAP_MASTER_PASSWORD")" > /etc/dovecot/private/master-users
printf '%s:%s\n' "$RELAY_USER" "$(doveadm pw -s ARGON2ID -p "$RELAY_PASSWORD")" > /etc/dovecot/private/relay-users
chown root:dovecot /etc/dovecot/private/master-users /etc/dovecot/private/relay-users
chmod 0640 /etc/dovecot/private/master-users /etc/dovecot/private/relay-users
umask 022

if [ ! -s /etc/ssl/mail/fullchain.pem ] || [ ! -s /etc/ssl/mail/privkey.pem ]; then
  echo "dovecot: no certificate mounted at /etc/ssl/mail, generating a self-signed one (development only)" >&2
  mkdir -p /etc/ssl/mail
  openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj "/CN=${MAIL_HOSTNAME}" \
    -keyout /etc/ssl/mail/privkey.pem -out /etc/ssl/mail/fullchain.pem >/dev/null 2>&1
fi

chown vmail:vmail /var/vmail

doveconf -n > /dev/null
exec dovecot -F
