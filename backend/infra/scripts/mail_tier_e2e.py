"""Mail-tier checks against real Postfix + Dovecot + Rspamd (blueprint 06/08).

Standard library only. Ports default to the local test layout used in
frontend/resources/wiki/Project-Status.md; in production run the relay/MX checks from an EXTERNAL host
(see frontend/resources/wiki/Operations.md section 5).

    python infra/scripts/mail_tier_e2e.py --base http://127.0.0.1:8088 \
        --mx 127.0.0.1:2525 --submission 127.0.0.1:2587 --relay 127.0.0.1:10587 \
        --imap 127.0.0.1:1143 --relay-user relay@platform.internal --relay-password ...
"""
from __future__ import annotations

import argparse
import imaplib
import json
import smtplib
import ssl
import sys
import time
import urllib.request

sys.path.insert(0, __import__("os").path.dirname(__file__))
from mailbox_e2e import Client, check  # noqa: E402


def smtp(hostport: str) -> smtplib.SMTP:
    host, port = hostport.split(":")
    s = smtplib.SMTP(host, int(port), timeout=30)
    s.ehlo("tester.example.net")
    return s


def insecure_tls() -> ssl.SSLContext:
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE  # development certificate is self-signed
    return ctx


def expect_refused(fn, label: str, code_prefix: str = "5"):
    try:
        fn()
        check(False, label, "accepted but should have been refused")
    except smtplib.SMTPRecipientsRefused as e:
        codes = [str(v[0]) for v in e.recipients.values()]
        check(all(c.startswith(code_prefix) for c in codes), label, e.recipients)
    except (smtplib.SMTPSenderRefused, smtplib.SMTPDataError, smtplib.SMTPResponseException) as e:
        check(str(e.smtp_code).startswith(code_prefix), label, e)
    except smtplib.SMTPAuthenticationError as e:
        check(True, label, e)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8088")
    ap.add_argument("--mailpit", default="http://127.0.0.1:8025")
    ap.add_argument("--mx", default="127.0.0.1:2525")
    ap.add_argument("--submission", default="127.0.0.1:2587")
    ap.add_argument("--relay", default="127.0.0.1:10587")
    ap.add_argument("--imap", default="127.0.0.1:1143")
    ap.add_argument("--relay-user", default="relay@platform.internal")
    ap.add_argument("--relay-password", default="dev-relay-password-123")
    a = ap.parse_args()
    stamp = str(int(time.time()))
    domain = f"school{stamp}.example"

    # Provision a tenant with a hosted mailbox.
    admin = Client(a.base)
    st, s, _ = admin.req("POST", "/v1/auth/register", {"full_name": "Tier Admin", "email": f"tier-{stamp}@example.com",
                                                      "password": "mail tier admin 2026", "organization_name": f"School {stamp}"})
    admin.csrf = s["csrf_token"]
    for _ in range(20):
        data = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/search?query=to:tier-{stamp}@example.com").read())
        if data.get("messages"):
            text = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/message/{data['messages'][0]['ID']}").read())["Text"]
            admin.req("POST", "/v1/auth/email/verify", {"token": text.split("verify-email?token=")[1].split()[0]})
            break
        time.sleep(1)
    st, d, _ = admin.req("POST", "/v1/domains", {"domain": domain})
    did = d["id"]
    admin.req("POST", f"/v1/domains/{did}/verification-check", {})
    admin.req("PATCH", f"/v1/domains/{did}", {"mail_hosting_enabled": True})
    st, m, _ = admin.req("POST", "/v1/mailboxes", {"domain_id": did, "local_part": "teacher", "quota_bytes": 60 * 1024 * 1024})
    address, password = m["address"], m["password"]
    admin.req("POST", "/v1/aliases", {"domain_id": did, "local_part": "office", "destinations": [address]})
    check(st == 201, f"provisioned {address} and office@{domain}")

    # ── MX (port 25) ──
    s = smtp(a.mx)
    expect_refused(lambda: s.sendmail("spammer@gmail.com", ["victim@yahoo.com"], "Subject: relay\r\n\r\nx"),
                   "MX refuses to relay to external domains (no open relay)")
    s.rset()
    expect_refused(lambda: s.sendmail("someone@gmail.com", [f"nobody@{domain}"], "Subject: x\r\n\r\nx"),
                   "MX rejects unknown recipients during SMTP (no backscatter)")
    s.rset()
    body = f"From: Parent <parent@gmail.com>\r\nTo: {address}\r\nSubject: Inbound via MX {stamp}\r\nMessage-ID: <mx-{stamp}@gmail.com>\r\n\r\nHello teacher.\r\n"
    refused = s.sendmail("parent@gmail.com", [address, f"office@{domain}"], body)
    check(not refused, "MX accepts mail for a hosted mailbox and its alias")
    alias_body = f"From: parent@gmail.com\r\nTo: office@{domain}\r\nSubject: Alias only {stamp}\r\n\r\nFor the office.\r\n"
    refused = s.sendmail("parent@gmail.com", [f"office@{domain}"], alias_body)
    check(not refused, "MX accepts mail for an alias address")
    code, _ = s.docmd("AUTH", "PLAIN AGFiYwBkZWY=")
    check(code >= 500, "MX does not offer AUTH (submission is separate)", code)
    s.quit()

    time.sleep(3)
    host, port = a.imap.split(":")
    im = imaplib.IMAP4(host, int(port))
    im.login(address, password)
    im.select("INBOX")
    _, ids = im.search(None, f'SUBJECT "Inbound via MX {stamp}"')
    msg_ids = ids[0].split()
    # Postfix eliminates the duplicate after alias expansion: one copy.
    check(len(msg_ids) == 1, "delivered through Postfix -> LMTP -> Dovecot, alias duplicate eliminated", ids)
    _, alias_ids = im.search(None, f'SUBJECT "Alias only {stamp}"')
    check(len(alias_ids[0].split()) == 1, "alias expanded to the mailbox by Postfix", alias_ids)
    if msg_ids:
        _, data = im.fetch(msg_ids[0], "(BODY.PEEK[HEADER])")
        headers = data[0][1].decode(errors="replace")
        check("X-Spamd-Bar" in headers or "X-Spam-Status" in headers or "Authentication-Results" in headers,
              "Rspamd milter added its headers", headers[:400])
    im.logout()

    # ── submission (587, STARTTLS + AUTH) ──
    s = smtp(a.submission)
    try:
        s.login(address, password)
        check(False, "submission refuses AUTH before STARTTLS")
    except smtplib.SMTPException:
        check(True, "submission refuses AUTH before STARTTLS")
    s.starttls(context=insecure_tls())
    s.ehlo("tester.example.net")
    s.login(address, password)
    check(True, "submission AUTH after STARTTLS with mailbox credentials")
    expect_refused(lambda: s.sendmail(f"principal@{domain}", [address], "Subject: spoof\r\n\r\nx"),
                   "submission refuses sending as another address (sender login mismatch)")
    s.rset()
    refused = s.sendmail(f"office@{domain}", [address], f"From: office@{domain}\r\nSubject: Alias send {stamp}\r\n\r\nok")
    check(not refused, "submission allows sending as an alias that delivers to the user")
    s.quit()

    # ── platform relay (10587) ──
    s = smtp(a.relay)
    try:
        s.login(address, password)
        refused = s.sendmail(address, ["victim@yahoo.com"], "Subject: x\r\n\r\nx")
        check(False, "relay port refuses mailbox users (only the platform identity)", refused)
    except (smtplib.SMTPRecipientsRefused, smtplib.SMTPSenderRefused, smtplib.SMTPAuthenticationError) as e:
        check(True, "relay port refuses mailbox users (only the platform identity)", e)
    s.quit()
    s = smtp(a.relay)
    expect_refused(lambda: s.sendmail("x@gmail.com", ["victim@yahoo.com"], "Subject: x\r\n\r\nx"),
                   "relay port refuses unauthenticated clients")
    s.quit()
    s = smtp(a.relay)
    s.login(a.relay_user, a.relay_password)
    check(True, "relay port authenticates the platform relay identity")
    s.quit()

    print(f"\n{'ALL MAIL-TIER CHECKS PASSED' if not check.failed else f'{check.failed} CHECK(S) FAILED'}")
    return 1 if check.failed else 0


if __name__ == "__main__":
    sys.exit(main())
