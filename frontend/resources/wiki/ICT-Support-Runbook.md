The **ICT support runbook** is for members with the ICT support role (and for admins).

Its principle: you can get anyone back into their mailbox **without ever knowing, choosing or seeing their password**, and you never ask for it.

## Before you start

1. Confirm the person's identity through your normal process (staff ID,
   known phone number, in person). Reset links go to the **personal email on
   file**, which is the second factor.
2. Ask only for: their work address, device, app, and the exact error text or
   a screenshot. Never ask for the password.

## Situations

| Situation | Action in the console (Mailboxes → person) |
|---|---|
| New starter never received the invitation | **Resend invitation** (to the personal email), or **New activation link** and hand it over in person (QR code shown) |
| Activation link expired | **New activation link**; old links stop working |
| Forgot password | **Email a reset link** (1 hour, single use, to the personal address). Admins may **Create reset link** to hand over in person |
| "Locked" badge / too many attempts | **Unlock sign-in**, then help them retype the password and update it in the app |
| Phone app cannot connect | Open **Phone and app settings** on the mailbox page and compare with the app; send them to `/mail/setup/help` |
| Certificate warning | Do **not** tell them to accept it. Escalate to the platform operator (certificate/hostname issue) |
| Mailbox full | Ask an admin to raise the storage quota, or help them clean up Trash and large attachments |
| Lost or stolen phone | Have them change the password in webmail (or send a reset link). Password changes sign out webmail sessions and force apps to re-authenticate |
| Leaver | An admin **suspends** the mailbox (mail still arrives, sign-in blocked) and later deletes it after the retention period |

## Checking progress

**Mailboxes → Phone setup report** lists active people without a verified
phone connection and offers their printable **welcome sheet**. The mailbox
page's **Setup activity** shows which steps they reached (no message
content is ever shown to you).

## Escalation to the platform operator

Escalate with the mailbox address, time, app/device and error text when:
readiness checks fail for many people, certificates are rejected, sending
fails on both 465 and 587, or the console shows a reference code
(`Reference: req_…`). Operators find the request by that reference in the logs.

Every support action (invitation resent, reset link sent, unlock) is written
to the audit log with your name.

## See also

- [Administrator guide](Administrator-Guide.md)
- [Phone setup guide](Phone-Setup-Guide.md)
- [Troubleshooting](Troubleshooting.md)
