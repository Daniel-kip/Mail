<?php $reasons = ['' => 'All reasons', 'hard_bounce' => 'Hard bounce', 'complaint' => 'Complaint', 'unsubscribe' => 'Unsubscribed', 'manual' => 'Added manually']; ?>
<?= $view->insert('partials/page-head', ['heading' => 'Suppressions', 'lede' => 'Addresses this workspace will not send to. Hard bounces, spam complaints and one-click unsubscribes are added automatically to protect your sender reputation.']) ?>
<div class="grid grid-2 mb-3" style="grid-template-columns:2fr 1fr;align-items:start">
  <form class="toolbar" method="get" action="/console/suppressions">
    <input class="input" type="search" name="q" value="<?= e($filters['q'] ?? '') ?>" placeholder="Search address" aria-label="Search" style="max-width:280px">
    <select class="input" name="reason" aria-label="Reason" style="max-width:190px" data-autosubmit><?php foreach ($reasons as $k => $v): ?><option value="<?= e($k) ?>"<?= selected($filters['reason'] ?? '', $k) ?>><?= e($v) ?></option><?php endforeach; ?></select>
    <button class="btn" type="submit"><?= icon('search') ?> Search</button>
  </form>
  <?php if (can('suppressions:write')): ?>
  <form method="post" action="/console/suppressions" class="inline-form">
    <?= csrf_field() ?>
    <input class="input" type="email" name="email" placeholder="address@example.com" aria-label="Address to suppress" required value="<?= e(old('email')) ?>"<?= error_attrs('email') ?>>
    <button class="btn" type="submit"><?= icon('ban') ?> Suppress</button>
    <?= error_text('email') ?>
  </form>
  <?php endif; ?>
</div>
<section class="card">
  <?php if ($rows === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'shield-check', 'heading' => 'No suppressed addresses', 'text' => 'Good news: nothing is being held back.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Address</th><th>Reason</th><th>Details</th><th>Added</th><th></th></tr></thead>
    <tbody><?php foreach ($rows as $r): ?>
      <tr>
        <td class="cell-title"><?= e($r['email']) ?></td>
        <td><?= badge($r['reason'] === 'manual' ? 'neutral' : 'failed', $reasons[$r['reason']] ?? $r['reason']) ?></td>
        <td class="small text-2"><?= e($r['details'] ?? '') ?><?= $r['source_message_id'] ? ' <a href="/console/messages/' . e($r['source_message_id']) . '">message</a>' : '' ?></td>
        <td class="small muted nowrap"><?= time_tag($r['created_at']) ?></td>
        <td><?php if (can('suppressions:write')): ?><form method="post" action="/console/suppressions/delete" data-confirm="Remove <?= e($r['email']) ?> from the list? Only do this if the address is valid and wants your mail again." data-confirm-label="Remove"><?= csrf_field() ?><input type="hidden" name="email" value="<?= e($r['email']) ?>"><button class="btn btn-sm" type="submit">Remove</button></form><?php endif; ?></td>
      </tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php if ($next): ?><div class="card-foot pagination"><a class="btn btn-sm" href="/console/suppressions?<?= e(http_build_query(array_filter(['q' => $filters['q'] ?? null, 'reason' => $filters['reason'] ?? null, 'cursor' => $next]))) ?>">More <?= icon('chevron-right', 'icon-sm') ?></a></div><?php endif; ?>
  <?php endif; ?>
</section>
