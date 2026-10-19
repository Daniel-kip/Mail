"""End-to-end test for hosted mailboxes and webmail against a real Dovecot.

Standard library only. Requires a development stack where the API has
IMAP configured and Dovecot's LMTP port is reachable (see README):

    python infra/scripts/mailbox_e2e.py --base http://127.0.0.1:8088 \
        --lmtp 127.0.0.1:2424 --imap 127.0.0.1:1143 --mailpit http://127.0.0.1:8025

Inbound delivery uses LMTP exactly as Postfix does in production.
"""
from __future__ import annotations

import argparse
import base64
import http.cookiejar
import imaplib
import json
import smtplib
import sys
import time
import urllib.error
import urllib.request
import uuid
from email.message import EmailMessage

PNG_1PX = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
)


class Client:
    def __init__(self, base: str):
        self.base = base
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar))
        self.csrf = None

    def req(self, method, path, body=None, headers=None, raw=False, form=None):
        data = None
        hdrs = dict(headers or {})
        if form is not None:
            boundary = uuid.uuid4().hex
            parts = []
            for name, value, filename, ctype in form:
                disp = f'form-data; name="{name}"' + (f'; filename="{filename}"' if filename else "")
                parts.append(f"--{boundary}\r\nContent-Disposition: {disp}\r\nContent-Type: {ctype}\r\n\r\n".encode() + value + b"\r\n")
            data = b"".join(parts) + f"--{boundary}--\r\n".encode()
            hdrs["Content-Type"] = f"multipart/form-data; boundary={boundary}"
        elif body is not None:
            data = json.dumps(body).encode()
            hdrs["Content-Type"] = "application/json"
        if self.csrf and method != "GET":
            hdrs["X-CSRF-Token"] = self.csrf
        r = urllib.request.Request(self.base + path, data=data, method=method, headers=hdrs)
        try:
            resp = self.opener.open(r, timeout=60)
            content = resp.read()
            return resp.status, (content if raw else (json.loads(content) if content else None)), resp.headers
        except urllib.error.HTTPError as e:
            content = e.read()
            try:
                return e.code, json.loads(content), e.headers
            except ValueError:
                return e.code, content, e.headers


def check(cond, label, detail=""):
    print(("PASS " if cond else "FAIL ") + label + ("" if cond else f"\n     {detail}"))
    if not cond:
        check.failed += 1


