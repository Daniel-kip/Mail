<h1>Welcome back</h1>
<p class="text-2">Sign in to manage domains, API keys, mailboxes and billing.</p>
<?php if ($google): ?>
  <div class="mt-3"><?= $view->insert('partials/google-button', ['query' => array_filter(['next' => $next])]) ?></div>
  <div class="divider-text mt-3"><span>or with your details</span></div>
<?php endif; ?>
<form method="post" action="/login" class="mt-3">
  <?= csrf_field() ?>
  <input type="hidden" name="next" value="<?= e($next) ?>">
  <?= $view->insert('partials/field', ['name' => 'email', 'label' => 'Email, username or phone', 'required' => true, 'autocomplete' => 'username', 'placeholder' => 'you@company.co.ke, @username or 0712 345 678', 'attrs' => ' autofocus autocapitalize="none" spellcheck="false"']) ?>
  <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'Password', 'type' => 'password', 'required' => true, 'autocomplete' => 'current-password']) ?>
  <p class="small" style="text-align:right;margin-top:-.5rem"><a href="/forgot-password">Forgot your password?</a></p>
  <button class="btn btn-primary btn-block btn-lg" type="submit">Sign in</button>
</form>
<?php if ($signupEnabled): ?><p class="small muted mt-3" style="text-align:center">New to <?= e($appName) ?>? <a href="/register">Create an account</a></p><?php endif; ?>
<div class="divider-text mt-3"><span>Have a work mailbox?</span></div>
<a class="btn btn-block mt-2" href="/mail/login"><?= icon('mail') ?> Sign in to webmail</a>
