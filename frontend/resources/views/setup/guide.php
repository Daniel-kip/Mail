<?php
use App\Mail\GuideCatalog;
use App\View\PhoneMockup;

$fill = static fn (string $t): string => GuideCatalog::fill($t, $placeholders);
$base = '/mail/setup/guide/' . rawurlencode($guide['id']);
$stepFields = array_values(array_filter(array_map(static fn ($k) => $fields[$k] ?? null, $s['fields'] ?? [])));
$isLast = $step === $total;
$helpTopic = $s['help'] ?? null;
?>
<section class="step-layout" data-setup-step="<?= (int) $step ?>" data-event-url="/mail/setup/event" data-device="<?= e($guide['device'] ?? '') ?>" data-client="<?= e($guide['app'] ?? '') ?>">
  <div class="step-card">
    <p class="eyebrow"><?= e($guide['title']) ?> · <?= e($guide['time'] ?? '') ?></p>
    <div class="progress" aria-label="Step <?= (int) $step ?> of <?= (int) $total ?>">
      <div class="progress-label small muted">Step <?= (int) $step ?> of <?= (int) $total ?></div>
      <div class="progress-track"><span style="width:<?= (int) round($step / $total * 100) ?>%"></span></div>
    </div>
    <h1 class="mt-2"><span class="step-number"><?= (int) $step ?></span> <?= e($fill($s['title'])) ?></h1>
    <p class="step-body"><?= e($fill($s['body'])) ?></p>

    <?php if ($stepFields !== []): ?>
      <div class="settings-list">
        <?php foreach ($stepFields as $i => [$label, $value, $hint]): $id = 'fld-' . $step . '-' . $i; ?>
          <div class="setting">
            <span class="s-label"><?= e($label) ?></span>
            <span class="s-value" id="<?= $id ?>"><?= e($value) ?></span>
            <span class="s-help"><?= e($hint) ?></span>
            <button class="btn btn-sm copy-btn" type="button" data-copy-target="<?= $id ?>"><?= icon('copy') ?><span class="copy-label">Copy</span></button>
          </div>
        <?php endforeach; ?>
      </div>
    <?php endif; ?>

    <?php if (!empty($guide['profile']) && $step === 1): ?>
      <div class="callout mt-2">
        <strong><?= icon('sparkles') ?> Quicker option for iPhone and iPad</strong>
        <p class="small mb-1">Download a configuration profile that fills in all the server settings for you. It does not contain your password; your iPhone asks for it during installation.</p>
        <a class="btn btn-sm" href="/mail/setup/profile.mobileconfig"><?= icon('download') ?> Download profile</a>
        <p class="small muted mb-0 mt-1">Open this page in Safari on the iPhone itself. After downloading, go to Settings → Profile Downloaded → Install. You can remove the profile at any time in Settings → General → VPN &amp; Device Management.</p>
      </div>
    <?php endif; ?>

    <?php if (!empty($s['why'])): ?><p class="why small"><?= icon('info', 'icon-sm') ?> <strong>Why:</strong> <?= e($fill($s['why'])) ?></p><?php endif; ?>
    <?php if (!empty($s['cant_find'])): ?>
      <details class="cant-find" data-help-topic="cant-find-step-<?= (int) $step ?>" data-event-url="/mail/setup/event" data-client="<?= e($guide['app'] ?? '') ?>">
        <summary><?= icon('help') ?> I can't find this</summary>
        <div><?= e($fill($s['cant_find'])) ?></div>
      </details>
    <?php endif; ?>
    <?php if ($helpTopic && isset($helpTopics[$helpTopic])): ?>
      <p class="small mt-2"><a href="/mail/setup/help/<?= e($helpTopic) ?>"><?= icon('help', 'icon-sm') ?> <?= e($helpTopics[$helpTopic]['title']) ?></a></p>
    <?php endif; ?>

    <div class="step-nav"><div class="inner">
      <?php if ($step > 1): ?><a class="btn btn-ghost" href="<?= e($base . '?step=' . ($step - 1)) ?>"><?= icon('arrow-left') ?> Back</a><?php else: ?><a class="btn btn-ghost" href="/mail/setup/app?device=<?= e(rawurlencode($guide['device'] ?? 'android')) ?>"><?= icon('arrow-left') ?> Change app</a><?php endif; ?>
      <?php if ($isLast): ?>
        <a class="btn btn-primary btn-lg" href="/mail/setup/done/<?= e(rawurlencode($guide['id'])) ?>">Done. Test my connection <?= icon('arrow-right') ?></a>
      <?php else: ?>
        <a class="btn btn-primary btn-lg" href="<?= e($base . '?step=' . ($step + 1)) ?>">Next <?= icon('arrow-right') ?></a>
      <?php endif; ?>
    </div></div>
    <p class="small muted mt-2 mb-0">Stuck? <a href="/mail/setup/help">See common problems</a> · <a href="/mail/setup/settings">All settings on one page</a></p>
  </div>
  <?php if (!empty($s['mock'])): ?>
    <figure class="step-visual" aria-hidden="true">
      <?= PhoneMockup::render($s['mock'], $placeholders, 'Illustration: menus may look slightly different on your phone.') ?>
    </figure>
  <?php endif; ?>
</section>
<?= $view->insert('setup/partials/support', ['setup' => $setup]) ?>
<?php if (!empty($guide['sources'])): ?>
  <p class="small muted mt-2">Steps checked against: <?php foreach ($guide['sources'] as $i => $src): ?><?= $i ? ' · ' : '' ?><a href="<?= e($src['url']) ?>" rel="noopener" target="_blank"><?= e($src['label']) ?></a><?php endforeach; ?></p>
<?php endif; ?>
