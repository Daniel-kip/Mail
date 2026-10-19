<?php $noindex = true; ?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<main id="main" class="content" style="max-width:900px">
  <div class="row-between no-print mb-2">
    <a class="btn btn-ghost" href="<?= e($backUrl ?? '/console') ?>"><?= icon('arrow-left') ?> Back</a>
    <span class="muted small">Use your browser's Print command to save as PDF.</span>
  </div>
  <?= $content ?>
</main>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
