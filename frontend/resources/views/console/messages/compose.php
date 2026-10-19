<?= $view->insert('partials/page-head', ['heading' => 'Send a message', 'lede' => 'Send a one-off message or test a template. For applications, use the API.']) ?>
<?php if ($domains === []): ?>
  <section class="card"><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'globe', 'heading' => 'Verify a domain first', 'text' => 'Mail can only be sent from a verified domain.', 'action' => '<a class="btn btn-primary btn-sm" href="/console/domains">Go to domains</a>']) ?></div></section>
<?php else: ?>
<form method="post" action="/console/messages" class="card" style="max-width:860px">
  <?= csrf_field() ?>
  <div class="card-body">
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'from_name', 'label' => 'From name', 'placeholder' => auth()['organization']['name'] ?? '']) ?>
      <?= $view->insert('partials/field', ['name' => 'from', 'label' => 'From address', 'type' => 'email', 'required' => true, 'default' => 'no-reply@' . $domains[0]['domain'], 'hint' => 'Must be on a verified domain: ' . implode(', ', array_column($domains, 'domain'))]) ?>
    </div>
    <?= $view->insert('partials/field', ['name' => 'to', 'label' => 'To', 'required' => true, 'placeholder' => 'jane@example.com, peter@example.com', 'hint' => 'Separate several addresses with commas (maximum 50).']) ?>
    <?php if ($templates !== []): ?>
      <?= $view->insert('partials/field', ['name' => 'template', 'label' => 'Template', 'type' => 'select', 'options' => ['' => 'No template: write the message below'] + array_column(array_map(static fn ($t) => [$t['slug'], $t['name'] . ' (v' . ($t['active_version'] ?? 1) . ')'], $templates), 1, 0)]) ?>
      <?= $view->insert('partials/field', ['name' => 'variables', 'label' => 'Template variables (JSON)', 'type' => 'textarea', 'rows' => 3, 'placeholder' => '{"name": "Jane", "invoice_number": "INV-1042"}', 'attrs' => ' spellcheck="false" style="font-family:var(--mono)"']) ?>
    <?php endif; ?>
    <?= $view->insert('partials/field', ['name' => 'subject', 'label' => 'Subject', 'noOptional' => true]) ?>
    <?= $view->insert('partials/field', ['name' => 'text', 'label' => 'Message', 'type' => 'textarea', 'rows' => 8, 'noOptional' => true]) ?>
    <details class="cant-find"><summary>More options</summary>
      <div class="mt-2">
        <?= $view->insert('partials/field', ['name' => 'html', 'label' => 'HTML version', 'type' => 'textarea', 'rows' => 6, 'attrs' => ' spellcheck="false" style="font-family:var(--mono);font-size:.82rem"']) ?>
        <div class="grid grid-3">
          <?= $view->insert('partials/field', ['name' => 'reply_to', 'label' => 'Reply-To', 'type' => 'email']) ?>
          <?= $view->insert('partials/field', ['name' => 'tags', 'label' => 'Tags', 'placeholder' => 'invoice, march']) ?>
          <?= $view->insert('partials/field', ['name' => 'send_at', 'label' => 'Send later', 'type' => 'datetime-local', 'hint' => 'Your local time; up to 30 days ahead.']) ?>
        </div>
        <label class="check"><input type="checkbox" name="unsubscribe" value="1"<?= checked((bool) old('unsubscribe')) ?>> Add one-click unsubscribe headers (for newsletters and bulk updates)</label>
      </div>
    </details>
  </div>
  <div class="card-foot form-actions"><button class="btn btn-primary" type="submit"><?= icon('send') ?> Send</button><a class="btn btn-ghost" href="/console/messages">Cancel</a></div>
</form>
<?php endif; ?>
