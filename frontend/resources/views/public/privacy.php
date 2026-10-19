<?php $company = $config->string('company.name'); ?>
<section class="section">
  <div class="container prose">
    <h1>Privacy notice</h1>
    <p class="muted">This notice explains how <?= e($company) ?> ("we") processes personal data when you use <?= e($appName) ?>. The operator of this installation must review it against their contracts and the data protection law that applies to them (for example Kenya's Data Protection Act, 2019, or the GDPR).</p>

    <h2>Roles</h2>
    <p>For the email that organizations send and receive through the platform, the organization is the data controller and we act as its processor. For console accounts, billing and platform security, we are the controller.</p>

    <h2>What we process</h2>
    <ul>
      <li><strong>Account data:</strong> names, usernames, email addresses, mobile numbers, organization details, roles and security settings.</li>
      <li><strong>Billing data:</strong> billing contact, address, KRA PIN or tax number, invoices, and M-Pesa payment references (phone number and receipt). We never see your M-Pesa PIN or store card numbers.</li>
      <li><strong>Email data:</strong> message metadata (sender, recipients, subject, status, timestamps) and, for a limited retention period, message content needed to deliver and troubleshoot mail. Hosted mailboxes store the mail their owners keep.</li>
      <li><strong>Security data:</strong> IP addresses, sign-in events and audit records.</li>
      <li><strong>Setup analytics:</strong> which device type and email app was chosen during mailbox setup and whether a connection test succeeded. No message content, passwords or device identifiers.</li>
    </ul>

    <h2>Why</h2>
    <p>To provide and secure the service, deliver and track email, prevent abuse, issue invoices, meet legal obligations, and support customers.</p>

    <h2>Retention</h2>
    <p>Message content is deleted after the retention period configured for the installation; delivery metadata and audit logs are kept for reporting and security. Setup analytics are deleted after 13 months. Account data is kept while the account exists and as required by law.</p>

    <h2>Sharing and processors</h2>
    <p>We do not sell personal data. Email is necessarily transmitted to the recipients' mail providers. We use no third-party analytics or advertising trackers on this site. We use these processors only for the purposes stated:</p>
    <ul>
      <li><strong>PesaTide</strong> (Kenya) to collect M-Pesa payments: your phone number, the amount and the invoice reference.</li>
      <li><strong>Africa's Talking</strong> (Kenya) to text verification codes: your mobile number and the code.</li>
      <li><strong>Google</strong>, only if you choose "Continue with Google": we receive your name, email address and a Google account identifier.</li>
      <li><strong>Have I Been Pwned (Pwned Passwords)</strong> to refuse passwords exposed in public breaches. Only the first five characters of an irreversible hash leave our servers; your password is never sent.</li>
    </ul>

    <h2>Security</h2>
    <p>See the <a href="/security">security page</a> for the technical and organizational measures in place.</p>

    <h2>Your rights</h2>
    <p>You can ask to access, correct or delete your personal data, or object to processing, by contacting us<?php if ($config->get('support_email')): ?> at <a href="mailto:<?= e($config->get('support_email')) ?>"><?= e($config->get('support_email')) ?></a><?php endif; ?>. If your mailbox is provided by your employer or school, contact them first: they control that data. In Kenya you may also lodge a complaint with the Office of the Data Protection Commissioner (ODPC).</p>

    <h2>Cookies</h2>
    <p>We use only strictly necessary cookies: a session cookie that keeps you signed in and protects forms. Your colour theme preference is stored in your browser.</p>
  </div>
</section>
