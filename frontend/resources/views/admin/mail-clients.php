<?php
$d = $s['default'] ?? null;
$env = $s['environment'] ?? [];
$form = static function (array $v, string $prefix) use ($view): string {
    ob_start(); ?>
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'imap_host', 'id' => $prefix . 'ih', 'label' => 'Incoming (IMAP) host', 'required' => true, 'value' => $v['imap_host'] ?? '']) ?>
      <?= $view->insert('partials/field', ['name' => 'imap_port', 'id' => $prefix . 'ip', 'label' => 'IMAP port (SSL/TLS)', 'type' => 'number', 'required' => true, 'value' => (string) ($v['imap_port'] ?? 993)]) ?>
      <?= $view->insert('partials/field', ['name' => 'smtp_host', 'id' => $prefix . 'sh', 'label' => 'Outgoing (SMTP) host', 'required' => true, 'value' => $v['smtp_host'] ?? '']) ?>
      <div class="grid grid-2">
        <?= $view->insert('partials/field', ['name' => 'smtp_port', 'id' => $prefix . 'sp', 'label' => 'SMTP port', 'type' => 'number', 'required' => true, 'value' => (string) ($v['smtp_port'] ?? 465)]) ?>
        <?= $view->insert('partials/field', ['name' => 'smtp_security', 'id' => $prefix . 'ss', 'label' => 'Security', 'type' => 'select', 'required' => true, 'value' => $v['smtp_security'] ?? 'tls', 'options' => ['tls' => 'SSL/TLS', 'starttls' => 'STARTTLS']]) ?>
      </div>
      <div class="grid grid-2">
        <?= $view->insert('partials/field', ['name' => 'smtp_alt_port', 'id' => $prefix . 'ap', 'label' => 'Alternative port', 'type' => 'number', 'value' => (string) ($v['smtp_alt_port'] ?? '')]) ?>
        <?= $view->insert('partials/field', ['name' => 'smtp_alt_security', 'id' => $prefix . 'as', 'label' => 'Alt. security', 'type' => 'select', 'noOptional' => true, 'value' => $v['smtp_alt_security'] ?? 'starttls', 'options' => ['starttls' => 'STARTTLS', 'tls' => 'SSL/TLS']]) ?>
      </div>
      <?= $view->insert('partials/field', ['name' => 'change_note', 'id' => $prefix . 'cn', 'label' => 'Change note', 'placeholder' => 'Moved IMAP to new server']) ?>
    </div>
    <?php return (string) ob_get_clean();
};
?>
<?= $view->insert('partials/page-head', ['heading' => 'Mail app settings', 'lede' => 'The server names and ports shown in phone guides, welcome sheets, autoconfig/Autodiscover and Apple profiles. Hosts must have valid public TLS certificates; never tell users to accept certificate warnings.']) ?>
<div class="alert alert-info mb-3"><?= icon('info') ?><div>Resolution order: domain override, then platform default, then server environment (<code><?= e(($env['imap_host'] ?? '') . ':' . ($env['imap_port'] ?? '')) ?></code>, <code><?= e(($env['smtp_host'] ?? '') . ':' . ($env['smtp_port'] ?? '')) ?></code>). RFC 8314 recommends implicit TLS: IMAP 993 and submission 465, with 587 STARTTLS as the alternative.</div></div>
<div class="grid grid-2" style="align-items:start">
  <section class="card"><div class="card-head"><h2>Platform default</h2><?= $d ? '<span class="muted small">v' . e($d['version']) . ' · ' . e(date_short($d['updated_at'] ?? null)) . '</span>' : badge('pending', 'Using environment') ?></div>
    <form method="post" action="/admin/mail-clients/default" class="card-body"><?= csrf_field() ?><?= $form($d ?? $env, 'd-') ?><button class="btn btn-primary" type="submit">Save default</button></form>
  </section>
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Domain overrides</h2></div>
      <?php if (empty($s['overrides'])): ?><div class="card-body"><p class="muted small mb-0">No overrides. Every domain uses the platform default.</p></div><?php else: ?>
      <div class="table-wrap"><table class="table table-compact"><tbody><?php foreach ($s['overrides'] as $o): ?>
        <tr><td><?= e($o['domain']) ?><div class="cell-sub"><?= e($o['organization'] ?? '') ?></div></td><td class="small"><code><?= e($o['imap_host'] . ':' . $o['imap_port']) ?></code><br><code><?= e($o['smtp_host'] . ':' . $o['smtp_port']) ?></code> <?= e($o['smtp_security']) ?></td><td>v<?= e($o['version']) ?></td>
        <td><form method="post" action="/admin/mail-clients/domains/<?= e($o['domain_id']) ?>/delete" data-confirm="Remove the override for <?= e($o['domain']) ?>?" data-confirm-label="Remove"><?= csrf_field() ?><button class="btn btn-sm btn-ghost" type="submit" aria-label="Remove"><?= icon('trash') ?></button></form></td></tr>
      <?php endforeach; ?></tbody></table></div>
      <?php endif; ?>
      <details class="card-foot"><summary>Add or replace an override</summary>
        <form method="post" action="/admin/mail-clients/domains" class="mt-2"><?= csrf_field() ?>
          <?= $view->insert('partials/field', ['name' => 'domain_id', 'id' => 'o-dom', 'label' => 'Domain ID', 'required' => true, 'hint' => 'From the organization page in this console.']) ?>
          <?= $form($d ?? $env, 'o-') ?><button class="btn btn-primary" type="submit">Save override</button>
        </form>
      </details>
    </section>
    <section class="card"><div class="card-head"><h2>Change history</h2></div><ol class="timeline card-body">
      <?php foreach ($s['history'] ?? [] as $h): ?><li><span class="dot"></span><strong><?= e(ucfirst($h['action'])) ?></strong> <?= e($h['scope']) ?> <span class="muted small"><?= e($h['changed_by'] ?? '') ?> · <?= time_tag($h['at']) ?></span><?= !empty($h['change_note']) ? '<div class="small text-2">' . e($h['change_note']) . '</div>' : '' ?></li><?php endforeach; ?>
      <?php if (empty($s['history'])): ?><li class="muted small">No changes yet.</li><?php endif; ?>
    </ol></section>
  </div>
</div>
