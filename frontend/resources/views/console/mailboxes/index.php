<?php
$actions = '<a class="btn" href="/console/mailboxes/onboarding">' . icon('chart') . ' Phone setup report</a>';
if (can('mailboxes:write')) {
    $actions .= ' <a class="btn" href="/console/mailboxes/import">' . icon('upload') . ' Import CSV</a> <a class="btn btn-primary" href="/console/mailboxes/new">' . icon('user-plus') . ' New mailbox</a>';
}
$setupLabels = ['verified' => ['success', 'Phone connected'], 'partially_verified' => ['warning', 'Partly verified'], 'guide_completed' => ['info', 'Guide finished'], 'guide_started' => ['info', 'Started setup'], 'not_started' => ['neutral', 'Not started']];
?>
<?= $view->insert('partials/page-head', ['heading' => 'Mailboxes', 'lede' => number($total) . ' work ' . plural($total, 'mailbox', 'mailboxes'), 'actions' => $actions]) ?>
<form class="toolbar mb-2" method="get" action="/console/mailboxes">
  <input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Search name, address or department" aria-label="Search mailboxes" style="max-width:320px">
  <select class="input" name="status" aria-label="Status" style="max-width:200px" data-autosubmit>
    <?php foreach (['' => 'All statuses', 'active' => 'Active', 'pending' => 'Awaiting activation', 'suspended' => 'Suspended', 'not_connected' => 'Phone not connected'] as $k => $v): ?><option value="<?= e($k) ?>"<?= selected($status, $k) ?>><?= e($v) ?></option><?php endforeach; ?>
  </select>
  <button class="btn" type="submit"><?= icon('search') ?> Filter</button>
</form>
<section class="card">
  <?php if ($mailboxes === []): ?>
    <div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'inbox', 'heading' => $total === 0 ? 'No mailboxes yet' : 'No mailboxes match', 'text' => $total === 0 ? 'Enable mail hosting on a verified domain, then create mailboxes for your people.' : 'Try a different search or filter.', 'action' => $total === 0 && can('mailboxes:write') ? '<a class="btn btn-primary btn-sm" href="/console/mailboxes/new">Create the first mailbox</a>' : '']) ?></div>
  <?php else: ?>
    <div class="table-wrap"><table class="table">
      <thead><tr><th>Person</th><th>Department</th><th>Status</th><th>Phone setup</th><th class="num">Storage</th><th>Last sign-in</th></tr></thead>
      <tbody>
        <?php foreach ($mailboxes as $m): $s = $setupLabels[$m['client_setup']['state'] ?? 'not_started'] ?? $setupLabels['not_started']; ?>
          <tr>
            <td><a class="cell-title" href="/console/mailboxes/<?= e($m['id']) ?>"><?= e($m['display_name'] ?: $m['address']) ?></a><div class="cell-sub"><?= e($m['address']) ?></div></td>
            <td class="small"><?= e($m['department'] ?? '') ?><?= !empty($m['job_title']) ? '<div class="cell-sub">' . e($m['job_title']) . '</div>' : '' ?></td>
            <td><?= badge($m['status'], $m['status'] === 'pending' ? 'Awaiting activation' : null) ?><?= !empty($m['locked']) ? ' ' . badge('failed', 'Locked') : '' ?></td>
            <td><span class="badge badge-<?= e($s[0]) ?>"><?= e($s[1]) ?></span></td>
            <td class="num nowrap small"><?= bytes($m['quota_bytes']) ?></td>
            <td class="nowrap small muted"><?= $m['last_login_at'] ? time_tag($m['last_login_at']) : 'Never' ?></td>
          </tr>
        <?php endforeach; ?>
      </tbody>
    </table></div>
  <?php endif; ?>
</section>
