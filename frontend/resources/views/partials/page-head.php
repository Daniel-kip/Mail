<div class="page-head">
  <div>
    <h1><?= e($heading) ?></h1>
    <?php if (!empty($lede)): ?><p><?= e($lede) ?></p><?php endif; ?>
  </div>
  <?php if (!empty($actions)): ?><div class="page-actions"><?= $actions ?></div><?php endif; ?>
</div>
