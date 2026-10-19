<?php
$c = $s['counts'];
$levelIcon = ['pass' => 'check-circle', 'warn' => 'alert-triangle', 'fail' => 'x-circle', 'info' => 'info'];
$controls = [
    'breached_password_screening' => 'Breached-password screening (k-anonymity)',
    'operator_mfa_enforced' => 'Two-factor required for operators',
    'secure_cookies' => 'Secure, HttpOnly session cookies',
    'sms_verification' => 'SMS account verification',
    'google_sign_in' => 'Google sign-in (OIDC + PKCE)',
    'mpesa_payments' => 'M-Pesa payments (signed webhooks)',
];
?>
<?= $view->insert('partials/page-head', ['heading' => 'Security', 'lede' => 'Authentication signals, privileged activity and the controls protecting the platform.']) ?>
<section class="card card-body mb-3">
  <ul class="tool-findings mb-0"><?php foreach ($s['findings'] as $f): ?><li class="is-<?= e($f['level']) ?>"><?= icon($levelIcon[$f['level']] ?? 'info', 'icon-sm') ?><span><?= e($f['message']) ?></span></li><?php endforeach; ?></ul>
</section>
<div class="grid grid-4 mb-3">
  <div class="stat"><div class="stat-label">Failed sign-ins, 24 h</div><div class="stat-value"><?= number($c['failed_logins_24h']) ?></div><div class="stat-foot"><?= number($c['failed_mfa_24h']) ?> failed 2FA codes</div></div>
  <div class="stat"><div class="stat-label">Locked accounts</div><div class="stat-value"><?= number($c['locked_users'] + $c['locked_mailboxes']) ?></div><div class="stat-foot"><?= number($c['locked_users']) ?> console · <?= number($c['locked_mailboxes']) ?> mailboxes</div></div>
  <div class="stat"><div class="stat-label">Operators without 2FA</div><div class="stat-value"><?= number($c['admins_without_mfa']) ?></div><div class="stat-foot">of <?= number($c['admins_total']) ?> operators</div></div>
  <div class="stat"><div class="stat-label">Owners without 2FA</div><div class="stat-value"><?= number($c['owners_without_mfa']) ?></div><div class="stat-foot"><?= number($c['unverified_users']) ?> unverified accounts</div></div>
</div>
<div class="grid grid-2 mb-3">
  <section class="card"><div class="card-head"><h2>Security controls</h2></div><div class="card-body">
    <ul class="control-list">
      <?php foreach ($controls as $k => $label): $on = !empty($s['controls'][$k]); ?><li><?= icon($on ? 'check-circle' : 'minus', 'icon-sm') ?><span><?= e($label) ?></span><?= $on ? badge('active', 'On') : badge('neutral', 'Off') ?></li><?php endforeach; ?>
      <li><?= icon('clock', 'icon-sm') ?><span>Message content retention</span><span class="tag"><?= (int) $s['controls']['content_retention_days'] ?> days</span></li>
    </ul>
  </div></section>
  <section class="card"><div class="card-head"><h2>Last 7 days</h2></div><div class="card-body"><dl class="dl">
    <dt>Password resets (24 h)</dt><dd><?= number($c['password_resets_24h']) ?></dd>
    <dt>Two-factor changes</dt><dd><?= number($c['mfa_changes_7d']) ?></dd>
    <dt>Role changes</dt><dd><?= number($c['role_changes_7d']) ?></dd>
    <dt>API keys created (24 h)</dt><dd><?= number($c['keys_created_24h']) ?></dd>
  </dl></div></section>
</div>
<div class="grid grid-2 mb-3">
  <section class="card"><div class="card-head"><h2>Most failed accounts, 24 h</h2></div>
    <?php if ($s['top_failing_accounts'] === []): ?><div class="card-body"><p class="muted mb-0">No failed sign-ins.</p></div><?php else: ?>
    <div class="table-wrap"><table class="table table-compact"><tbody><?php foreach ($s['top_failing_accounts'] as $a): ?><tr><td><?= e($a['account'] ?? 'unknown') ?></td><td class="num"><?= number($a['count']) ?></td></tr><?php endforeach; ?></tbody></table></div><?php endif; ?>
  </section>
  <section class="card"><div class="card-head"><h2>Most failing source addresses, 24 h</h2></div>
    <?php if ($s['top_failing_ips'] === []): ?><div class="card-body"><p class="muted mb-0">No failed sign-ins.</p></div><?php else: ?>
    <div class="table-wrap"><table class="table table-compact"><tbody><?php foreach ($s['top_failing_ips'] as $a): ?><tr><td><code><?= e($a['ip'] ?? '?') ?></code> <a class="small" href="/tools/blocklist?<?= e(http_build_query(['target' => $a['ip']])) ?>">check reputation</a></td><td class="num"><?= number($a['count']) ?></td></tr><?php endforeach; ?></tbody></table></div><?php endif; ?>
  </section>
</div>
<div class="grid grid-2">
  <section class="card"><div class="card-head"><h2>Platform operators</h2></div>
    <div class="table-wrap"><table class="table table-compact"><tbody><?php foreach ($s['operators'] as $o): ?>
      <tr><td><?= e($o['full_name']) ?><div class="cell-sub"><?= e($o['email']) ?></div></td><td><?= $o['mfa_enabled'] ? badge('active', '2FA on') : badge('failed', 'No 2FA') ?></td><td class="small muted nowrap"><?= $o['last_login_at'] ? time_tag($o['last_login_at']) : 'Never' ?></td></tr>
    <?php endforeach; ?></tbody></table></div>
  </section>
  <section class="card"><div class="card-head"><h2>Privileged activity</h2><a class="small" href="/console/audit">Audit log</a></div>
    <?php if ($s['privileged_activity'] === []): ?><div class="card-body"><p class="muted mb-0">No privileged changes recorded yet.</p></div><?php else: ?>
    <ol class="timeline card-body"><?php foreach ($s['privileged_activity'] as $a): ?><li><span class="dot"></span><code class="small"><?= e($a['action']) ?></code> <span class="small muted"><?= e($a['actor'] ?? 'system') ?><?= $a['ip'] ? ' · ' . e($a['ip']) : '' ?> · <?= time_tag($a['at']) ?></span></li><?php endforeach; ?></ol><?php endif; ?>
  </section>
</div>
