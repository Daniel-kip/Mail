The **web application** is the server-rendered PHP 8.2+ frontend that serves everything people see: the public website, pricing and [deliverability tools](Deliverability-Tools.md), sign-up and sign-in, the organisation console, the operator console, webmail and the guided phone setup.

It is a *backend-for-frontend*: the browser talks only to PHP, and PHP talks to the Rust API over a private address, keeping the API session cookies inside the PHP session. No Composer packages or build step are needed at runtime.

## Structure

```
frontend/
├── config/            app.php (validated environment), routes.php, navigation.php
├── public/            web root: index.php, router.php (php -S), .htaccess, assets/
├── resources/
│   ├── views/         plain-PHP templates: layouts/, partials/, public/, auth/,
│   │                  console/, admin/, webmail/, setup/, errors/
│   ├── guides/        mail-clients.php: device/app setup guides and help topics
│   └── wiki/          this documentation (Home.md, _Sidebar.md, one page per topic)
├── src/
│   ├── Kernel/        Application (container, pipeline, error pages), Container
│   ├── Http/          Request, Response, Router, Links (sealed links), Middleware/,
│   │                  Controllers/{PublicSite, Auth, Console, Admin, Webmail, Setup}
│   ├── Api/           ApiClient interface, HttpApiClient (cURL), ApiResponse
│   ├── Session/       NativeSession (hardened), ArraySession (tests)
│   ├── View/          View, helpers, Icons, Format, Charts (SVG), QrCode, PhoneMockup
│   ├── Mail/          GuideCatalog, AppleProfileSigner
│   └── Support/       Env, Config, Validator, Logger, Wiki
├── storage/           sessions/ and logs/ (writable)
└── tests/             run.php, Unit/, Feature/, Support/ (fake API), ui_smoke.py
```

## Request lifecycle

1. `public/index.php` builds the application and calls `Application::run()`.
2. The **headers** and **session** middleware run first.
3. [Sealed links](Sealed-Links.md) are opened: `/app?ref=…` becomes the real path.
4. The router matches the path; CSRF, authentication and permission middleware run.
5. The controller calls the API and renders a view.
6. Private addresses in the response are sealed before it is sent.

## Security model

- CSRF synchronizer tokens on every form.
- A strict Content Security Policy with no inline scripts; JavaScript is progressive enhancement only.
- `frame-ancestors 'none'`.
- Hardened sessions with idle expiry and periodic ID rotation.
- [Sealed links](Sealed-Links.md) for every signed-in page.
- Secrets (API keys, webhook secrets, generated passwords, handover links) are shown once.
- Received email is rendered from a separate route in a sandboxed iframe with its own CSP.

## Design system

`public/assets/css/app.css` defines the tokens: a refined emerald accent (`#0b7a5c`, `#5fd4a8` in dark mode) on graphite neutrals with a faint green undertone, matching the landing-page video. Light, dark and "match my device" themes are available from the theme switch. Status colours are reserved for state and always paired with an icon and text.

The landing-page hero video (`public/assets/video/`) plays automatically, muted, in an endless loop with no controls (`public/assets/js/home-video.js`). Visitors whose device asks for reduced motion see the still poster instead.

## Documentation pages

The wiki in `resources/wiki` is rendered by the application itself (`App\Support\Wiki` and `App\View\Markdown`):

| Address | Who | Shows |
|---|---|---|
| `/docs` | Everyone | Customer guides: administrator guide, sending API, billing, tools, DNS checklist, phone setup, ICT support, troubleshooting, privacy |
| `/admin/docs` | Platform operators | The whole wiki, including deployment, operations and security internals |

Operators also have the **System Infrastructure PDF** at `/admin/infrastructure` (architecture, system flows, security flows; vector diagrams). Its source is `resources/infrastructure/system-infrastructure.html`; rebuild it with `python scripts/build_infrastructure_pdf.py`, which also refreshes the previews and the checksum in `manifest.json`.

The sidebar comes from `_Sidebar.md`; operator-only pages are listed in `Wiki::OPERATOR_PAGES`. The renderer escapes all text, so a page cannot inject HTML, and links to pages a reader may not open render as plain text. To add a page, create `Page-Name.md`, add it to `_Sidebar.md` and `Home.md`, and decide whether it is public.

## Running and testing

From the project root run `start-local.cmd`; see [Quick start](Quick-Start.md).

```powershell
php tests/run.php                                        # unit and feature tests (API faked)
python tests/ui_smoke.py --base http://127.0.0.1:8080    # real Chrome journey, needs the stack
```

## Configuration

Copy `.env.example` to `.env` for development; use real environment variables in production. The settings are listed in [Configuration](Configuration.md#web-application). In production the app refuses to start without https URLs, secure cookies and debug off.

## Deployment

- **Docker (recommended):** the `frontend` service in `backend/docker-compose.yml` builds this folder (Apache, PHP and OPcache). See [VPS deployment](VPS-Deployment.md).
- **Plain PHP hosting:** point the web root at `public/`, route every request that is not a file to `public/index.php`, make `storage/` writable and set the environment.

## See also

- [Architecture](Architecture.md)
- [Sealed links](Sealed-Links.md)
- [Configuration](Configuration.md)
