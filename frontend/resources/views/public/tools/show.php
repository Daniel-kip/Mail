<?php
$levelIcon = ['pass' => 'check-circle', 'info' => 'info', 'warn' => 'alert-triangle', 'fail' => 'x-circle'];
$findings = static function (array $list) use ($levelIcon): string {
    $out = '<ul class="tool-findings">';
    foreach ($list as $f) {
        [$level, $msg] = isset($f['level']) ? [$f['level'], $f['message']] : [$f[0], $f[1]];
        $out .= '<li class="is-' . e($level) . '">' . icon($levelIcon[$level] ?? 'info', 'icon-sm') . '<span>' . e($msg) . '</span></li>';
    }
    return $out . '</ul>';
};
$copyRow = static function (string $id, string $value): string {
    return '<div class="copy-field"><span class="value" id="' . e($id) . '">' . e($value) . '</span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="' . e($id) . '" aria-label="Copy">' . icon('copy') . '</button></div>';
};
[$tIcon, $tName, $tCat, $tSummary, , $tTone] = $tools[$tool];
$r = $result;
?>
<section class="hero hero-dark hero-compact">
  <div class="container">
    <a class="back-link" href="/tools"><?= icon('arrow-left', 'icon-xs') ?> All tools</a>
    <div class="tool-title tone-<?= e($tTone) ?>"><span class="tool-icon"><?= icon($tIcon) ?></span><div><span class="eyebrow eyebrow-glow"><?= e($tCat) ?></span><h1><?= e($tName) ?></h1></div></div>
    <p class="lead"><?= e($tSummary) ?></p>
    <nav class="tool-tabs" aria-label="Tools">
      <?php foreach ($tools as $slug => [$ic, $label]): ?>
        <a href="/tools/<?= e($slug) ?>" class="<?= $tool === $slug ? 'is-active' : '' ?>"<?= $tool === $slug ? ' aria-current="page"' : '' ?>><?= icon($ic, 'icon-sm') ?> <?= e($label) ?></a>
      <?php endforeach; ?>
    </nav>
  </div>
</section>

<section class="section tools-section">
  <div class="container">
    <?php if ($error): ?><div class="alert alert-error mb-3"><?= icon('alert-circle') ?><div><?= e($error) ?></div></div><?php endif; ?>

