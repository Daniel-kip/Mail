<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<?php $signedIn = app()->api()->hasSession('console'); ?>
<header class="site-header">
  <nav class="site-nav" aria-label="Main">
    <?= $view->insert('partials/brand', ['href' => '/']) ?>
    <div class="site-links">
      <a href="/#product">Product</a>
      <a href="/pricing">Pricing</a>
      <a href="/tools">Tools</a>
      <a href="/docs">Docs</a>
      <a href="/developers">Developers</a>
      <a href="/security">Security</a>
    </div>
    <div class="actions">
      <?= $view->insert('partials/theme-switch') ?>
      <a class="btn btn-ghost" href="/mail/login"><?= icon('inbox') ?> <span>Webmail</span></a>
      <?php if ($signedIn): ?>
        <a class="btn btn-primary" href="/console">Open console</a>
      <?php else: ?>
        <a class="btn" href="/login">Sign in</a>
        <a class="btn btn-primary" href="/register">Get started</a>
      <?php endif; ?>
    </div>
  </nav>
</header>
<main id="main"><?= $content ?></main>
<footer class="site-footer">
  <div class="container">
    <div class="cols">
      <div>
        <?= $view->insert('partials/brand', ['href' => '/']) ?>
        <p class="mt-2" style="max-width:340px"><?= e($config->string('tagline')) ?>. Transactional email for applications and hosted work email for institutions, on infrastructure you control.</p>
      </div>
      <div>
        <h4>Product</h4>
        <a href="/#developers">For developers</a>
        <a href="/#institutions">For institutions</a>
        <a href="/pricing">Pricing</a>
        <a href="/tools">Domain &amp; DNS tools</a>
      </div>
      <div>
        <h4>Resources</h4>
        <a href="/docs">Documentation</a>
        <a href="/developers">API reference</a>
        <a href="/security">Security</a>
        <a href="/mail/login">Webmail</a>
      </div>
      <div>
        <h4>Legal</h4>
        <a href="/legal/privacy">Privacy</a>
        <a href="/legal/terms">Terms</a>
        <?php if ($config->get('support_email')): ?><a href="mailto:<?= e($config->get('support_email')) ?>">Contact support</a><?php endif; ?>
      </div>
    </div>
    <p class="mt-3 mb-0 footer-legal"><span>© <?= e(date('Y')) ?> <?= e($config->string('company.name')) ?>. All rights reserved.</span><span class="powered-by">Powered by <strong>TECHCORE SOLUTIONS</strong></span></p>
  </div>
</footer>
<?= $view->insert('partials/help-widget', get_defined_vars()) ?>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
