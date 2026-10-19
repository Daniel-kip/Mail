<?php
$p = $payment;
$state = match ($p['status']) {
    'succeeded' => ['check-circle', 'Payment received', 'Thank you. Your invoice is paid and your plan stays active.', 'success'],
    'failed' => ['x-circle', 'Payment failed', $p['failure_reason'] ?: 'M-Pesa could not complete the payment.', 'error'],
    'cancelled' => ['x-circle', 'Payment cancelled', 'The prompt was cancelled on the phone. You can try again.', 'warning'],
    'expired' => ['clock', 'Prompt expired', 'The prompt was not approved in time. Send a new one when you are ready.', 'warning'],
    default => ['smartphone-check', 'Check your phone', 'Approve the M-Pesa prompt on ' . $p['phone'] . ' by entering your M-Pesa PIN.', 'info'],
};
?>
<section class="card pay-card pay-<?= e($state[3]) ?>">
  <div class="card-body" style="text-align:center">
    <div class="pay-icon<?= $p['final'] ? '' : ' is-waiting' ?>"><?= icon($state[0], 'icon-xl') ?></div>
    <h1 style="font-size:1.5rem"><?= e($state[1]) ?></h1>
    <p class="text-2"><?= e($state[2]) ?></p>
    <div class="pay-amount"><?= e(money($p['amount_cents'], $p['currency'])) ?></div>
    <dl class="dl" style="max-width:360px;margin:1rem auto;text-align:left">
      <dt>Reference</dt><dd><code><?= e($p['reference']) ?></code></dd>
      <dt>Phone</dt><dd><?= e($p['phone']) ?></dd>
      <?php if ($p['receipt']): ?><dt>M-Pesa receipt</dt><dd><strong><?= e($p['receipt']) ?></strong></dd><?php endif; ?>
      <dt>Started</dt><dd><?= e(datetime($p['created_at'])) ?></dd>
    </dl>
    <?php if (!$p['final']): ?>
      <p class="small muted" role="status">This page updates automatically. Do not close it until the payment completes.</p>
    <?php endif; ?>
    <div class="row" style="justify-content:center">
      <a class="btn<?= $p['status'] === 'succeeded' ? ' btn-primary' : '' ?>" href="/console/billing/invoices/<?= e($p['invoice_id']) ?>">View invoice</a>
      <a class="btn btn-ghost" href="/console/billing">Back to billing</a>
    </div>
  </div>
</section>
