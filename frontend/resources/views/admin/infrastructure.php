<?php
/** Operator page for the System Infrastructure PDF. */
$contents = [
    ['1', 'Executive summary', 'Purpose, principles and key properties'],
    ['2', 'System context', 'Users, roles and external services'],
    ['3', 'Deployment architecture', 'Hosts, containers, networks, ports, volumes'],
    ['4', 'Component inventory', 'Every component and external service'],
    ['5', 'System flows', 'Outbound and inbound mail, mailbox access, M-Pesa payments, verification'],
    ['6', 'Security architecture', 'Request security pipeline, trust boundaries, defence in depth'],
    ['7', 'Security controls', 'Controls matrix, secrets, threats and mitigations'],
    ['8', 'Data protection', 'Data inventory, retention, backups, Kenya DPA'],
    ['9', 'Operations', 'Monitoring, incident response, release gates'],
    ['A–B', 'Appendices', 'Port reference, host names, document control'],
];
$previewTitles = ['cover' => 'Cover', 'deployment' => 'Figure 2 · Deployment architecture', 'security' => 'Figure 7 · Request security pipeline'];
$actions = $available ? '<a class="btn btn-primary" href="/admin/infrastructure/download">' . icon('download') . ' Download PDF</a>' : '';
?>
<?= $view->insert('partials/page-head', ['heading' => 'System infrastructure', 'lede' => 'Architecture, system flows and security flows of the platform, as a printable PDF for operators and reviewers.', 'actions' => $actions]) ?>

<?php if (!$available): ?>
  <section class="card card-body">
    <p class="mb-0">The document has not been built on this server. Run <code>python frontend/scripts/build_infrastructure_pdf.py</code> and deploy the files in <code>frontend/resources/infrastructure/</code>.</p>
  </section>
<?php else: ?>
  <div class="grid grid-4 mb-3">
    <div class="stat"><div class="stat-label">Version</div><div class="stat-value"><?= e($doc['version']) ?></div><div class="stat-foot">built <?= e(date_short($doc['built_at'])) ?></div></div>
    <div class="stat"><div class="stat-label">Pages</div><div class="stat-value"><?= (int) $doc['pages'] ?></div><div class="stat-foot">A4 · <?= count($doc['landscape_pages'] ?? []) ?> landscape diagram pages</div></div>
    <div class="stat"><div class="stat-label">Size</div><div class="stat-value"><?= e(number_format($doc['bytes'] / 1048576, 1)) ?> MB</div><div class="stat-foot">vector diagrams, any zoom</div></div>
    <div class="stat"><div class="stat-label">Classification</div><div class="stat-value" style="font-size:1.15rem">Confidential</div><div class="stat-foot">operators and authorised reviewers</div></div>
  </div>

  <?php if (!empty($doc['previews'])): ?>
    <div class="infra-previews mb-3">
      <?php foreach ($doc['previews'] as $file): $name = preg_replace('/^preview-|\.png$/', '', $file); ?>
        <figure class="card infra-preview">
          <a href="/admin/infrastructure/download" title="Download the PDF"><img src="/admin/infrastructure/preview/<?= e($name) ?>" alt="<?= e($previewTitles[$name] ?? $name) ?>" loading="lazy"></a>
          <figcaption><?= e($previewTitles[$name] ?? $name) ?></figcaption>
        </figure>
      <?php endforeach; ?>
    </div>
  <?php endif; ?>

  <div class="grid grid-2 mb-3">
    <section class="card"><div class="card-head"><h2>Contents</h2></div>
      <ol class="infra-contents">
        <?php foreach ($contents as [$n, $t, $d]): ?><li><b><?= e($n) ?></b><span><strong><?= e($t) ?></strong><small><?= e($d) ?></small></span></li><?php endforeach; ?>
      </ol>
    </section>
    <section class="card"><div class="card-head"><h2>Handling and integrity</h2></div><div class="card-body">
      <ul class="control-list">
        <li><?= icon('lock', 'icon-sm') ?><span>Share only with operators, auditors and penetration testers under NDA. It describes internal ports, roles and controls, never secrets.</span></li>
        <li><?= icon('shield-check', 'icon-sm') ?><span>Every download is recorded in the application log with the operator's account.</span></li>
        <li><?= icon('refresh', 'icon-sm') ?><span>Rebuild after any change to topology, ports or security controls: <code>python frontend/scripts/build_infrastructure_pdf.py</code>.</span></li>
      </ul>
      <p class="small text-2 mt-2 mb-1">SHA-256 of the current file, to confirm a copy has not been altered:</p>
      <div class="copy-field"><span class="value" id="infra-sha"><?= e($doc['sha256']) ?></span><button class="btn btn-ghost btn-sm" type="button" data-copy-target="infra-sha" aria-label="Copy checksum"><?= icon('copy') ?></button></div>
      <p class="mt-3 mb-0"><a class="btn btn-primary" href="/admin/infrastructure/download"><?= icon('download') ?> Download PDF (<?= (int) $doc['pages'] ?> pages)</a> <a class="btn" href="/admin/docs"><?= icon('book') ?> Operator wiki</a></p>
    </div></section>
  </div>
<?php endif; ?>
