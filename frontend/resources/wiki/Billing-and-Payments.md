**Billing and payments** covers Mailvora's plans, invoices and M-Pesa payment. Every price is a flat monthly amount in Kenyan shillings (KES), whatever the usage within the plan's limits.

| | |
|---|---|
| **Currency** | Kenyan shilling (KES) |
| **Payment** | M-Pesa STK push through PesaTide; bank transfer recorded by operators |
| **Tax** | `BILLING_TAX_RATE_BPS` (0 by default; 1600 adds 16% VAT) |
| **Trial** | Premium starts with 14 days free |

## Plans

| Plan | For | Price per month |
|---|---|---|
| Developer (free) | Apps | KES 0 |
| Starter | Apps | KES 120 |
| Standard | Apps | KES 200 |
| Institution Free | Institutions | KES 0 |
| Premium | Institutions | KES 1,200 (whole institution, no per-mailbox fees) |
| Enterprise | Institutions | Custom, arranged by the platform operator |

Upgrades apply immediately with a prorated invoice. Downgrades and cancellations apply at the end of the paid period. Operators can change prices, limits and visibility under **Plans & pricing**; issued invoices keep the price they were issued with.

## Paying an invoice with M-Pesa

1. The owner or a billing admin opens the invoice under **Billing** and enters a Safaricom number.
2. Mailvora asks PesaTide to send an STK push; the payer approves it on the phone with their M-Pesa PIN.
3. PesaTide confirms through a signed webhook (`X-PesaTide-Signature`, HMAC-SHA256 of the raw body). The payment page also polls the status, so the result appears even if the webhook is delayed.
4. The invoice is marked paid with the M-Pesa receipt number, and the event is recorded in the audit log.

Final states (paid, failed, cancelled, expired) are never overwritten, so repeated or out-of-order webhooks are harmless. A payment request expires after ten minutes, and only one can be in progress per invoice at a time.

## Setting up PesaTide

1. Create a live API key in the PesaTide dashboard.
2. Set `PESATIDE_API_KEY`, `PESATIDE_API_SECRET`, `PESATIDE_WEBHOOK_SECRET` and, if you have one, `PESATIDE_ROUTE_ID` (see [Configuration](Configuration.md#integrations)).
3. Bind the webhook URL `https://api.<your-domain>/v1/payments/pesatide/webhook` to the key.
4. Pay a test invoice of KES 1 to prove the flow end to end.

## Invoices and reminders

Renewals, overdue reminders and grace-period handling run automatically in the API worker (`BILLING_DUE_DAYS`, `BILLING_GRACE_DAYS`). Invoices are emailed to the billing contact with a link to the billing page and can be printed or saved as PDF from the browser.

## See also

- [Administrator guide](Administrator-Guide.md)
- [Configuration](Configuration.md)
- [Decision log](Decision-Log.md) (D-033)
