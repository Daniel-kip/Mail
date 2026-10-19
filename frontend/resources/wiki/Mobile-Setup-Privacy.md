This note explains how Mailvora's **mobile onboarding** handles passwords, server settings, encryption and personal data.

## Passwords

- Mailbox passwords are chosen by the person (invitation or activation link)
  and stored only as Argon2id hashes. They are never displayed, logged,
  emailed or included in QR codes, welcome sheets, Apple profiles or
  autoconfig responses.
- The ICT support role cannot set, generate or view passwords; it can only
  send single-use reset links to the personal address on file, resend
  invitations and unlock accounts. Administrator-generated passwords are shown
  once, to the administrator, and are discouraged in the UI.
- Activation tokens (7 days) and reset tokens (1 hour) are random, stored
  hashed, single-use, and invalidated when a newer one is issued. Expired
  tokens are purged daily.
- Password changes and resets revoke webmail sessions and kick IMAP sessions
  (doveadm), so a lost phone is signed out.

## No SSRF, no client-chosen servers

Every server name, port and security mode shown to employees comes from the
validated platform or per-domain profile set by operators. Readiness probes
and connection tests only contact those configured endpoints; nothing a user
types is used as a host. Probes are rate-limited per mailbox (8 per minute).

## Encryption

The guide only offers implicit TLS (IMAP 993, SMTP 465) or STARTTLS on 587
with authentication required. It never suggests disabling SSL/TLS or
accepting certificate warnings; the help topic for certificate errors tells
people to stop and contact IT.

## Webmail isolation

Received HTML is sanitized by the API, then rendered from a separate route
inside a sandboxed iframe (no scripts, opaque origin) with its own strict
Content-Security-Policy; remote images are blocked until the reader opts in.
The PHP app sends `default-src 'self'`-style CSP with no inline scripts,
`frame-ancestors 'none'`, HSTS behind TLS, and CSRF tokens on every form.

## What is recorded

Setup analytics record event names (step viewed, guide completed, help topic
opened, connection verified), device type and app name, and timestamps. They
never record passwords, message content or IP-derived location. Events are
retained 13 months, then deleted. Connection tests store a short random code
(not the message) and expire after 30 minutes.

## Apple configuration profiles

Profiles contain the server names, ports, and the address only (no
password; iOS prompts for it). They can be CMS-signed with the platform's
certificate so iOS shows them as verified; unsigned profiles are clearly
labeled as such by iOS.

## Data subject requests

Admins can export the audit log and message metadata as CSV. Deleting a
mailbox deletes its stored mail; message content of transactional mail is
purged after `CONTENT_RETENTION_DAYS` (default 30).

## See also

- [Mobile mail setup](Mobile-Mail-Setup.md)
- [Security model](Security-Model.md)
- [Phone setup guide](Phone-Setup-Guide.md)
