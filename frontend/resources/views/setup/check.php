<?php
$r = $readiness;
$rows = $r ? [
    ['Your mailbox', $r['mailbox']], ['Your institution\'s email domain', $r['domain']], ['Incoming mail server', $r['imap']],
    ['Outgoing mail server', $r['smtp']], ['Mailbox storage', $r['quota']],
] : [];
$blocking = $r && $r['overall'] !== 'ready' && ($r['mailbox']['status'] !== 'ready' || $r['domain']['status'] !== 'ready');
$next = '/mail/setup/guide/' . rawurlencode($guide);
?>
<section class="step-card">
  <h1>Checking your account</h1>
  <p class="text-2">Before you change anything on your phone, we check that everything on our side is ready.</p>
  <?php if (!$r): ?>
    <div class="alert alert-warning"><?= icon('alert-triangle') ?><div>The automatic check could not run right now (<?= e($readinessError) ?>). You can still continue with the guide.</div></div>
  <?php else: ?>
    <ul class="stack mt-2" style="list-style:none;padding:0" aria-live="polite">
      <?php foreach ($rows as [$label, $row]): $ok = in_array($row['status'], ['ready', 'ok'], true); $warn = in_array($row['status'], ['nearly_full', 'unknown'], true); ?>
        <li class="check-item <?= $ok ? 'ok' : ($warn ? 'unknown' : 'bad') ?>">
          <span class="ci-icon"><?= icon($ok ? 'check' : ($warn ? 'alert-triangle' : 'x')) ?></span>
          <div><div class="ci-title"><?= e($label) ?> <?= badge($row['status']) ?></div><p class="ci-text"><?= e($row['message'] ?? '') ?></p></div>
        </li>
      <?php endforeach; ?>
    </ul>
    <p class="small muted mt-2"><?= e($r['note']) ?></p>
  <?php endif; ?>
  <?php if ($blocking): ?>
    <div class="alert alert-error mt-2"><?= icon('ban') ?><div>Something on your institution's side needs fixing before your phone can connect. Please contact your IT team; there is nothing to change on your phone yet.</div></div>
  <?php endif; ?>
  <div class="step-nav"><div class="inner">
    <a class="btn btn-ghost" href="/mail/setup/app?device=<?= e(rawurlencode($device)) ?>"><?= icon('arrow-left') ?> Back</a>
    <a class="btn btn-primary btn-lg" href="<?= e($next) ?>"<?= $blocking ? ' aria-disabled="true"' : '' ?>><?= $blocking ? 'Continue anyway' : 'Everything looks good. Continue' ?> <?= icon('arrow-right') ?></a>
  </div></div>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
</section>
