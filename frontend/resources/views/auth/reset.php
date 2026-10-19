<h1><?= e($heading ?? 'Choose a new password') ?></h1>
<?php if (!$valid): ?>
  <div class="alert alert-error mt-3"><?= icon('alert-circle') ?><div>This link is incomplete or has expired. Open the full link from the email, or request a new one.</div></div>
  <a class="btn btn-block mt-3" href="<?= e($retryUrl ?? '/forgot-password') ?>">Request a new link</a>
<?php else: ?>
  <?php if (!empty($intro)): ?><p class="text-2"><?= e($intro) ?></p><?php endif; ?>
  <form method="post" action="<?= e($action ?? '/reset-password') ?>" class="mt-3">
    <?= csrf_field() ?>
    <input type="hidden" name="token" value="<?= e($token) ?>">
    <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'New password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password', 'hint' => 'At least ' . ($min ?? 12) . ' characters. Do not reuse a password from another site.', 'attrs' => ' minlength="' . ($min ?? 12) . '" autofocus']) ?>
    <?= $view->insert('partials/field', ['name' => 'password_confirmation', 'label' => 'Confirm new password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password']) ?>
    <button class="btn btn-primary btn-block btn-lg" type="submit"><?= e($submit ?? 'Change password') ?></button>
  </form>
<?php endif; ?>
