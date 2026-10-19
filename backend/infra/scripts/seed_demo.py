"""Seed realistic demo data on a local stack and print the credentials.

Creates two customers that exercise both products:

* Riverside Academy (institution): verified domain with hosted mailboxes,
  an ICT support member, a shared alias, support contacts, a 14-day trial
  of Premium (KES 1,200/month), a template, API keys and sample mail. The three
  mailboxes are provisioned through each onboarding path the platform
  offers: administrator-set password, handover activation link, and an
  emailed invitation (completed through the link caught in Mailpit).
* Kevin Mwangi (individual developer): a verified sending domain and a
  restricted API key that relays invoices from his application.

Standard library only. Requires the API, Mailpit and Dovecot from
start-local.cmd. Safe to run again: existing demo data is detected and the
saved credentials are printed.

    python infra/scripts/seed_demo.py   (from backend/)
"""
from __future__ import annotations

import argparse
import json
import os
import re
import smtplib
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from email.message import EmailMessage
from email.utils import formatdate

sys.path.insert(0, os.path.dirname(__file__))
from mailbox_e2e import Client  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))  # backend/
CRED_FILE = os.path.join(ROOT, "demo-credentials.txt")


def find_web() -> str:
    """The PHP frontend's address (start-local picks 8080, or 8081-8090 if taken)."""
    if os.environ.get("PUBLIC_BASE_URL"):
        return os.environ["PUBLIC_BASE_URL"].rstrip("/")
    for port in range(8080, 8091):
        try:
            body = urllib.request.urlopen(f"http://127.0.0.1:{port}/healthz", timeout=2).read().decode()
            if '"web":"ok"' in body:
                return f"http://127.0.0.1:{port}"
        except Exception:  # noqa: BLE001
            continue
    return "http://127.0.0.1:8080"


WEB = find_web()

DOMAIN = "riverside-academy.test"
ADMIN_EMAIL = "admin@riverside-academy.test"
ADMIN_PASSWORD = "Riverside console 2026"
ICT_EMAIL = "ict@riverside-academy.test"
ICT_PASSWORD = "Riverside support 2026"
MAILBOX_PASSWORD = "Riverside mail 2026"

DEV_DOMAIN = "kevin-invoices.test"
DEV_EMAIL = "kevin@kevin-invoices.test"
DEV_PASSWORD = "Mombasa relay 2026"


def fail(msg, detail=None):
    print(f"ERROR: {msg}" + (f"\n  {detail}" if detail else ""))
    sys.exit(1)


def ok(msg):
    print(f"  + {msg}")


class Mailpit:
    def __init__(self, base: str):
        self.base = base

    def link(self, to: str, pattern: str, after: float = 0, timeout: int = 40) -> str:
        """Token from the newest message to `to` whose text matches `pattern`."""
        q = urllib.parse.quote(f"to:{to}")
        for _ in range(timeout):
            data = json.loads(urllib.request.urlopen(f"{self.base}/api/v1/search?query={q}").read())
            for m in data.get("messages") or []:
                text = json.loads(urllib.request.urlopen(f"{self.base}/api/v1/message/{m['ID']}").read())["Text"]
                found = re.search(pattern, text)
                if found:
                    return found.group(1)
            time.sleep(1)
        fail(f"no email matching {pattern!r} arrived for {to} (is the API worker running?)")
        return ""


def register(base, mailpit, full_name, email, password, account_type, org=None):
    c = Client(base)
    body = {"full_name": full_name, "email": email, "password": password, "account_type": account_type}
    if org:
        body["organization_name"] = org
    st, s, _ = c.req("POST", "/v1/auth/register", body)
    if st != 201:
        fail(f"registration of {email} failed", s)
    c.csrf = s["csrf_token"]
    token = mailpit.link(email, r"verify-email\?token=([\w-]+)")
    c.req("POST", "/v1/auth/email/verify", {"token": token})
    return c


