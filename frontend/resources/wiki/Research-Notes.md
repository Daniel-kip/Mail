**Research notes** record the standards and vendor behaviour that shaped each release, and the decision each finding led to.

Short record of the standards and vendor behaviour that shaped this release
(reviewed 13–14 October 2026). Each item names the resulting decision.

| Topic | Finding | Decision in Mailvora |
|---|---|---|
| Client submission security (RFC 8314, 2018) | Implicit TLS on 465 ("submissions") is preferred over STARTTLS on 587; STARTTLS remains acceptable | Default client SMTP = 465 SSL/TLS, alternative 587 STARTTLS shown everywhere (D-026) |
| Service discovery (RFC 6186) | SRV records `_imaps._tcp`, `_submission(s)._tcp` let clients find servers; unsupported services can be marked with target `.` | Console lists them as optional DNS; guides never rely on them |
| Thunderbird autoconfig | Clients fetch `autoconfig.<domain>/mail/config-v1.1.xml` (also over HTTP) and `/.well-known/autoconfig/` | Served by the API; nginx answers `autoconfig.*` on port 80 |
| Outlook Autodiscover (POX) | Desktop Outlook POSTs `/autodiscover/autodiscover.xml`; Outlook mobile mostly uses Microsoft's cloud lookup | Implemented for desktop; mobile guides use manual IMAP |
| Gmail app (Android/iOS) | Non-Google accounts: Add account → Other → Personal (IMAP) → manual settings | Guide android-gmail / ios-gmail with "why" and "can't find" for each step |
| Apple Mail | Add Account → Other → Mail Account; configuration profiles (`com.apple.mail.managed`) may omit the password so iOS prompts | Optional profile download with no password; optional CMS signing |
| One-click unsubscribe (RFC 8058; Gmail/Yahoo bulk-sender rules, 2024) | Bulk mail needs `List-Unsubscribe` + `List-Unsubscribe-Post: List-Unsubscribe=One-Click`, DKIM-covered | `unsubscribe: true` on sends; headers DKIM-signed; recipients auto-suppressed (D-030) |
| Vacation responses (RFC 5230) and ManageSieve (RFC 5804) | Server-side Sieve vacation avoids clients needing to stay online; ManageSieve uploads scripts | Out-of-office in webmail, installed via ManageSieve as the master user (D-031) |
| Password handling (NIST SP 800-63B) | Favour long passphrases, check against context words, avoid composition rules, never send passwords by email | Minimum length + "must not contain your email name"; self-chosen passwords via invitations; support role cannot see passwords (D-028) |
| Email link scanners | Security gateways prefetch links, consuming single-use GET tokens | Verification and activation pages require a button press (POST) |
| Webmail HTML safety (OWASP) | Sanitize plus isolate: sandboxed iframe, CSP, remote content blocked by default | Body served from its own route with `default-src 'none'` in a sandboxed iframe (D-029) |
| Billing in East Africa | Card penetration is low; bank transfer and M-Pesa dominate B2B payments; VAT 16% (Kenya) | Invoice-based billing with configurable tax and payment instructions; operators record payments (D-027) |

Not verified on physical devices in this release; see
[Release checklist](Release-Checklist.md).

## See also

- [Decision log](Decision-Log.md)
- [Deliverability tools](Deliverability-Tools.md)
