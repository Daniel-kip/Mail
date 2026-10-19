The **deliverability tools** are free public diagnostics at `/tools` for developers, system administrators and ICT teams. They need no account, store nothing, and follow the published standards.

| Tool | Address | What it does |
|---|---|---|
| Domain health check | `/tools/domain` | Scores MX, SPF (with the RFC 7208 ten-lookup limit), DKIM (23 common selectors), DMARC, MTA-STS, TLS-RPT, BIMI, CAA, reverse DNS and blocklists in one run |
| DNS lookup | `/tools/dns` | Queries A, AAAA, MX, TXT, CNAME, NS, SOA, CAA, SRV and PTR records through a validating public resolver |
| DNS record generator | `/tools/generator` | Builds SPF, DMARC, MTA-STS and TLS-RPT records for the chosen providers, ready to paste |
| Email header analyzer | `/tools/headers` | Traces every hop, delay and TLS use, and reads the SPF, DKIM and DMARC verdicts |
| Blocklist checker | `/tools/blocklist` | Checks an IP, or every mail server of a domain, against SpamCop, Barracuda, PSBL, UCEPROTECT and Mailspike |
| Mail port checker | `/tools/ports` | Tests SMTP 25/465/587, IMAP 993/143, POP3 995/110 and HTTPS 443 from the internet |

## Safety

- Every tool is rate-limited per IP address.
- Lookups use DNS only, except MTA-STS policy fetches and port probes, which go through the SSRF guard: private, loopback and other special-purpose addresses are refused, including after DNS rebinding.
- Pasted headers are analysed in memory and discarded.
- Spamhaus is not queried: it refuses lookups made through public resolvers.

## API

The same checks are available to the web application at `GET /v1/tools/dns`, `GET /v1/tools/domain`, `POST /v1/tools/headers`, `GET /v1/tools/blocklist` and `GET /v1/tools/ports`.

## See also

- [DNS and TLS checklist](DNS-and-TLS-Checklist.md)
- [Research notes](Research-Notes.md)
- [Decision log](Decision-Log.md) (D-036)
