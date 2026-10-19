**Troubleshooting** collects the problems people most often meet with mail apps, the console, the API and local development, and how to fix them.

The same topics appear inside the phone guide (`/mail/setup/help`) with the
person's own address and server names filled in.

## Mail apps (employees)

| Symptom | Likely cause | What to try |
|---|---|---|
| "Password incorrect" | Typo, recent password change, or the user name is not the full address | Retype; use the full address as user name; test by signing in to webmail; use "Forgot password" there or ask ICT for a reset link |
| "Account not found" / "Cannot find server" | App tried automatic setup or Exchange/POP | Choose **Other / IMAP / Manual setup** and enter the servers from the guide |
| Cannot connect / times out | Wrong port or security, or the network blocks mail ports | Incoming 993 SSL/TLS; outgoing 465 SSL/TLS, else 587 STARTTLS; try mobile data instead of Wi-Fi |
| Certificate warning | Wrong server name, or a server problem | Check the server name exactly. **Do not accept the warning**; report it to ICT |
| Can receive but not send | Outgoing server needs sign-in, or port 465 blocked | Turn on "outgoing server requires authentication" with the same user name and password; try 587 STARTTLS |
| Can send but nothing arrives | Mailbox full, or looking at the wrong account | Check storage in webmail settings; pull to refresh the work inbox |
| "Mailbox full" | Storage quota reached | Delete or archive old mail (empty Trash); ask ICT for more space |
| "Account disabled/suspended" | Mailbox or institution suspended | Contact ICT; nothing to fix on the phone |
| Locked after many attempts | 10 failed sign-ins locks for 15 minutes | Wait, or ask ICT to unlock; then update the password in the app |
| Phone keeps asking for the password | Password changed in webmail | Update it in the app's account settings |

## Console and API

| Symptom | Cause and fix |
|---|---|
| Domain stays "pending" | Records not visible yet (allow up to an hour), or the provider appended the domain twice (`_domainkey.example.com.example.com`). Enter only the host part if your provider adds the domain |
| API `401 invalid_api_key` | Revoked, expired or rotated key |
| API `403 api_key_ip_not_allowed` / sender domain error | The key's IP or sender-domain restriction; edit the key |
| API `429` | Plan quota or rate limit; see Usage, upgrade, or wait for the reset |
| Messages `deferred` | The receiving server is temporarily refusing; retries are automatic |
| Messages `bounced` | Permanent failure; the address is suppressed automatically |
| Webhook deliveries failing | Endpoint must be public HTTPS and answer 2xx within 10 s; check the delivery log, fix, then **Retry** |
| Out-of-office "not applied yet" | ManageSieve (`MANAGESIEVE_HOST`) unreachable; the setting is saved and re-applied on the next save |
| Invitation/reset email not received | Check spam; check `/admin/queue`; locally, every email is in Mailpit (http://127.0.0.1:8025) |

## Local development

| Symptom | Fix |
|---|---|
| `start-local.cmd` says port 8080 is used | Another app owns it; the launcher picks 8081–8090 and passes the address to the API, so links still work |
| PHP "extension not loaded" | The launcher loads curl, mbstring and openssl from PHP's `ext` folder automatically; install PHP 8.2+ (`winget install PHP.PHP.8.5`) |
| Docker errors | Start Docker Desktop; `backend\stop-local.cmd -Reset` wipes local data for a fresh start |
| Phones refuse the local server certificate | Expected: the dev Dovecot certificate is self-signed. Real-device tests need the production setup with a trusted certificate |

## See also

- [Phone setup guide](Phone-Setup-Guide.md)
- [ICT support runbook](ICT-Support-Runbook.md)
- [Quick start](Quick-Start.md)
