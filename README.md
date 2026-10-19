# Mailvora

**Mailvora** is a self-hosted, multi-tenant business email platform: a sending API for applications, hosted mailboxes for institutions, and flat Kenyan-shilling pricing paid with M-Pesa.

| Folder | Contents |
|---|---|
| [`backend/`](backend/README.md) | Rust API and workers, database migrations, mail infrastructure (Postfix, Dovecot, Rspamd), scripts and tests |
| [`frontend/`](frontend/README.md) | PHP web application: website, console, operator console, webmail and phone setup |
| [`frontend/resources/wiki/`](frontend/resources/wiki/Home.md) | Documentation (a GitHub-style wiki), rendered in the app at `/docs` and, in full, for operators at `/admin/docs` |

## Quick start (Windows)

```powershell
start-local.cmd              # Docker services, Rust API and PHP web app
backend\seed-demo.cmd        # demo accounts, saved to backend\demo-credentials.txt
backend\run-tests.cmd        # every automated check
```

Then open the address the launcher prints (normally <http://127.0.0.1:8080>). Caught mail is at <http://127.0.0.1:8025>. See [Quick start](frontend/resources/wiki/Quick-Start.md).

## Documentation

Read it in the running app at **/docs** (operators: **/admin/docs**), or start at the **[wiki home page](frontend/resources/wiki/Home.md)**. Popular pages: [Architecture](frontend/resources/wiki/Architecture.md) · [Configuration](frontend/resources/wiki/Configuration.md) · [VPS deployment](frontend/resources/wiki/VPS-Deployment.md) · [Sending API](frontend/resources/wiki/Sending-API.md) · [Security model](frontend/resources/wiki/Security-Model.md)

---

© Mailvora. Powered by **TECHCORE SOLUTIONS**.
