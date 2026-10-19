The **DNS and TLS checklist** lists every record and certificate the platform and each institution domain need before employees are invited.

Complete before inviting employees. Replace `example.com` with the platform
domain and `school.ac.ke` with an institution's domain.

## Platform (once per VPS)

- [ ] Static IPv4; **PTR (reverse DNS) = `mail.example.com`**, and
      `mail.example.com` A → that IP (forward-confirmed reverse DNS).
- [ ] Hosting provider allows outbound TCP 25 (ask; many block it by default).
- [ ] `app.example.com`, `api.example.com`, `autoconfig.example.com` A → VPS.
- [ ] Let's Encrypt certificates (certbot) for the three web hosts, renewed
      automatically (`systemctl list-timers | grep certbot`).
- [ ] Mail certificate for `mail.example.com` installed in the Postfix and
      Dovecot cert volumes (`fullchain.pem`, `privkey.pem`), with a renewal
      hook that restarts both containers.
- [ ] SPF for the bounce domain: `bounces.example.com TXT "v=spf1 ip4:<IP> -all"`,
      MX → `mail.example.com`; `_spf.example.com TXT "v=spf1 ip4:<IP> -all"`.
- [ ] `SYSTEM_FROM_ADDRESS` domain registered and verified as a tenant domain
      so platform mail is DKIM-signed.
- [ ] Ports open in the firewall: 25, 80, 443, 465, 587, 993 only.

## Each institution domain

| Record | Host | Value | Required |
|---|---|---|---|
| TXT | `_mailvora.school.ac.ke` (shown in console) | `mailvora-verification=…` | Yes |
| TXT | `<selector>._domainkey.school.ac.ke` | DKIM public key | Yes |
| TXT | `school.ac.ke` | `v=spf1 include:_spf.example.com ~all` (merge with existing SPF; one SPF record only) | Recommended |
| TXT | `_dmarc.school.ac.ke` | `v=DMARC1; p=none; rua=mailto:dmarc@school.ac.ke`, tighten to `quarantine` later | Recommended |
| MX | `school.ac.ke` | `10 mail.example.com.` | For hosted mailboxes |
| SRV | `_imaps._tcp` | `0 1 993 mail.example.com.` | Optional (RFC 6186) |
| SRV | `_submissions._tcp` | `0 1 465 mail.example.com.` | Optional (RFC 8314) |
| SRV | `_submission._tcp` | `0 1 587 mail.example.com.` | Optional |
| SRV | `_imap._tcp`, `_pop3._tcp` | `0 0 0 .` (service not offered) | Optional |
| CNAME | `autoconfig.school.ac.ke` | `autoconfig.example.com.` | Optional (Thunderbird) |

The console shows exactly these values with copy buttons and checks them.
SRV and autoconfig help some apps fill settings automatically; Gmail and
Outlook mobile do not reliably use them, which is why the guided setup always
shows exact settings.

## Verify from outside the server

```sh
dig +short MX school.ac.ke
dig +short TXT _dmarc.school.ac.ke
dig +short -x <VPS IP>                                 # must be mail.example.com
openssl s_client -connect mail.example.com:993 -servername mail.example.com </dev/null | openssl x509 -noout -subject -issuer -dates
openssl s_client -connect mail.example.com:465 -servername mail.example.com </dev/null
openssl s_client -starttls smtp -connect mail.example.com:587 -servername mail.example.com </dev/null
```

- [ ] Certificates chain to a public root and name the exact host.
- [ ] Open-relay test from another network fails (e.g. `swaks --to a@gmail.com
      --server mail.example.com` without auth must be rejected).
- [ ] Send to a Gmail and an Outlook.com account; "Show original" shows
      `SPF: PASS`, `DKIM: PASS`, `DMARC: PASS`.
- [ ] The phone guide's **readiness check** shows all green for a test mailbox.

## See also

- [VPS deployment](VPS-Deployment.md)
- [Mobile mail setup](Mobile-Mail-Setup.md)
- [Deliverability tools](Deliverability-Tools.md)
