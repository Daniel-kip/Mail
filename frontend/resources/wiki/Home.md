**Mailvora** is a self-hosted, multi-tenant business email platform built for Kenyan institutions and developers. It relays transactional email for applications through a REST API, hosts work mailboxes on an organisation's own domain, and bills in Kenyan shillings through M-Pesa.

| | |
|---|---|
| **Products** | Sending API for apps · Hosted mailboxes for institutions |
| **Web application** | PHP 8.2+ (server-rendered, backend-for-frontend) |
| **Control plane** | Rust (Axum, SQLx) with PostgreSQL |
| **Mail transport** | Postfix (SMTP), Dovecot (IMAP, LMTP, Sieve), Rspamd |
| **Payments** | M-Pesa STK push through PesaTide, flat KES pricing |
| **Verification** | SMS codes (Africa's Talking) or Google sign-in |
| **Deployment** | One Linux VPS, Docker Compose, no git required |

## Overview

Mailvora serves two kinds of customer:

- **Individual developers** send invoices, receipts, sign-in codes and alerts from their applications with a scoped API key. Plans: Free, Starter (KES 120/month) and Standard (KES 200/month).
- **Institutions** (schools, companies, NGOs) get the same API plus hosted mailboxes for staff or students, webmail, guided phone setup and an ICT support role. Premium costs KES 1,200/month for the whole institution, with no per-mailbox fees.

Every signed-in page is reached through [sealed links](Sealed-Links.md): opaque, session-bound addresses that cannot be read, edited or shared.

## Contents

### Getting started
- [Quick start](Quick-Start.md): run everything on a Windows machine and load demo data
- [Architecture](Architecture.md): components, mail flows, tenancy and data model
- [Web application](Web-Application.md): the PHP frontend, its structure and security model
- [Configuration](Configuration.md): every environment variable explained

### Using Mailvora
- [Administrator guide](Administrator-Guide.md): console and operator console
- [Sending API](Sending-API.md): authentication, endpoints, idempotency and webhooks
- [Billing and payments](Billing-and-Payments.md): plans, invoices and M-Pesa
- [Deliverability tools](Deliverability-Tools.md): domain score, DNS lookup, header analyzer and more
- [Mobile mail setup](Mobile-Mail-Setup.md): guided phone onboarding for employees
- [Phone setup guide](Phone-Setup-Guide.md): instructions for staff and students
- [ICT support runbook](ICT-Support-Runbook.md): helping people without seeing passwords

### Running Mailvora
- [VPS deployment](VPS-Deployment.md): install on a Linux server
- [DNS and TLS checklist](DNS-and-TLS-Checklist.md): records and certificates
- [Operations](Operations.md): monitoring, backups and incidents
- [Release checklist](Release-Checklist.md): what must pass before each release
- [Troubleshooting](Troubleshooting.md): common problems and fixes

### Security
- **System infrastructure (PDF)**: architecture, system flows and security flows with vector diagrams, for operators in the operator console
- [Security model](Security-Model.md): threat model, controls and release gates
- [Security readiness](Security-Readiness.md): roadmap criteria mapped to evidence
- [Sealed links](Sealed-Links.md): opaque addresses for signed-in pages
- [Mobile setup privacy](Mobile-Setup-Privacy.md): how onboarding handles personal data

### Project
- [Project status](Project-Status.md): what each phase delivered and how it was verified
- [Decision log](Decision-Log.md): production-affecting choices and their reasons
- [Research notes](Research-Notes.md): the standards behind those choices
