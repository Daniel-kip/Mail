<?php
$cs = $m['client_settings'] ?? [];
$setup = $m['client_setup'] ?? [];
$usage = $m['usage'] ?? null;
$pending = $m['status'] === 'pending';
$actions = '<a class="btn" href="/console/mailboxes/' . e($m['id']) . '/handout">' . icon('printer') . ' Welcome sheet</a>';
?>
<?= $view->insert('partials/page-head', ['heading' => $m['display_name'] ?: $m['address'], 'lede' => $m['address'] . (!empty($m['job_title']) ? ' · ' . $m['job_title'] : '') . (!empty($m['department']) ? ' · ' . $m['department'] : ''), 'actions' => $actions]) ?>

<?php if ($handover): ?>
  <section class="secret-box mb-3" aria-live="polite">
    <strong><?= icon('key') ?> Shown only once</strong>
    <?php if (!empty($handover['password'])): ?>
      <p class="small mb-1">Password for <?= e($m['address']) ?>. Share it over a secure channel and ask the user to change it after signing in.</p>
      <div class="copy-field"><span class="value" id="handover-pw"><?= e($handover['password']) ?></span><button class="btn btn-sm" type="button" data-copy-target="handover-pw"><?= icon('copy') ?><span class="copy-label">Copy</span></button></div>
    <?php endif; ?>
    <?php if (!empty($handover['activation_url']) || !empty($handover['reset_url'])): ?>
      <p class="small mb-1"><?= !empty($handover['reset_url']) ? 'Password reset link' : 'Activation link' ?>, single-use<?= !empty($handover['expires_at']) ? ', expires ' . e(datetime($handover['expires_at'])) : '' ?>. The user opens it and chooses their own password.</p>
      <div class="copy-field"><span class="value" id="handover-link"><?= e($handover['activation_url'] ?? $handover['reset_url']) ?></span><button class="btn btn-sm" type="button" data-copy-target="handover-link"><?= icon('copy') ?><span class="copy-label">Copy</span></button></div>
      <div class="qr-box mt-2" style="max-width:180px"><?= \App\View\QrCode::svg((string) ($handover['activation_url'] ?? $handover['reset_url']), 3, 'QR code for the link') ?></div>
    <?php endif; ?>
    <?php if (!empty($handover['sent_to'])): ?><p class="small mb-0">Invitation emailed to <strong><?= e($handover['sent_to']) ?></strong>.</p><?php endif; ?>
  </section>
<?php endif; ?>

<div class="grid grid-4 mb-3">
  <div class="stat"><div class="stat-label">Status</div><div class="stat-value" style="font-size:1.05rem"><?= badge($m['status'], $pending ? 'Awaiting activation' : null) ?><?= !empty($m['locked']) ? ' ' . badge('failed', 'Locked') : '' ?></div></div>
  <div class="stat"><div class="stat-label">Phone setup</div><div class="stat-value" style="font-size:1.05rem"><?= badge($setup['state'] ?? 'not_started') ?></div><div class="stat-foot"><?= e($setup['last_client'] ?? '') ?></div></div>
  <div class="stat"><div class="stat-label">Storage</div><div class="stat-value" style="font-size:1.05rem"><?= is_array($usage) && isset($usage['used_bytes']) ? bytes($usage['used_bytes']) . ' / ' . bytes($usage['limit_bytes'] ?: $m['quota_bytes']) : bytes($m['quota_bytes']) ?></div></div>
  <div class="stat"><div class="stat-label">Last sign-in</div><div class="stat-value" style="font-size:1.05rem"><?= $m['last_login_at'] ? time_tag($m['last_login_at']) : 'Never' ?></div><div class="stat-foot"><?= $m['password_changed_at'] ? 'Password changed ' . ago($m['password_changed_at']) : '' ?></div></div>
</div>

