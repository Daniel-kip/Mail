**Sealed links** are the opaque, session-bound addresses Mailvora uses for every signed-in page. Instead of `/console/domains/0192a000-…`, the browser shows `/app?ref=Xk3…`. The address cannot be read, edited or reused in another session.

| | |
|---|---|
| **Applies to** | Console, operator console, webmail and phone setup |
| **Format** | `/app?ref=<token>` |
| **Token** | AES-256-GCM over the real path and query, with a synthetic IV |
| **Key** | 32 random bytes held only in the visitor's session |
| **Lifetime** | Until the session ends (sign-out or idle expiry) |
| **Code** | `frontend/src/Http/Links.php` |

## Why

People sometimes edit an address to reach another record ("directory manipulation"), and readable addresses reveal how the application is organised. Sealed links make the address bar meaningless outside the session:

- Changing a single character makes the token fail authentication.
- A link copied to another browser or person does not open, because their session has a different key.
- Signing out retires every link the session ever produced.

Sealing is a second layer, not a replacement for authorisation. Every API call is still scoped to the signed-in organisation or mailbox (see [Security model](Security-Model.md)), so even a valid record ID from another tenant would return nothing.

## How it works

**Outgoing.** After a page is rendered, the kernel replaces every private address in `href`, `src`, `action`, `formaction` and `data-*` attributes, and in `Location` headers (including the `?next=` return address on sign-in redirects). A GET form carries the token in a hidden `ref` field, because browsers drop the query string of a GET form's action.

**Incoming.** `/app?ref=…` is decrypted and routed as the sealed path. Parameters inside the token cannot be overridden; extra query parameters (search terms and filters from GET forms) are added.

**Typed addresses.**

| Request | Result |
|---|---|
| A fixed page without record IDs, such as `/console/domains` | Redirects to its sealed form (menu entries, emailed section links) |
| A page naming a record, such as `/console/domains/<id>` | Refused: redirects to the area's home with a notice |
| A form submission (POST) to a private address | `404 Not Found` |
| An edited, expired or foreign token | Redirects to the area's home: "That link has expired or was changed" |

## What stays readable

Public pages, email-link pages and standard endpoints keep readable addresses, because they must work from emails, QR codes, search engines and other devices: `/`, `/pricing`, `/tools/…`, `/login`, `/register`, `/verify-email`, `/reset-password`, `/mail/login`, `/mail/activate`, `/invitations/…`, `/auth/google/callback`, `/.well-known/security.txt` and the autoconfig endpoints. The REST API (`/v1/…`) is a developer contract and also keeps its paths.

Emails link to fixed pages only (for example `/console/billing` rather than a specific invoice), so they open correctly through the redirect above.

## Configuration

`SEALED_LINKS` (web application) is always on in production. Development may set it to `false` to debug routes.

## Tests

- `frontend/tests/Feature/SealedLinksTest.php`: redirects, sealed pages, tampering, foreign sessions, forms and filters, public pages.
- `frontend/tests/ui_smoke.py`: the full browser journey runs with sealed links on.

## See also

- [Security model](Security-Model.md)
- [Web application](Web-Application.md)
- [Decision log](Decision-Log.md) (D-038)
