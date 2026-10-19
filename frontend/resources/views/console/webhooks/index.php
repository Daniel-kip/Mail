<?php $chosen = (array) old('events', ['message.delivered', 'message.bounced', 'message.failed', 'message.complained']); ?>
<?= $view->insert('partials/page-head', ['heading' => 'Webhooks', 'lede' => 'Get an HTTPS POST when messages are delivered, bounce or are reported as spam. Deliveries are signed and retried with backoff.']) ?>
<?= $view->insert('console/webhooks/secret', ['secret' => $secret]) ?>
<?php if (can('webhooks:write')): ?>
<details class="card mb-3"<?= $hooks === [] || field_error('url') ? ' open' : '' ?>>
  <summary class="card-head" style="cursor:pointer"><h2><?= icon('plus') ?> Add an endpoint</h2></summary>
  <form method="post" action="/console/webhooks" class="card-body">
    <?= csrf_field() ?>
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'url', 'label' => 'Endpoint URL', 'type' => 'url', 'required' => true, 'placeholder' => 'https://app.example.com/webhooks/mail', 'hint' => 'HTTPS only. Private and internal addresses are refused.']) ?>
      <?= $view->insert('partials/field', ['name' => 'description', 'label' => 'Description']) ?>
    </div>
    <fieldset class="field" style="border:0;padding:0"><legend class="label">Events</legend>
      <div class="scope-grid"><?php foreach ($events as $ev): ?><label class="check"><input type="checkbox" name="events[]" value="<?= e($ev) ?>"<?= checked(in_array($ev, $chosen, true)) ?>> <code><?= e($ev) ?></code></label><?php endforeach; ?></div>
      <?= error_text('events') ?></fieldset>
    <button class="btn btn-primary" type="submit">Add endpoint</button>
  </form>
</details>
<?php endif; ?>
<section class="card">
  <?php if ($hooks === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'webhook', 'heading' => 'No webhooks yet', 'text' => 'Add an endpoint to update your application when delivery status changes.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Endpoint</th><th>Events</th><th>Status</th><th>Created</th></tr></thead>
    <tbody><?php foreach ($hooks as $h): ?>
      <tr><td><a class="cell-title break" href="/console/webhooks/<?= e($h['id']) ?>"><?= e($h['url']) ?></a><?= !empty($h['description']) ? '<div class="cell-sub">' . e($h['description']) . '</div>' : '' ?></td><td class="small"><?= e(implode(', ', $h['events'])) ?></td><td><?= badge($h['status']) ?><?= ($h['consecutive_failures'] ?? 0) > 0 ? '<div class="cell-sub">' . number($h['consecutive_failures']) . ' failures in a row</div>' : '' ?></td><td class="small muted nowrap"><?= time_tag($h['created_at']) ?></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
