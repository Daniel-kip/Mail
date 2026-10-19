#!/bin/sh
set -eu
if [ -z "${RSPAMD_CONTROLLER_PASSWORD:-}" ]; then
  echo "rspamd: RSPAMD_CONTROLLER_PASSWORD is required" >&2
  exit 1
fi
hash=$(rspamadm pw -p "$RSPAMD_CONTROLLER_PASSWORD")
printf 'password = "%s";
enable_password = "%s";
' "$hash" "$hash" > /etc/rspamd/local.d/controller-password.inc
chown _rspamd:_rspamd /etc/rspamd/local.d/controller-password.inc
chmod 0640 /etc/rspamd/local.d/controller-password.inc
rspamadm configtest
exec rspamd -f -u _rspamd -g _rspamd
