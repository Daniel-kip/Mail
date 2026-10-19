<?php $planId = is_array($org['plan']) ? ($org['plan']['id'] ?? '') : $org['plan']; ?>
<?= $view->insert('partials/page-head', ['heading' => $org['name'], 'lede' => $org['slug'] . ' · ' . ucfirst($org['account_type']) . ' · created ' . date_short($org['created_at'])]) ?>
<?php if ($org['status'] === 'suspended'): ?><div class="alert alert-error mb-3"><?= icon('ban') ?><div>Suspended: <?= e($org['suspended_reason'] ?? '') ?></div></div><?php endif; ?>
<div class="grid grid-4 mb-3">
  <div class="stat"><div class="stat-label">Plan</div><div class="stat-value" style="font-size:1.1rem"><?= e(is_array($org['plan']) ? ($org['plan']['name'] ?? '') : $org['plan']) ?></div><div class="stat-foot"><?= e(($org['subscription']['status'] ?? '') . ' · ' . ($org['subscription']['billing_cycle'] ?? '')) ?></div></div>
  <div class="stat"><div class="stat-label">Mailboxes</div><div class="stat-value"><?= number($org['mailboxes'] ?? 0) ?></div></div>
  <div class="stat"><div class="stat-label">Active API keys</div><div class="stat-value"><?= number($org['active_api_keys'] ?? 0) ?></div></div>
  <div class="stat"><div class="stat-label">Messages, 30 d</div><div class="stat-value"><?= number($org['messages_30d'] ?? 0) ?></div></div>
</div>
<div class="grid grid-2" style="align-items:start">
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Members</h2></div><div class="table-wrap"><table class="table table-compact"><tbody>
      <?php foreach ($org['members'] ?? [] as $m): ?><tr><td><?= e($m['full_name']) ?><div class="cell-sub"><?= e($m['email']) ?></div></td><td><?= e(ucfirst($m['role'])) ?></td><td><?= !empty($m['mfa_enabled']) ? badge('active', '2FA') : '' ?></td><td class="small muted"><?= $m['last_login_at'] ? time_tag($m['last_login_at']) : 'Never' ?></td></tr><?php endforeach; ?>
    </tbody></table></div></section>
    <section class="card"><div class="card-head"><h2>Domains</h2></div><div class="table-wrap"><table class="table table-compact"><tbody>
      <?php foreach ($org['domains'] ?? [] as $d): ?><tr><td><?= e($d['domain']) ?><div class="cell-sub"><code><?= e($d['id'] ?? '') ?></code></div></td><td><?= badge($d['status']) ?></td><td class="small"><?= !empty($d['mail_hosting_enabled']) ? 'Hosting on' : '' ?></td></tr><?php endforeach; ?>
      <?php if (empty($org['domains'])): ?><tr><td class="muted">No domains.</td></tr><?php endif; ?>
    </tbody></table></div></section>
  </div>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Plan</h2></div>
      <form method="post" action="/admin/organizations/<?= e($org['id']) ?>/plan" class="card-body inline-form"><?= csrf_field() ?>
        <select class="input" name="plan" aria-label="Plan"><?php foreach ($plans as $p): if ($p['audience'] !== $org['account_type']) { continue; } ?><option value="<?= e($p['id']) ?>"<?= selected($planId, $p['id']) ?>><?= e($p['name']) ?></option><?php endforeach; ?></select>
        <button class="btn" type="submit">Change plan</button>
      </form>
    </section>
    <section class="card card-danger"><div class="card-head"><h2><?= $org['status'] === 'suspended' ? 'Reinstate' : 'Suspend' ?></h2></div><div class="card-body">
      <?php if ($org['status'] === 'suspended'): ?>
        <form method="post" action="/admin/organizations/<?= e($org['id']) ?>/unsuspend"><?= csrf_field() ?><button class="btn btn-primary" type="submit">Reinstate organization</button></form>
      <?php else: ?>
        <form method="post" action="/admin/organizations/<?= e($org['id']) ?>/suspend" data-confirm="Suspend <?= e($org['name']) ?>? Sending and changes stop immediately." data-confirm-label="Suspend"><?= csrf_field() ?>
          <?= $view->insert('partials/field', ['name' => 'reason', 'label' => 'Reason (shown to the organization)', 'required' => true]) ?>
          <button class="btn btn-danger" type="submit">Suspend</button>
        </form>
      <?php endif; ?>
    </div></section>
    <section class="card"><div class="card-head"><h2>Recent activity</h2></div><ol class="timeline card-body">
      <?php foreach ($org['recent_activity'] ?? [] as $a): ?><li><span class="dot"></span><code class="small"><?= e($a['action']) ?></code> <span class="muted small"><?= e($a['actor'] ?? '') ?> · <?= time_tag($a['at']) ?></span></li><?php endforeach; ?>
    </ol></section>
  </div>
</div>
