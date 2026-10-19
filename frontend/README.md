# Mailvora web application

Server-rendered PHP 8.2+ application for everything people see: the public website and deliverability tools, sign-up and sign-in, the organisation console, the operator console, webmail and the guided phone setup. The browser talks only to PHP; PHP talks to the Rust API over a private address. No Composer packages or build step are needed.

```powershell
php tests/run.php                                        # unit and feature tests (API faked)
python tests/ui_smoke.py --base http://127.0.0.1:8080    # real Chrome journey, needs the stack
```

## Documentation

- [Web application](resources/wiki/Web-Application.md): structure, request lifecycle, security model, design system
- [Sealed links](resources/wiki/Sealed-Links.md): opaque, session-bound addresses for signed-in pages
- [Configuration](resources/wiki/Configuration.md#web-application): environment variables
- [Wiki home](resources/wiki/Home.md): the documentation lives here and is rendered in the app at `/docs` (operators see all of it at `/admin/docs`)
