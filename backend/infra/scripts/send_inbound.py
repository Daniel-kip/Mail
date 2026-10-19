"""Deliver a test email into a local hosted mailbox, the way Postfix does
(LMTP to Dovecot). Use it to simulate mail arriving from the internet.

    python infra/scripts/send_inbound.py --to teacher@riverside-academy.test
    python infra/scripts/send_inbound.py --to student@riverside-academy.test --from "Coach <coach@example.org>" \
        --subject "Training moved" --body "Training is at 4pm today." --attach notes.pdf
"""
from __future__ import annotations

import argparse
import mimetypes
import os
import smtplib
import sys
import time
from email.message import EmailMessage
from email.utils import formatdate, make_msgid, parseaddr


def main() -> int:
    ap = argparse.ArgumentParser(description="Deliver a test email into a local mailbox via LMTP.")
    ap.add_argument("--to", required=True, help="hosted mailbox address (not an alias: aliases are expanded by Postfix)")
    ap.add_argument("--from", dest="sender", default="Test Sender <sender@example.org>")
    ap.add_argument("--subject", default=f"Test message {time.strftime('%H:%M:%S')}")
    ap.add_argument("--body", default="This message was delivered locally over LMTP, exactly as Postfix delivers inbound mail.")
    ap.add_argument("--html", help="optional HTML body")
    ap.add_argument("--attach", action="append", default=[], help="file to attach (repeatable)")
    ap.add_argument("--lmtp", default="127.0.0.1:2424")
    a = ap.parse_args()

    msg = EmailMessage()
    msg["From"], msg["To"], msg["Subject"] = a.sender, a.to, a.subject
    msg["Date"] = formatdate(localtime=True)
    msg["Message-ID"] = make_msgid(domain="example.org")
    msg.set_content(a.body)
    if a.html:
        msg.add_alternative(a.html, subtype="html")
    for path in a.attach:
        ctype = mimetypes.guess_type(path)[0] or "application/octet-stream"
        maintype, subtype = ctype.split("/", 1)
        with open(path, "rb") as f:
            msg.add_attachment(f.read(), maintype=maintype, subtype=subtype, filename=os.path.basename(path))

    host, port = a.lmtp.split(":")
    try:
        with smtplib.LMTP(host, int(port)) as lmtp:
            refused = lmtp.send_message(msg, from_addr=parseaddr(a.sender)[1], to_addrs=[a.to])
    except smtplib.SMTPRecipientsRefused as e:
        print(f"Refused: {e.recipients}. Is {a.to} an active hosted mailbox?")
        return 1
    except OSError as e:
        print(f"Cannot reach Dovecot LMTP on {a.lmtp}: {e}. Start the stack with start-local.cmd.")
        return 1
    if refused:
        print(f"Refused: {refused}")
        return 1
    print(f"Delivered '{a.subject}' to {a.to}. Open http://localhost:8088/mail to read it.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
