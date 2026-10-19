<h1>Reset your mailbox password</h1>
<?php if ($sent): ?>
  <div class="alert alert-success mt-3"><?= icon('check-circle') ?><div>If the mailbox has a personal recovery email on file, a reset link is on its way there. It works once and expires in one hour.</div></div>
  <p class="small text-2 mt-2">No email? Your ICT support team can send you a reset link without knowing your password.</p>
  <a class="btn btn-block mt-3" href="/mail/login">Back to sign in</a>
<?php else: ?>
  <p class="text-2">Enter your work email address. We send the reset link to the personal email your organization has on file.</p>
  <form method="post" action="/mail/forgot-password" class="mt-3">
    <?= csrf_field() ?>
    <?= $view->insert('partials/field', ['name' => 'address', 'label' => 'Work email address', 'type' => 'email', 'required' => true, 'autocomplete' => 'username', 'attrs' => ' autofocus']) ?>
    <button class="btn btn-primary btn-block btn-lg" type="submit">Send reset link</button>
  </form>
  <p class="small muted mt-3" style="text-align:center"><a href="/mail/login">Back to sign in</a></p>
<?php endif; ?>
