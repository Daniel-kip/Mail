**Mobile mail setup** is Mailvora's guided onboarding that connects an employee's phone to their work mailbox. This page indexes the documentation for each audience and explains the server settings that phones are given.

Documentation set for the zero-friction mobile onboarding:

| Audience | Document |
|---|---|
| Staff and students | [Phone setup guide](Phone-Setup-Guide.md) |
| Institution admins and operators | [Administrator guide](Administrator-Guide.md) |
| ICT support | [ICT support runbook](ICT-Support-Runbook.md), [Troubleshooting](Troubleshooting.md) |
| Deployment | [DNS and TLS checklist](DNS-and-TLS-Checklist.md), [Release checklist](Release-Checklist.md), [VPS deployment](VPS-Deployment.md) |
| Security review | [Mobile setup privacy](Mobile-Setup-Privacy.md) |
| Background | [Research notes](Research-Notes.md) |

The wizard lives at `/mail/setup` (stages: welcome, device, app, readiness
check, guide steps, connection test, help). Guide content is data in
`frontend/resources/guides/mail-clients.php`.

**Update (14 October 2026):** client submission now defaults to port 465
(SSL/TLS) with 587 (STARTTLS) as the alternative (RFC 8314, decision D-026);
settings are managed in the operator console. The sections below describe
the original design; where they mention 587 as primary, D-026 supersedes them.

## What employees can do

After signing in to their hosted mailbox at `/mail`, an employee can choose **Connect work email to phone** in the folder panel. The guide asks for a device and email app, checks platform-side readiness, and then gives one step at a time for Gmail, Apple Mail, Outlook, or another IMAP app. It also links to webmail, mailbox settings, and recovery guidance.

The guide never reads or displays a mailbox password, never embeds one in a QR code or setup URL, and never asks the employee to disable encryption. Copy buttons include only the address and configured server settings. Setup completion means the instructions were shown; it does **not** mean the employee's phone connected. The optional test message is addressed to the employee's own mailbox and is sent only after they choose the action.

The readiness endpoint is authenticated by the existing mailbox session, which is bound to one mailbox. It accepts no mailbox ID, hostname, URL, or port from the caller. It rate-limits to eight checks per mailbox per minute and probes only the public endpoints loaded from trusted process configuration. The probe verifies DNS resolution, TCP reachability, and the configured TLS handshake/certificate. It does not authenticate to IMAP/SMTP with the user's password, send mail, or observe a phone. Mailbox activity is checked through the existing live-mailbox authorization and tenant/domain state; quota is reported only when Dovecot provides it.

## Configure advertised client settings

Set these environment variables on the Rust API service. They are separate from `IMAP_HOST` (the private API-to-Dovecot connection) and `SMTP_RELAY_HOST` (the platform's outbound worker relay):

```dotenv
EMAIL_CLIENT_IMAP_HOST=mail.example.edu
EMAIL_CLIENT_IMAP_PORT=993
EMAIL_CLIENT_IMAP_TLS=tls
EMAIL_CLIENT_SMTP_HOST=mail.example.edu
EMAIL_CLIENT_SMTP_PORT=587
EMAIL_CLIENT_SMTP_TLS=starttls
```

The IMAP hostname is configured for implicit TLS, normally on port 993. SMTP submission may use STARTTLS (normally port 587) or implicit TLS (normally port 465). Startup rejects invalid hostnames, port zero, plaintext, and unsupported TLS modes. The default hostname falls back to `MAIL_HOSTNAME`; development defaults use the local mapped ports 1993 and 2587, while production defaults use 993 and 587. Production operators must still confirm that public DNS, firewall/NAT, Postfix/Dovecot listeners, and certificates match these settings. The development certificate is self-signed; the sample values are not suitable for employee phones or a production deployment.

The existing mail server advertises one platform-wide endpoint. Per-domain endpoints, live configuration editing in the operator console, aggregated client analytics, and a user-visible arbitrary connection test are not implemented. Configure these values through deployment secrets/configuration and roll out changes with the normal change-control process. Do not put passwords or certificate private keys in these values.

## Readiness and support

Status labels distinguish `ready`, `needs_attention`, and `unknown`. A full or unknown quota is not shown as a successful check. If a check fails, ask institutional IT to verify DNS, public port reachability, service listeners, account/domain activation, quota, and certificate hostname/chain. Never tell a user to ignore a certificate warning or use an unencrypted connection. Support should request the app name and error text only, never the mailbox password.

The current local Dovecot certificate is self-signed and will correctly fail public-client certificate validation. That is a development limitation. A production installation needs a publicly trusted certificate covering the advertised mail hostname and must expose the configured IMAP and authenticated SMTP submission ports.

## Vendor setup behavior checked

Vendor menus change over time. The guide describes the purpose of each step and calls out that wording can vary by version. Reviewed 13 October 2026:

- Google Gmail supports adding a non-Google account by choosing **Other**, entering the full address, and choosing **Personal (IMAP)** for manual setup. Gmail versions can offer provider discovery first. See [Add another email account in Gmail](https://support.google.com/mail/answer/6078445?hl=en).
- Apple Mail supports automatic setup for listed providers and manual **Add Other Account → Mail Account** for less common providers, then IMAP and incoming/outgoing settings when automatic discovery fails. See [Add an email account to your iPhone or iPad](https://support.apple.com/en-us/102619).
- Outlook mobile tries account discovery after the employee enters the address; if necessary, its IMAP flow allows advanced incoming and outgoing settings. Exact labels vary by version. See [Set up Outlook for iOS and Android](https://support.microsoft.com/en-us/office/setup-outlook-for-ios-and-android-5a8bb76e-9bcb-4cf0-ae94-bda414104d16) and [Set up an IMAP account](https://support.microsoft.com/en-au/outlook/how-do-i-set-up-an-imap-account).
- RFC 6186 defines SRV records for IMAP and message submission service discovery. Publishing those records is a DNS/deployment choice and does not guarantee every Gmail or Outlook version will consume them. This release does not claim universal automatic discovery. See [RFC 6186](https://www.rfc-editor.org/rfc/rfc6186.html).

For a domain where the mail endpoint is `mail.example.edu`, the relevant SRV targets are typically `_imaps._tcp.example.edu` (port 993) and `_submission._tcp.example.edu` (port 587). Publish records only when those services really accept encrypted client connections and their certificate matches the target. These records complement the MX record; they do not replace it or guarantee discovery in every app.

## Verification

Rust integration coverage checks mailbox-session authorization, tenant-bound mailbox resolution, active-session revocation on suspension, and that only public client settings are returned. Deployment verification should additionally use the documented mail-tier E2E test and inspect the public DNS, TLS chain, and ports from outside the host network. No real Gmail, Apple Mail, or Outlook mobile device is claimed to have been tested by this implementation.

## See also

- [Phone setup guide](Phone-Setup-Guide.md)
- [ICT support runbook](ICT-Support-Runbook.md)
- [Mobile setup privacy](Mobile-Setup-Privacy.md)
- [DNS and TLS checklist](DNS-and-TLS-Checklist.md)
