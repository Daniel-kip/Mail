<?php
$scopeHelp = [
    'messages:send' => 'Send email', 'messages:read' => 'Read message status and history', 'templates:read' => 'Read templates', 'templates:write' => 'Create and edit templates',
    'domains:read' => 'Read domains and DNS status', 'domains:write' => 'Add and verify domains', 'suppressions:read' => 'Read the suppression list', 'suppressions:write' => 'Add or remove suppressions',
    'webhooks:read' => 'Read webhooks', 'webhooks:write' => 'Manage webhooks', 'mailboxes:read' => 'Read mailboxes', 'mailboxes:write' => 'Create and manage mailboxes', 'usage:read' => 'Read usage',
];
$checkedScopes = (array) old('scopes', $defaultScopes);
?>
<?= $view->insert('partials/page-head', ['heading' => 'API keys', 'lede' => is_institution() ? 'Keys let your systems send mail and manage resources. Give each system its own key with only the permissions it needs.' : 'Keys let your application relay invoices, receipts and updates. Give each application its own key.']) ?>

<?php if ($newKey): ?>
  <section class="secret-box mb-3" aria-live="polite">
    <strong><?= icon('key') ?> <?= !empty($newKey['rotated']) ? 'New secret for' : 'Your new key' ?> "<?= e($newKey['name']) ?>"</strong>
    <p class="small">Copy it now and store it in your application's secret store or environment. It will not be shown again.<?= !empty($newKey['rotated']) ? ' The previous secret stopped working.' : '' ?></p>
    <div class="copy-field"><span class="value" id="new-key"><?= e($newKey['key']) ?></span><button class="btn btn-sm btn-primary" type="button" data-copy-target="new-key"><?= icon('copy') ?><span class="copy-label">Copy key</span></button></div>
  </section>
<?php endif; ?>

<details class="card mb-3"<?= field_error('name') || field_error('scopes') || $keys === [] ? ' open' : '' ?>>
  <summary class="card-head" style="cursor:pointer"><h2><?= icon('plus') ?> Create a key</h2></summary>
  <form method="post" action="/console/api-keys" class="card-body">
    <?= csrf_field() ?>
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name', 'required' => true, 'placeholder' => 'Billing service (production)']) ?>
      <?= $view->insert('partials/field', ['name' => 'environment', 'label' => 'Environment', 'type' => 'select', 'required' => true, 'options' => ['live' => 'Live: delivers mail', 'test' => 'Test: accepted but never delivered']]) ?>
    </div>
    <?= $view->insert('partials/field', ['name' => 'description', 'label' => 'Description', 'placeholder' => 'Used by the invoicing job on app-server-1']) ?>
    <fieldset class="field" style="border:0;padding:0"><legend class="label">Permissions</legend>
      <div class="scope-grid"><?php foreach ($scopes as $s): ?><label class="check"><input type="checkbox" name="scopes[]" value="<?= e($s) ?>"<?= checked(in_array($s, $checkedScopes, true)) ?>> <span><code><?= e($s) ?></code> <span class="small muted"><?= e($scopeHelp[$s] ?? '') ?></span></span></label><?php endforeach; ?></div>
      <?= error_text('scopes') ?></fieldset>
    <div class="grid grid-3">
      <?= $view->insert('partials/field', ['name' => 'allowed_ips', 'label' => 'Allowed IP addresses', 'placeholder' => '203.0.113.10, 10.0.0.0/24', 'hint' => 'Requests from other addresses are refused.']) ?>
      <?= $view->insert('partials/field', ['name' => 'allowed_sender_domains', 'label' => 'Allowed sender domains', 'placeholder' => implode(', ', array_slice($domains, 0, 2)) ?: 'example.com', 'hint' => 'Restrict the From: domain.']) ?>
      <?= $view->insert('partials/field', ['name' => 'expires_at', 'label' => 'Expires on', 'type' => 'date', 'hint' => 'Rotate regularly; expiry enforces it.']) ?>
    </div>
    <button class="btn btn-primary" type="submit">Create key</button>
  </form>
</details>

