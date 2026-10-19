<?php $funnel = $m['funnel'] ?? []; $max = max(1, ...array_map(static fn ($f) => (int) $f['mailboxes'], $funnel ?: [['mailboxes' => 0]])); ?>
<?= $view->insert('partials/page-head', ['heading' => 'Onboarding analytics', 'lede' => 'Phone setup across all institutions, last ' . (int) ($m['window_days'] ?? 30) . ' days. Counts only; no message content or passwords are ever recorded.']) ?>
<div class="grid grid-2 mb-3" style="align-items:start">
  <section class="card"><div class="card-head"><h2>Funnel</h2></div><div class="card-body stack">
    <?php foreach ($funnel as $f): ?><div class="meter"><div class="meter-head"><span><?= e(\App\View\Format::statusLabel((string) $f['event'])) ?></span><span class="meter-value"><?= number($f['mailboxes']) ?></span></div><div class="meter-track"><span style="width:<?= (int) round($f['mailboxes'] / $max * 100) ?>%"></span></div></div><?php endforeach; ?>
    <?php if ($funnel === []): ?><p class="muted mb-0">No activity in this window.</p><?php endif; ?>
  </div></section>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>By app</h2></div><div class="table-wrap"><table class="table table-compact"><thead><tr><th>App</th><th class="num">Mailboxes</th><th class="num">Verified</th></tr></thead><tbody>
      <?php foreach ($m['by_client'] ?? [] as $r): ?><tr><td><?= e(\App\View\Format::statusLabel((string) $r['client'])) ?></td><td class="num"><?= number($r['mailboxes'] ?? 0) ?></td><td class="num"><?= number($r['verified'] ?? 0) ?></td></tr><?php endforeach; ?>
    </tbody></table></div></section>
    <section class="card"><div class="card-head"><h2>Connection checks</h2></div><div class="table-wrap"><table class="table table-compact"><tbody>
      <?php foreach ($m['checks'] ?? $m['verified'] ?? [] as $r): if (!is_array($r)) { continue; } ?><tr><td><?= e(ucfirst((string) ($r['direction'] ?? ''))) ?></td><td><?= badge((string) ($r['result'] ?? $r['status'] ?? '')) ?></td><td class="num"><?= number($r['count'] ?? $r['mailboxes'] ?? 0) ?></td></tr><?php endforeach; ?>
    </tbody></table></div></section>
    <section class="card"><div class="card-head"><h2>Help topics opened</h2></div><div class="table-wrap"><table class="table table-compact"><tbody>
      <?php foreach ($m['help_topics'] ?? [] as $r): ?><tr><td><?= e(ucfirst(str_replace('-', ' ', (string) $r['topic']))) ?></td><td class="num"><?= number($r['count']) ?></td></tr><?php endforeach; ?>
    </tbody></table></div></section>
  </div>
</div>
