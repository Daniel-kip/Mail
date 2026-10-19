<h1>Two-factor authentication</h1>
<p class="text-2">Enter the 6-digit code from your authenticator app.</p>
<form method="post" action="/login/mfa" class="mt-3">
  <?= csrf_field() ?>
  <div class="field">
    <label for="code">Authentication code</label>
    <input class="input otp" id="code" name="code" inputmode="numeric" autocomplete="one-time-code" maxlength="7" required autofocus<?= error_attrs('code') ?>>
    <?= error_text('code') ?>
  </div>
  <button class="btn btn-primary btn-block btn-lg" type="submit">Verify</button>
</form>
<form method="post" action="/logout" class="mt-2" style="text-align:center"><?= csrf_field() ?><button class="link-button small" type="submit">Use a different account</button></form>