<div class="grid grid-2">
  <div class="stack">
    <?php if (can('mailboxes:write')): ?>
    <section class="card">
      <div class="card-head"><h2>Details</h2></div>
      <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>" class="card-body">
        <?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'display_name', 'label' => 'Full name', 'required' => true, 'default' => $m['display_name'] ?? '']) ?>
        <div class="grid grid-2">
          <?= $view->insert('partials/field', ['name' => 'department', 'label' => 'Department', 'default' => $m['department'] ?? '']) ?>
          <?= $view->insert('partials/field', ['name' => 'job_title', 'label' => 'Job title', 'default' => $m['job_title'] ?? '']) ?>
        </div>
        <?= $view->insert('partials/field', ['name' => 'recovery_email', 'label' => 'Personal email (for resets)', 'type' => 'email', 'default' => $m['recovery_email'] ?? '']) ?>
        <?= $view->insert('partials/field', ['name' => 'quota_mb', 'label' => 'Storage (MB)', 'type' => 'number', 'default' => (string) intdiv((int) $m['quota_bytes'], 1048576), 'attrs' => ' min="50" step="50"']) ?>
        <button class="btn btn-primary" type="submit">Save</button>
      </form>
    </section>
    <?php else: ?>
    <section class="card"><div class="card-head"><h2>Details</h2></div><div class="card-body"><dl class="dl">
      <dt>Personal email</dt><dd><?= e($m['recovery_email'] ?? ($m['has_recovery_email'] ? 'On file' : 'None')) ?></dd>
      <dt>Created</dt><dd><?= e(date_short($m['created_at'])) ?></dd><dt>Activated</dt><dd><?= e($m['activated_at'] ? date_short($m['activated_at']) : '—') ?></dd>
    </dl></div></section>
    <?php endif; ?>

    <section class="card" id="access">
      <div class="card-head"><h2>Access and support</h2></div>
      <div class="card-body stack">
        <p class="small text-2 mb-0">Support staff can help without ever seeing or choosing the user's password.</p>
        <?php if ($pending && can('mailboxes:assist')): ?>
          <?php if (!empty($m['pending_activation'])): ?><p class="small mb-0">Current activation: <?= e($m['pending_activation']['delivery'] ?? '') ?>, expires <?= e(datetime($m['pending_activation']['expires_at'] ?? null)) ?>.</p><?php endif; ?>
          <div class="row">
            <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/activation"><?= csrf_field() ?><input type="hidden" name="method" value="invite"><button class="btn btn-sm" type="submit"<?= empty($m['has_recovery_email']) ? ' disabled title="Add a personal email first"' : '' ?>><?= icon('send') ?> Resend invitation</button></form>
            <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/activation"><?= csrf_field() ?><input type="hidden" name="method" value="link"><button class="btn btn-sm" type="submit"><?= icon('link') ?> New activation link</button></form>
          </div>
        <?php endif; ?>
        <?php if (!$pending && can('mailboxes:assist')): ?>
          <div class="row">
            <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/send-reset"><?= csrf_field() ?><input type="hidden" name="method" value="email"><button class="btn btn-sm" type="submit"<?= empty($m['has_recovery_email']) ? ' disabled title="No personal email on file"' : '' ?>><?= icon('send') ?> Email a reset link</button></form>
            <?php if (can('mailboxes:write')): ?><form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/send-reset"><?= csrf_field() ?><input type="hidden" name="method" value="link"><button class="btn btn-sm" type="submit"><?= icon('link') ?> Create reset link</button></form><?php endif; ?>
            <?php if (!empty($m['locked'])): ?><form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/unlock"><?= csrf_field() ?><button class="btn btn-sm" type="submit"><?= icon('unlock') ?> Unlock sign-in</button></form><?php endif; ?>
          </div>
        <?php endif; ?>
        <?php if (!$pending && can('mailboxes:write')): ?>
          <details class="cant-find"><summary>Set or generate a password instead</summary>
            <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/password" class="mt-2" data-confirm="Change this password? The user is signed out and must update it on every device." data-confirm-label="Change password">
              <?= csrf_field() ?>
              <label class="check"><input type="radio" name="mode" value="generate" checked> Generate a strong password (shown once)</label>
              <label class="check"><input type="radio" name="mode" value="set"> Set this password:</label>
              <input class="input mt-1" type="password" name="password" autocomplete="new-password" minlength="10" aria-label="New password"<?= error_attrs('password') ?>><?= error_text('password') ?>
              <button class="btn btn-sm mt-2" type="submit">Change password</button>
            </form>
          </details>
        <?php endif; ?>
      </div>
    </section>
  </div>

  <div class="stack">
    <section class="card">
      <div class="card-head"><h2>Phone and app settings</h2><?php if (!empty($cs['source'])): ?><span class="muted small"><?= e($cs['source']) ?> v<?= e($cs['version'] ?? 1) ?></span><?php endif; ?></div>
      <div class="card-body"><dl class="dl">
        <dt>User name</dt><dd><code><?= e($cs['username'] ?? $m['address']) ?></code></dd>
        <dt>Incoming (IMAP)</dt><dd><code><?= e(($cs['imap']['host'] ?? '') . ':' . ($cs['imap']['port'] ?? '')) ?></code> · <?= e($cs['imap']['security'] ?? '') ?></dd>
        <dt>Outgoing (SMTP)</dt><dd><code><?= e(($cs['smtp']['host'] ?? '') . ':' . ($cs['smtp']['port'] ?? '')) ?></code> · <?= e($cs['smtp']['security'] ?? '') ?></dd>
        <?php if (!empty($cs['smtp_alternative'])): ?><dt>Alternative outgoing</dt><dd><code><?= e($cs['smtp_alternative']['port']) ?></code> · <?= e($cs['smtp_alternative']['security']) ?></dd><?php endif; ?>
        <dt>Verified</dt><dd>Incoming <?= $setup['incoming_verified_at'] ?? null ? icon('check-circle', 'icon-sm') . ' ' . e(date_short($setup['incoming_verified_at'])) : '—' ?> · Outgoing <?= $setup['outgoing_verified_at'] ?? null ? icon('check-circle', 'icon-sm') . ' ' . e(date_short($setup['outgoing_verified_at'])) : '—' ?></dd>
      </dl></div>
    </section>
    <section class="card">
      <div class="card-head"><h2>Setup activity</h2></div>
      <?php if (empty($m['setup_events'])): ?><div class="card-body"><p class="muted small mb-0">No phone setup activity yet. Print the welcome sheet to help them start.</p></div><?php else: ?>
      <ol class="timeline card-body">
        <?php foreach ($m['setup_events'] as $ev): ?><li><span class="dot"></span><strong><?= e(\App\View\Format::statusLabel((string) $ev['event'])) ?></strong> <span class="muted small"><?= e(trim(($ev['device'] ?? '') . ' ' . ($ev['client'] ?? ''))) ?> · <?= time_tag($ev['at']) ?></span></li><?php endforeach; ?>
      </ol>
      <?php endif; ?>
    </section>
    <?php if (!empty($m['aliases'])): ?>
    <section class="card"><div class="card-head"><h2>Aliases delivering here</h2></div><div class="card-body"><?php foreach ($m['aliases'] as $a): ?><span class="tag"><?= e(is_array($a) ? $a['address'] : $a) ?></span> <?php endforeach; ?></div></section>
    <?php endif; ?>
    <?php if (can('mailboxes:write')): ?>
    <section class="card card-danger">
      <div class="card-head"><h2>Suspend or delete</h2></div>
      <div class="card-body stack">
        <?php if (in_array($m['status'], ['active', 'suspended'], true)): ?>
          <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/status"><?= csrf_field() ?><input type="hidden" name="status" value="<?= $m['status'] === 'active' ? 'suspended' : 'active' ?>"><button class="btn btn-sm" type="submit"><?= $m['status'] === 'active' ? icon('pause') . ' Suspend sign-in' : icon('play') . ' Reactivate' ?></button></form>
        <?php endif; ?>
        <form method="post" action="/console/mailboxes/<?= e($m['id']) ?>/delete" class="inline-form">
          <?= csrf_field() ?><input type="hidden" name="address" value="<?= e($m['address']) ?>">
          <input class="input" name="confirm_address" placeholder="Type <?= e($m['address']) ?>" aria-label="Type the address to confirm" autocomplete="off">
          <button class="btn btn-danger btn-sm" type="submit">Delete mailbox and mail</button>
        </form>
      </div>
    </section>
    <?php endif; ?>
  </div>
</div>
