<?= $view->insert('partials/page-head', ['heading' => 'Audit log', 'lede' => 'Who changed what, and when. Entries cannot be edited or deleted.', 'actions' => '<a class="btn" href="/console/audit/export' . ($action !== '' ? '?action=' . e(rawurlencode($action)) : '') . '">' . icon('download') . ' Export CSV</a>']) ?>
<form class="toolbar mb-2" method="get" action="/console/audit">
  <input class="input" name="action" value="<?= e($action) ?>" placeholder="Action, e.g. mailbox.created" aria-label="Filter by action" style="max-width:300px">
  <button class="btn" type="submit"><?= icon('filter') ?> Filter</button>
  <?php if ($action !== ''): ?><a class="btn btn-ghost" href="/console/audit">Clear</a><?php endif; ?>
</form>
<section class="card">
  <?php if ($rows === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'activity', 'heading' => 'No entries', 'text' => 'Actions in this workspace are recorded here.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table table-compact">
    <thead><tr><th>When</th><th>Actor</th><th>Action</th><th>Target</th><th>Details</th><th>IP</th></tr></thead>
    <tbody><?php foreach ($rows as $r): ?>
      <tr>
        <td class="nowrap small"><?= e(datetime($r['created_at'])) ?></td>
        <td class="small"><?= e($r['actor'] ?? $r['actor_type']) ?><div class="cell-sub"><?= e($r['actor_type']) ?></div></td>
        <td><code class="small"><?= e($r['action']) ?></code></td>
        <td class="small"><?= e($r['target_type'] ?? '') ?></td>
        <td class="small text-2 break" style="max-width:340px"><?= e(is_array($r['details'] ?? null) ? implode(' · ', array_map(static fn ($k, $v) => $k . ': ' . (is_scalar($v) || $v === null ? var_export($v, true) : json_encode($v)), array_keys($r['details']), $r['details'])) : '') ?></td>
        <td class="small nowrap"><?= e($r['ip'] ?? '') ?></td>
      </tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php if ($next): ?><div class="card-foot pagination"><a class="btn btn-sm" href="/console/audit?<?= e(http_build_query(array_filter(['action' => $action ?: null, 'before_id' => $next]))) ?>">Older <?= icon('chevron-right', 'icon-sm') ?></a></div><?php endif; ?>
  <?php endif; ?>
</section>
