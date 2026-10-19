<?php
$cs = $m['client_settings'] ?? [];
$setupUrl = $appUrl . '/mail/setup';
?>
<article class="handout">
  <header class="row-between">
    <div><h1 style="margin:0">Welcome to your work email</h1><p class="text-2 mb-0"><?= e($org) ?></p></div>
    <?= $view->insert('partials/brand', ['href' => '#']) ?>
  </header>
  <section class="mt-3">
    <p><strong><?= e($m['display_name'] ?: '') ?></strong>, your work email address is:</p>
    <p style="font-size:1.5rem;font-weight:700;font-family:var(--mono)"><?= e($m['address']) ?></p>
    <p class="text-2"><?= $m['status'] === 'pending' ? 'First, open the activation link you received and choose your own password.' : 'Use the password you chose when activating your account.' ?> Never share your password with anyone, including ICT staff.</p>
  </section>
  <div class="grid grid-2 mt-3">
    <section>
      <h2>Put it on your phone</h2>
      <ol>
        <li>Sign in to webmail on your phone at <strong><?= e($appUrl) ?>/mail</strong></li>
        <li>Open <strong>Connect my phone</strong> and follow the steps for your phone and app.</li>
        <li>Or scan this code with your phone camera:</li>
      </ol>
      <div class="qr-box" style="max-width:180px"><?= \App\View\QrCode::svg($setupUrl, 4, 'QR code for the phone setup page') ?></div>
    </section>
    <section>
      <h2>Settings (if your app asks)</h2>
      <table class="table table-compact">
        <tr><th>User name</th><td><?= e($m['address']) ?></td></tr>
        <tr><th>Incoming server</th><td><?= e($cs['imap']['host'] ?? '') ?></td></tr>
        <tr><th>Incoming port / security</th><td><?= e(($cs['imap']['port'] ?? '') . ' / ' . ($cs['imap']['security'] ?? '')) ?></td></tr>
        <tr><th>Outgoing server</th><td><?= e($cs['smtp']['host'] ?? '') ?></td></tr>
        <tr><th>Outgoing port / security</th><td><?= e(($cs['smtp']['port'] ?? '') . ' / ' . ($cs['smtp']['security'] ?? '')) ?></td></tr>
        <?php if (!empty($cs['smtp_alternative'])): ?><tr><th>If sending fails</th><td>port <?= e($cs['smtp_alternative']['port']) ?> / <?= e($cs['smtp_alternative']['security']) ?></td></tr><?php endif; ?>
        <tr><th>Password</th><td>Your own (normal password)</td></tr>
      </table>
    </section>
  </div>
  <?php if (array_filter([$support['email'] ?? null, $support['phone'] ?? null, $support['url'] ?? null])): ?>
  <section class="mt-3 callout">
    <strong>Need help?</strong>
    <?= !empty($support['email']) ? ' Email ' . e($support['email']) . '.' : '' ?><?= !empty($support['phone']) ? ' Call ' . e($support['phone']) . '.' : '' ?><?= !empty($support['hours']) ? ' (' . e($support['hours']) . ')' : '' ?><?= !empty($support['url']) ? ' ' . e($support['url']) : '' ?>
  </section>
  <?php endif; ?>
</article>