<?php if ($tool === 'domain'): ?>
    <form class="card card-body tool-form" method="get" action="/tools/domain">
      <label class="label" for="t-domain">Domain or email address</label>
      <div class="input-group"><input class="input input-lg" id="t-domain" name="domain" value="<?= e($input['domain'] ?? '') ?>" placeholder="yourcompany.co.ke" required autocomplete="off" spellcheck="false" autofocus><button class="btn btn-primary btn-lg" type="submit"><?= icon('search') ?> Check domain</button></div>
      <p class="small muted mb-0 mt-1">MX, SPF (with the RFC 7208 10-lookup limit), DMARC, DKIM (23 common selectors), MTA-STS, TLS-RPT, BIMI, reverse DNS and five blocklists.</p>
    </form>
    <?php if ($r): ?>
      <div class="tool-report mt-3">
        <div class="card card-body tool-score">
          <div class="grade grade-<?= e(strtolower($r['grade'])) ?>"><?= e($r['grade']) ?></div>
          <div class="grow"><h2 class="mb-0"><?= e($r['domain']) ?></h2><p class="text-2 mb-0">Deliverability score <strong><?= (int) $r['score'] ?>/100</strong> · checked <?= e(datetime($r['checked_at'])) ?></p>
            <div class="score-bar" role="img" aria-label="Score <?= (int) $r['score'] ?> of 100"><span style="width:<?= (int) $r['score'] ?>%"></span></div></div>
          <a class="btn btn-sm" href="/tools/domain?<?= e(http_build_query(['domain' => $r['domain']])) ?>"><?= icon('refresh') ?> Recheck</a>
        </div>
        <?php foreach ([['mx', 'Mail servers (MX)', 'server'], ['spf', 'SPF', 'shield'], ['dmarc', 'DMARC', 'shield-check'], ['dkim', 'DKIM', 'key'], ['mta_sts', 'MTA-STS', 'lock'], ['tls_rpt', 'TLS reporting', 'activity'], ['bimi', 'BIMI', 'sparkles']] as [$key, $label, $ic]):
            $sec = $r[$key];
            $status = ['pass' => ['verified', 'Pass'], 'fail' => ['failed', 'Needs fixing'], 'warn' => ['pending', 'Improve'], 'info' => ['neutral', 'Optional']][$sec['status']] ?? ['neutral', 'Info']; ?>
          <details class="card tool-section is-<?= e($sec['status']) ?>"<?= in_array($sec['status'], ['fail', 'warn'], true) ? ' open' : '' ?>>
            <summary class="card-head"><span class="row"><?= icon($ic) ?> <strong><?= e($label) ?></strong></span><?= badge($status[0], $status[1]) ?></summary>
            <div class="card-body">
              <?= $findings($sec['findings'] ?? []) ?>
              <?php if ($key === 'mx'): ?>
                <?php foreach ($sec['records'] as $mx): ?>
                  <div class="tool-mx"><code><?= e($mx['preference'] . ' ' . $mx['host']) ?></code>
                    <?php foreach ($mx['addresses'] as $a): ?><div class="small"><code><?= e($a['ip']) ?></code> · PTR <?= e(implode(', ', $a['ptr']) ?: 'none') ?> <?= $a['fcrdns'] ? badge('verified', 'FCrDNS') : '' ?> <?= $a['listed_on'] ? badge('failed', 'Listed: ' . implode(', ', $a['listed_on'])) : badge('active', 'Not listed') ?></div><?php endforeach; ?>
                  </div>
                <?php endforeach; ?>
              <?php elseif ($key === 'spf' && !empty($sec['record'])): ?>
                <p class="small mb-1">DNS lookups: <strong><?= (int) $sec['lookups'] ?>/10</strong></p><?= $copyRow('rec-spf', $sec['record']) ?>
              <?php elseif ($key === 'dkim'): ?>
                <?php foreach ($sec['keys'] as $i => $k): ?><p class="small mb-1 mt-1"><strong><?= e($k['selector']) ?>._domainkey</strong></p><pre class="tool-record"><?= e($k['record']) ?></pre><?php endforeach; ?>
                <p class="small muted mb-0">Checked <?= (int) $sec['selectors_checked'] ?> common selectors.</p>
              <?php elseif (!empty($sec['record'])): ?>
                <?= $copyRow('rec-' . $key, (string) $sec['record']) ?>
              <?php elseif (!empty($sec['records'])): ?>
                <?php foreach ($sec['records'] as $i => $rec): ?><?= $copyRow('rec-' . $key . $i, is_string($rec) ? $rec : json_encode($rec)) ?><?php endforeach; ?>
              <?php endif; ?>
              <?php if ($key === 'mta_sts' && !empty($sec['policy'])): ?><pre class="tool-record"><?= e($sec['policy']) ?></pre><?php endif; ?>
              <?php if (in_array($sec['status'], ['fail', 'warn', 'info'], true) && in_array($key, ['spf', 'dmarc', 'mta_sts', 'tls_rpt'], true)): ?>
                <a class="btn btn-sm mt-1" href="/tools/generator?<?= e(http_build_query(['domain' => $r['domain']])) ?>"><?= icon('file-text') ?> Generate a correct record</a>
              <?php endif; ?>
            </div>
          </details>
        <?php endforeach; ?>
        <div class="callout"><?= icon('sparkles') ?> Sending from this domain with <?= e($appName) ?>? The console shows your exact records and checks them for you. <a href="/register">Create a free account</a></div>
      </div>
    <?php endif; ?>

<?php elseif ($tool === 'dns'): ?>
    <form class="card card-body tool-form" method="get" action="/tools/dns">
      <div class="tool-inline">
        <div class="field mb-0 grow"><label for="t-name">Name or IP address</label><input class="input input-lg" id="t-name" name="name" value="<?= e($input['name'] ?? '') ?>" placeholder="example.com or 8.8.8.8" required autocomplete="off" spellcheck="false" autofocus></div>
        <div class="field mb-0"><label for="t-type">Record type</label><select class="input input-lg" id="t-type" name="type"><?php foreach ($types as $t): ?><option<?= selected($input['type'] ?? 'A', $t) ?>><?= e($t) ?></option><?php endforeach; ?></select></div>
        <button class="btn btn-primary btn-lg" type="submit"><?= icon('search') ?> Look up</button>
      </div>
      <div class="tool-quick mt-2"><span class="small muted">Quick:</span><?php foreach (['MX', 'TXT', 'A', 'NS', 'CAA', 'SOA'] as $q): ?><a class="tag" href="/tools/dns?<?= e(http_build_query(['name' => ($input['name'] ?? '') !== '' ? $input['name'] : 'gmail.com', 'type' => $q])) ?>"><?= e($q) ?></a><?php endforeach; ?></div>
    </form>
    <?php if ($r): ?>
      <div class="card mt-3">
        <div class="card-head"><h2 class="mb-0"><code><?= e($r['name']) ?></code> <span class="tag"><?= e($r['type']) ?></span></h2><span class="small muted"><?= (int) $r['elapsed_ms'] ?> ms · <?= e($r['resolver']) ?></span></div>
        <?php if ($r['answers'] === []): ?><div class="card-body"><p class="muted mb-0">No <?= e($r['type']) ?> records found.</p></div><?php else: ?>
          <div class="card-body stack"><?php foreach ($r['answers'] as $i => $a): ?><?= $copyRow('ans-' . $i, $a) ?><?php endforeach; ?></div>
        <?php endif; ?>
      </div>
    <?php endif; ?>

