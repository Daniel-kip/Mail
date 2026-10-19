<section class="step-card">
  <h1>Which email app do you want to use?</h1>
  <p class="text-2">On your <?= e(strtolower($deviceInfo['label'])) ?>. If you are not sure, pick the app you already use for personal email.</p>
  <div class="big-choice mt-2">
    <?php foreach ($apps as $key => $a): ?>
      <a href="/mail/setup/check?<?= e(http_build_query(['device' => $device, 'app' => $key])) ?>">
        <span><?= icon($a['icon'], 'icon-lg') ?></span>
        <span class="grow"><strong><?= e($a['label']) ?></strong><br><span class="small muted"><?= e($a['hint']) ?></span></span>
        <?= icon('chevron-right') ?>
      </a>
    <?php endforeach; ?>
  </div>
  <div class="step-nav"><div class="inner"><a class="btn btn-ghost" href="/mail/setup/device"><?= icon('arrow-left') ?> Back</a></div></div>
</section>
