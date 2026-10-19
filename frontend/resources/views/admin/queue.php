<?= $view->insert('partials/page-head', ['heading' => 'Failed jobs', 'lede' => 'Jobs that exhausted their retries. Fix the cause, then requeue.']) ?>
<section class="card">
  <?php if ($jobs === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'check-circle', 'heading' => 'No failed jobs', 'text' => 'The dead-letter queue is empty.']) ?></div><?php else: ?>
  <div class="table-wrap"><table class="table table-compact">
    <thead><tr><th>ID</th><th>Kind</th><th>Attempts</th><th>Last error</th><th>Updated</th><th></th></tr></thead>
    <tbody><?php foreach ($jobs as $j): ?>
      <tr><td><?= e($j['id']) ?></td><td><code><?= e($j['kind']) ?></code><?= $j['message_id'] ? '<div class="cell-sub">' . e($j['message_id']) . '</div>' : '' ?></td><td><?= number($j['attempts']) ?></td><td class="small break" style="max-width:420px"><?= e($j['last_error'] ?? '') ?></td><td class="small muted nowrap"><?= time_tag($j['updated_at']) ?></td>
      <td><form method="post" action="/admin/queue/<?= e($j['id']) ?>/requeue"><?= csrf_field() ?><button class="btn btn-sm" type="submit"><?= icon('refresh') ?> Requeue</button></form></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
  <?php endif; ?>
</section>
