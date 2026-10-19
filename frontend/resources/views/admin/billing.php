<?php $cur = $summary['currency'] ?? 'KES'; ?>
<?= $view->insert('partials/page-head', ['heading' => 'Receivables', 'lede' => 'Invoices across all organizations. Record bank or mobile-money payments here.', 'actions' => '<a class="btn" href="/admin/plans">' . icon('layers') . ' Plans</a> <form method="post" action="/admin/billing/run" style="display:inline" data-confirm="Run renewals and payment-status updates now? This is safe to repeat." data-confirm-label="Run billing">' . csrf_field() . '<button class="btn" type="submit">' . icon('refresh') . ' Run billing now</button></form>']) ?>
<div class="grid grid-3 mb-3">
  <div class="stat"><div class="stat-label">Open</div><div class="stat-value"><?= number($summary['open'] ?? 0) ?></div><div class="stat-foot"><?= e(money($summary['open_total_cents'] ?? 0, $cur)) ?></div></div>
  <div class="stat"><div class="stat-label">Overdue</div><div class="stat-value"><?= number($summary['overdue'] ?? 0) ?></div></div>
</div>
<div class="tabs mb-2" role="tablist"><?php foreach (['' => 'All', 'open' => 'Open', 'overdue' => 'Overdue', 'paid' => 'Paid', 'void' => 'Void'] as $k => $v): ?><a class="tab" role="tab" aria-selected="<?= (string) $status === $k ? 'true' : 'false' ?>" href="/admin/billing<?= $k ? '?status=' . $k : '' ?>"><?= e($v) ?></a><?php endforeach; ?></div>
<section class="card">
  <?php if ($invoices === []): ?><div class="card-body"><p class="muted mb-0">No invoices.</p></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Number</th><th>Organization</th><th>Status</th><th class="num">Total</th><th>Issued</th><th>Due</th></tr></thead>
    <tbody><?php foreach ($invoices as $i): ?>
      <tr><td><a class="cell-title" href="/admin/billing/invoices/<?= e($i['id']) ?>"><?= e($i['number']) ?></a><div class="cell-sub"><?= e($i['kind']) ?></div></td><td><?= e($i['organization'] ?? '') ?></td><td><?= badge(!empty($i['overdue']) ? 'overdue' : $i['status'], !empty($i['overdue']) ? 'Overdue' : null) ?></td><td class="num"><?= e(money($i['total_cents'], $i['currency'] ?? $cur)) ?></td><td class="small nowrap"><?= e(date_short($i['issued_at'])) ?></td><td class="small nowrap"><?= e(date_short($i['due_at'])) ?></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
