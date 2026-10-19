#!/bin/sh
# Postfix pipe(8) transport for the return-path domain.
# Reads one message on stdin and posts it to the platform API together with
# the envelope recipient (the VERP address that identifies the message).
#
# pipe(8) runs with a minimal environment (only TZ MAIL_CONFIG LANG), so the
# API URL and token are read from files written by entrypoint.sh.
#
# Exit codes follow sysexits(3) so Postfix knows whether to retry:
#   0   delivered to the API (including "ignored" for unknown return paths)
#   75  EX_TEMPFAIL: API unreachable or 5xx, Postfix retries later
set -u
recipient="${1:-}"
secrets="/run/mailvora"

if [ ! -r "$secrets/internal_api_token" ] || [ ! -r "$secrets/api_url" ]; then
  echo "bounce-to-api: configuration files missing in $secrets" >&2
  exit 75
fi
api_url=$(cat "$secrets/api_url")

# Percent-encode every byte of the recipient for the query string.
encoded=$(printf '%s' "$recipient" | od -An -tx1 -v | tr -d ' \n' | sed 's/../%&/g')

status=$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
  --max-time 30 \
  -H "Authorization: Bearer $(cat "$secrets/internal_api_token")" \
  -H "Content-Type: message/rfc822" \
  --data-binary @- \
  "${api_url}/internal/v1/bounces?recipient=${encoded}") || exit 75

case "$status" in
  2*) exit 0 ;;
  4*) echo "bounce-to-api: rejected with HTTP $status" >&2; exit 0 ;;
  *)  echo "bounce-to-api: HTTP $status, will retry" >&2; exit 75 ;;
esac
