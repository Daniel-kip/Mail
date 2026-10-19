<?php $chart = \App\View\Charts::deliveryBars($usage['daily'] ?? []); ?>
<?= $view->insert('partials/page-head', ['heading' => 'Usage', 'lede' => 'Sending volume, quota and delivery outcomes for the ' . e($usage['plan'] ?? '') . ' plan.']) ?>
<div class="grid grid-3 mb-3">
  <div class="stat"><div class="stat-label">Sent, 7 days</div><div class="stat-value"><?= number($usage['last_7_days']['sent']) ?></div></div>
  <div class="stat"><div class="stat-label">Bounced, 7 days</div><div class="stat-value"><?= number($usage['last_7_days']['bounced']) ?></div><div class="stat-foot"><?= e(number_format($usage['last_7_days']['bounce_rate'] * 100, 2)) ?>%</div></div>
  <div class="stat"><div class="stat-label">Failed, 7 days</div><div class="stat-value"><?= number($usage['last_7_days']['failed']) ?></div></div>
</div>
<section class="card mb-3"><div class="card-head"><h2>Messages per day</h2></div><div class="card-body"><?= $view->insert('console/partials/delivery-chart', ['chart' => $chart]) ?></div></section>
<div class="grid grid-2">
  <section class="card"><div class="card-head"><h2>Quota</h2></div><div class="card-body stack">
    <?= \App\View\Charts::meter('Today', (int) $usage['quota']['daily']['used'], (int) $usage['quota']['daily']['limit']) ?>
    <?= \App\View\Charts::meter('This month', (int) $usage['quota']['monthly']['used'], (int) $usage['quota']['monthly']['limit']) ?>
    <p class="small muted mb-0">Daily quota resets at 00:00 UTC; monthly quota on the 1st.</p>
  </div></section>
  <section class="card"><div class="card-head"><h2>Resources</h2></div><div class="card-body"><dl class="dl">
    <?php foreach ($usage['totals'] as $k => $v): ?><dt><?= e(ucfirst(str_replace('_', ' ', $k))) ?></dt><dd><?= number($v) ?></dd><?php endforeach; ?>
  </dl></div></section>
</div>
