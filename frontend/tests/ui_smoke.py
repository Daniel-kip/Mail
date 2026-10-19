"""Real-browser smoke test of the PHP frontend, driven through Chrome DevTools.

Standard library only (a minimal WebSocket client is included), so it runs
anywhere Python 3.10+ and Chrome/Edge are installed.

    python frontend/tests/ui_smoke.py --base http://127.0.0.1:8080 \
        --mailpit http://127.0.0.1:8025 --chrome "C:/Program Files/Google/Chrome/Application/chrome.exe"

Walks a new institution from sign-up to a phone-ready mailbox: register
with username and phone, verify the SMS code (development SMS lands in
Mailpit), add and verify a domain, enable hosting, create an API key and a
template (live preview), send from the console, create a mailbox with a
handover link, activate it, open the phone setup wizard, use webmail, browse
every console page, use the deliverability tools, check billing is in KES,
and switch themes. Signed-in pages must show only sealed `/app?ref=` addresses
and refuse record addresses typed by hand.

Requires a development stack (DEV_SKIP_DNS_VERIFICATION=true, Mailpit).
Fails on any uncaught JavaScript exception, console error or CSP
violation, and saves a screenshot of every step to --out.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request


class WebSocket:
    """Just enough RFC 6455 for the DevTools protocol (text frames, client masking)."""

    def __init__(self, url: str):
        m = re.match(r"ws://([^:/]+):(\d+)(/.*)", url)
        host, port, path = m.group(1), int(m.group(2)), m.group(3)
        self.sock = socket.create_connection((host, port), timeout=60)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            f"GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode()
        )
        resp = b""
        while b"\r\n\r\n" not in resp:
            resp += self.sock.recv(4096)
        if b" 101 " not in resp.split(b"\r\n")[0]:
            raise RuntimeError(f"websocket handshake failed: {resp[:200]!r}")
        self.buf = resp.split(b"\r\n\r\n", 1)[1]

    def send(self, text: str) -> None:
        payload = text.encode()
        header = bytearray([0x81])
        n = len(payload)
        if n < 126:
            header.append(0x80 | n)
        elif n < 65536:
            header.append(0x80 | 126)
            header += struct.pack(">H", n)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", n)
        mask = os.urandom(4)
        header += mask
        self.sock.sendall(bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))

    def _read(self, n: int) -> bytes:
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("websocket closed")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def recv(self) -> str:
        message = b""
        while True:
            b1, b2 = self._read(2)
            n = b2 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._read(8))[0]
            message += self._read(n)
            if b1 & 0x80:
                return message.decode("utf-8", "replace")


class Page:
    def __init__(self, ws_url: str):
        self.ws = WebSocket(ws_url)
        self.next_id = 0
        self.errors: list[str] = []
        for method in ("Runtime.enable", "Page.enable", "Log.enable"):
            self.call(method)

    def call(self, method: str, **params):
        self.next_id += 1
        my_id = self.next_id
        self.ws.send(json.dumps({"id": my_id, "method": method, "params": params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == my_id:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})
            self._event(msg)

    def _event(self, msg: dict) -> None:
        method = msg.get("method")
        p = msg.get("params", {})
        if method == "Runtime.exceptionThrown":
            d = p["exceptionDetails"]
            self.errors.append("exception: " + (d.get("exception", {}).get("description") or d.get("text", "")))
        elif method == "Runtime.consoleAPICalled" and p.get("type") == "error":
            self.errors.append("console.error: " + " ".join(str(a.get("value", a.get("description", ""))) for a in p.get("args", [])))
        elif method == "Log.entryAdded" and p["entry"]["level"] == "error":
            text = p["entry"]["text"]
            # Expected 4xx responses from negative-path checks are not page errors.
            if "status of 4" not in text:
                self.errors.append("log: " + text)

    def js(self, expression: str):
        r = self.call("Runtime.evaluate", expression=expression, awaitPromise=True, returnByValue=True)
        if "exceptionDetails" in r:
            raise RuntimeError("evaluate failed: " + json.dumps(r["exceptionDetails"])[:400])
        return r.get("result", {}).get("value")

    def goto(self, url: str) -> None:
        self.call("Page.navigate", url=url)
        self.wait("document.readyState === 'complete'")

    def wait(self, condition: str, timeout: float = 20.0, what: str | None = None):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                v = self.js(f"(() => {{ try {{ const v = ({condition}); return (v && typeof v === 'object') ? true : v; }} catch (e) {{ return false; }} }})()")
                if v:
                    return v
            except RuntimeError:
                pass
            time.sleep(0.25)
        raise TimeoutError(f"timed out waiting for: {what or condition}\nURL: {self.js('location.href')}\nBODY: {self.js('document.body.innerText.slice(0, 600)')}")

    def screenshot(self, path: str) -> None:
        data = self.call("Page.captureScreenshot", format="png")["data"]
        with open(path, "wb") as f:
            f.write(base64.b64decode(data))


HELPERS = r"""
window.__t = {
  fill(sel, value) {
    const el = document.querySelector(sel);
    if (!el) throw new Error('no element ' + sel);
    if (el.type === 'checkbox' || el.type === 'radio') { el.checked = !!value; } else { el.value = value; }
    el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true }));
  },
  submit(sel) {
    const el = document.querySelector(sel);
    if (!el) throw new Error('no element ' + sel);
    if (el.tagName === 'FORM') { el.requestSubmit(); } else { el.click(); }
  },
  formOf(sel) {
    const el = document.querySelector(sel);
    if (!el || !el.form) throw new Error('no form around ' + sel);
    el.form.requestSubmit();
  },
  byText(sel, text) {
    return Array.from(document.querySelectorAll(sel)).find(e => e.textContent.replace(/\s+/g, ' ').includes(text));
  },
  click(sel, text) {
    const el = this.byText(sel, text);
    if (!el) throw new Error('nothing matching ' + sel + ' with text ' + text);
    el.click();
  },
  href(text) {
    const el = this.byText('a', text);
    if (!el) throw new Error('no link with text ' + text);
    return el.href;
  },
};
true
"""


SAMPLE_HEADERS = """Received: from mail.example.org (mail.example.org [203.0.113.7])
	by mx.example.net with ESMTPS id 4Xyz (version=TLS1_3 cipher=TLS_AES_256_GCM_SHA384)
	for <you@example.net>; Tue, 13 Oct 2026 09:00:05 +0000
