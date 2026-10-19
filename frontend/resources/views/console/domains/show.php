<?php
$records = $domain['dns_records'] ?? [];
$discovery = $domain['client_discovery_records'] ?? ($domain['dns_records_optional'] ?? []);
$rows = static function (array $records, $view): string {
    ob_start(); ?>
    <div class="table-wrap"><table class="table dns-records">
      <thead><tr><th>Type</th><th>Host / name</th><th>Value</th><th>Purpose</th><th>Status</th></tr></thead>
      <tbody>
      <?php foreach ($records as $i => $r): $id = 'rec-' . md5($r['type'] . $r['host'] . $r['value']); ?>
        <tr>
          <td><span class="tag"><?= e($r['type']) ?></span><?= empty($r['required']) ? '<div class="cell-sub">Optional</div>' : '' ?></td>
          <td><div class="copy-field"><span class="value" id="<?= $id ?>-h"><?= e($r['host']) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="<?= $id ?>-h" aria-label="Copy host"><?= icon('copy') ?></button></div></td>
          <td><div class="copy-field dns-value"><span class="value" id="<?= $id ?>-v"><?= e($r['value']) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="<?= $id ?>-v" aria-label="Copy value"><?= icon('copy') ?></button></div></td>
          <td class="small text-2"><?= e($r['purpose'] ?? '') ?></td>
          <td><?= badge($r['status'] ?? 'unchecked') ?></td>
        </tr>
      <?php endforeach; ?>
      </tbody>
    </table></div>
    <?php return (string) ob_get_clean();
};
?>
<?= $view->insert('partials/page-head', [
    'heading' => $domain['domain'],
    'lede' => 'Added ' . date_short($domain['created_at']) . ($domain['last_checked_at'] ? ' · last checked ' . ago($domain['last_checked_at']) : ''),
    'actions' => can('domains:write') ? '<form method="post" action="/console/domains/' . e($domain['id']) . '/verify">' . csrf_field() . '<button class="btn btn-primary" type="submit">' . icon('refresh') . ' Check DNS</button></form>' : '',
]) ?>
<div class="grid grid-4 mb-3">
  <div class="stat"><div class="stat-label">Status</div><div class="stat-value" style="font-size:1.1rem"><?= badge($domain['status']) ?></div></div>
  <div class="stat"><div class="stat-label">Sending</div><div class="stat-value" style="font-size:1.1rem"><?= !empty($domain['sending_enabled']) ? badge('active', 'Enabled') : badge('pending', 'After verification') ?></div></div>
  <div class="stat"><div class="stat-label">DKIM selector</div><div class="stat-value" style="font-size:1.1rem"><code><?= e($domain['dkim_selector'] ?? '') ?></code></div></div>
  <div class="stat"><div class="stat-label">Mailboxes</div><div class="stat-value"><?= number($domain['mailbox_count'] ?? 0) ?></div></div>
</div>
<section class="card mb-3">
  <div class="card-head"><h2>Required DNS records</h2></div>
  <div class="card-body"><p class="small text-2">Add these at your DNS provider. Some providers add the domain automatically to the host name; if yours does, enter only the part before <code><?= e($domain['domain']) ?></code>. Changes usually appear within an hour.</p></div>
  <?= $rows(array_values(array_filter($records, static fn ($r) => !empty($r['required']) || ($r['type'] ?? '') === 'MX')), $view) ?>
</section>
<?php $optional = array_values(array_filter($records, static fn ($r) => empty($r['required']) && ($r['type'] ?? '') !== 'MX')); ?>
<?php if ($optional !== [] || $discovery !== []): ?>
<section class="card mb-3">
  <div class="card-head"><h2>Recommended records</h2></div>
  <div class="card-body"><p class="small text-2">Optional records that improve reporting and let mail apps find your server settings automatically (RFC 6186 SRV records and autoconfig). Mail still works without them; the guided phone setup gives employees exact settings either way.</p></div>
  <?= $rows(array_merge($optional, $discovery), $view) ?>
