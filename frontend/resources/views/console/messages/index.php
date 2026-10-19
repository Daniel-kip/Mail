<?php
$qs = array_filter(['status' => $filters['status'] ?? null, 'q' => $filters['q'] ?? null, 'tag' => $filters['tag'] ?? null]);
$actions = '<a class="btn" href="/console/messages/export' . ($qs ? '?' . e(http_build_query($qs)) : '') . '">' . icon('download') . ' Export CSV</a>';
if (can('messages:send')) {
    $actions .= ' <a class="btn btn-primary" href="/console/messages/compose">' . icon('send') . ' Send</a>';
}
?>
<?= $view->insert('partials/page-head', ['heading' => 'Messages', 'lede' => 'Every message sent from this workspace, with its delivery status.', 'actions' => $actions]) ?>
<form class="toolbar mb-2" method="get" action="/console/messages">
  <input class="input" type="search" name="q" value="<?= e($filters['q'] ?? '') ?>" placeholder="Recipient, subject or message ID" aria-label="Search" style="max-width:300px">
  <select class="input" name="status" aria-label="Status" style="max-width:190px" data-autosubmit><option value="">All statuses</option><?php foreach (\App\Http\Controllers\Console\MessageController::STATUSES as $s): ?><option value="<?= e($s) ?>"<?= selected($filters['status'] ?? '', $s) ?>><?= e(\App\View\Format::statusLabel($s)) ?></option><?php endforeach; ?></select>
  <input class="input" name="tag" value="<?= e($filters['tag'] ?? '') ?>" placeholder="Tag" aria-label="Tag" style="max-width:140px">
  <button class="btn" type="submit"><?= icon('filter') ?> Filter</button>
  <?php if ($qs): ?><a class="btn btn-ghost" href="/console/messages">Clear</a><?php endif; ?>
</form>
<section class="card">
  <?php if ($messages === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'send', 'heading' => $qs ? 'No messages match' : 'No messages yet', 'text' => $qs ? 'Try a different filter.' : 'Send with the API or from the console and track delivery here.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Subject</th><th>To</th><th>Status</th><th>Source</th><th>Created</th></tr></thead>
    <tbody><?php foreach ($messages as $m): ?>
      <tr>
        <td><a class="cell-title" href="/console/messages/<?= e($m['id']) ?>"><?= e($m['subject'] ?: '(no subject)') ?></a><div class="cell-sub"><?= e($m['from']) ?><?php foreach ((array) ($m['tags'] ?? []) as $t): ?> <span class="tag"><?= e($t) ?></span><?php endforeach; ?></div></td>
        <td class="small truncate" style="max-width:220px"><?= e(implode(', ', (array) $m['to'])) ?></td>
        <td><?= badge($m['status']) ?><?= !empty($m['sandbox']) ? ' <span class="badge badge-neutral">test</span>' : '' ?><?= $m['status'] === 'scheduled' && $m['scheduled_at'] ? '<div class="cell-sub">' . e(datetime($m['scheduled_at'])) . '</div>' : '' ?></td>
        <td class="small"><?= e(\App\View\Format::statusLabel((string) ($m['source'] ?? 'api'))) ?></td>
        <td class="small muted nowrap"><?= time_tag($m['created_at']) ?></td>
      </tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php if ($next): ?><div class="card-foot pagination"><a class="btn btn-sm" href="/console/messages?<?= e(http_build_query($qs + ['cursor' => $next])) ?>">Older messages <?= icon('chevron-right', 'icon-sm') ?></a></div><?php endif; ?>
  <?php endif; ?>
</section>
