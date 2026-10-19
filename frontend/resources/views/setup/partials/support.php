<?php $sup = $setup['support'] ?? []; ?>
<?php if (array_filter([$sup['email'] ?? null, $sup['phone'] ?? null, $sup['url'] ?? null, $sup['setup_help_text'] ?? null])): ?>
  <aside class="support-card card card-body mt-3" aria-label="Get help from your IT team">
    <span class="stat-icon"><?= icon('help') ?></span>
    <div>
    <strong>Need a person?</strong>
    <?php if (!empty($sup['setup_help_text'])): ?><p class="small mb-1"><?= e($sup['setup_help_text']) ?></p><?php endif; ?>
    <p class="small mb-0">
      <?php if (!empty($sup['email'])): ?><a href="mailto:<?= e($sup['email']) ?>"><?= e($sup['email']) ?></a><?php endif; ?>
      <?php if (!empty($sup['phone'])): ?> · <a href="tel:<?= e(preg_replace('/[^0-9+]/', '', (string) $sup['phone'])) ?>"><?= e($sup['phone']) ?></a><?php endif; ?>
      <?php if (!empty($sup['hours'])): ?> · <?= e($sup['hours']) ?><?php endif; ?>
      <?php if (!empty($sup['url'])): ?> · <a href="<?= e($sup['url']) ?>" rel="noopener" target="_blank">Help page</a><?php endif; ?>
    </p>
    <p class="small muted mb-0">IT staff will never ask for your password.</p>
    </div>
  </aside>
<?php endif; ?>
