<?php $mb = $setup['mailbox'] ?? []; $v = $setup['verification'] ?? []; ?>
<section class="step-card">
  <h1>Put your work email on your phone</h1>
  <p class="text-2">We will walk you through it one step at a time. It usually takes about five minutes, and you can keep your personal email apps exactly as they are.</p>
  <div class="callout">
    <span class="small muted">Your work email address</span><br>
    <strong style="font-family:var(--mono);font-size:1.1rem"><?= e($mb['address'] ?? '') ?></strong><br>
    <span class="small muted"><?= e($mb['organization'] ?? '') ?></span>
  </div>
  <?php if (($v['state'] ?? '') === 'verified'): ?>
    <div class="alert alert-success mt-2"><?= icon('check-circle') ?><div>You already connected a device and tested it. You can add another phone or computer.</div></div>
  <?php endif; ?>
  <h2 class="section-title mt-3">Before you start</h2>
  <ul class="check-list">
    <li><?= icon('smartphone-check') ?> Have your phone with you, connected to the internet.</li>
    <li><?= icon('lock') ?> Know your mailbox password. If you forgot it, <a href="/mail/forgot-password">reset it first</a>.</li>
    <li><?= icon('shield-check') ?> You will never need to turn off security settings or accept a certificate warning.</li>
  </ul>
  <div class="step-nav"><div class="inner">
    <a class="btn btn-primary btn-lg" href="/mail/setup/device">Start <?= icon('arrow-right') ?></a>
    <a class="btn btn-ghost" href="/mail/setup/settings">I just need the server settings</a>
  </div></div>
  <details class="cant-find mt-3">
    <summary><?= icon('qr') ?> Open this guide on your phone instead</summary>
    <div class="row mt-2" style="align-items:center">
      <img src="/mail/setup/qr.svg" alt="QR code that opens this setup guide" width="160" height="160" class="qr-box">
      <p class="small text-2 mb-0">Point your phone's camera at the code, open the link, and sign in to webmail there. Following the guide on the same phone you are setting up is easiest.</p>
    </div>
  </details>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
</section>
