The **administrator guide** is for institution owners and admins, who work in the console, and for platform operators, who work in the operator console.

## Account types

| | Individual developer | Institution |
|---|---|---|
| Purpose | Relay invoices, receipts, alerts and updates from an application | The same, plus hosted work mailboxes for staff or students |
| Sending API, templates, webhooks, suppressions | Yes | Yes |
| Hosted mailboxes, webmail, phone setup, aliases, directory | No | Yes |
| ICT support role | No | Yes |
| Plans (monthly, KES) | Developer (free), Starter 120, Standard 200 | Institution Free, Premium 1,200 (flat, any number of mailboxes within limits), Enterprise (custom) |

An individual workspace can be converted to an institution by its owner
(**Settings → Become an institution**), keeping keys, domains and history.
One person can belong to several workspaces and switch between them.

## First-time setup of an institution

1. **Domains → Add a domain.** Publish the records shown (ownership TXT,
   DKIM, SPF, DMARC, and MX when you host mail). Select **Check DNS**. See
   [DNS and TLS checklist](DNS-and-TLS-Checklist.md).
2. On the verified domain, select **Enable mail hosting**.
3. **Settings → Help for employees.** Enter the ICT support email, phone,
   hours and a short message. These appear in the phone guide, invitations
   and welcome sheets.
4. **Team.** Invite colleagues. Roles: *Viewer* (read-only), *ICT support*
   (helps mailbox users, cannot see or set passwords), *Developer* (keys,
   templates, sending), *Admin* (everything but ownership), *Owner*.
5. **Mailboxes → New mailbox** (or **Import CSV** for many at once).

## Creating mailboxes: choose how people get access

| Method | What happens | Use when |
|---|---|---|
| **Email an invitation** (recommended) | A single-use link (7 days) goes to the person's personal email; they choose their password and land in the phone guide | You have a personal address on file |
| **Activation link** | You get a single-use link and QR code to hand over in person or by chat | No personal address; onboarding day |
| Generate a password | Shown to you once | Shared or service mailboxes |
| Set a password | You choose it | Avoid; prefer an invitation |

CSV import columns: `local_part` (or `address`), `display_name`,
`recovery_email`, `department`, `job_title`. Rows with a recovery email get
invitations; the rest get activation links from their mailbox page. Never put
passwords in spreadsheets.

**Welcome sheet:** every mailbox has a printable sheet with the address,
server settings, a QR code to the phone guide and your support contacts. It
never contains a password.

## Helping people (ICT support role)

See [ICT support runbook](ICT-Support-Runbook.md). In short, from a
mailbox page: resend the invitation, create a new activation link, email a
reset link to the personal address, unlock after too many failed attempts.
The **Phone setup report** (Mailboxes) shows who has started, finished and
verified their phone connection, which apps are used and which help topics
people open, without any message content.

## Sending from applications

- **API keys:** one key per application, only the permissions it needs.
  Restrict keys by **IP address** and by **sender domain**; set an expiry and
  rotate. The secret is shown once.
- **Templates:** versioned, with live preview. Activate a version when ready.
- **Messages:** delivery timeline, CSV export, scheduled sends (cancel before
  they go), one-click unsubscribe headers (RFC 8058) for bulk mail.
- **Webhooks:** signed (`Mailvora-Signature`), retried with backoff; test and
  replay from the console.
- **Suppressions:** hard bounces, complaints and unsubscribes are added
  automatically.

## Billing

**Billing** shows the plan, trial, the next invoice and open balance, all in
Kenyan shillings. Upgrades apply at once with a prorated invoice; downgrades
and cancellations apply at the end of the paid period; Premium starts with a
14-day trial. To pay, open an invoice, enter a Safaricom number and approve
the M-Pesa prompt: PesaTide confirms through a signed webhook and the invoice
is marked paid with the M-Pesa receipt. Operators can still record bank
payments manually in `/admin/billing`.

## Operator console (`/admin`)

| Page | Use |
|---|---|
| Overview | Health: queue, failures, relay/storage configured, receivables |
| Organizations | Plans, suspension with a reason, activity |
| Users | Disable or re-enable console accounts |
| Receivables | Record bank/M-Pesa payments, void invoices, run billing now |
| Plans | Edit prices, limits, trials and visibility (versioned by invoices) |
| Mail app settings | Server names/ports shown to phones: platform default and per-domain overrides, with change history |
| Onboarding analytics | Setup funnel, apps, verification results, help topics |
| Failed jobs | Requeue dead-lettered jobs after fixing the cause |

**Mail app settings rules:** hosts must have publicly trusted certificates
for the exact hostname; use implicit TLS first (IMAP 993, SMTP 465) and offer
587 STARTTLS as the alternative (RFC 8314). Changes apply immediately to the
guide, welcome sheets, autoconfig/Autodiscover and Apple profiles. Every
change is versioned with who changed it and a note.

## See also

- [ICT support runbook](ICT-Support-Runbook.md)
- [Billing and payments](Billing-and-Payments.md)
- [Sending API](Sending-API.md)
- [Mobile mail setup](Mobile-Mail-Setup.md)
