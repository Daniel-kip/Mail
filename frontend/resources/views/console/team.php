<?php
$roles = ['viewer' => 'Viewer: read-only', 'support' => 'ICT support: help mailbox users', 'developer' => 'Developer: keys, templates, sending', 'admin' => 'Admin: everything except ownership', 'owner' => 'Owner: full control and billing'];
if (!is_institution()) {
    unset($roles['support']);
}
$me = auth()['organization']['role'] ?? 'viewer';
?>
<?= $view->insert('partials/page-head', ['heading' => 'Team', 'lede' => 'People who can use this workspace console. Mailbox users do not need a console account.']) ?>
<?php if (can('members:manage')): ?>
<form method="post" action="/console/team/invitations" class="card card-body mb-3 inline-form">
  <?= csrf_field() ?>
  <div class="field grow mb-0"><label for="inv-email">Invite by email</label><input class="input" type="email" id="inv-email" name="email" required value="<?= e(old('email')) ?>"<?= error_attrs('email') ?>><?= error_text('email') ?></div>
  <div class="field mb-0"><label for="inv-role">Role</label><select class="input" id="inv-role" name="role"><?php foreach ($roles as $k => $v): if ($k === 'owner' && $me !== 'owner') { continue; } ?><option value="<?= e($k) ?>"<?= selected(old('role', 'developer'), $k) ?>><?= e($v) ?></option><?php endforeach; ?></select></div>
  <button class="btn btn-primary" type="submit"><?= icon('user-plus') ?> Send invitation</button>
</form>
<?php endif; ?>
<section class="card mb-3">
  <div class="card-head"><h2>Members</h2><span class="muted small"><?= count($members) ?></span></div>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Person</th><th>Role</th><th>Two-factor</th><th>Last sign-in</th><th></th></tr></thead>
    <tbody><?php foreach ($members as $m): ?>
      <tr>
        <td><span class="cell-title"><?= e($m['full_name']) ?></span><?= !empty($m['is_you']) ? ' <span class="badge badge-neutral">You</span>' : '' ?><div class="cell-sub"><?= e($m['email']) ?></div></td>
        <td><?php if (can('members:manage') && empty($m['is_you'])): ?>
          <form method="post" action="/console/team/<?= e($m['user_id']) ?>/role"><?= csrf_field() ?><select class="input" name="role" aria-label="Role for <?= e($m['full_name']) ?>" data-autosubmit><?php foreach ($roles as $k => $v): ?><option value="<?= e($k) ?>"<?= selected($m['role'], $k) ?>><?= e(ucfirst($k === 'support' ? 'ICT support' : $k)) ?></option><?php endforeach; ?></select><noscript><button class="btn btn-sm" type="submit">Save</button></noscript></form>
        <?php else: ?><?= e(ucfirst($m['role'] === 'support' ? 'ICT support' : $m['role'])) ?><?php endif; ?></td>
        <td><?= !empty($m['mfa_enabled']) ? badge('active', 'On') : badge('pending', 'Off') ?></td>
        <td class="small muted nowrap"><?= $m['last_login_at'] ? time_tag($m['last_login_at']) : 'Never' ?></td>
        <td><?php if (can('members:manage') && empty($m['is_you'])): ?><form method="post" action="/console/team/<?= e($m['user_id']) ?>/remove" data-confirm="Remove <?= e($m['full_name']) ?> from this workspace?" data-confirm-label="Remove"><?= csrf_field() ?><button class="btn btn-sm btn-ghost link-danger" type="submit">Remove</button></form><?php endif; ?></td>
      </tr>
    <?php endforeach; ?></tbody>
  </table></div>
</section>
<?php if ($invitations !== []): ?>
<section class="card">
  <div class="card-head"><h2>Pending invitations</h2></div>
  <div class="table-wrap"><table class="table table-compact"><tbody><?php foreach ($invitations as $i): ?>
    <tr><td><?= e($i['email']) ?></td><td><?= e(ucfirst($i['role'])) ?></td><td class="small muted">Expires <?= e(date_short($i['expires_at'])) ?></td><td><?php if (can('members:manage')): ?><form method="post" action="/console/team/invitations/<?= e($i['id']) ?>/revoke"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Revoke</button></form><?php endif; ?></td></tr>
  <?php endforeach; ?></tbody></table></div>
</section>
<?php endif; ?>