<?php elseif ($tool === 'generator'): ?>
    <?php $in = $input; $sel = (array) ($in['providers'] ?? (isset($in['domain']) ? [] : ['mailvora'])); ?>
    <form class="card card-body tool-form" method="get" action="/tools/generator">
      <div class="grid grid-2">
        <?= $view->insert('partials/field', ['name' => 'domain', 'label' => 'Your domain', 'required' => true, 'value' => $in['domain'] ?? '', 'placeholder' => 'yourcompany.co.ke']) ?>
        <?= $view->insert('partials/field', ['name' => 'rua', 'label' => 'DMARC report address', 'type' => 'email', 'value' => $in['rua'] ?? '', 'placeholder' => 'dmarc@yourcompany.co.ke']) ?>
      </div>
      <fieldset class="field" style="border:0;padding:0"><legend class="label">Who sends email for this domain?</legend>
        <div class="choice-grid">
          <?php foreach (array_filter(['mailvora' => $spfInclude !== '' ? $appName : null, 'google' => 'Google Workspace', 'microsoft' => 'Microsoft 365', 'zoho' => 'Zoho Mail', 'sendgrid' => 'SendGrid', 'mailchimp' => 'Mailchimp']) as $k => $label): ?>
            <label class="choice"><input type="checkbox" name="providers[]" value="<?= e($k) ?>"<?= checked(in_array($k, $sel, true)) ?>><span><strong><?= e($label) ?></strong></span></label>
          <?php endforeach; ?>
          <label class="choice"><input type="checkbox" name="mx" value="1"<?= checked(!empty($in['mx'])) ?>><span><strong>My MX servers</strong></span></label>
        </div>
      </fieldset>
      <div class="grid grid-3">
        <?= $view->insert('partials/field', ['name' => 'ips', 'label' => 'Extra sending IPs', 'value' => $in['ips'] ?? '', 'placeholder' => '203.0.113.10, 198.51.100.0/24']) ?>
        <?= $view->insert('partials/field', ['name' => 'all', 'label' => 'Unlisted senders', 'type' => 'select', 'noOptional' => true, 'value' => $in['all'] ?? '~all', 'options' => ['~all' => 'Soft fail (~all)', '-all' => 'Reject (-all)']]) ?>
        <?= $view->insert('partials/field', ['name' => 'policy', 'label' => 'DMARC policy', 'type' => 'select', 'noOptional' => true, 'value' => $in['policy'] ?? 'none', 'options' => ['none' => 'Monitor (p=none)', 'quarantine' => 'Spam folder (quarantine)', 'reject' => 'Reject']]) ?>
      </div>
      <details class="cant-find"><summary><?= icon('settings') ?> Advanced: alignment, MTA-STS and TLS reporting</summary>
        <div class="grid grid-3 mt-2">
          <?= $view->insert('partials/field', ['name' => 'pct', 'label' => 'Apply policy to % of mail', 'type' => 'number', 'value' => (string) ($in['pct'] ?? '100'), 'attrs' => ' min="1" max="100"', 'noOptional' => true]) ?>
          <?= $view->insert('partials/field', ['name' => 'adkim', 'label' => 'DKIM alignment', 'type' => 'select', 'noOptional' => true, 'value' => $in['adkim'] ?? 'r', 'options' => ['r' => 'Relaxed', 's' => 'Strict']]) ?>
          <?= $view->insert('partials/field', ['name' => 'aspf', 'label' => 'SPF alignment', 'type' => 'select', 'noOptional' => true, 'value' => $in['aspf'] ?? 'r', 'options' => ['r' => 'Relaxed', 's' => 'Strict']]) ?>
          <?= $view->insert('partials/field', ['name' => 'sts_mode', 'label' => 'MTA-STS mode', 'type' => 'select', 'noOptional' => true, 'value' => $in['sts_mode'] ?? '', 'options' => ['' => 'Not now', 'testing' => 'Testing', 'enforce' => 'Enforce']]) ?>
          <?= $view->insert('partials/field', ['name' => 'mx_hosts', 'label' => 'MX hosts for MTA-STS', 'value' => $in['mx_hosts'] ?? '', 'placeholder' => 'mail.yourcompany.co.ke']) ?>
          <?= $view->insert('partials/field', ['name' => 'tlsrpt', 'label' => 'TLS report address', 'type' => 'email', 'value' => $in['tlsrpt'] ?? '', 'placeholder' => 'tls@yourcompany.co.ke']) ?>
        </div>
      </details>
      <button class="btn btn-primary btn-lg mt-2" type="submit"><?= icon('file-text') ?> Generate records</button>
    </form>
    <?php if ($r): ?>
      <div class="tool-report mt-3">
        <?php if ($r['notes']): ?><div class="card card-body"><?= $findings($r['notes']) ?></div><?php endif; ?>
        <div class="card">
          <div class="card-head"><h2 class="mb-0">Records for <?= e($r['domain']) ?></h2><span class="small muted">Add these at your DNS provider</span></div>
          <div class="table-wrap"><table class="table">
            <thead><tr><th>Type</th><th>Host</th><th>Value</th></tr></thead>
            <tbody><?php foreach ($r['records'] as $i => $rec): ?>
              <tr><td><span class="tag"><?= e($rec['type']) ?></span></td><td><?= $copyRow('gh' . $i, $rec['host']) ?><div class="cell-sub"><?= e($rec['purpose']) ?></div></td><td><?= $copyRow('gv' . $i, $rec['value']) ?></td></tr>
            <?php endforeach; ?></tbody>
          </table></div>
        </div>
        <?php if ($r['policy_file']): ?><div class="card card-body"><h3>MTA-STS policy file</h3><pre class="tool-record" id="sts-policy"><?= e($r['policy_file']) ?></pre><button class="btn btn-sm" type="button" data-copy-target="sts-policy"><?= icon('copy') ?><span class="copy-label">Copy policy</span></button></div><?php endif; ?>
        <a class="btn" href="/tools/domain?<?= e(http_build_query(['domain' => $r['domain']])) ?>"><?= icon('shield-check') ?> Check the published records</a>
      </div>
    <?php endif; ?>

