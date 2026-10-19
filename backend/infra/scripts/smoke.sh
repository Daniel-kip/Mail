#!/usr/bin/env bash
# End-to-end smoke test through the public HTTP API.
#
#   BASE=http://localhost:8088 MAILPIT=http://localhost:8025 infra/scripts/smoke.sh
#
# Requires: curl, jq. Against a development stack (DEV_SKIP_DNS_VERIFICATION
# and Mailpit) it exercises the full path: register → verify email → domain →
# API key → send → worker → relay → status, and checks Mailpit received it.
set -euo pipefail

BASE="${BASE:-http://localhost:8088}"
MAILPIT="${MAILPIT:-http://localhost:8025}"
STAMP="$(date +%s)"
EMAIL="smoke-${STAMP}@example.com"
DOMAIN="smoke${STAMP}.example"
JAR="$(mktemp)"
trap 'rm -f "$JAR"' EXIT

pass() { printf '  \033[32m✓\033[0m %s\n' "$1"; }
fail() { printf '  \033[31m✗\033[0m %s\n' "$1"; exit 1; }
api() { # method path [json]
  local method="$1" path="$2" body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -sS -b "$JAR" -c "$JAR" -X "$method" "$BASE$path" -H "Content-Type: application/json" -H "X-CSRF-Token: ${CSRF:-}" -d "$body"
  else
    curl -sS -b "$JAR" -c "$JAR" -X "$method" "$BASE$path" -H "X-CSRF-Token: ${CSRF:-}"
  fi
}

echo "Smoke test against $BASE"
[[ "$(curl -s "$BASE/readyz" | jq -r .status)" == "ready" ]] && pass "API ready" || fail "API not ready"

R=$(api POST /v1/auth/register "{\"full_name\":\"Smoke Test\",\"email\":\"$EMAIL\",\"password\":\"smoke test passphrase 42\",\"organization_name\":\"Smoke $STAMP\"}")
CSRF=$(jq -r .csrf_token <<<"$R"); [[ "$CSRF" != null ]] && pass "registered $EMAIL" || fail "register: $R"

# Confirm the email through the link delivered to Mailpit.
sleep 3
LINK=""
for _ in $(seq 1 10); do
  ID=$(curl -s "$MAILPIT/api/v1/search?query=to:$EMAIL" | jq -r '.messages[0].ID // empty')
  if [[ -n "$ID" ]]; then
    LINK=$(curl -s "$MAILPIT/api/v1/message/$ID" | jq -r .Text | grep -o 'http[^ ]*verify-email?token=[A-Za-z0-9_-]*' | head -1)
    break
  fi
  sleep 2
done
[[ -n "$LINK" ]] && pass "verification email delivered via outbox → relay" || fail "verification email not found in Mailpit"
TOKEN="${LINK##*token=}"
[[ "$(api POST /v1/auth/email/verify "{\"token\":\"$TOKEN\"}" | jq -r .ok)" == "true" ]] && pass "email verified" || fail "verify email"

D=$(api POST /v1/domains "{\"domain\":\"$DOMAIN\"}"); DID=$(jq -r .id <<<"$D")
[[ "$DID" != null ]] && pass "domain $DOMAIN added" || fail "add domain: $D"
V=$(api POST "/v1/domains/$DID/verification-check" '{}')
[[ "$(jq -r .status <<<"$V")" == "verified" ]] && pass "domain verified" || fail "verify domain: $(jq -c .check <<<"$V")"

K=$(api POST /v1/api-keys '{"name":"smoke","environment":"live","scopes":["messages:send","messages:read"]}')
KEY=$(jq -r .key <<<"$K"); [[ "$KEY" == mv_live_* ]] && pass "API key created" || fail "api key: $K"

S=$(curl -sS "$BASE/v1/messages" -H "Authorization: Bearer $KEY" -H "Idempotency-Key: smoke-$STAMP" -H "Content-Type: application/json" \
  -d "{\"from\":\"Smoke <no-reply@$DOMAIN>\",\"to\":\"recipient-$STAMP@example.org\",\"subject\":\"Smoke $STAMP\",\"text\":\"Hello from the smoke test\",\"html\":\"<p>Hello from the <b>smoke test</b></p>\"}")
MID=$(jq -r '.messages[0].id' <<<"$S"); [[ "$MID" != null ]] && pass "message accepted ($MID)" || fail "send: $S"
S2=$(curl -sS "$BASE/v1/messages" -H "Authorization: Bearer $KEY" -H "Idempotency-Key: smoke-$STAMP" -H "Content-Type: application/json" \
  -d "{\"from\":\"Smoke <no-reply@$DOMAIN>\",\"to\":\"recipient-$STAMP@example.org\",\"subject\":\"Smoke $STAMP\",\"text\":\"Hello from the smoke test\",\"html\":\"<p>Hello from the <b>smoke test</b></p>\"}")
[[ "$(jq -r '.messages[0].id' <<<"$S2")" == "$MID" ]] && pass "idempotent retry returned the same message" || fail "idempotency"

STATUS=""
for _ in $(seq 1 20); do
  STATUS=$(curl -s "$BASE/v1/messages/$MID" -H "Authorization: Bearer $KEY" | jq -r .status)
  [[ "$STATUS" == "accepted_by_mta" || "$STATUS" == "failed" ]] && break
  sleep 1
done
[[ "$STATUS" == "accepted_by_mta" ]] && pass "worker delivered to relay (status $STATUS)" || fail "status: $STATUS"

FOUND=$(curl -s "$MAILPIT/api/v1/search?query=subject:%22Smoke%20$STAMP%22" | jq -r '.messages | length')
[[ "$FOUND" -ge 1 ]] && pass "message present at the relay (Mailpit)" || fail "not found in Mailpit"
RAWID=$(curl -s "$MAILPIT/api/v1/search?query=subject:%22Smoke%20$STAMP%22" | jq -r '.messages[0].ID')
curl -s "$MAILPIT/api/v1/message/$RAWID/raw" | tr -d '\r' | grep -A6 '^DKIM-Signature:' | grep -q "d=$DOMAIN" && pass "DKIM-Signature present for $DOMAIN" || fail "DKIM signature missing"

echo "All smoke checks passed."
