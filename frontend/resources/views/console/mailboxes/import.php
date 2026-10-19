<?= $view->insert('partials/page-head', ['heading' => 'Import mailboxes', 'lede' => 'Create many mailboxes from a spreadsheet. Each person receives an invitation to choose their own password.']) ?>
<?php if (is_array($result)): ?>
  <section class="card mb-3">
    <div class="card-head"><h2>Import result</h2></div>
    <div class="table-wrap"><table class="table table-compact">
      <thead><tr><th>Line</th><th>Address</th><th>Result</th></tr></thead>
      <tbody><?php foreach ($result as $r): ?><tr><td><?= (int) $r['line'] ?></td><td><?= $r['id'] ? '<a href="/console/mailboxes/' . e($r['id']) . '">' . e($r['address']) . '</a>' : e($r['address']) ?></td><td><?= $r['ok'] ? badge('active', 'Created') : badge('failed', 'Failed') ?> <span class="small text-2"><?= e($r['message']) ?></span></td></tr><?php endforeach; ?></tbody>
    </table></div>
  </section>
<?php endif; ?>
<?php if ($domains === []): ?>
  <section class="card"><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'globe', 'heading' => 'No domain is ready for mailboxes', 'text' => 'Verify a domain and turn on mail hosting first.']) ?></div></section>
<?php else: ?>
<div class="grid grid-2">
  <form method="post" action="/console/mailboxes/import" enctype="multipart/form-data" class="card card-body">
    <?= csrf_field() ?>
    <?= $view->insert('partials/field', ['name' => 'domain_id', 'label' => 'Domain', 'type' => 'select', 'required' => true, 'options' => array_column(array_map(static fn ($d) => [$d['id'], '@' . $d['domain']], $domains), 1, 0)]) ?>
    <div class="field"><label for="csv">CSV file</label><input class="input" type="file" id="csv" name="csv" accept=".csv,text/csv" required<?= error_attrs('csv') ?>><?= error_text('csv') ?><p class="hint">UTF-8, comma or semicolon separated, up to 500 rows and 1 MB.</p></div>
    <button class="btn btn-primary" type="submit"><?= icon('upload') ?> Import</button>
  </form>
  <section class="card card-body">
    <h2 class="section-title">File format</h2>
    <p class="small text-2">The first line is a header. Columns can be in any order:</p>
    <pre class="small">local_part,display_name,recovery_email,department,job_title
jane.wanjiru,Jane Wanjiru,jane@gmail.com,Finance,Accountant
peter.otieno,Peter Otieno,,ICT,Engineer</pre>
    <ul class="small text-2 mb-0">
      <li>With a <code>recovery_email</code>, an invitation is emailed automatically.</li>
      <li>Without one, open the mailbox afterwards to create a handover link.</li>
      <li>Never put passwords in spreadsheets.</li>
    </ul>
  </section>
</div>
<?php endif; ?>
