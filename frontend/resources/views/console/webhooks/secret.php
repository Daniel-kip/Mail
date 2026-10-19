<?php if ($secret && !empty($secret['secret'])): ?>
  <section class="secret-box mb-3" aria-live="polite">
    <strong><?= icon('key') ?> Signing secret, shown once</strong>
    <p class="small">Store it with your endpoint. Verify each request's <code>Mailvora-Signature</code> header (HMAC-SHA256 of <code>timestamp.body</code>) and reject old timestamps.</p>
    <div class="copy-field"><span class="value" id="wh-secret"><?= e($secret['secret']) ?></span><button class="btn btn-sm btn-primary" type="button" data-copy-target="wh-secret"><?= icon('copy') ?><span class="copy-label">Copy</span></button></div>
  </section>
<?php endif; ?>
