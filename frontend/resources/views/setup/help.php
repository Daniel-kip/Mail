<section class="step-card">
  <h1>Help with email setup</h1>
  <p class="text-2">Choose what you are seeing. Each answer explains the likely cause and what to try.</p>
  <div class="help-grid mt-2">
    <?php foreach ($topics as $slug => $t): ?>
      <a class="card card-body" href="/mail/setup/help/<?= e($slug) ?>"><strong><?= e($t['title']) ?></strong><span class="small muted"><?= e($t['cause']) ?></span></a>
    <?php endforeach; ?>
  </div>
  <div class="step-nav"><div class="inner"><a class="btn btn-ghost" href="/mail/setup"><?= icon('arrow-left') ?> Back to setup</a><a class="btn" href="/mail/setup/settings">Server settings</a></div></div>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
  <p class="small muted mt-2 mb-0">Guides last reviewed <?= e(date_short($reviewed)) ?>.</p>
</section>
