<h1>Sign in to your work email</h1>
<p class="text-2">Use your full work email address.</p>
<form method="post" action="/mail/login" class="mt-3">
  <?= csrf_field() ?>
  <input type="hidden" name="next" value="<?= e($next) ?>">
  <?= $view->insert('partials/field', ['name' => 'address', 'label' => 'Work email address', 'type' => 'email', 'required' => true, 'autocomplete' => 'username', 'placeholder' => 'name@yourinstitution.ac.ke', 'attrs' => ' autofocus autocapitalize="none" spellcheck="false"']) ?>
  <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'Password', 'type' => 'password', 'required' => true, 'autocomplete' => 'current-password']) ?>
  <p class="small" style="text-align:right;margin-top:-.5rem"><a href="/mail/forgot-password">Forgot your password?</a></p>
  <button class="btn btn-primary btn-block btn-lg" type="submit">Sign in</button>
</form>
<p class="small muted mt-3">New mailbox? Open the activation link from your invitation email first. Administrators manage the workspace from the <a href="/login">console</a>.</p>
