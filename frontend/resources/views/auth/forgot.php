<h1>Reset your password</h1>
<?php if ($sent): ?>
  <div class="alert alert-success mt-3"><?= icon('check-circle') ?><div>If an account exists for that address, a reset link is on its way. It expires in one hour. Check your spam folder if it does not arrive within a few minutes.</div></div>
  <a class="btn btn-block mt-3" href="/login">Back to sign in</a>
<?php else: ?>
  <p class="text-2">Enter your account email and we will send you a single-use reset link.</p>
  <form method="post" action="/forgot-password" class="mt-3">
    <?= csrf_field() ?>
    <?= $view->insert('partials/field', ['name' => 'email', 'label' => 'Email', 'type' => 'email', 'required' => true, 'autocomplete' => 'email', 'attrs' => ' autofocus']) ?>
    <button class="btn btn-primary btn-block btn-lg" type="submit">Send reset link</button>
  </form>
  <p class="small muted mt-3" style="text-align:center"><a href="/login">Back to sign in</a> · Work mailbox user? <a href="/mail/forgot-password">Reset it here</a></p>
<?php endif; ?>
