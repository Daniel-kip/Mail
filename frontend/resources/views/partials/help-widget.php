<?php
/** Floating help: works without JavaScript (native <details>). */
$supportEmail = (string) $config->get('support_email');
$inConsole = str_starts_with((string) (app()->request()?->path() ?? ''), '/console');
?>
<details class="help-widget">
  <summary aria-label="Help and support"><?= icon('help') ?><span class="sr-only">Help</span></summary>
  <div class="help-panel" role="dialog" aria-label="Help and support">
    <div class="help-head">
      <strong>How can we help?</strong>
      <span class="small muted">Answers, tools and a real person.</span>
    </div>
    <nav class="help-links">
      <?php if ($inConsole): ?>
        <a href="/console/developers"><?= icon('code') ?><span><b>Send your first email</b><small>API quickstart with copy-ready examples</small></span></a>
        <a href="/console/domains"><?= icon('globe') ?><span><b>Verify a domain</b><small>Exact SPF, DKIM, DMARC and MX records</small></span></a>
      <?php endif; ?>
      <a href="/tools/domain"><?= icon('shield-check') ?><span><b>Check my domain</b><small>Deliverability score in seconds</small></span></a>
      <a href="/tools/headers"><?= icon('file-text') ?><span><b>Why did mail go to spam?</b><small>Paste headers, see every hop</small></span></a>
      <a href="/mail/setup"><?= icon('smartphone-check') ?><span><b>Put work email on my phone</b><small>Gmail, Apple Mail and Outlook guides</small></span></a>
      <a href="/docs"><?= icon('book') ?><span><b>Guides and documentation</b><small>Domains, sending, billing, phones</small></span></a>
    </nav>
    <?php if ($supportEmail !== ''): ?>
      <a class="btn btn-primary btn-block" href="mailto:<?= e($supportEmail) ?>?subject=<?= e(rawurlencode('Help with ' . $appName)) ?>"><?= icon('mail') ?> Email support</a>
      <p class="small muted mb-0 mt-1" style="text-align:center">We never ask for your password.</p>
    <?php endif; ?>
  </div>
</details>
