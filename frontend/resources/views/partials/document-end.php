<?php if (!empty($flashes)): ?>
<div class="flash-stack" role="region" aria-label="Notifications">
<?php foreach ($flashes as $type => $message): ?>
  <?php $icon = ['success' => 'check-circle', 'error' => 'alert-circle', 'warning' => 'alert-triangle', 'info' => 'info'][$type] ?? 'info'; ?>
  <div class="alert alert-<?= e($type) ?>" role="<?= $type === 'error' ? 'alert' : 'status' ?>">
    <?= icon($icon) ?>
    <div class="grow"><?= e($message) ?></div>
    <button type="button" class="btn btn-ghost btn-sm btn-icon dismiss" aria-label="Dismiss"><?= icon('x', 'icon-sm') ?></button>
  </div>
<?php endforeach; ?>
</div>
<?php endif; ?>
<dialog class="modal" id="confirm-dialog" aria-labelledby="confirm-title" aria-describedby="confirm-text">
  <form method="dialog">
    <div class="modal-body">
      <h2 id="confirm-title">Are you sure?</h2>
      <p id="confirm-text" class="text-2 mb-0"></p>
    </div>
    <div class="modal-actions">
      <button class="btn" value="cancel" autofocus>Cancel</button>
      <button class="btn btn-danger" value="ok" id="confirm-ok">Confirm</button>
    </div>
  </form>
</dialog>
</body>
</html>
