<?= $view->insert('partials/page-head', ['heading' => 'Templates', 'lede' => 'Reusable, versioned messages. Send them by ID or slug with variables such as {{name}}.', 'actions' => can('templates:write') ? '<a class="btn btn-primary" href="/console/templates/new">' . icon('plus') . ' New template</a>' : '']) ?>
<section class="card">
  <?php if ($templates === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'file-text', 'heading' => 'No templates yet', 'text' => 'Design invoices, receipts and notifications once and send them from any application.', 'action' => can('templates:write') ? '<a class="btn btn-primary btn-sm" href="/console/templates/new">Create a template</a>' : '']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Name</th><th>Slug</th><th>Status</th><th>Active version</th><th>Updated</th></tr></thead>
    <tbody><?php foreach ($templates as $t): ?>
      <tr><td><a class="cell-title" href="/console/templates/<?= e($t['id']) ?>"><?= e($t['name']) ?></a><?= !empty($t['description']) ? '<div class="cell-sub">' . e($t['description']) . '</div>' : '' ?></td><td><code><?= e($t['slug']) ?></code></td><td><?= badge($t['status']) ?></td><td>v<?= e($t['active_version'] ?? '—') ?> <span class="muted small">of <?= e($t['latest_version'] ?? 1) ?></span></td><td class="small muted nowrap"><?= time_tag($t['updated_at']) ?></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
