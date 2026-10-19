<?= $view->insert('partials/page-head', ['heading' => 'Users', 'lede' => 'Console accounts across all organizations.']) ?>
<form class="toolbar mb-2" method="get"><input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Email or name" aria-label="Search" style="max-width:300px"><button class="btn" type="submit"><?= icon('search') ?> Search</button></form>
<section class="card"><div class="table-wrap"><table class="table">
  <thead><tr><th>User</th><th>Organizations</th><th>Status</th><th>Security</th><th>Last sign-in</th><th></th></tr></thead>
  <tbody><?php foreach ($users as $u): ?>
    <tr>
      <td><span class="cell-title"><?= e($u['full_name']) ?></span><?= !empty($u['is_platform_admin']) ? ' <span class="badge badge-brand">Operator</span>' : '' ?><div class="cell-sub"><?= e($u['email']) ?></div></td>
      <td class="small"><?= e(implode(', ', array_map(static fn ($o) => is_array($o) ? ($o['name'] ?? '') . (isset($o['role']) ? ' (' . $o['role'] . ')' : '') : (string) $o, (array) ($u['organizations'] ?? [])))) ?></td>
      <td><?= badge($u['status']) ?></td>
      <td class="small"><?= !empty($u['mfa_enabled']) ? '2FA on' : '2FA off' ?> · <?= !empty($u['email_verified']) ? 'verified' : 'unverified' ?></td>
      <td class="small muted nowrap"><?= $u['last_login_at'] ? time_tag($u['last_login_at']) : 'Never' ?></td>
      <td><?php if (empty($u['is_platform_admin'])): ?><form method="post" action="/admin/users/<?= e($u['id']) ?>/<?= $u['status'] === 'disabled' ? 'enable' : 'disable' ?>"<?= $u['status'] === 'disabled' ? '' : ' data-confirm="Disable ' . e($u['email']) . '? They are signed out everywhere." data-confirm-label="Disable"' ?>><?= csrf_field() ?><button class="btn btn-sm" type="submit"><?= $u['status'] === 'disabled' ? 'Enable' : 'Disable' ?></button></form><?php endif; ?></td>
    </tr>
  <?php endforeach; ?></tbody>
</table></div></section>