<section class="card">
  <?php if ($keys === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'key', 'heading' => 'No API keys yet', 'text' => 'Create a key to send from your application.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Key</th><th>Permissions</th><th>Restrictions</th><th class="num">Sent, 30 days</th><th>Last used</th><th></th></tr></thead>
    <tbody>
    <?php foreach ($keys as $k): $active = $k['status'] === 'active'; ?>
      <tr<?= $active ? '' : ' class="muted"' ?>>
        <td><span class="cell-title"><?= e($k['name']) ?></span> <span class="badge badge-<?= $k['environment'] === 'live' ? 'brand' : 'neutral' ?>"><?= e($k['environment']) ?></span><div class="cell-sub"><code><?= e($k['prefix']) ?>…</code> · <?= badge($k['status']) ?><?= $k['expires_at'] ? ' · expires ' . e(date_short($k['expires_at'])) : '' ?></div><?php if (!empty($k['description'])): ?><div class="cell-sub"><?= e($k['description']) ?></div><?php endif; ?></td>
        <td class="small"><?= e(implode(', ', $k['scopes'])) ?></td>
        <td class="small"><?= $k['allowed_ips'] ? 'IP: ' . e(implode(', ', $k['allowed_ips'])) . '<br>' : '' ?><?= $k['allowed_sender_domains'] ? 'From: ' . e(implode(', ', $k['allowed_sender_domains'])) : '' ?><?= !$k['allowed_ips'] && !$k['allowed_sender_domains'] ? '<span class="muted">None</span>' : '' ?></td>
        <td class="num"><?= number($k['messages_30d'] ?? 0) ?></td>
        <td class="small nowrap"><?= $k['last_used_at'] ? time_tag($k['last_used_at']) . '<div class="cell-sub">' . e($k['last_used_ip'] ?? '') . '</div>' : 'Never' ?></td>
        <td class="nowrap"><?php if ($active): ?>
          <a class="btn btn-sm" href="/console/api-keys?edit=<?= e($k['id']) ?>#edit-<?= e($k['id']) ?>">Edit</a>
          <form method="post" action="/console/api-keys/<?= e($k['id']) ?>/rotate" style="display:inline" data-confirm="Rotate this key? The current secret stops working immediately." data-confirm-label="Rotate"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Rotate</button></form>
          <form method="post" action="/console/api-keys/<?= e($k['id']) ?>/revoke" style="display:inline" data-confirm="Revoke &quot;<?= e($k['name']) ?>&quot;? Applications using it will fail." data-confirm-label="Revoke"><?= csrf_field() ?><button class="btn btn-sm btn-ghost link-danger" type="submit">Revoke</button></form>
        <?php endif; ?></td>
      </tr>
      <?php if ($editing === $k['id'] && $active): ?>
      <tr id="edit-<?= e($k['id']) ?>"><td colspan="6">
        <form method="post" action="/console/api-keys/<?= e($k['id']) ?>" class="grid grid-2">
          <?= csrf_field() ?>
          <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name', 'required' => true, 'id' => 'e-name', 'value' => $k['name']]) ?>
          <?= $view->insert('partials/field', ['name' => 'description', 'label' => 'Description', 'id' => 'e-desc', 'value' => $k['description'] ?? '']) ?>
          <?= $view->insert('partials/field', ['name' => 'allowed_ips', 'label' => 'Allowed IP addresses', 'id' => 'e-ips', 'value' => implode(', ', $k['allowed_ips'])]) ?>
          <?= $view->insert('partials/field', ['name' => 'allowed_sender_domains', 'label' => 'Allowed sender domains', 'id' => 'e-doms', 'value' => implode(', ', $k['allowed_sender_domains'])]) ?>
          <div class="form-actions"><button class="btn btn-primary btn-sm" type="submit">Save</button><a class="btn btn-ghost btn-sm" href="/console/api-keys">Cancel</a></div>
        </form>
      </td></tr>
      <?php endif; ?>
    <?php endforeach; ?>
    </tbody>
  </table></div>
  <?php endif; ?>
</section>