<?php elseif ($tool === 'headers'): ?>
    <form class="card card-body tool-form" method="post" action="/tools/headers">
      <?= csrf_field() ?>
      <label class="label" for="t-headers">Message headers</label>
      <textarea class="input" id="t-headers" name="headers" rows="10" required spellcheck="false" placeholder="Gmail: open the message → ⋮ → Show original → copy everything above the body. Outlook: File → Properties → Internet headers." style="font-family:var(--mono);font-size:.8rem"></textarea>
      <div class="row mt-2"><button class="btn btn-primary" type="submit"><?= icon('activity') ?> Analyze headers</button><span class="small muted">Analyzed in memory and discarded; nothing is stored.</span></div>
    </form>
    <?php if ($r): ?>
      <div class="tool-report mt-3" id="result">
        <div class="grid grid-3">
          <?php foreach (['spf' => 'SPF', 'dkim' => 'DKIM', 'dmarc' => 'DMARC'] as $k => $label): $v = $r['authentication'][$k] ?? null; ?>
            <div class="stat"><div class="stat-label"><?= e($label) ?></div><div class="stat-value" style="font-size:1.2rem"><?= badge($v === 'pass' ? 'verified' : ($v === null ? 'neutral' : 'failed'), $v ? strtoupper($v) : 'Not recorded') ?></div></div>
          <?php endforeach; ?>
        </div>
        <div class="card card-body"><?= $findings($r['findings']) ?>
          <dl class="dl mt-2"><?php foreach ($r['summary'] as $k => $v): if ($v === null) { continue; } ?><dt><?= e(ucfirst(str_replace('_', ' ', $k))) ?></dt><dd class="break small"><?= e($v) ?></dd><?php endforeach; ?></dl>
        </div>
        <?php if ($r['hops'] !== []): ?>
          <div class="card"><div class="card-head"><h2 class="mb-0">Delivery path</h2><span class="small muted">Total <?= (int) $r['total_delay_seconds'] ?> s across <?= count($r['hops']) ?> hops</span></div>
            <ol class="timeline card-body">
              <?php foreach ($r['hops'] as $h): ?><li><span class="dot"></span><strong>Hop <?= (int) $h['hop'] ?></strong> <?= $h['tls'] ? badge('active', 'TLS') : badge('pending', 'No TLS') ?> <span class="small muted"><?= $h['delay_seconds'] !== null ? '+' . (int) $h['delay_seconds'] . ' s' : '' ?></span>
                <div class="small text-2 break">from <code><?= e($h['from'] ?? '?') ?></code> by <code><?= e($h['by'] ?? '?') ?></code><?= $h['with'] ? ' with ' . e($h['with']) : '' ?></div></li><?php endforeach; ?>
            </ol>
          </div>
        <?php endif; ?>
      </div>
    <?php endif; ?>

