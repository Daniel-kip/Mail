<?= $view->insert('webmail/partials/list', ['list' => $list, 'currentFolder' => $currentFolder, 'q' => $q]) ?>
<section class="mail-reader">
  <div class="reader-empty">
    <div>
      <?= icon('mail', 'icon-xl') ?>
      <p>Select a message to read it.</p>
      <p class="small">Shortcuts: <kbd>c</kbd> compose · <kbd>/</kbd> search · <kbd>j</kbd>/<kbd>k</kbd> next/previous</p>
    </div>
  </div>
</section>
