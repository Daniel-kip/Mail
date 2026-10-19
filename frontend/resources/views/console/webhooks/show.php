<?php
$actions = '';
if (can('webhooks:write')) {
    $actions = '<form method="post" action="/console/webhooks/' . e($hook['id']) . '/test">' . csrf_field() . '<button class="btn" type="submit">' . icon('zap') . ' Send test event</button></form>';
}
?>
<?= $view->insert('partials/page-head', ['heading' => $hook['description'] ?: 'Webhook endpoint', 'lede' => $hook['url'], 'actions' => $actions]) ?>
<?= $view->insert('console/webhooks/secret', ['secret' => $secret]) ?>
<div class="grid grid-2" style="grid-template-columns:1fr 1.6fr;align-items:start">
  <div class="stack">
    <?php if (can('webhooks:write')): ?>
    <section class="card"><div class="card-head"><h2>Settings</h2><?= badge($hook['status']) ?></div>
      <form method="post" action="/console/webhooks/<?= e($hook['id']) ?>" class="card-body">
        <?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'url', 'label' => 'Endpoint URL', 'type' => 'url', 'required' => true, 'default' => $hook['url']]) ?>
        <?= $view->insert('partials/field', ['name' => 'description', 'label' => 'Description', 'default' => $hook['description'] ?? '']) ?>
        <fieldset class="field" style="border:0;padding:0"><legend class="label">Events</legend>
          <?php foreach ($events as $ev): ?><label class="check"><input type="checkbox" name="events[]" value="<?= e($ev) ?>"<?= checked(in_array($ev, $hook['events'], true)) ?>> <code><?= e($ev) ?></code></label><?php endforeach; ?>
        </fieldset>
        <button class="btn btn-primary" type="submit">Save</button>
      </form>
      <div class="card-foot row">
        <form method="post" action="/console/webhooks/<?= e($hook['id']) ?>"><?= csrf_field() ?><input type="hidden" name="status" value="<?= $hook['status'] === 'active' ? 'disabled' : 'active' ?>"><button class="btn btn-sm" type="submit"><?= $hook['status'] === 'active' ? 'Pause deliveries' : 'Resume deliveries' ?></button></form>
        <form method="post" action="/console/webhooks/<?= e($hook['id']) ?>/rotate" data-confirm="Rotate the signing secret? Update your endpoint right away; old signatures stop matching." data-confirm-label="Rotate secret"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Rotate secret</button></form>
        <form method="post" action="/console/webhooks/<?= e($hook['id']) ?>/delete" data-confirm="Delete this webhook?" data-confirm-label="Delete"><?= csrf_field() ?><button class="btn btn-sm btn-ghost link-danger" type="submit">Delete</button></form>
      </div>
    </section>
    <?php endif; ?>
  </div>
  <section class="card">
    <div class="card-head"><h2>Recent deliveries</h2></div>
    <?php if ($deliveries === []): ?><div class="card-body"><p class="muted mb-0">No deliveries yet. Send a test event to try your endpoint.</p></div><?php else: ?>
    <div class="table-wrap"><table class="table table-compact">
      <thead><tr><th>Event</th><th>Status</th><th>Response</th><th>When</th><th></th></tr></thead>
      <tbody><?php foreach ($deliveries as $d): ?>
        <tr>
          <td><code class="small"><?= e($d['event_type']) ?></code><div class="cell-sub"><?= number($d['attempts']) ?> attempt<?= $d['attempts'] === 1 ? '' : 's' ?></div></td>
          <td><?= badge($d['status']) ?></td>
          <td class="small"><?= $d['response_status'] ? 'HTTP ' . e($d['response_status']) : '' ?><?= $d['last_error'] ? '<div class="cell-sub">' . e($d['last_error']) . '</div>' : '' ?></td>
          <td class="small muted nowrap"><?= time_tag($d['created_at']) ?></td>
          <td><?php if (can('webhooks:write') && in_array($d['status'], ['failed', 'dead'], true)): ?><form method="post" action="/console/webhooks/<?= e($hook['id']) ?>/deliveries/<?= e($d['id']) ?>/retry"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Retry</button></form><?php endif; ?></td>
        </tr>
      <?php endforeach; ?></tbody>
    </table></div>
    <?php endif; ?>
  </section>
</div>
