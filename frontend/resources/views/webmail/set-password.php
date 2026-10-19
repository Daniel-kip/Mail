<?php
$activate = $purpose === 'activate';
$valid = $info !== null && ($info['purpose'] ?? $purpose) === $purpose;
?>
<h1><?= $activate ? 'Activate your work email' : 'Choose a new password' ?></h1>
<?php if (!$valid): ?>
  <div class="alert alert-error mt-3"><?= icon('alert-circle') ?><div>This link is invalid, already used or expired. Ask your ICT support team for a new <?= $activate ? 'invitation' : 'reset link' ?>.</div></div>
  <a class="btn btn-block mt-3" href="/mail/login">Go to sign in</a>
<?php else: ?>
  <div class="callout mt-2">
    <strong><?= e($info['display_name'] ?? '') ?></strong><br>
    <span style="font-family:var(--mono)"><?= e($info['address']) ?></span><br>
    <span class="small muted"><?= e($info['organization'] ?? '') ?> · link expires <?= e(datetime($info['expires_at'] ?? null)) ?></span>
  </div>
  <p class="text-2 mt-2"><?= $activate ? 'Choose a password only you know. You will use it in webmail and in the mail app on your phone.' : 'Choose a new password. Afterwards, update it in the mail app on your phone.' ?></p>
  <form method="post" action="<?= $activate ? '/mail/activate' : '/mail/reset-password' ?>">
    <?= csrf_field() ?>
    <input type="hidden" name="token" value="<?= e($token) ?>">
    <input type="text" name="username" value="<?= e($info['address']) ?>" autocomplete="username" hidden>
    <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'New password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password', 'hint' => 'At least 10 characters. A short sentence is easy to remember and hard to guess.', 'attrs' => ' minlength="10" autofocus']) ?>
    <?= $view->insert('partials/field', ['name' => 'password_confirmation', 'label' => 'Type it again', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password']) ?>
    <button class="btn btn-primary btn-block btn-lg" type="submit"><?= $activate ? 'Activate and continue' : 'Change password' ?></button>
  </form>
<?php endif; ?>
