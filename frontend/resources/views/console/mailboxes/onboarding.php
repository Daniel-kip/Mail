<?php
$t = $report['totals'] ?? [];
$total = max(1, (int) ($t['mailboxes'] ?? 0));
$funnel = [
    ['Mailboxes', $t['mailboxes'] ?? 0], ['Awaiting activation', $t['pending_activation'] ?? 0], ['Started phone setup', $t['guide_started'] ?? 0],
    ['Finished a guide', $t['guide_completed'] ?? 0], ['Incoming mail verified', $t['incoming_verified'] ?? 0], ['Outgoing mail verified', $t['outgoing_verified'] ?? 0], ['Fully verified', $t['fully_verified'] ?? 0],
];
$notConnected = array_values(array_filter($mailboxes, static fn ($m) => $m['status'] === 'active' && ($m['client_setup']['state'] ?? '') !== 'verified'));
?>
<?= $view->insert('partials/page-head', ['heading' => 'Phone setup report', 'lede' => 'How far your people have got with connecting work email to their phones. Counts come from the guided setup and connection tests; nothing here includes message content.']) ?>
<div class="grid grid-2 mb-3">
  <section class="card"><div class="card-head"><h2>Progress</h2></div><div class="card-body stack">
    <?php foreach ($funnel as [$label, $n]): ?>
      <div class="meter"><div class="meter-head"><span><?= e($label) ?></span><span class="meter-value"><?= number($n) ?></span></div><div class="meter-track"><span style="width:<?= (int) round($n / $total * 100) ?>%"></span></div></div>
    <?php endforeach; ?>
  </div></section>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Apps chosen</h2></div><div class="card-body"><dl class="dl">
      <?php foreach ($report['by_client'] ?? [] as $r): ?><dt><?= e(\App\View\Format::statusLabel((string) $r['client'])) ?></dt><dd><?= number($r['mailboxes']) ?></dd><?php endforeach; ?>
      <?php if (empty($report['by_client'])): ?><p class="muted small mb-0">No setup activity yet.</p><?php endif; ?>
    </dl></div></section>
    <section class="card"><div class="card-head"><h2>Most-opened help topics</h2></div><div class="card-body"><dl class="dl">
      <?php foreach ($report['help_topics'] ?? [] as $r): ?><dt><?= e(ucfirst(str_replace('-', ' ', (string) $r['topic']))) ?><?= !empty($r['client']) ? ' <span class="muted small">(' . e($r['client']) . ')</span>' : '' ?></dt><dd><?= number($r['count']) ?></dd><?php endforeach; ?>
      <?php if (empty($report['help_topics'])): ?><p class="muted small mb-0">No help topics opened yet.</p><?php endif; ?>
    </dl></div></section>
  </div>
</div>
<section class="card">
  <div class="card-head"><h2>Active mailboxes without a verified phone</h2><span class="muted small"><?= count($notConnected) ?></span></div>
  <?php if ($notConnected === []): ?><div class="card-body"><p class="muted mb-0">Everyone active has verified their phone connection.</p></div><?php else: ?>
  <div class="table-wrap"><table class="table table-compact"><thead><tr><th>Person</th><th>Progress</th><th>Last app</th><th></th></tr></thead><tbody>
    <?php foreach ($notConnected as $m): ?><tr><td><a href="/console/mailboxes/<?= e($m['id']) ?>"><?= e($m['display_name'] ?: $m['address']) ?></a><div class="cell-sub"><?= e($m['address']) ?></div></td><td><?= badge($m['client_setup']['state'] ?? 'not_started') ?></td><td class="small"><?= e($m['client_setup']['last_client'] ?? '—') ?></td><td><a class="btn btn-sm" href="/console/mailboxes/<?= e($m['id']) ?>/handout"><?= icon('printer') ?> Welcome sheet</a></td></tr><?php endforeach; ?>
  </tbody></table></div>
  <?php endif; ?>
</section>
