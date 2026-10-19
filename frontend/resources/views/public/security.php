<section class="section">
  <div class="container prose">
    <span class="eyebrow"><?= icon('shield-check', 'icon-sm') ?> Security</span>
    <h1>How <?= e($appName) ?> protects your mail and your data</h1>
    <p>This page describes the controls built into the platform. It is maintained alongside the software and reflects how it actually works.</p>

    <h2>Accounts and access</h2>
    <ul>
      <li>Passwords are hashed with Argon2id (OWASP parameters). We never store or display a password in readable form.</li>
      <li>Optional two-factor authentication (TOTP) for every console user; required for platform operators in production.</li>
      <li>Repeated failed sign-ins lock the account for 15 minutes. Session cookies are HttpOnly, Secure and SameSite, and every state-changing request carries a CSRF token.</li>
      <li>Roles limit what each person can do. ICT support staff can send activation or reset links to an employee's own personal address and unlock sign-ins, but cannot set, read or receive a password.</li>
      <li>Employees activate their mailbox through a single-use link and choose their own password; nobody at their institution ever knows it.</li>
    </ul>

    <h2>Tenant isolation</h2>
    <p>Every request is bound to one organization derived from the signed-in user or the API key — never from a value the client sends. Automated tests try to read, change and delete other tenants' data and must fail.</p>

    <h2>API keys</h2>
    <ul>
      <li>Keys contain about 238 bits of randomness and are shown once. Only a SHA-256 digest is stored.</li>
      <li>Keys carry explicit scopes and can be limited to IP ranges and to specific sending domains. Rotation issues a new key and revokes the old one in a single step.</li>
    </ul>

    <h2>Email authentication and transport</h2>
    <ul>
      <li>Each domain has its own 2048-bit DKIM key, stored encrypted (AES-256-GCM). The console guides you through SPF, DKIM, DMARC and MX records and checks them.</li>
      <li>Mail apps connect only over TLS: IMAP with implicit TLS and submission with TLS (port 465, RFC 8314) or STARTTLS (port 587). Plaintext connections are not offered.</li>
      <li>The setup guide never asks employees to accept certificate warnings or turn encryption off.</li>
    </ul>

    <h2>Content safety</h2>
    <ul>
      <li>HTML email is sanitized. Received messages are displayed in a sandbox with scripts disabled and remote images blocked by default.</li>
      <li>Executable attachment types are refused. Template variables are escaped and header injection is rejected.</li>
      <li>Webhook URLs must be public HTTPS addresses; private and internal networks are blocked (SSRF protection). Payloads are signed with HMAC-SHA256.</li>
    </ul>

    <h2>Abuse prevention</h2>
    <p>Per-plan rate limits and daily and monthly quotas, suppression of bounced and complaining addresses, one-click unsubscribe for bulk mail, and an emergency suspension switch that immediately stops sending, API keys and mailbox access for an organization.</p>

    <h2>Accountability</h2>
    <p>Security-relevant actions are written to an audit log with the actor, time and IP address. Organization administrators can review and export it. Passwords, keys, tokens and message bodies are never logged.</p>

    <h2>Data retention</h2>
    <p>Message content is purged after a configurable retention period while delivery history is kept for reporting. Mail-app setup analytics contain no message content or device identifiers and are deleted after 13 months.</p>

    <h2>Reporting a vulnerability</h2>
    <p>Please report security issues privately<?php if ($config->get('support_email')): ?> to <a href="mailto:<?= e($config->get('support_email')) ?>"><?= e($config->get('support_email')) ?></a><?php endif; ?>. Include steps to reproduce and do not access other users' data.</p>
  </div>
</section>
