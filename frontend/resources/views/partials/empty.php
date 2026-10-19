<div class="empty">
  <div class="empty-icon"><?= icon($icon ?? 'inbox') ?></div>
  <h3><?= e($heading) ?></h3>
  <?php if (!empty($text)): ?><p><?= e($text) ?></p><?php endif; ?>
  <?php if (!empty($action)): ?><?= $action ?><?php endif; ?>
</div>
