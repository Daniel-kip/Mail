<?php
$s = $b['subscription'];
$plan = $s['plan'];
$cur = $b['currency'];
$d = $b['details'];
$paid = ($plan['price_monthly_cents'] ?? 0) > 0 || ($plan['price_per_mailbox_cents'] ?? 0) > 0;
?>
<?= $view->insert('partials/page-head', ['heading' => 'Billing', 'lede' => 'Your plan, upcoming charges and invoices.', 'actions' => '<a class="btn btn-primary" href="/console/billing/plans">' . icon('layers') . ' Change plan</a>']) ?>
<?php if ($b['open_balance_cents'] > 0): ?>
  <div class="alert alert-warning mb-3"><?= icon('alert-triangle') ?><div><strong>Open balance <?= e(money($b['open_balance_cents'], $cur)) ?>.</strong> Open the invoice and pay with M-Pesa in seconds. <?= e($b['payment_instructions'] ?? '') ?></div></div>
<?php endif; ?>
<div class="grid grid-3 mb-3">
  <section class="card card-body">
    <div class="stat-label">Current plan</div>
    <div class="stat-value" style="font-size:1.4rem"><?= e($plan['name']) ?></div>
    <p class="small text-2 mb-1"><?= badge($s['status']) ?> · billed <?= e($s['billing_cycle']) ?></p>
    <?php if ($s['status'] === 'trialing' && $s['trial_ends_at']): ?><p class="small mb-0">Free trial until <strong><?= e(date_short($s['trial_ends_at'])) ?></strong>.</p><?php endif; ?>
    <?php if (!empty($s['pending_plan'])): ?><p class="small mb-0">Changes to <strong><?= e($s['pending_plan']['name']) ?></strong> on <?= e(date_short($s['current_period_end'])) ?>.</p><?php endif; ?>
    <?php if ($s['cancel_at_period_end']): ?><p class="small mb-0">Ends on <strong><?= e(date_short($s['current_period_end'])) ?></strong>; the workspace then moves to the free plan.</p><?php endif; ?>
  </section>
  <section class="card card-body">
    <div class="stat-label">Current period</div>
    <div class="stat-value" style="font-size:1.1rem"><?= e(date_short($s['current_period_start'])) ?> – <?= e(date_short($s['current_period_end'])) ?></div>
    <?php if (is_institution()): ?><p class="small text-2 mb-0"><?= plural((int) $b['seats'], 'mailbox', 'mailboxes') ?> · flat price, no per-mailbox fees</p><?php endif; ?>
  </section>
  <section class="card card-body">
    <div class="stat-label">Next invoice</div>
    <?php if ($b['next_invoice'] && $b['next_invoice']['total_cents'] > 0): ?>
      <div class="stat-value" style="font-size:1.4rem"><?= e(money($b['next_invoice']['total_cents'], $cur)) ?></div>
      <p class="small text-2 mb-0">on <?= e(date_short($b['next_invoice']['date'])) ?><?= ($b['next_invoice']['tax_cents'] ?? 0) > 0 ? ', including ' . e(money($b['next_invoice']['tax_cents'], $cur)) . ' tax' : '' ?></p>
    <?php else: ?><div class="stat-value" style="font-size:1.4rem">None</div><p class="small text-2 mb-0"><?= $paid ? 'No further charges scheduled.' : 'Free plans are never invoiced.' ?></p><?php endif; ?>
  </section>
</div>
<?php if ($b['next_invoice'] && $b['next_invoice']['lines']): ?>
<section class="card mb-3"><div class="card-head"><h2>Upcoming charges</h2></div>
  <div class="table-wrap"><table class="table table-compact"><thead><tr><th>Item</th><th class="num">Qty</th><th class="num">Unit</th><th class="num">Amount</th></tr></thead><tbody>
    <?php foreach ($b['next_invoice']['lines'] as $l): ?><tr><td><?= e($l['description']) ?></td><td class="num"><?= number($l['quantity']) ?></td><td class="num"><?= e(money($l['unit_cents'], $cur)) ?></td><td class="num"><?= e(money($l['amount_cents'], $cur)) ?></td></tr><?php endforeach; ?>
  </tbody></table></div>
