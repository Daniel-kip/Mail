<?php $noindex = true; ?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<div class="auth">
  <aside class="auth-art" aria-hidden="true">
    <?= $view->insert('partials/brand', ['href' => '/']) ?>
    <?= $view->insert('partials/illustration-envelope') ?>
    <h2><?= e($artTitle ?? 'Email that is delivered, traceable and yours.') ?></h2>
    <p><?= e($artText ?? 'Send transactional email from your applications and give your whole institution professional mailboxes on its own domain.') ?></p>
    <ul class="auth-points">
      <li><?= icon('shield-check') ?> DKIM, SPF and DMARC guidance for every domain</li>
      <li><?= icon('key') ?> Scoped, rotatable API keys with IP restrictions</li>
      <li><?= icon('phone') ?> Guided setup for Gmail, Apple Mail and Outlook</li>
    </ul>
  </aside>
  <main id="main" class="auth-panel">
    <div class="auth-mobile-brand"><?= $view->insert('partials/brand', ['href' => '/']) ?></div>
    <div style="width:min(420px,100%);margin:0 auto">
      <?= $content ?>
    </div>
  </main>
</div>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
