"""End-to-end smoke test through the public HTTP API (Python port of smoke.sh;
standard library only, works on Windows without bash/jq).

    python infra/scripts/smoke.py --base http://127.0.0.1:8088 --mailpit http://127.0.0.1:8025
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.request

sys.path.insert(0, __import__("os").path.dirname(__file__))
from mailbox_e2e import Client, check  # noqa: E402


def mailpit_search(mailpit: str, query: str) -> list:
    url = f"{mailpit}/api/v1/search?query={urllib.request.quote(query)}"
    return json.loads(urllib.request.urlopen(url).read()).get("messages", [])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8088")
    ap.add_argument("--mailpit", default="http://127.0.0.1:8025")
    a = ap.parse_args()
    stamp = str(int(time.time()))
    email, domain = f"smoke-{stamp}@example.com", f"smoke{stamp}.example"

    ready = json.loads(urllib.request.urlopen(f"{a.base}/readyz").read())
    check(ready.get("status") == "ready", "API ready")

    c = Client(a.base)
    st, s, _ = c.req("POST", "/v1/auth/register", {"full_name": "Smoke Test", "email": email,
                                                  "password": "smoke test passphrase 42", "organization_name": f"Smoke {stamp}"})
    check(st == 201, f"registered {email}", s)
    c.csrf = s["csrf_token"]

    token = None
    for _ in range(20):
        found = mailpit_search(a.mailpit, f"to:{email}")
        if found:
            text = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/message/{found[0]['ID']}").read())["Text"]
            m = re.search(r"verify-email\?token=([\w-]+)", text)
            token = m.group(1) if m else None
            break
        time.sleep(1)
    check(bool(token), "verification email delivered via outbox and relay")
    st, _, _ = c.req("POST", "/v1/auth/email/verify", {"token": token})
    check(st == 200, "email verified")

    st, d, _ = c.req("POST", "/v1/domains", {"domain": domain})
    check(st == 201, f"domain {domain} added", d)
    st, d, _ = c.req("POST", f"/v1/domains/{d['id']}/verification-check", {})
    check(d.get("status") == "verified", "domain verified", d.get("check"))

    st, k, _ = c.req("POST", "/v1/api-keys", {"name": "smoke", "environment": "live", "scopes": ["messages:send", "messages:read"]})
    key = k.get("key", "")
    check(key.startswith("mv_live_"), "API key created", k)

    def send():
        body = {"from": f"Smoke <no-reply@{domain}>", "to": f"recipient-{stamp}@example.org", "subject": f"Smoke {stamp}",
                "text": "Hello from the smoke test", "html": "<p>Hello from the <b>smoke test</b></p>"}
        r = urllib.request.Request(f"{a.base}/v1/messages", data=json.dumps(body).encode(), method="POST",
                                   headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json",
                                            "Idempotency-Key": f"smoke-{stamp}"})
        return json.loads(urllib.request.urlopen(r).read())

    first = send()
    mid = first["messages"][0]["id"]
    check(bool(mid), f"message accepted ({mid})")
    check(send()["messages"][0]["id"] == mid, "idempotent retry returned the same message")

    status = None
    for _ in range(30):
        r = urllib.request.Request(f"{a.base}/v1/messages/{mid}", headers={"Authorization": f"Bearer {key}"})
        status = json.loads(urllib.request.urlopen(r).read())["status"]
        if status in ("accepted_by_mta", "failed"):
            break
        time.sleep(1)
    check(status == "accepted_by_mta", f"worker delivered to relay (status {status})")

    found = mailpit_search(a.mailpit, f'subject:"Smoke {stamp}"')
    check(len(found) >= 1, "message present at the relay (Mailpit)")
    if found:
        raw = urllib.request.urlopen(f"{a.mailpit}/api/v1/message/{found[0]['ID']}/raw").read().decode(errors="replace")
        check(f"d={domain}" in raw.replace("\r\n ", " ") and "DKIM-Signature:" in raw, f"DKIM-Signature present for {domain}")

    print(f"\n{'ALL SMOKE CHECKS PASSED' if not check.failed else f'{check.failed} CHECK(S) FAILED'}")
    return 1 if check.failed else 0


if __name__ == "__main__":
    sys.exit(main())