</section>
<?php endif; ?>
<?php $cs = $domain['client_settings'] ?? null; ?>
<?php if (is_array($cs) && !empty($domain['mail_hosting_enabled'])): ?>
<section class="card mb-3">
  <div class="card-head"><h2>Email server settings</h2><span class="small muted">For mail apps, websites and contact forms</span></div>
  <div class="card-body">
    <p class="small text-2">Your <b>username</b> is the full mailbox address and the <b>password</b> is the one the mailbox owner chose. Employees get these with a guided setup at <a href="/mail/setup">/mail/setup</a>.</p>
    <div class="server-cards">
      <div class="server-card">
        <h3><?= icon('send') ?> Outgoing mail (SMTP) <span class="tag">sending</span></h3>
        <?php foreach ([['Server', $cs['smtp']['host'] ?? ''], ['Port', (string) ($cs['smtp']['port'] ?? '') . ' (' . ($cs['smtp']['security'] ?? '') . ')'], ['Alternative', !empty($cs['smtp_alternative']) ? $cs['smtp_alternative']['port'] . ' (' . $cs['smtp_alternative']['security'] . ')' : 'None'], ['Authentication', 'Normal password, required'], ['Username', 'mailbox@' . $domain['domain']]] as $i => [$k, $v]): ?>
          <div class="server-row"><span><?= e($k) ?></span><div class="copy-field"><span class="value" id="smtp-<?= $i ?>"><?= e($v) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="smtp-<?= $i ?>" aria-label="Copy"><?= icon('copy') ?></button></div></div>
        <?php endforeach; ?>
      </div>
      <div class="server-card">
        <h3><?= icon('inbox') ?> Incoming mail (IMAP) <span class="tag">receiving</span></h3>
        <?php foreach ([['Server', $cs['imap']['host'] ?? ''], ['Port', (string) ($cs['imap']['port'] ?? '')], ['Encryption', $cs['imap']['security'] ?? 'SSL/TLS'], ['Authentication', 'Normal password'], ['Username', 'mailbox@' . $domain['domain']]] as $i => [$k, $v]): ?>
          <div class="server-row"><span><?= e($k) ?></span><div class="copy-field"><span class="value" id="imap-<?= $i ?>"><?= e($v) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="imap-<?= $i ?>" aria-label="Copy"><?= icon('copy') ?></button></div></div>
        <?php endforeach; ?>
      </div>
    </div>
  </div>
</section>
<?php endif; ?>
<?php
$zone = '; ' . $domain['domain'] . " — records generated by " . $appName . "\n";
foreach (array_merge($records, $discovery) as $rec) {
    $host = rtrim((string) $rec['host'], '.') . '.';
    $value = in_array($rec['type'], ['TXT'], true) ? '"' . implode('" "', str_split((string) $rec['value'], 255)) . '"' : (string) $rec['value'];
    $zone .= sprintf("%-48s 3600 IN %-5s %s\n", $host, $rec['type'], $value);
}
?>
<details class="card mb-3">
  <summary class="card-head" style="cursor:pointer"><h2><?= icon('download') ?> Zone file (BIND format)</h2><span class="small muted">Import into Cloudflare, cPanel or any DNS host that accepts zone files</span></summary>
  <div class="card-body"><pre class="tool-record" id="zone-file"><?= e($zone) ?></pre><button class="btn btn-sm" type="button" data-copy-target="zone-file"><?= icon('copy') ?><span class="copy-label">Copy zone file</span></button></div>
</details>
<?php if (is_institution() && can('domains:write')): ?>
<section class="card mb-3">
  <div class="card-head"><h2>Mail hosting</h2><?= !empty($domain['mail_hosting_enabled']) ? badge('active', 'On') : badge('disabled', 'Off') ?></div>
  <div class="card-body row-between">
    <p class="text-2 mb-0">Receive mail for <strong>@<?= e($domain['domain']) ?></strong> in hosted mailboxes with webmail and phone access.</p>
    <form method="post" action="/console/domains/<?= e($domain['id']) ?>/hosting"><?= csrf_field() ?>
      <input type="hidden" name="enabled" value="<?= empty($domain['mail_hosting_enabled']) ? '1' : '0' ?>">
      <button class="btn <?= empty($domain['mail_hosting_enabled']) ? 'btn-primary' : '' ?>" type="submit"<?= $domain['status'] !== 'verified' && empty($domain['mail_hosting_enabled']) ? ' disabled title="Verify the domain first"' : '' ?>><?= empty($domain['mail_hosting_enabled']) ? 'Enable mail hosting' : 'Disable mail hosting' ?></button>
    </form>
  </div>
</section>
<?php endif; ?>
<?php if (can('domains:write')): ?>
<section class="card card-danger">
  <div class="card-head"><h2>Remove domain</h2></div>
  <div class="card-body row-between"><p class="text-2 mb-0">Sending from this domain stops immediately. Domains with mailboxes cannot be removed.</p>
    <form method="post" action="/console/domains/<?= e($domain['id']) ?>/delete" data-confirm="Remove <?= e($domain['domain']) ?>? Sending from it stops immediately." data-confirm-label="Remove domain"><?= csrf_field() ?><button class="btn btn-danger" type="submit">Remove</button></form>
  </div>
</section>
<?php endif; ?>
