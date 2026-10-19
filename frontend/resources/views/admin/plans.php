<?php
$labels = ['price_monthly_cents' => 'Base price / month (cents)', 'price_per_mailbox_cents' => 'Per mailbox / month (cents)', 'annual_discount_percent' => 'Annual discount %', 'trial_days' => 'Trial days',
    'daily_send_limit' => 'Emails / day', 'monthly_send_limit' => 'Emails / month', 'max_domains' => 'Domains', 'max_mailboxes' => 'Mailboxes', 'max_members' => 'Members', 'max_api_keys' => 'API keys', 'max_webhooks' => 'Webhooks',
    'max_recipients_per_request' => 'Recipients / request', 'api_requests_per_minute' => 'API requests / minute'];
?>
<?= $view->insert('partials/page-head', ['heading' => 'Plans', 'lede' => 'The catalogue shown on the pricing page and in billing. Price changes apply from each customer\'s next invoice.']) ?>
<div class="stack">
<?php foreach ($plans as $p): $l = $p['limits'] ?? $p; ?>
  <details class="card" id="plan-<?= e($p['id']) ?>">
    <summary class="card-head" style="cursor:pointer"><h2><?= e($p['name']) ?> <span class="muted small">· <?= e($p['id']) ?> · <?= e($p['audience']) ?></span></h2><span><?= e(money($p['price_monthly_cents'], $p['currency'] ?? 'KES')) ?><?= $p['price_per_mailbox_cents'] ? ' + ' . e(money($p['price_per_mailbox_cents'], $p['currency'] ?? 'KES')) . '/mbx' : '' ?></span></summary>
    <form method="post" action="/admin/plans/<?= e($p['id']) ?>" class="card-body"><?= csrf_field() ?>
      <div class="grid grid-2">
        <?= $view->insert('partials/field', ['name' => 'name', 'id' => 'n-' . $p['id'], 'label' => 'Name', 'required' => true, 'value' => $p['name']]) ?>
        <?= $view->insert('partials/field', ['name' => 'description', 'id' => 'd-' . $p['id'], 'label' => 'Description', 'value' => $p['description'] ?? '']) ?>
      </div>
      <div class="grid grid-4">
        <?php foreach ($labels as $k => $label): ?><?= $view->insert('partials/field', ['name' => $k, 'id' => $k . '-' . $p['id'], 'label' => $label, 'type' => 'number', 'noOptional' => true, 'value' => (string) ($p[$k] ?? $l[$k] ?? ''), 'attrs' => ' min="0"']) ?><?php endforeach; ?>
      </div>
      <label class="check"><input type="checkbox" name="is_public" value="1"<?= checked(!empty($p['is_public'])) ?>> Shown on the pricing page</label>
      <label class="check"><input type="checkbox" name="self_service" value="1"<?= checked(!empty($p['self_service'])) ?>> Customers can choose it themselves</label>
      <button class="btn btn-primary mt-2" type="submit">Save plan</button>
    </form>
  </details>
<?php endforeach; ?>
</div>
