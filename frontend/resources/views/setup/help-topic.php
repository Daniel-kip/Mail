<?php $fill = static fn (string $t): string => \App\Mail\GuideCatalog::fill($t, $placeholders); ?>
<section class="step-card">
  <p class="eyebrow"><a href="/mail/setup/help">Help</a></p>
  <h1><?= e($topic['title']) ?></h1>
  <p class="text-2"><strong>Likely cause:</strong> <?= e($fill($topic['cause'])) ?></p>
  <h2 class="section-title mt-2">What to try</h2>
  <ol class="stack">
    <?php foreach ($topic['steps'] as $step): ?><li><?= e($fill($step)) ?></li><?php endforeach; ?>
  </ol>
  <div class="callout mt-2"><?= icon('help') ?> <?= e($fill($topic['support'])) ?></div>
  <div class="step-nav"><div class="inner"><a class="btn btn-ghost" href="/mail/setup/help"><?= icon('arrow-left') ?> All problems</a><a class="btn" href="/mail/setup/settings">Check the server settings</a><a class="btn btn-primary" href="/mail/setup/verify">Test my connection</a></div></div>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
</section>
