<div class="verify-hero"><?= icon('smartphone-check', 'icon-xl') ?></div>
<h1>Verify your phone</h1>
<?php if ($phone !== ''): ?>
  <p class="text-2">Enter the six-digit code we texted to <strong><?= e($sentTo ?? $phone) ?></strong>. It expires in 10 minutes.</p>
  <form method="post" action="/console/verify-phone" class="mt-3">
    <?= csrf_field() ?>
    <input type="hidden" name="next" value="<?= e($next) ?>">
    <div class="field">
      <label for="code">Verification code</label>
      <input class="input otp" id="code" name="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9 ]{6,7}" maxlength="7" required autofocus placeholder="••••••"<?= error_attrs('code') ?>>
      <?= error_text('code') ?>
    </div>
    <button class="btn btn-primary btn-block btn-lg" type="submit">Verify and continue</button>
  </form>
  <form method="post" action="/console/verify-phone/send" class="mt-2" style="text-align:center">
    <?= csrf_field() ?>
    <button class="link-button small" type="submit">Didn't get it? Send a new code</button>
  </form>
  <details class="cant-find mt-3">
    <summary><?= icon('edit') ?> Use a different number</summary>
    <form method="post" action="/console/verify-phone/send" class="mt-2">
      <?= csrf_field() ?>
      <?= $view->insert('partials/field', ['name' => 'phone', 'label' => 'New mobile number', 'type' => 'tel', 'required' => true, 'id' => 'new-phone', 'value' => '', 'autocomplete' => 'tel', 'placeholder' => '0712 345 678', 'attrs' => ' inputmode="tel"']) ?>
      <button class="btn btn-block" type="submit">Text a code to this number</button>
    </form>
  </details>
<?php else: ?>
  <p class="text-2">Add your mobile number and we will text you a six-digit code.</p>
  <form method="post" action="/console/verify-phone/send" class="mt-3">
    <?= csrf_field() ?>
    <?= $view->insert('partials/field', ['name' => 'phone', 'label' => 'Mobile number', 'type' => 'tel', 'required' => true, 'autocomplete' => 'tel', 'placeholder' => '0712 345 678', 'attrs' => ' inputmode="tel" autofocus']) ?>
    <button class="btn btn-primary btn-block btn-lg" type="submit">Send code</button>
  </form>
<?php endif; ?>
<p class="small muted mt-3" style="text-align:center"><a href="<?= e($next !== '' ? $next : '/console') ?>">Do this later</a></p>
