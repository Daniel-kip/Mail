<?php
$auth = auth();
$first = explode(' ', (string) ($auth['user']['full_name'] ?? ''))[0];
$done = count(array_filter($checklist, static fn ($c) => $c[1]));
$chart = $usage ? \App\View\Charts::deliveryBars(array_slice($usage['daily'] ?? [], -14)) : null;
?>
<?= $view->insert('partials/page-head', [
    'heading' => 'Welcome back' . ($first !== '' ? ', ' . $first : ''),
    'lede' => $auth['organization']['name'] . ' · ' . (is_institution() ? 'Institution workspace' : 'Individual developer workspace'),
    'actions' => (can('messages:send') ? '<a class="btn btn-primary" href="/console/messages/compose">' . icon('send') . ' Send a message</a>' : ''),
]) ?>

<?php if ($checklist !== [] && $done < count($checklist)): ?>
  <section class="card mb-3">
    <div class="card-head"><h2>Get started</h2><span class="muted small"><?= $done ?> of <?= count($checklist) ?> done</span></div>
    <div class="card-body">
      <div class="progress"><div class="progress-track"><span style="width:<?= (int) round($done / max(1, count($checklist)) * 100) ?>%"></span></div></div>
      <ul class="checklist mt-2" style="margin:0 -1.25rem -1.25rem">
        <?php foreach ($checklist as [$label, $ok, $href]): ?>
          <li class="<?= $ok ? 'done' : '' ?>"><span class="tick"><?= icon('check', 'icon-xs') ?></span><span class="ck-title"><?= $ok ? e($label) : '<a href="' . e($href) . '">' . e($label) . '</a>' ?></span></li>
        <?php endforeach; ?>
      </ul>
    </div>
  </section>
<?php endif; ?>

<?php if ($usage): ?>
  <div class="grid grid-4 mb-3">
    <div class="stat"><div class="stat-label">Sent, last 7 days</div><div class="stat-value"><?= number($usage['last_7_days']['sent']) ?></div><div class="stat-foot"><?= number($usage['totals']['in_flight']) ?> in flight</div></div>
    <div class="stat"><div class="stat-label">Bounce rate</div><div class="stat-value"><?= e(number_format(($usage['last_7_days']['bounce_rate'] ?? 0) * 100, 2)) ?>%</div><div class="stat-foot"><?= number($usage['last_7_days']['bounced']) ?> bounced · keep below 2%</div></div>
    <div class="stat"><div class="stat-label">Today</div><div class="stat-value"><?= number($usage['quota']['daily']['used']) ?></div><div class="stat-foot">of <?= number($usage['quota']['daily']['limit']) ?> daily</div></div>
    <div class="stat"><div class="stat-label"><?= is_institution() ? 'Mailboxes' : 'Active API keys' ?></div><div class="stat-value"><?= number(is_institution() ? $usage['totals']['mailboxes'] : $usage['totals']['active_api_keys']) ?></div><div class="stat-foot"><?= number($usage['totals']['verified_domains']) ?> verified domain<?= $usage['totals']['verified_domains'] === 1 ? '' : 's' ?></div></div>
  </div>
  <div class="grid grid-2 mb-3" style="grid-template-columns:2fr 1fr">
    <section class="card">
      <div class="card-head"><h2>Delivery, last 14 days</h2><a class="small" href="/console/usage">Usage details</a></div>
      <div class="card-body"><?= $view->insert('console/partials/delivery-chart', ['chart' => $chart]) ?></div>
    </section>
    <section class="card">
      <div class="card-head"><h2>Quota</h2></div>
      <div class="card-body stack">
        <?= \App\View\Charts::meter('Today', (int) $usage['quota']['daily']['used'], (int) $usage['quota']['daily']['limit']) ?>
        <?= \App\View\Charts::meter('This month', (int) $usage['quota']['monthly']['used'], (int) $usage['quota']['monthly']['limit']) ?>
        <?php if (can('billing:read')): ?><a class="btn btn-sm" href="/console/billing/plans">Compare plans</a><?php endif; ?>
      </div>
    </section>
  </div>
<?php endif; ?>

<div class="grid grid-2">
  <?php if (can('messages:read')): ?>
  <section class="card">
    <div class="card-head"><h2>Recent messages</h2><a class="small" href="/console/messages">All messages</a></div>
    <?php if ($messages === []): ?>
      <div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'send', 'heading' => 'No messages yet', 'text' => 'Messages sent with the API or the console appear here with their delivery status.']) ?></div>
    <?php else: ?>
      <div class="table-wrap"><table class="table table-compact"><tbody>
        <?php foreach ($messages as $m): ?>
          <tr><td><a class="cell-title" href="/console/messages/<?= e($m['id']) ?>"><?= e($m['subject'] ?: '(no subject)') ?></a><div class="cell-sub truncate"><?= e(implode(', ', (array) $m['to'])) ?></div></td><td><?= badge($m['status']) ?></td><td class="nowrap muted small"><?= time_tag($m['created_at']) ?></td></tr>
        <?php endforeach; ?>
      </tbody></table></div>
    <?php endif; ?>
  </section>
  <?php endif; ?>
  <?php if ($onboarding): ?>
  <section class="card">
    <div class="card-head"><h2>Work email adoption</h2><a class="small" href="/console/mailboxes/onboarding">Report</a></div>
    <div class="card-body"><dl class="dl">
      <?php foreach (($onboarding['totals'] ?? []) as $k => $v): if (!is_scalar($v)) { continue; } ?>
        <dt><?= e(ucfirst(str_replace('_', ' ', (string) $k))) ?></dt><dd><?= e(is_float($v) ? number_format($v * 100, 0) . '%' : number($v)) ?></dd>
      <?php endforeach; ?>
    </dl></div>
  </section>
  <?php elseif (can('domains:read')): ?>
  <section class="card">
    <div class="card-head"><h2>Domains</h2><a class="small" href="/console/domains">Manage</a></div>
    <?php if ($domains === []): ?>
      <div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'globe', 'heading' => 'No domains yet', 'text' => 'Add the domain you send from to authenticate your mail.', 'action' => '<a class="btn btn-primary btn-sm" href="/console/domains">Add a domain</a>']) ?></div>
    <?php else: ?>
      <div class="table-wrap"><table class="table table-compact"><tbody>
        <?php foreach ($domains as $d): ?><tr><td><a class="cell-title" href="/console/domains/<?= e($d['id']) ?>"><?= e($d['domain']) ?></a></td><td><?= badge($d['status']) ?></td></tr><?php endforeach; ?>
      </tbody></table></div>
    <?php endif; ?>
  </section>
  <?php endif; ?>
</div>
