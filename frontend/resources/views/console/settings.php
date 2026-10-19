<?php
$plan = $org['plan'] ?? [];
$owner = (auth()['organization']['role'] ?? '') === 'owner';
?>
<?= $view->insert('partials/page-head', ['heading' => 'Settings', 'lede' => $org['name'] . ' · ' . ($org['account_type'] === 'individual' ? 'Individual developer workspace' : 'Institution workspace')]) ?>
<div class="grid grid-2" style="align-items:start">
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Workspace</h2></div>
      <?php if (can('organization:manage')): ?>
      <form method="post" action="/console/settings/organization" class="card-body"><?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name', 'required' => true, 'default' => $org['name']]) ?>
        <div class="field"><span class="label">Workspace ID</span><div class="copy-field"><span class="value" id="org-id"><?= e($org['id']) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="org-id" aria-label="Copy ID"><?= icon('copy') ?></button></div></div>
        <button class="btn btn-primary" type="submit">Save</button>
      </form>
      <?php else: ?><div class="card-body"><dl class="dl"><dt>Name</dt><dd><?= e($org['name']) ?></dd><dt>ID</dt><dd><code><?= e($org['id']) ?></code></dd></dl></div><?php endif; ?>
    </section>
    <?php if ($org['account_type'] === 'institution'): ?>
    <section class="card" id="support"><div class="card-head"><h2>Help for employees</h2></div>
      <form method="post" action="/console/settings/support" class="card-body"><?= csrf_field() ?>
        <p class="small text-2">Shown in the phone setup guide, on welcome sheets and in invitation emails, so people know whom to ask.</p>
        <fieldset<?= can('organization:manage') ? '' : ' disabled' ?> style="border:0;padding:0;margin:0">
        <div class="grid grid-2">
          <?= $view->insert('partials/field', ['name' => 'email', 'label' => 'Support email', 'type' => 'email', 'default' => $support['email'] ?? '', 'placeholder' => 'ict@example.ac.ke']) ?>
          <?= $view->insert('partials/field', ['name' => 'phone', 'label' => 'Support phone', 'type' => 'tel', 'default' => $support['phone'] ?? '', 'placeholder' => '+254 700 000 000']) ?>
          <?= $view->insert('partials/field', ['name' => 'hours', 'label' => 'Hours', 'default' => $support['hours'] ?? '', 'placeholder' => 'Mon–Fri, 8:00–17:00']) ?>
          <?= $view->insert('partials/field', ['name' => 'url', 'label' => 'Help page', 'type' => 'url', 'default' => $support['url'] ?? '']) ?>
        </div>
        <?= $view->insert('partials/field', ['name' => 'setup_help_text', 'label' => 'Message shown during setup', 'type' => 'textarea', 'rows' => 3, 'default' => $support['setup_help_text'] ?? '', 'placeholder' => 'Visit the ICT office in Block B if you get stuck.']) ?>
        <?php if (can('organization:manage')): ?><button class="btn btn-primary" type="submit">Save support contacts</button><?php endif; ?>
        </fieldset>
      </form>
    </section>
    <?php elseif ($owner): ?>
    <section class="card"><div class="card-head"><h2>Become an institution</h2></div><div class="card-body">
      <p class="text-2">Add hosted work mailboxes, webmail and phone setup for your people. Your keys, domains and history stay.</p>
      <form method="post" action="/console/settings/convert" data-confirm="Convert this workspace to an institution? This cannot be undone." data-confirm-label="Convert"><?= csrf_field() ?><button class="btn btn-primary" type="submit"><?= icon('building') ?> Convert to institution</button></form>
    </div></section>
    <?php endif; ?>
  </div>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Plan limits</h2><?php if (can('billing:read')): ?><a class="small" href="/console/billing/plans">Change plan</a><?php endif; ?></div>
      <div class="card-body"><dl class="dl">
        <dt>Plan</dt><dd><?= e($plan['name'] ?? '') ?></dd>
        <dt>Emails per day / month</dt><dd><?= number($plan['daily_send_limit'] ?? 0) ?> / <?= number($plan['monthly_send_limit'] ?? 0) ?></dd>
        <dt>Domains</dt><dd><?= number($plan['max_domains'] ?? 0) ?></dd>
        <?php if ($org['account_type'] === 'institution'): ?><dt>Mailboxes</dt><dd><?= number($plan['max_mailboxes'] ?? 0) ?>, <?= bytes($plan['default_mailbox_quota_bytes'] ?? 0) ?> each (max <?= bytes($plan['max_mailbox_quota_bytes'] ?? 0) ?>)</dd><?php endif; ?>
        <dt>Team members</dt><dd><?= number($plan['max_members'] ?? 0) ?></dd>
        <dt>API keys / webhooks</dt><dd><?= number($plan['max_api_keys'] ?? 0) ?> / <?= number($plan['max_webhooks'] ?? 0) ?></dd>
        <dt>Recipients per request</dt><dd><?= number($plan['max_recipients_per_request'] ?? 0) ?></dd>
        <dt>API requests per minute</dt><dd><?= number($plan['api_requests_per_minute'] ?? 0) ?></dd>
      </dl></div>
    </section>
    <?php if (!$owner): ?>
    <section class="card card-danger"><div class="card-head"><h2>Leave workspace</h2></div><div class="card-body row-between">
      <p class="small text-2 mb-0">You lose access immediately. An admin can invite you again.</p>
      <form method="post" action="/console/settings/leave" data-confirm="Leave <?= e($org['name']) ?>?" data-confirm-label="Leave"><?= csrf_field() ?><button class="btn btn-danger btn-sm" type="submit">Leave</button></form>
    </div></section>
    <?php endif; ?>
  </div>
</div>
