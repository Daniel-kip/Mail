<?= $view->insert('partials/page-head', ['heading' => 'Aliases', 'lede' => 'Shared addresses such as info@ or admissions@ that deliver to one or more mailboxes.']) ?>
<?php if (can('mailboxes:write') && $domains !== []): ?>
<form method="post" action="/console/aliases" class="card card-body mb-3">
  <?= csrf_field() ?>
  <div class="grid grid-2">
    <div class="field"><label for="a-local">Alias address</label>
      <div class="input-group"><input class="input" id="a-local" name="local_part" value="<?= e(old('local_part')) ?>" placeholder="info" required<?= error_attrs('local_part') ?>>
        <select class="input" name="domain_id" aria-label="Domain"><?php foreach ($domains as $d): ?><option value="<?= e($d['id']) ?>">@<?= e($d['domain']) ?></option><?php endforeach; ?></select></div>
      <?= error_text('local_part') ?></div>
    <div class="field"><label for="a-dest">Deliver to</label>
      <select class="input" id="a-dest" name="destinations[]" multiple size="4"<?= error_attrs('destinations') ?>><?php foreach ($mailboxes as $m): ?><option value="<?= e($m['address']) ?>"><?= e(($m['display_name'] ?: $m['address']) . ' — ' . $m['address']) ?></option><?php endforeach; ?></select>
      <p class="hint">Hold Ctrl (or Cmd) to choose several.</p><?= error_text('destinations') ?></div>
  </div>
  <div><button class="btn btn-primary" type="submit"><?= icon('plus') ?> Create alias</button></div>
</form>
<?php endif; ?>
<section class="card">
  <?php if ($aliases === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'at-sign', 'heading' => 'No aliases yet', 'text' => 'Create shared addresses so mail to a role reaches the right people.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Alias</th><th>Delivers to</th><th>Status</th><th></th></tr></thead>
    <tbody><?php foreach ($aliases as $a): ?>
      <tr>
        <td class="cell-title"><?= e($a['address']) ?></td>
        <td><?php foreach ($a['destinations'] as $d): ?><span class="tag"><?= e($d) ?></span> <?php endforeach; ?></td>
        <td><?= badge($a['status']) ?></td>
        <td class="nowrap"><?php if (can('mailboxes:write')): ?>
          <form method="post" action="/console/aliases/<?= e($a['id']) ?>" style="display:inline"><?= csrf_field() ?><input type="hidden" name="status" value="<?= $a['status'] === 'active' ? 'disabled' : 'active' ?>"><button class="btn btn-sm" type="submit"><?= $a['status'] === 'active' ? 'Disable' : 'Enable' ?></button></form>
          <form method="post" action="/console/aliases/<?= e($a['id']) ?>/delete" style="display:inline" data-confirm="Delete <?= e($a['address']) ?>? Mail to it will bounce." data-confirm-label="Delete"><?= csrf_field() ?><button class="btn btn-sm btn-ghost" type="submit" aria-label="Delete"><?= icon('trash') ?></button></form>
        <?php endif; ?></td>
      </tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
