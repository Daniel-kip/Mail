<section class="step-card">
  <h1>Which device are you setting up?</h1>
  <p class="text-2">Choose the phone, tablet or computer you want to read work email on.</p>
  <div class="big-choice mt-2">
    <?php foreach ($devices as $key => $d): ?>
      <a href="/mail/setup/app?device=<?= e($key) ?>"<?= $suggested === $key ? ' aria-describedby="suggested-' . e($key) . '"' : '' ?>>
        <span class="icon-lg"><?= icon($d['icon'], 'icon-lg') ?></span>
        <span class="grow"><strong><?= e($d['label']) ?></strong><br><span class="small muted"><?= e($d['hint']) ?></span></span>
        <?php if ($suggested === $key): ?><span class="badge badge-brand" id="suggested-<?= e($key) ?>">This device</span><?php endif; ?>
        <?= icon('chevron-right') ?>
      </a>
    <?php endforeach; ?>
  </div>
  <div class="step-nav"><div class="inner"><a class="btn btn-ghost" href="/mail/setup"><?= icon('arrow-left') ?> Back</a></div></div>
</section>