</section>
<?php endif; ?>
<section class="card mb-3">
  <div class="card-head"><h2>Invoices</h2></div>
  <?php if ($b['invoices'] === []): ?><div class="card-body"><p class="muted mb-0">No invoices yet.</p></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Number</th><th>Period</th><th>Status</th><th class="num">Total</th><th>Due</th><th></th></tr></thead>
    <tbody><?php foreach ($b['invoices'] as $i): ?>
      <tr><td class="cell-title"><?= e($i['number']) ?><div class="cell-sub"><?= e(ucfirst($i['kind'])) ?></div></td><td class="small"><?= e(date_short($i['period_start'])) ?> – <?= e(date_short($i['period_end'])) ?></td><td><?= badge(!empty($i['overdue']) ? 'overdue' : $i['status'], !empty($i['overdue']) ? 'Overdue' : null) ?></td><td class="num"><?= e(money($i['total_cents'], $i['currency'])) ?></td><td class="small nowrap"><?= e(date_short($i['due_at'])) ?></td><td class="nowrap"><a class="btn btn-sm<?= $i['status'] === 'open' ? ' btn-primary' : '' ?>" href="/console/billing/invoices/<?= e($i['id']) ?>"><?= $i['status'] === 'open' ? icon('smartphone-check') . ' Pay' : icon('file-text') . ' View' ?></a></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
<div class="grid grid-2">
  <section class="card" id="details">
    <div class="card-head"><h2>Billing details</h2></div>
    <?php if (can('billing:manage')): ?>
    <form method="post" action="/console/billing/details" class="card-body">
      <?= csrf_field() ?>
      <?= $view->insert('partials/field', ['name' => 'billing_name', 'label' => 'Legal name on invoices', 'default' => $d['billing_name'] ?? $d['organization']]) ?>
      <?= $view->insert('partials/field', ['name' => 'billing_email', 'label' => 'Invoices sent to', 'type' => 'email', 'default' => $d['billing_email'] ?? '', 'hint' => 'Defaults to the workspace owners.']) ?>
      <?= $view->insert('partials/field', ['name' => 'billing_address', 'label' => 'Postal address', 'type' => 'textarea', 'rows' => 3, 'default' => $d['billing_address'] ?? '']) ?>
      <div class="grid grid-2">
        <?= $view->insert('partials/field', ['name' => 'tax_id', 'label' => 'Tax ID / KRA PIN', 'default' => $d['tax_id'] ?? '']) ?>
        <?= $view->insert('partials/field', ['name' => 'country', 'label' => 'Country code', 'default' => $d['country'] ?? '', 'placeholder' => 'KE', 'attrs' => ' maxlength="2"']) ?>
      </div>
      <button class="btn btn-primary" type="submit">Save details</button>
    </form>
    <?php else: ?><div class="card-body"><dl class="dl"><dt>Name</dt><dd><?= e($d['billing_name'] ?? $d['organization']) ?></dd><dt>Email</dt><dd><?= e($d['billing_email'] ?? 'Owners') ?></dd></dl></div><?php endif; ?>
  </section>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>How to pay</h2></div><div class="card-body"><p class="text-2 mb-0" style="white-space:pre-line"><?= e($b['payment_instructions'] ?: 'Payment instructions are printed on each invoice.') ?></p></div></section>
    <?php if (can('billing:manage') && $paid): ?>
    <section class="card"><div class="card-head"><h2><?= $s['cancel_at_period_end'] ? 'Resume subscription' : 'Cancel subscription' ?></h2></div><div class="card-body">
      <?php if ($s['cancel_at_period_end']): ?>
        <form method="post" action="/console/billing/resume"><?= csrf_field() ?><button class="btn btn-primary" type="submit">Resume <?= e($plan['name']) ?></button></form>
      <?php else: ?>
        <p class="small text-2">You keep the plan until <?= e(date_short($s['current_period_end'])) ?>, then move to the free plan. Usage above free limits will be refused.</p>
        <form method="post" action="/console/billing/cancel" data-confirm="Cancel at the end of the current period?" data-confirm-label="Cancel subscription"><?= csrf_field() ?><button class="btn btn-danger" type="submit">Cancel subscription</button></form>
      <?php endif; ?>
    </div></section>
    <?php endif; ?>
  </div>
</div>
