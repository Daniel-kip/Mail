<section class="step-card">
  <h1>Server settings</h1>
  <p class="text-2">For any email app that supports IMAP. Copy each value exactly. Always use encrypted connections; if an app shows a certificate warning, stop and contact IT instead of accepting it.</p>
  <div class="settings-list mt-2">
    <?php foreach ($fields as $key => [$label, $value, $hint]): ?>
      <div class="setting">
        <span class="s-label"><?= e($label) ?></span>
        <span class="s-value" id="set-<?= e($key) ?>"><?= e($value) ?></span>
        <span class="s-help"><?= e($hint) ?></span>
        <?php if ($key !== 'auth'): ?><button class="btn btn-sm copy-btn" type="button" data-copy-target="set-<?= e($key) ?>"><?= icon('copy') ?><span class="copy-label">Copy</span></button><?php endif; ?>
      </div>
    <?php endforeach; ?>
    <div class="setting"><span class="s-label">Password</span><span class="s-value">Your own mailbox password</span><span class="s-help">We never show it here</span></div>
  </div>
  <?php if (!empty($setup['automation']['thunderbird_autoconfig']) || !empty($setup['automation']['apple_profile'])): ?>
    <h2 class="section-title mt-3">Automatic setup</h2>
    <ul class="small text-2">
      <?php if (!empty($setup['automation']['thunderbird_autoconfig'])): ?><li>Thunderbird and some other apps fill these settings in automatically after you type your email address.</li><?php endif; ?>
      <?php if (!empty($setup['automation']['apple_profile'])): ?><li>iPhone, iPad and Mac: <a href="/mail/setup/profile.mobileconfig">download a configuration profile</a> (no password inside).</li><?php endif; ?>
    </ul>
  <?php endif; ?>
  <div class="step-nav"><div class="inner"><a class="btn btn-ghost" href="/mail/setup"><?= icon('arrow-left') ?> Back</a><a class="btn btn-primary" href="/mail/setup/verify">Test my connection</a></div></div>
  <?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
</section>