def must(result, label, expect=(200, 201, 202)):
    st, body, _ = result
    if st not in expect:
        fail(label, body)
    return body


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8088")
    ap.add_argument("--mailpit", default="http://127.0.0.1:8025")
    ap.add_argument("--lmtp", default="127.0.0.1:2424")
    ap.add_argument("--exe", default=os.path.join(ROOT, "target", "debug", "mailvora.exe"))
    a = ap.parse_args()
    mailpit = Mailpit(a.mailpit)

    probe = Client(a.base)
    st, _, _ = probe.req("POST", "/v1/auth/login", {"email": ADMIN_EMAIL, "password": ADMIN_PASSWORD})
    if st == 200 and os.path.exists(CRED_FILE):
        print("Demo data already exists.\n")
        print(open(CRED_FILE, encoding="utf-8").read())
        return 0
    if st == 200:
        fail("The demo admin exists but demo-credentials.txt is missing. Run stop-local.cmd -Reset to start fresh.")

    # ?????? institution ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
    print("Seeding Riverside Academy (institution)...")
    c = register(a.base, mailpit, "Daniel Otieno", ADMIN_EMAIL, ADMIN_PASSWORD, "institution", "Riverside Academy")
    ok(f"owner {ADMIN_EMAIL} (email confirmed through the Mailpit link)")
    if os.path.exists(a.exe):
        r = subprocess.run([os.path.abspath(a.exe), "promote-admin", ADMIN_EMAIL], cwd=ROOT, capture_output=True, text=True)
        ok("granted platform-operator rights" if r.returncode == 0 else f"could not promote admin: {r.stderr.strip()[:200]}")

    d = must(c.req("POST", "/v1/domains", {"domain": DOMAIN}), "adding domain failed")
    did = d["id"]
    c.req("POST", f"/v1/domains/{did}/verification-check", {})
    must(c.req("PATCH", f"/v1/domains/{did}", {"mail_hosting_enabled": True}), "enabling hosting failed")
    ok(f"domain {DOMAIN} verified with hosted mailboxes enabled")

    must(c.req("PUT", "/v1/organization/support", {
        "email": ICT_EMAIL, "phone": "+254 700 123 456", "hours": "Mon-Fri, 8:00-17:00",
        "url": None, "setup_help_text": "Stuck? Visit the ICT office in the admin block, room 4.",
    }), "saving support contacts failed")
    sub = must(c.req("POST", "/v1/billing/subscription", {"plan_id": "pro", "billing_cycle": "monthly"}), "starting the trial failed")
    ok(f"support contacts saved; Premium {sub.get('outcome', 'changed').replace('_', ' ')}")

    # Principal: administrator-set password (quickest for a demo).
    must(c.req("POST", "/v1/mailboxes", {"domain_id": did, "local_part": "principal", "display_name": "Grace Mutua",
                                        "setup_method": "password", "password": MAILBOX_PASSWORD,
                                        "department": "Administration", "job_title": "Principal",
                                        "recovery_email": "grace.mutua@example.org"}), "creating principal@ failed")
    # Teacher: handover activation link, completed as the teacher would.
    m = must(c.req("POST", "/v1/mailboxes", {"domain_id": did, "local_part": "teacher", "display_name": "Amina Odhiambo",
                                            "setup_method": "link", "department": "Mathematics", "job_title": "Teacher"}),
             "creating teacher@ failed")
    token = re.search(r"token=([\w-]+)", m["activation"]["url"]).group(1)
    must(Client(a.base).req("POST", "/v1/webmail/activate", {"token": token, "password": MAILBOX_PASSWORD}), "activating teacher@ failed")
    # Student: emailed invitation to a personal address, completed from Mailpit.
    must(c.req("POST", "/v1/mailboxes", {"domain_id": did, "local_part": "student", "display_name": "Brian Kiprop",
                                        "setup_method": "invite", "recovery_email": "brian.kiprop@example.org",
                                        "department": "Form Three"}), "creating student@ failed")
    token = mailpit.link("brian.kiprop@example.org", r"mail/activate\?token=([\w-]+)")
    must(Client(a.base).req("POST", "/v1/webmail/activate", {"token": token, "password": MAILBOX_PASSWORD}), "activating student@ failed")
    c.req("POST", "/v1/aliases", {"domain_id": did, "local_part": "office", "destinations": [f"principal@{DOMAIN}", f"teacher@{DOMAIN}"]})
    ok("mailboxes principal@ (set password), teacher@ (activation link), student@ (emailed invitation) and alias office@")

    # ICT support member: registers, then accepts the emailed invitation.
    must(c.req("POST", "/v1/members/invitations", {"email": ICT_EMAIL, "role": "support"}), "inviting ICT support failed")
    ict = register(a.base, mailpit, "Peter Njoroge", ICT_EMAIL, ICT_PASSWORD, "individual")
    token = mailpit.link(ICT_EMAIL, r"invitations/([\w-]+)")
    must(ict.req("POST", f"/v1/invitations/{token}/accept", {}), "accepting the ICT invitation failed")
    ok(f"ICT support member {ICT_EMAIL} (can unlock accounts and send reset links, never sees passwords)")

    live = must(c.req("POST", "/v1/api-keys", {"name": "School website", "environment": "live", "description": "Admissions notifications",
                                              "scopes": ["messages:send", "messages:read", "templates:read"],
                                              "allowed_sender_domains": [DOMAIN]}), "creating the live key failed")
    test = must(c.req("POST", "/v1/api-keys", {"name": "Staging (test mode)", "environment": "test",
                                              "scopes": ["messages:send", "messages:read"]}), "creating the test key failed")
    ok("API keys: live (sender domain restricted) and test")

    must(c.req("POST", "/v1/templates", {
        "name": "Admission confirmation", "slug": "admission-confirmation",
        "description": "Sent when an application is accepted",
        "subject": "Welcome to {{ school }}, {{ first_name }}!",
        "html_body": "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:#f2f7f5;padding:24px 0\"><tr><td align=\"center\">"
                     "<table role=\"presentation\" width=\"100%\" style=\"max-width:560px;background:#ffffff;border-radius:8px;font-family:Arial,sans-serif\">"
                     "<tr><td style=\"background:#214b43;color:#ffffff;padding:18px 32px;font-size:18px;font-weight:bold\">{{ school }}</td></tr><tr><td style=\"padding:28px 32px\">"
                     "<h1 style=\"margin:0 0 12px;font-size:20px;color:#102c29\">Congratulations, {{ first_name }}!</h1>"
                     "<p style=\"font-size:15px;line-height:22px;color:#374151\">Your application to <b>{{ programme }}</b> has been accepted. Term starts on {{ start_date }}.</p>"
                     "<a href=\"{{ portal_url }}\" style=\"display:inline-block;background:#214b43;color:#ffffff;padding:11px 20px;border-radius:6px;text-decoration:none;font-weight:bold\">Open the student portal</a>"
                     "</td></tr></table></td></tr></table>",
        "text_body": "Congratulations, {{ first_name }}!\n\nYour application to {{ programme }} at {{ school }} has been accepted.\nTerm starts on {{ start_date }}.\nPortal: {{ portal_url }}",
        "variables": [
            {"name": "first_name", "required": True, "sample": "Wanjiru"},
            {"name": "programme", "required": True, "sample": "Form One"},
            {"name": "school", "required": False, "default": "Riverside Academy", "sample": "Riverside Academy"},
            {"name": "start_date", "required": True, "sample": "5 January 2027"},
            {"name": "portal_url", "required": False, "default": "https://portal.riverside-academy.test", "sample": "https://portal.riverside-academy.test"},
        ],
    }), "creating the template failed")
    ok("template admission-confirmation")

    def send(key, body):
        r = urllib.request.Request(f"{a.base}/v1/messages", data=json.dumps(body).encode(), method="POST",
                                   headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
        return json.loads(urllib.request.urlopen(r).read())

    for first, prog, to in [("Wanjiru", "Form One", "wanjiru.parent@example.org"), ("Kevin", "Form Two", "kevin.parent@example.org")]:
        send(live["key"], {"from": f"Riverside Admissions <admissions@{DOMAIN}>", "to": to, "template": "admission-confirmation",
                           "variables": {"first_name": first, "programme": prog, "start_date": "5 January 2027"},
                           "reply_mode": "reply_to_mailbox", "reply_to": f"office@{DOMAIN}", "tags": ["admissions"]})
    send(live["key"], {"from": f"Riverside Academy <news@{DOMAIN}>", "to": ["parents-list@example.org"], "subject": "Term dates for 2027",
                       "text": "Term one runs from 5 January to 2 April 2027.", "tags": ["announcements"], "unsubscribe": True})
    ok("sent 3 messages through the API (one with RFC 8058 one-click unsubscribe)")

    host, port = a.lmtp.split(":")
    inbound = [
        (f"teacher@{DOMAIN}", "Parent <mary.wambui@example.org>", "Question about homework",
         "Good evening Ms Odhiambo,\n\nCould you share this week's maths homework? Brian was absent on Tuesday.\n\nThank you,\nMary"),
        (f"teacher@{DOMAIN}", "Ministry Circulars <circulars@education.example>", "Updated examination timetable",
         "Please find the updated timetable attached."),
        (f"principal@{DOMAIN}", f"Board Secretary <board@{DOMAIN}>", "Board meeting agenda",
         "Dear Principal,\n\nThe board meets on Friday at 10:00. Agenda items: budget, new laboratory, staffing."),
        (f"student@{DOMAIN}", f"Library <library@{DOMAIN}>", "Your book is due tomorrow",
         "Reminder: 'Things Fall Apart' is due back tomorrow."),
    ]
    with smtplib.LMTP(host, int(port)) as lmtp:
        for i, (to, frm, subject, body) in enumerate(inbound):
            msg = EmailMessage()
            msg["From"], msg["To"], msg["Subject"] = frm, to, subject
            msg["Message-ID"] = f"<demo-{i}-{int(time.time())}@example.org>"
            msg["Date"] = formatdate(time.time() - (len(inbound) - i) * 5400, localtime=True)
            msg.set_content(body)
            if "timetable" in subject:
                msg.add_alternative("<p>Please find the <b>updated timetable</b> attached.</p>", subtype="html")
                msg.add_attachment(b"Day,Paper\nMonday,Mathematics\nTuesday,English\n", maintype="text", subtype="csv", filename="timetable.csv")
            lmtp.send_message(msg, to_addrs=[to])
    ok("delivered 4 inbound emails into the mailboxes (LMTP, as Postfix would)")

    # ?????? individual developer ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
    print("Seeding Kevin Mwangi (individual developer)...")
    k = register(a.base, mailpit, "Kevin Mwangi", DEV_EMAIL, DEV_PASSWORD, "individual")
    dd = must(k.req("POST", "/v1/domains", {"domain": DEV_DOMAIN}), "adding the developer domain failed")
    k.req("POST", f"/v1/domains/{dd['id']}/verification-check", {})
    dev_key = must(k.req("POST", "/v1/api-keys", {"name": "Invoicing app", "environment": "live", "description": "Relays invoices and receipts",
                                                 "scopes": ["messages:send", "messages:read"], "allowed_sender_domains": [DEV_DOMAIN]}),
                   "creating the developer key failed")
    send(dev_key["key"], {"from": f"Kevin Invoices <billing@{DEV_DOMAIN}>", "to": "client@example.org", "subject": "Invoice INV-1042",
                          "html": "<p>Hello,</p><p>Invoice <b>INV-1042</b> for KES 4,500 is attached to your account.</p>",
                          "text": "Hello,\n\nInvoice INV-1042 for KES 4,500 is ready.", "tags": ["invoice"]})
    ok(f"developer {DEV_EMAIL}: domain {DEV_DOMAIN}, restricted key, one invoice sent")

    creds = f"""Mailvora local demo credentials (development only)
==================================================

Website, console and webmail: {WEB}

Institution: Riverside Academy
  Console owner + platform operator   {ADMIN_EMAIL}  /  {ADMIN_PASSWORD}
  ICT support member                  {ICT_EMAIL}    /  {ICT_PASSWORD}
  Operator console                    {WEB}/admin

  Webmail ({WEB}/mail), password for all three: {MAILBOX_PASSWORD}
    principal@{DOMAIN}   (password set by an administrator)
    teacher@{DOMAIN}     (activated through a handover link)
    student@{DOMAIN}     (activated from an emailed invitation)
    office@{DOMAIN}      alias -> principal + teacher
  Phone setup guide: sign in to webmail, then {WEB}/mail/setup

  API keys (Authorization: Bearer <key>)
    Live (delivers to Mailpit; From must be @{DOMAIN}): {live['key']}
    Test (processed, never delivered):                   {test['key']}

Individual developer: Kevin Mwangi
  Console   {DEV_EMAIL}  /  {DEV_PASSWORD}
  API key   {dev_key['key']}   (From must be @{DEV_DOMAIN})

Try the API:
  curl -X POST {a.base}/v1/messages ^
    -H "Authorization: Bearer {dev_key['key']}" -H "Content-Type: application/json" ^
    -d "{{\\"from\\":\\"billing@{DEV_DOMAIN}\\",\\"to\\":\\"someone@example.org\\",\\"subject\\":\\"Receipt\\",\\"text\\":\\"Thank you\\"}}"

Caught mail (everything the platform sends): http://127.0.0.1:8025
Local IMAP for a desktop mail app: 127.0.0.1:1993 (self-signed TLS) or 1143 (plain, local only)
"""
    with open(CRED_FILE, "w", encoding="utf-8") as f:
        f.write(creds)
    print("\nDone. Saved to demo-credentials.txt\n")
    print(creds)
    return 0


if __name__ == "__main__":
    sys.exit(main())