Received: from app.example.org (app.example.org [198.51.100.2])
	by mail.example.org with ESMTPSA id 1Abc
	for <you@example.net>; Tue, 13 Oct 2026 09:00:01 +0000
Authentication-Results: mx.example.net; spf=pass smtp.mailfrom=example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org
From: Billing <billing@example.org>
To: you@example.net
Subject: Your invoice
Date: Tue, 13 Oct 2026 09:00:00 +0000
Message-ID: <inv-1@example.org>
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8080")
    ap.add_argument("--mailpit", default="http://127.0.0.1:8025")
    ap.add_argument("--chrome", default=r"C:\Program Files\Google\Chrome\Application\chrome.exe")
    ap.add_argument("--out", default=os.path.join(tempfile.gettempdir(), "mailvora-ui"))
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    port = 9333
    profile = tempfile.mkdtemp(prefix="mv-chrome-")
    chrome = subprocess.Popen([
        args.chrome, "--headless=new", f"--remote-debugging-port={port}", f"--user-data-dir={profile}",
        "--no-first-run", "--disable-gpu", "--window-size=1366,900", "about:blank",
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    results: list[tuple[str, bool, str]] = []
    B = args.base
    stamp = str(int(time.time()))
    email = f"ui-{stamp}@example.com"
    domain = f"ui{stamp}.example"
    phone = "07" + stamp[-8:]
    sms_inbox = f"sms-254{phone[1:]}@sms.localhost"
    state: dict[str, str] = {}
    try:
        for _ in range(60):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version", timeout=2)
                break
            except OSError:
                time.sleep(0.5)
        target = json.loads(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")).read())
        page = Page(target["webSocketDebuggerUrl"])

        def inject() -> None:
            # A redirect chain can replace the document between "complete" and
            # the injection; retry briefly instead of failing the step.
            for attempt in range(20):
                try:
                    page.wait("document.readyState === 'complete'")
                    page.js(HELPERS)
                    return
                except RuntimeError:
                    if attempt == 19:
                        raise
                    time.sleep(0.25)

        def go(path: str) -> None:
            page.goto(path if path.startswith("http") else B + path)
            inject()

        def navigate(action: str, what: str, expect_path: str | None = None) -> None:
            before = page.js("location.href")
            page.js("window.__navMark = 1")
            page.js(action)
            page.wait("window.__navMark === undefined && document.readyState === 'complete'", what=f"navigation after {what} (from {before})")
            inject()
            if expect_path:
                page.wait(f"location.pathname.startsWith({json.dumps(expect_path)})", what=f"path {expect_path}")

        def submit(selector: str, expect_path: str | None = None) -> None:
            navigate(f"__t.submit({json.dumps(selector)})", selector, expect_path)

        def submit_field(field: str, expect_path: str | None = None) -> None:
            """Submit the form that contains this field."""
            navigate(f"__t.formOf({json.dumps(field)})", f"form of {field}", expect_path)

        def press(label: str, expect_path: str | None = None) -> None:
            """Click the button with this label."""
            navigate(f"__t.click('button', {json.dumps(label)})", f"button {label!r}", expect_path)

        def follow(label: str) -> None:
            """Open the link with this label (signed-in links are sealed)."""
            go(page.js(f"__t.href({json.dumps(label)})"))

        def sealed() -> None:
            """Signed-in pages show only opaque addresses."""
            page.wait("location.pathname === '/app' && new URLSearchParams(location.search).get('ref')", what="sealed address in the address bar")
            if page.js("!!document.querySelector('.error-page, [data-error-status]') || /^Error [0-9]{3}$/m.test(document.body.innerText)"):
                raise RuntimeError(f"error page at {page.js('document.title')}")
            leaks = page.js("Array.from(document.querySelectorAll('a[href],form[action],[src]')).map(e => e.getAttribute('href') || e.getAttribute('action') || e.getAttribute('src')).filter(u => /^\\/(console|admin)(\\/|$|\\?)|^\\/mail(\\/(?!login|activate|reset-password|forgot-password|logout)|$|\\?)/.test(u))")
            if leaks:
                raise RuntimeError(f"readable private addresses on {page.js('document.title')}: {leaks[:5]}")

        def text_has(needle: str) -> None:
            page.wait(f"document.body.innerText.includes({json.dumps(needle)})", what=f"text {needle!r}")

        def step(name, fn) -> None:
            before = len(page.errors)
            try:
                fn()
                new = page.errors[before:]
                results.append((name, not new, "; ".join(new)))
            except Exception as e:  # noqa: BLE001
                results.append((name, False, str(e)[:700]))
            page.screenshot(os.path.join(args.out, re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-") + ".png"))

        def mailpit_token(to: str, pattern: str) -> str:
            q = urllib.parse.quote(f"to:{to}")
            for _ in range(40):
                data = json.loads(urllib.request.urlopen(f"{args.mailpit}/api/v1/search?query={q}").read())
                for m in data.get("messages") or []:
                    text = json.loads(urllib.request.urlopen(f"{args.mailpit}/api/v1/message/{m['ID']}").read())["Text"]
                    found = re.search(pattern, text)
                    if found:
                        return found.group(1)
                time.sleep(1)
            raise RuntimeError(f"no email matching {pattern} for {to}")

        def public_pages():
            for path, needle in [("/", "For institutions"), ("/pricing", "KES 1,200"), ("/pricing?for=developers", "KES 200"),
                                 ("/developers", "Idempotency-Key"), ("/security", "DKIM"), ("/mail/login", "Work email address"), ("/tools", "Domain health check"), ("/docs", "Administrator guide"), ("/docs/sending-api", "Mailvora-Signature"),
                                 ("/.well-known/security.txt", "Contact:")]:
                go(path)
                text_has(needle)
                # php -S is single-threaded: stop the hero video streaming so it
                # cannot hold the only worker while later pages load.
                page.js("document.querySelectorAll('video').forEach(v => { v.pause(); v.removeAttribute('src'); v.load(); }); true")

        def register():
            go("/register?type=institution")
            page.js("__t.fill('input[name=account_type][value=institution]', true)")
            page.js(f"__t.fill('#f-full_name', 'UI Tester'); __t.fill('#f-organization_name', 'UI Academy {stamp}'); __t.fill('#f-email', {json.dumps(email)});"
                    f"__t.fill('#f-username', 'ui{stamp}'); __t.fill('#f-phone', '{phone}');"
                    "__t.fill('#f-password', 'a long ui test password'); __t.fill('input[name=terms]', true)")
            submit("form[action='/register']", "/app")
            text_has("Enter the code we texted")

        def verify_phone():
            code = mailpit_token(sms_inbox, r"\b(\d{6}) is your")
            page.js(f"__t.fill('#code', '{code}')")
            submit_field("#code", "/app")
            text_has("Your phone number is verified")

        def domain_flow():
            go("/console/domains")
            page.js(f"__t.fill('#domain', {json.dumps(domain)})")
            submit_field("#domain", "/app")
            text_has(domain)
            sealed()
            state["domain_url"] = page.js("location.href")
            press("Check DNS")
            text_has("domain is verified")
            press("Enable mail hosting")
            text_has("Mail hosting enabled")
            # A record address typed by hand is refused.
            go(f"/console/domains/{'0' * 8}-0000-7000-8000-{'0' * 12}")
            text_has("can only be opened from inside the app")

        def api_key():
            go("/console/api-keys")
            page.js("document.querySelector('details.card').open = true; __t.fill('#f-name', 'UI key')")
            submit_field("#f-name")
            page.wait("document.querySelector('#new-key') && document.querySelector('#new-key').textContent.length > 20", what="one-time key")
            go("/console/api-keys")
            page.wait("!document.querySelector('#new-key')", what="key hidden after reload")

        def template_preview():
            go("/console/templates/new")
            page.wait("document.querySelector('#preview-subject').textContent.includes('INV-1042')", what="live preview subject")
            page.wait("(document.querySelector('#preview-frame').srcdoc || '').includes('KES 1,200')", what="live preview HTML")
            page.js("__t.fill('#f-name', 'UI invoice')")
            submit("form[data-template-editor]", "/app")
            text_has("Template created")

        def console_send():
            go("/console/messages/compose")
            page.js(f"__t.fill('#f-from', 'hello@{domain}'); __t.fill('#f-to', 'someone@example.org'); __t.fill('#f-subject', 'UI hello');"
                    "__t.fill('#f-text', 'Sent from the browser test')")
            submit_field("#f-subject", "/app")
            text_has("Message queued")
            text_has("someone@example.org")
            if page.js("document.body.innerText.includes('Page not found')"):
                raise RuntimeError("sending ended on an error page")

        def mailbox_link():
            go("/console/mailboxes/new")
            page.js("__t.fill('#f-display_name', 'Wanjiru Kamau'); __t.fill('#f-local_part', 'wanjiru'); __t.fill('input[name=setup_method][value=link]', true)")
            submit_field("#f-local_part", "/app")
            page.wait("document.querySelector('#handover-link')", what="handover link")
            state["activation"] = page.js("document.querySelector('#handover-link').textContent")
            page.wait("document.querySelector('.secret-box svg')", what="handover QR code")
            follow("Welcome sheet")
            text_has("Welcome to your work email")

        def activate_and_setup():
            go(re.sub(r"^https?://[^/]+", B, state["activation"]))
            text_has("Activate your work email")
            page.js("__t.fill('#f-password', 'green hills phone 7'); __t.fill('#f-password_confirmation', 'green hills phone 7')")
            submit("form[action='/mail/activate']", "/app")
            text_has(f"wanjiru@{domain}")
            go("/mail/setup/device")
            go("/mail/setup/app?device=android")
            text_has("Gmail")
            go("/mail/setup/check?device=android&app=gmail")
            text_has("Checking your account")
            sealed()
            follow("Continue")
            for _ in range(12):
                if page.js("document.body.innerText.includes('Incoming server')"):
                    break
                follow("Next")
            text_has("Incoming server")
            go("/mail/setup/help")
            follow("send")
            go("/mail/setup/verify?guide=android-gmail")
            press("Send me a code")
            text_has("Code from the message")

        def webmail():
            go("/mail")
            page.wait("document.querySelector('.mail-list, .mail-item, .empty-state')", what="inbox")
            sealed()
            page.wait("document.querySelector('.mail-item a')", what="connection-code message in the inbox")
            page.js("document.querySelector('.mail-item a').click()")
            page.wait("document.querySelector('iframe.mail-frame')", what="reading pane")
            go(f"/mail/compose?to=wanjiru@{domain}")
            page.js("__t.fill('#subject', 'Note to self'); __t.fill('#text', 'Hello from webmail')")
            submit("button[name=intent][value=send]")
            text_has("Message sent")
            for path in ("/mail/settings", "/mail/contacts", "/mail/directory"):
                go(path)
            text_has("Wanjiru Kamau")

        def browse_console():
            for path in ["/console", "/console/usage", "/console/developers", "/console/domains", state.get("domain_url", "/console/domains"),
                         "/console/mailboxes", "/console/mailboxes/onboarding", "/console/mailboxes/import", "/console/aliases", "/console/api-keys",
                         "/console/templates", "/console/messages", "/console/suppressions", "/console/webhooks", "/console/team",
                         "/console/billing", "/console/billing/plans", "/console/audit", "/console/settings", "/console/account"]:
                go(path)
                if page.js("!!document.querySelector('main') && !document.body.innerText.includes('Reference:')") is not True:
                    raise RuntimeError(f"{path} rendered an error page")
                sealed()

        def tools():
            go(f"/tools/generator?domain={domain}&providers[]=google&policy=quarantine")
            text_has(f"Records for {domain}")
            text_has("v=spf1")
            go("/tools/headers")
            page.js("__t.fill('#t-headers', " + json.dumps(SAMPLE_HEADERS) + ")")
            submit("form[action='/tools/headers']", "/tools/headers")
            text_has("Delivery path")
            go("/tools/dns?name=localhost&type=A")
            if page.js("document.body.innerText.includes('Reference:')"):
                raise RuntimeError("DNS lookup rendered an error page")

        def billing_kes():
            go("/console/billing/plans")
            text_has("KES")
            if page.js("/\\$\\s?\\d|USD/.test(document.querySelector('main').innerText)"):
                raise RuntimeError("billing shows dollar amounts")

        def theme_and_mfa():
            go("/console/account")
            page.js("document.querySelector('[data-theme-set=dark]').click()")
            page.wait("document.documentElement.getAttribute('data-theme') === 'dark'", what="dark theme applied")
            go("/console")
            page.wait("document.documentElement.getAttribute('data-theme') === 'dark'", what="dark theme remembered")
            page.js("document.querySelector('.help-widget > summary').click()")
            page.wait("document.querySelector('.help-widget').open", what="help widget opens")
            go("/console/account")
            page.js("document.querySelector('[data-theme-set=system]').click()")
            press("Set up two-factor")
            page.wait("document.querySelector('.qr-box svg')", what="MFA QR code")

        def sign_out():
            submit("form[action='/logout']", "/login")

        step("public site", public_pages)
        step("register institution", register)
        step("verify phone with SMS code", verify_phone)
        step("domain verify and hosting", domain_flow)
        step("API key shown once", api_key)
        step("template live preview", template_preview)
        step("send from console", console_send)
        step("mailbox with handover link", mailbox_link)
        step("activate and phone setup wizard", activate_and_setup)
        step("webmail read and send", webmail)
        step("every console page", browse_console)
        step("deliverability tools", tools)
        step("billing in Kenyan shillings", billing_kes)
        step("theme switch, help and MFA QR", theme_and_mfa)
        step("sign out", sign_out)
    finally:
        chrome.terminate()
    width = max((len(n) for n, _, _ in results), default=10)
    for name, ok, detail in results:
        print(f"{'PASS' if ok else 'FAIL'}  {name.ljust(width)}" + ("" if ok else f"\n      {detail}"))
    failed = sum(1 for _, ok, _ in results if not ok)
    print(f"\n{len(results) - failed} passed, {failed} failed. Screenshots: {args.out}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
