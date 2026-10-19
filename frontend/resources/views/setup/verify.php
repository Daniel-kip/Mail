<?php
$v = $verification;
$inOk = !empty($v['incoming_verified_at']);
$outOk = !empty($v['outgoing_verified_at']);
$client = preg_replace('/^(android|ios|any|computer)-/', '', $guide) ?: '';
$hidden = '<input type="hidden" name="guide" value="' . e($guide) . '"><input type="hidden" name="client" value="' . e($client) . '">';
?>
<section class="step-card">
  <h1>Test your connection</h1>
  <p class="text-2">Two quick tests prove that your phone can receive and send work email. Nothing is sent to anyone else.</p>

  <?php if ($inOk && $outOk): ?>
    <div class="alert alert-success mt-2"><?= icon('check-circle') ?><div><strong>All set!</strong> Your phone receives and sends work email. You can close this page.</div></div>
  <?php endif; ?>

  <div class="verify-grid mt-3">
    <article class="card card-body">
      <h2><?= $inOk ? icon('check-circle') : icon('inbox') ?> 1. Receiving</h2>
      <?php if ($inOk): ?>
        <p class="small text-2 mb-0">Verified <?= e(datetime($v['incoming_verified_at'])) ?>.</p>
      <?php elseif ($incoming): ?>
        <p class="small">We sent a message titled <strong>"<?= e($incoming['subject'] ?? 'Your connection code') ?>"</strong>. Open it in the email app on your phone and type the code:</p>
        <form method="post" action="/mail/setup/verify/incoming/confirm"><?= csrf_field() ?><?= $hidden ?>
          <div class="field"><label for="code">Code from the message</label><input class="input verify-code" id="code" name="code" required autocomplete="one-time-code" autocapitalize="characters" spellcheck="false" maxlength="12"<?= error_attrs('code') ?>><?= error_text('code') ?></div>
          <button class="btn btn-primary" type="submit">Check code</button>
        </form>
        <form method="post" action="/mail/setup/verify/incoming" class="mt-1"><?= csrf_field() ?><?= $hidden ?><button class="link-button small" type="submit">Send a new code</button></form>
        <p class="small muted mt-1 mb-0">Not there after a minute? Pull down to refresh the inbox. Still nothing: <a href="/mail/setup/help/cannot-receive">help with receiving</a>.</p>
      <?php else: ?>
        <p class="small text-2">We send a short code to your work inbox. You read it on your phone.</p>
        <form method="post" action="/mail/setup/verify/incoming"><?= csrf_field() ?><?= $hidden ?><button class="btn btn-primary" type="submit"><?= icon('send') ?> Send me a code</button></form>
      <?php endif; ?>
    </article>

    <article class="card card-body">
      <h2><?= $outOk ? icon('check-circle') : icon('send') ?> 2. Sending</h2>
      <?php if ($outOk): ?>
        <p class="small text-2 mb-0">Verified <?= e(datetime($v['outgoing_verified_at'])) ?>.</p>
      <?php elseif ($outgoing): ?>
        <p class="small">In the email app on your phone, write a <strong>new message</strong> from your work account:</p>
        <div class="settings-list">
          <div class="setting"><span class="s-label">To</span><span class="s-value" id="out-to"><?= e($outgoing['send_to']) ?></span><button class="btn btn-sm copy-btn" type="button" data-copy-target="out-to"><?= icon('copy') ?><span class="copy-label">Copy</span></button></div>
          <div class="setting"><span class="s-label">Subject</span><span class="s-value" id="out-subj"><?= e($outgoing['subject']) ?></span><button class="btn btn-sm copy-btn" type="button" data-copy-target="out-subj"><?= icon('copy') ?><span class="copy-label">Copy</span></button></div>
        </div>
        <p class="small">The message body can be empty. Send it; this page checks automatically.</p>
        <div data-poll-outgoing="/mail/setup/verify/outgoing/check" data-done-url="/mail/setup/verify?<?= e(http_build_query(['guide' => $guide])) ?>">
          <p class="small muted" data-poll-status role="status">Waiting for your message…</p>
        </div>
        <form method="post" action="/mail/setup/verify/outgoing/check"><?= csrf_field() ?><?= $hidden ?><button class="btn" type="submit"><?= icon('refresh') ?> Check again</button></form>
        <p class="small muted mt-1 mb-0">Phone shows a sending error? <a href="/mail/setup/help/cannot-send">Help with sending</a>.</p>
      <?php else: ?>
        <p class="small text-2">You send one short message from your phone to yourself. We confirm it went through the work server.</p>
        <form method="post" action="/mail/setup/verify/outgoing"><?= csrf_field() ?><?= $hidden ?><button class="btn btn-primary" type="submit"<?= $inOk ? '' : ' disabled title="Finish the receiving test first"' ?>><?= icon('send') ?> Start sending test</button></form>
      <?php endif; ?>
    </article>
  </div>

  <div class="step-nav"><div class="inner">
    <?php if ($guide !== ''): ?><a class="btn btn-ghost" href="/mail/setup/guide/<?= e(rawurlencode($guide)) ?>"><?= icon('arrow-left') ?> Back to the guide</a><?php endif; ?>
    <a class="btn" href="/mail"><?= ($inOk && $outOk) ? 'Go to webmail' : 'Skip for now' ?></a>
  </div></div>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
</section>
