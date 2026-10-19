<?php $noindex = true; ?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<div class="setup">
  <header class="setup-top">
    <?= $view->insert('partials/brand', ['href' => '/mail', 'subtitle' => 'WORK EMAIL SETUP']) ?>
    <span class="grow"></span>
    <a class="btn btn-ghost btn-sm" href="/mail/setup/help"><?= icon('help') ?> <span>Help</span></a>
    <a class="btn btn-ghost btn-sm" href="/mail" aria-label="Close setup and open webmail"><?= icon('x') ?></a>
  </header>
  <main id="main" class="setup-main">
    <?php if (!empty($stage)): ?>
      <?php
      $stages = ['welcome' => 'Welcome', 'device' => 'Device', 'app' => 'App', 'check' => 'Check', 'guide' => 'Set up', 'verify' => 'Test'];
      $keys = array_keys($stages);
      $pos = array_search($stage, $keys, true);
      ?>
      <ol class="stages" aria-label="Setup progress">
        <?php foreach ($stages as $key => $label): ?>
          <?php $i = array_search($key, $keys, true); ?>
          <li class="<?= $i < $pos ? 'is-done' : ($i === $pos ? 'is-current' : '') ?>"<?= $i === $pos ? ' aria-current="step"' : '' ?>>
            <?= $i < $pos ? icon('check', 'icon-xs') : '' ?><?= e($label) ?>
          </li>
        <?php endforeach; ?>
      </ol>
    <?php endif; ?>
    <?= $content ?>
  </main>
</div>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
