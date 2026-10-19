**Quick start** explains how to run the whole Mailvora platform on a Windows development machine, load demo data and run the automated checks.

Everything runs on your Windows machine. Nothing is sent to the internet:
all outgoing mail is caught by Mailpit, and incoming mail is simulated.

## What you need

| Tool | Why | Check |
|------|-----|-------|
| Docker Desktop | Runs Postgres, Mailpit (mail catcher) and Dovecot (mailbox server) | It starts automatically if installed |
| Rust (rustup) | Builds the API | `cargo --version` |
| A Rust linker | Either Visual Studio Build Tools ("Desktop development with C++") **or** the Rust GNU toolchain plus MSYS2 GCC | Already set up on this machine |
| Python 3.10+ | Demo data and test scripts | `python --version` |
| Chrome or Edge | Your browser (and the automated browser test) | |

## 1. Start everything

Double-click **`start-local.cmd`** in the project folder (the one containing
`backend` and `frontend`), or run it from a terminal. It needs PHP 8.2+ on
PATH (`winget install PHP.PHP.8.5`). On the first run it:

1. starts Docker Desktop if needed, then Postgres, Mailpit and Dovecot,
2. creates `backend\.env` with a fresh encryption key,
3. builds the API (the first build takes several minutes; later builds are fast),
4. runs the API in the background, then starts the PHP web app and prints its address.
   `backend\stop-local.cmd` stops both.

The root **`start-local.cmd`** starts the PHP web app too (or run `frontend\start-local.cmd`, which calls it). If port 8080 is taken by another program, the launcher uses the next free port (8081–8090), prints it, and passes it to the API so emailed links match. Then open:

| Address | What |
|---------|------|
| <http://127.0.0.1:8080> (or the printed port) | Website, console, operator console (`/admin`), webmail (`/mail`), phone setup (`/mail/setup`) |
| <http://localhost:8088> | Rust API (local only) |
| <http://localhost:8025> | Mailpit: every email the platform sends (verification links, API sends, webmail replies) |

To keep your API terminal free, use `backend\start-local.cmd -Background`. The log goes
to `backend\server.log`.

## 2. Load demo data (recommended)

With the API running, double-click **`backend\seed-demo.cmd`**. It creates a
ready-to-use school, "Riverside Academy", and writes all logins to
**`demo-credentials.txt`**:

- console admin `admin@riverside-academy.test` / `Riverside console 2026`
  (also a platform operator, so you can see the operator console)
- mailboxes `principal@`, `teacher@`, `student@riverside-academy.test` /
  `Riverside mail 2026`, and an alias `office@`
- a live and a test API key, an "Admission confirmation" template, a few
  sent messages, and some received emails in the mailboxes

Running it again just shows the saved credentials.

## 3. Things to try

**Console (sign in as the demo admin)**
- *Overview:* quota tiles and the 14-day delivery chart.
- *Domains:* open `riverside-academy.test` to see the exact DNS records a
  real customer would publish. Add a new domain and select **Check DNS now**.
  In development, DNS checks pass automatically (`DEV_SKIP_DNS_VERIFICATION`);
  in production they query real DNS.
- *Messages → Compose:* send a message, watch its status move to
  "accepted", open it to see the event timeline, then find it in Mailpit.
- *Templates:* edit the admission template and watch the live preview
  (desktop, mobile, plain text); save a new version, then roll back.
- *API keys:* create a key. It is shown once only. Revoke or rotate it.
- *Mailboxes:* create a mailbox (a strong password is generated and shown
  once), reset a password, disable a mailbox and confirm the user is signed
  out of webmail immediately.
- *Members:* invite someone; the invitation email appears in Mailpit, and
  the link in it opens the acceptance page.
- *Settings:* turn on two-factor authentication by scanning the QR code with
  any authenticator app, then sign out and back in.
- *Operator console* (sidebar → Platform): suspend an organization and watch
  its API keys, sending and mailboxes stop; reinstate it.

**Webmail (sign in as `teacher@riverside-academy.test`)**
- Read the received emails. The timetable email has an attachment.
- Reply, reply all and forward; the reply appears in Mailpit and in *Sent*.
- Save a draft, move a message to Archive, delete to Trash, and search.
- Keyboard shortcuts: `c` compose, `j`/`k` next/previous, `r` reply,
  `e` archive, `Delete` delete.

**Receive a new email** (simulates mail arriving from the internet):

```
backend\send-test-mail.cmd --to teacher@riverside-academy.test --from "Parent <parent@example.org>" --subject "Hello" --body "Testing inbound mail"
backend\send-test-mail.cmd --to student@riverside-academy.test --attach C:\path\to\file.pdf
```

**The API from a terminal** (use the live key from `demo-credentials.txt`):

```
curl -X POST http://localhost:8088/v1/messages ^
  -H "Authorization: Bearer YOUR_LIVE_KEY" -H "Content-Type: application/json" ^
  -d "{\"from\":\"no-reply@riverside-academy.test\",\"to\":\"someone@example.org\",\"subject\":\"Hello\",\"text\":\"It works\"}"

curl -X POST http://localhost:8088/v1/messages ^
  -H "Authorization: Bearer YOUR_LIVE_KEY" -H "Content-Type: application/json" ^
  -d "{\"from\":\"admissions@riverside-academy.test\",\"to\":\"parent@example.org\",\"template\":\"admission-confirmation\",\"variables\":{\"first_name\":\"Achieng\",\"programme\":\"Form One\",\"start_date\":\"5 January 2027\"}}"
```

The in-app **API quickstart** page has Node, Python and PHP examples.

**Mail apps** (Thunderbird, Outlook): IMAP server `127.0.0.1`, port `1993`
(SSL), your mailbox address and password. Accept the self-signed
development certificate when the app asks.

## 4. Run the automated checks

With both API and frontend running, double-click **`backend\run-tests.cmd`**. It runs:

1. the Rust unit and integration tests (each integration test uses its own
   temporary database),
2. the end-to-end smoke test (API → outbox → relay → DKIM),
3. the mailbox and webmail test against Dovecot,
4. the console in a headless browser.

It finishes with a PASS/FAIL list. Add `-MailTier` to also start the real
Postfix + Rspamd containers and check relay protection, recipient validation
and submission rules. The end-to-end checks add test organizations to your
local database; `backend\stop-local.cmd -Reset` clears them.

## 5. Stop or reset

| Command | Effect |
|---------|--------|
| Ctrl+C in the API and frontend windows | Stops those servers |
| `backend\stop-local.cmd` | Stops the API and Docker services; data is kept |
| `backend\stop-local.cmd -Reset` | Stops everything and **deletes** local data (database, mailboxes). Run `backend\start-local.cmd` and `backend\seed-demo.cmd` again afterwards |

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| "No working Rust linker was found" | Install Visual Studio Build Tools (C++), or the GNU toolchain plus MSYS2 GCC as the message describes |
| Port already in use (8080, 8088, 5434, 1025, 8025, 1143, 2424) | Stop the other program, or run `backend\stop-local.cmd` then start again |
| Verification or invitation emails don't arrive | Open Mailpit (<http://localhost:8025>); all mail goes there, never to real inboxes |
| Webmail says "storage is not connected" | Docker's Dovecot container isn't running: run `backend\start-local.cmd` again |
| Demo login fails after a reset | Run `backend\seed-demo.cmd` again (a reset deletes the demo data) |
| Script window closes immediately | Run the `.cmd` from a terminal to read the message |

## See also

- [Configuration](Configuration.md)
- [Troubleshooting](Troubleshooting.md)
- [Release checklist](Release-Checklist.md)
- [Architecture](Architecture.md)