<?php elseif ($tool === 'blocklist' || $tool === 'ports'): ?>
    <form class="card card-body tool-form" method="get" action="/tools/<?= e($tool) ?>">
      <label class="label" for="t-target"><?= $tool === 'blocklist' ? 'IP address or domain' : 'Mail server hostname or IP address' ?></label>
      <div class="input-group"><input class="input input-lg" id="t-target" name="target" value="<?= e($input['target'] ?? '') ?>" placeholder="<?= $tool === 'blocklist' ? '203.0.113.5 or yourcompany.co.ke' : 'mail.yourcompany.co.ke' ?>" required autocomplete="off" spellcheck="false" autofocus><button class="btn btn-primary btn-lg" type="submit"><?= icon($tool === 'blocklist' ? 'ban' : 'server') ?> <?= $tool === 'blocklist' ? 'Check lists' : 'Check ports' ?></button></div>
      <p class="small muted mb-0 mt-1"><?= $tool === 'blocklist' ? 'For a domain we check every MX server and the domain\'s own address.' : 'Probes come from our server over the public internet; private and reserved addresses are refused.' ?></p>
    </form>
    <?php if ($r && $tool === 'blocklist'): ?>
      <div class="tool-report mt-3">
        <div class="alert alert-<?= $r['status'] === 'pass' ? 'success' : 'error' ?>"><?= icon($r['status'] === 'pass' ? 'check-circle' : 'alert-triangle') ?><div><?= $r['addresses'] === [] ? 'No addresses were found for ' . e($r['target']) . '.' : ($r['listed_count'] > 0 ? e($r['listed_count']) . ' address(es) are listed. Request delisting from each list after fixing the cause.' : 'Not listed on any of the ' . count($r['lists_checked']) . ' blocklists checked.') ?></div></div>
        <?php foreach ($r['addresses'] as $a): ?>
          <div class="card card-body"><div class="row-between"><div><code><?= e($a['ip']) ?></code> <span class="small muted"><?= e($a['host']) ?> · PTR <?= e(implode(', ', $a['ptr']) ?: 'none') ?></span></div><?= $a['listed_on'] ? badge('failed', 'Listed') : badge('verified', 'Clean') ?></div>
            <div class="bl-grid mt-2"><?php foreach ($r['lists_checked'] as $l): $hit = in_array($l['name'], $a['listed_on'], true); ?><span class="bl-item <?= $hit ? 'is-listed' : 'is-clean' ?>"><?= icon($hit ? 'x-circle' : 'check-circle', 'icon-xs') ?> <?= e($l['name']) ?></span><?php endforeach; ?></div>
          </div>
        <?php endforeach; ?>
      </div>
    <?php elseif ($r && $tool === 'ports'): ?>
      <div class="card mt-3">
        <div class="card-head"><h2 class="mb-0"><?= e($r['host']) ?> <span class="small muted"><?= e($r['ip']) ?></span></h2><span class="small muted"><?= e(datetime($r['checked_at'])) ?></span></div>
        <div class="table-wrap"><table class="table">
          <thead><tr><th>Port</th><th>Service</th><th>Status</th><th class="num">Time</th></tr></thead>
          <tbody><?php foreach ($r['ports'] as $p): ?><tr><td><code><?= (int) $p['port'] ?></code></td><td><?= e($p['service']) ?></td><td><?= $p['open'] ? badge('verified', 'Open') : badge('neutral', 'Closed / filtered') ?></td><td class="num small muted"><?= $p['open'] ? (int) $p['ms'] . ' ms' : '—' ?></td></tr><?php endforeach; ?></tbody>
        </table></div>
        <div class="card-body"><p class="small text-2 mb-0">A mail server for <?= e($appName) ?> needs 25 (inbound), 465 and 587 (sending from apps and phones) and 993 (reading mail). Port 25 is often blocked by home and office networks, so test it from the server's own provider if it shows closed.</p></div>
      </div>
    <?php endif; ?>
<?php endif; ?>
  </div>
</section>
