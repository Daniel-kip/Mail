<?php $noindex = true; ?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<main id="main" class="minimal">
  <div class="minimal-card">
    <div class="mb-3"><?= $view->insert('partials/brand', ['href' => '/']) ?></div>
    <?= $content ?>
  </div>
</main>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
