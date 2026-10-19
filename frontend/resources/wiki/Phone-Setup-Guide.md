The **phone setup guide** shows staff and students how to put their work email on their phone.

*For staff and students. About five minutes. Reviewed 13 October 2026.*

The easiest way is the guided setup in webmail, which shows the exact steps
for your phone and app, with your own settings ready to copy, and then tests
that everything works.

1. On your phone, open your institution's webmail address (for example
   `https://app.example.ac.ke/mail`) and sign in with your **full work email
   address** and your mailbox password.
2. Tap **Connect my phone** (in the menu, or on the left on a computer).
3. Choose your device (Android, iPhone/iPad, or computer), then your app
   (Gmail, Apple Mail, Outlook, another app, or just the browser).
4. The guide checks your account first. If anything on the institution's side
   is not ready, it tells you to contact IT before you change anything.
5. Follow the steps. Each step says *why* it is needed and has an
   **"I can't find this"** hint, because menus differ between app versions.
6. At the end, run the two **connection tests**:
   - **Receiving:** we send a short code to your inbox; type it on the page.
   - **Sending:** you send one message from your phone to yourself with the
     subject shown; the page confirms when it arrives.

You can keep your personal Gmail or iCloud account. Work email is added
next to it.

## First time? Activate your mailbox

New mailboxes are activated by you, so nobody else ever knows your password:

- If you received an **invitation email** (sent to your personal address),
  open the link, choose a password, and you go straight to the phone guide.
- If ICT handed you a **link or QR code**, open it on your phone and do the same.
- Links work once and expire (invitations after 7 days, reset links after
  1 hour). Ask ICT for a new one if yours expired.

## If your app asks for settings

| Setting | Value |
|---|---|
| Account type | **IMAP** (not POP3, not Exchange) |
| User name | your full email address |
| Password | your mailbox password ("normal password") |
| Incoming server | shown in the guide (for example `mail.example.ac.ke`) |
| Incoming port and security | **993, SSL/TLS** |
| Outgoing server | shown in the guide |
| Outgoing port and security | **465, SSL/TLS** (if your network blocks it: **587, STARTTLS**) |
| Outgoing server requires sign-in | **Yes**, same user name and password |

The **Server settings** page of the guide lists your exact values with copy
buttons. iPhone and iPad users can instead download a **configuration
profile** that fills everything in; it does not contain your password, and
your phone asks for it during installation.

## Stay safe

- Never turn off SSL/TLS or accept a "certificate not trusted" warning. Stop
  and contact IT instead; it means something is wrong.
- IT staff will **never** ask for your password. They can unlock your account
  and send you a reset link without knowing it.
- Changed your password? Update it in the phone app too (Account settings),
  or the app will keep asking.
- Lost your phone? Change your password in webmail straight away; that signs
  the phone out.

## Common problems

See [Troubleshooting](Troubleshooting.md), or the **Help** button inside
the guide, for: wrong password, account not found, cannot connect,
certificate warning, cannot send, cannot receive, mailbox full, and account
suspended.

## See also

- [Troubleshooting](Troubleshooting.md)
- [Mobile setup privacy](Mobile-Setup-Privacy.md)
- [Mobile mail setup](Mobile-Mail-Setup.md)
