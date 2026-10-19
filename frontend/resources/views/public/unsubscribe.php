<div class="card"><div class="card-body">
<?php if ($state === 'invalid'): ?>
  <?= $view->insert('partials/empty', ['icon' => 'link', 'heading' => 'This unsubscribe link is not valid', 'text' => 'It may be incomplete. Copy the whole link from the email, or reply to the sender and ask to be removed.']) ?>
<?php elseif ($state === 'done'): ?>
  <div class="empty">
    <div class="empty-icon"><?= icon('check-circle') ?></div>
    <h1 style="font-size:1.4rem">You are unsubscribed</h1>
    <p><?= e($info['email'] ?? 'Your address') ?> will no longer receive these emails from <strong><?= e($info['organization'] ?? 'this sender') ?></strong>.</p>
    <p class="small muted">Important account messages, such as receipts, may still be sent where the law allows.</p>
  </div>
<?php else: ?>
  <h1 style="font-size:1.4rem">Unsubscribe from <?= e($info['organization']) ?>?</h1>
  <p class="text-2">Emails from <strong><?= e($info['sender']) ?></strong> to <strong><?= e($info['email']) ?></strong> will stop.</p>
  <form method="post" action="/u/<?= e($token) ?>">
    <button class="btn btn-primary btn-block btn-lg" type="submit">Unsubscribe</button>
  </form>
  <p class="small muted mt-2 mb-0">Changed your mind? Simply close this page.</p>
<?php endif; ?>
</div></div>