check.failed = 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8088")
    ap.add_argument("--lmtp", default="127.0.0.1:2424")
    ap.add_argument("--imap", default="127.0.0.1:1143")
    ap.add_argument("--mailpit", default="http://127.0.0.1:8025")
    ap.add_argument("--verify-email-sql", default=None, help="shell command that confirms the user's email (dev only)")
    a = ap.parse_args()
    stamp = str(int(time.time()))
    domain = f"campus{stamp}.example"

    # ── provision through the console API ──
    admin = Client(a.base)
    st, s, _ = admin.req("POST", "/v1/auth/register", {"full_name": "Mail Admin", "email": f"admin-{stamp}@example.com",
                                                      "password": "mailbox admin pass 2026", "organization_name": f"Campus {stamp}"})
    check(st == 201, "register organization admin", s)
    admin.csrf = s["csrf_token"]
    # Confirm the email via the link delivered to Mailpit.
    link = None
    for _ in range(20):
        data = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/search?query=to:admin-{stamp}@example.com").read())
        if data.get("messages"):
            text = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/message/{data['messages'][0]['ID']}").read())["Text"]
            link = text.split("verify-email?token=")[1].split()[0]
            break
        time.sleep(1)
    st, _, _ = admin.req("POST", "/v1/auth/email/verify", {"token": link})
    check(st == 200, "confirm admin email")
    st, d, _ = admin.req("POST", "/v1/domains", {"domain": domain})
    did = d["id"]
    st, d, _ = admin.req("POST", f"/v1/domains/{did}/verification-check", {})
    check(d.get("status") == "verified", "verify domain", d)
    st, d, _ = admin.req("PATCH", f"/v1/domains/{did}", {"mail_hosting_enabled": True})
    check(d.get("mail_hosting_enabled") is True, "enable hosted mailboxes", d)
    st, m, _ = admin.req("POST", "/v1/mailboxes", {"domain_id": did, "local_part": "amina", "display_name": "Amina Odhiambo"})
    check(st == 201 and m.get("password"), "create mailbox with generated password", m)
    address, password = m["address"], m["password"]
    st, al, _ = admin.req("POST", "/v1/aliases", {"domain_id": did, "local_part": "admissions", "destinations": [address]})
    check(st == 201, "create alias admissions@ -> mailbox", al)

    # ── inbound delivery over LMTP (what Postfix does) ──
    msg = EmailMessage()
    msg["From"] = "Registrar <registrar@university.example>"
    msg["To"] = address
    msg["Subject"] = f"Welcome pack {stamp}"
    msg["Message-ID"] = f"<welcome-{stamp}@university.example>"
    msg.set_content("Welcome! Your timetable is attached.")
    msg.add_alternative(
        '<p>Welcome <b>Amina</b>!</p><img src="cid:logo1"><img src="https://tracker.example/pixel.gif">'
        '<script>alert("xss")</script><a href="javascript:alert(1)">bad</a><a href="https://university.example">site</a>',
        subtype="html",
    )
    msg.get_payload()[1].add_related(PNG_1PX, "image", "png", cid="<logo1>")
    msg.add_attachment(b"Mon: Algebra\nTue: Physics\n", maintype="text", subtype="plain", filename="timetable.txt")
    host, port = a.lmtp.split(":")
    with smtplib.LMTP(host, int(port)) as lmtp:
        refused = lmtp.send_message(msg, from_addr="registrar@university.example", to_addrs=[address])
    check(not refused, "deliver inbound message via LMTP")
    alias_msg = EmailMessage()
    alias_msg["From"] = "applicant@example.org"
    alias_msg["To"] = f"admissions@{domain}"
    alias_msg["Subject"] = f"Application question {stamp}"
    alias_msg.set_content("When does term start?")
    with smtplib.LMTP(host, int(port)) as lmtp:
        lmtp.send_message(alias_msg, to_addrs=[address])  # Postfix expands the alias before LMTP

    # ── standard IMAP client with the mailbox password ──
    ihost, iport = a.imap.split(":")
    imap = imaplib.IMAP4(ihost, int(iport))
    typ, _ = imap.login(address, password)
    check(typ == "OK", "IMAP login with mailbox password (Dovecot SQL passdb, Argon2id)")
    typ, folders = imap.list()
    names = b" ".join(folders).decode()
    check(all(f in names for f in ["Sent", "Drafts", "Trash", "Junk", "Archive"]), "special-use folders auto-created", names)
    imap.select("INBOX")
    typ, data = imap.search(None, "ALL")
    check(len(data[0].split()) == 2, "both messages in INBOX via IMAP", data)
    imap.logout()
    bad = imaplib.IMAP4(ihost, int(iport))
    try:
        bad.login(address, "wrong password here")
        check(False, "IMAP rejects wrong password")
    except imaplib.IMAP4.error:
        check(True, "IMAP rejects wrong password")

    # ── webmail ──
    wm = Client(a.base)
    st, s, _ = wm.req("POST", "/v1/webmail/login", {"address": address, "password": password})
    check(st == 200, "webmail login", s)
    wm.csrf = s["csrf_token"]
    st, f, _ = wm.req("GET", "/v1/webmail/folders")
    roles = {x["role"]: x for x in f["data"]}
    check(st == 200 and roles.get("inbox", {}).get("unseen") == 2 and "sent" in roles and "trash" in roles,
          "folder list with roles and unread counts", f)
    st, lst, _ = wm.req("GET", "/v1/webmail/folders/INBOX/messages")
    welcome = next((x for x in lst["data"] if x["subject"].startswith("Welcome pack")), None)
    check(welcome and welcome["from_address"] == "registrar@university.example" and not welcome["seen"] and welcome["has_attachments"],
          "message list: sender, unread, attachment flag", lst)
    uid = welcome["uid"]
    st, rd, _ = wm.req("GET", f"/v1/webmail/folders/INBOX/messages/{uid}")
    doc = rd["html_document"]
    check("<script" not in doc and "javascript:" not in doc, "received HTML is sanitized (no script, no javascript: URLs)")
    check("data:image/png;base64," in doc, "inline cid: image converted to data URI")
    check(rd["has_remote_content"] and "img-src data:;" in doc, "remote images blocked by CSP until opted in")
    check(len(rd["attachments"]) == 1 and rd["attachments"][0]["filename"] == "timetable.txt", "attachment listed", rd["attachments"])
    st, rd2, _ = wm.req("GET", f"/v1/webmail/folders/INBOX/messages/{uid}?remote=true")
    check("img-src data: https: http:" in rd2["html_document"], "remote images allowed after opt-in")
    att_index = rd["attachments"][0]["index"]
    st, body, hdrs = wm.req("GET", f"/v1/webmail/folders/INBOX/messages/{uid}/attachments/{att_index}", raw=True)
    check(st == 200 and body.startswith(b"Mon: Algebra") and hdrs.get("Content-Type") == "application/octet-stream"
          and "attachment" in hdrs.get("Content-Disposition", ""), "attachment download is forced, neutral type", dict(hdrs))
    st, lst, _ = wm.req("GET", "/v1/webmail/folders/INBOX/messages")
    check(next(x for x in lst["data"] if x["uid"] == uid)["seen"], "opening a message marks it read")
    st, lst, _ = wm.req("GET", "/v1/webmail/folders/INBOX/messages?q=timetable")
    check(len(lst["data"]) == 1, "server-side search (IMAP TEXT)", lst)

    # Reply with an attachment -> outbox -> relay; copy saved to Sent.
    payload = {"to": ["registrar@university.example"], "subject": f"Re: Welcome pack {stamp}", "text": "Thank you!",
               "in_reply_to": rd["message_id"] and f"<{rd['message_id']}>"}
    st, sent, _ = wm.req("POST", "/v1/webmail/send", form=[("payload", json.dumps(payload).encode(), None, "application/json"),
                                                           ("attachment", b"signed form", "form.pdf", "application/pdf")])
    check(st == 202 and not sent.get("warnings"), "send reply from webmail", sent)
    delivered = False
    for _ in range(30):
        data = json.loads(urllib.request.urlopen(f"{a.mailpit}/api/v1/search?query=subject:%22Re:%20Welcome%20pack%20{stamp}%22").read())
        if data.get("messages"):
            delivered = True
            break
        time.sleep(1)
    check(delivered, "reply reached the relay (Mailpit)")
    st, sentlst, _ = wm.req("GET", f"/v1/webmail/folders/{urllib.request.quote(roles['sent']['name'])}/messages")
    check(any(x["subject"].startswith("Re: Welcome pack") for x in sentlst["data"]), "copy appended to Sent", sentlst)
    st, lst, _ = wm.req("GET", "/v1/webmail/folders/INBOX/messages")
    check(next(x for x in lst["data"] if x["uid"] == uid)["answered"], "original marked \\Answered")

    # Draft, move, delete, quota.
    st, dr, _ = wm.req("POST", "/v1/webmail/drafts", form=[("payload", json.dumps({"to": [], "subject": "Unfinished", "text": "..."}).encode(), None, "application/json")])
    check(st == 200, "save draft without recipients", dr)
    st, drafts, _ = wm.req("GET", f"/v1/webmail/folders/{urllib.request.quote(roles['drafts']['name'])}/messages")
    check(any(x["subject"] == "Unfinished" for x in drafts["data"]), "draft saved to Drafts", drafts)
    other = next(x for x in lst["data"] if x["uid"] != uid)
    st, mv, _ = wm.req("POST", "/v1/webmail/folders/INBOX/move", {"uids": [other["uid"]], "destination": roles["archive"]["name"]})
    st, arch, _ = wm.req("GET", f"/v1/webmail/folders/{urllib.request.quote(roles['archive']['name'])}/messages")
    check(len(arch["data"]) == 1, "move to Archive", arch)
    st, dl, _ = wm.req("POST", "/v1/webmail/folders/INBOX/delete", {"uids": [uid]})
    check(dl.get("result") == "moved_to_trash", "delete moves to Trash", dl)
    st, tr, _ = wm.req("GET", f"/v1/webmail/folders/{urllib.request.quote(roles['trash']['name'])}/messages")
    tuid = tr["data"][0]["uid"]
    st, dl2, _ = wm.req("POST", f"/v1/webmail/folders/{urllib.request.quote(roles['trash']['name'])}/delete", {"uids": [tuid]})
    check(dl2.get("result") == "deleted", "delete from Trash is permanent", dl2)
    st, sess, _ = wm.req("GET", "/v1/webmail/session")
    q = sess.get("quota") or {}
    check(q.get("limit_bytes", 0) > 0 and q.get("used_bytes", 0) > 0, "quota reported from Dovecot", sess)
    st, mbox, _ = admin.req("GET", f"/v1/mailboxes/{m['id']}")
    check((mbox.get("usage") or {}).get("used_bytes", 0) > 0, "console shows mailbox usage", mbox.get("usage"))

    # Isolation and disablement.
    st, _, _ = wm.req("GET", "/v1/domains")
    check(st in (401, 403), "webmail session cannot use console API")
    st, _, _ = admin.req("PATCH", f"/v1/mailboxes/{m['id']}", {"status": "disabled"})
    st, _, _ = wm.req("GET", "/v1/webmail/folders")
    check(st == 401, "disabling a mailbox ends webmail access immediately")
    imap = imaplib.IMAP4(ihost, int(iport))
    try:
        imap.login(address, password)
        check(False, "disabled mailbox cannot log in over IMAP")
    except imaplib.IMAP4.error:
        check(True, "disabled mailbox cannot log in over IMAP")

    print(f"\n{'ALL MAILBOX CHECKS PASSED' if not check.failed else f'{check.failed} CHECK(S) FAILED'}")
    return 1 if check.failed else 0


if __name__ == "__main__":
    sys.exit(main())
