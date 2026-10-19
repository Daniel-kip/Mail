<?php $q = $o['queue'] ?? []; $b = $o['billing'] ?? []; $cur = $b['currency'] ?? 'KES'; ?>
<?= $view->insert('partials/page-head', ['heading' => 'Platform overview', 'lede' => 'Health, delivery and receivables across all tenants.']) ?>
<?php if (empty($o['smtp_relay']) || empty($o['mail_storage_configured'])): ?>
  <div class="alert alert-warning mb-3"><?= icon('alert-triangle') ?><div><?= empty($o['smtp_relay']) ? 'No SMTP relay is configured: outbound mail cannot be delivered. ' : '' ?><?= empty($o['mail_storage_configured']) ? 'Mail storage (IMAP) is not configured: webmail and mailbox usage are unavailable.' : '' ?></div></div>
<?php endif; ?>
<div class="grid grid-4 mb-3">
  <div class="stat"><div class="stat-label">Organizations</div><div class="stat-value"><?= number($o['organizations'] ?? 0) ?></div><div class="stat-foot"><?= number($o['suspended_organizations'] ?? 0) ?> suspended</div></div>
  <div class="stat"><div class="stat-label">Users / mailboxes</div><div class="stat-value"><?= number($o['users'] ?? 0) ?> / <?= number($o['mailboxes'] ?? 0) ?></div></div>
  <div class="stat"><div class="stat-label">Messages, 24 h</div><div class="stat-value"><?= number($o['messages_24h'] ?? 0) ?></div><div class="stat-foot"><?= number($o['bounced_24h'] ?? 0) ?> bounced · <?= number($o['failed_24h'] ?? 0) ?> failed</div></div>
  <div class="stat"><div class="stat-label">Queue</div><div class="stat-value"><?= number($q['pending'] ?? 0) ?></div><div class="stat-foot"><?= number($q['processing'] ?? 0) ?> processing · <a href="/admin/queue"><?= number($q['dead'] ?? 0) ?> dead</a><?= ($q['oldest_pending_seconds'] ?? 0) > 300 ? ' · oldest ' . (int) round($q['oldest_pending_seconds'] / 60) . ' min' : '' ?></div></div>
</div>
<div class="grid grid-2">
  <section class="card"><div class="card-head"><h2>Receivables</h2><a class="small" href="/admin/billing">Invoices</a></div><div class="card-body"><dl class="dl">
    <dt>Open invoices</dt><dd><?= number($b['open_invoices'] ?? 0) ?> · <?= e(money($b['open_total_cents'] ?? 0, $cur)) ?></dd>
    <dt>Overdue</dt><dd><?= number($b['overdue_invoices'] ?? 0) ?></dd>
    <dt>Collected, 30 days</dt><dd><?= e(money($b['paid_last_30_days_cents'] ?? 0, $cur)) ?></dd>
    <dt>Past-due subscriptions</dt><dd><?= number($b['past_due_subscriptions'] ?? 0) ?></dd>
    <dt>In trial</dt><dd><?= number($b['trialing'] ?? 0) ?></dd>
  </dl></div></section>
  <section class="card"><div class="card-head"><h2>Organizations by plan</h2></div><div class="card-body"><dl class="dl">
    <?php foreach ($o['organizations_by_plan'] ?? [] as $p): ?><dt><?= e($p['name'] ?? $p['plan']) ?></dt><dd><?= number($p['organizations'] ?? $p['count'] ?? 0) ?></dd><?php endforeach; ?>
    <dt>Webhook failures, 24 h</dt><dd><?= number($o['webhook_failures_24h'] ?? 0) ?></dd>
    <dt>Out-of-office (Sieve)</dt><dd><?= !empty($o['auto_reply_configured']) ? 'Configured' : 'Not configured' ?></dd>
  </dl></div></section>
</div>
