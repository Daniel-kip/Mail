<?php
$t = old('account_type', $type ?? 'institution');
$google = !empty($providers['google']);
$phoneVerification = $providers['phone_verification'] ?? true;
?>
<h1>Create your account</h1>
<p class="text-2">Free to start. Pay later with M-Pesa when you upgrade.</p>
<form method="post" action="/register" class="mt-3" id="register-form">
  <?= csrf_field() ?>
  <input type="hidden" name="plan" value="<?= e(old('plan', $plan)) ?>">
  <fieldset class="field" style="border:0;padding:0;margin:0 0 1rem">
    <legend class="label">I am setting up email for</legend>
    <div class="choice-grid" style="grid-template-columns:1fr 1fr">
      <label class="choice"><input type="radio" name="account_type" value="institution"<?= checked($t === 'institution') ?>><span><strong>An institution</strong><span class="small muted">Work mailboxes for staff or students, plus the sending API</span></span></label>
      <label class="choice"><input type="radio" name="account_type" value="individual"<?= checked($t === 'individual') ?>><span><strong>My app</strong><span class="small muted">Send invoices, receipts and updates from your software</span></span></label>
    </div>
    <?= error_text('account_type') ?>
  </fieldset>
  <?php if ($google): ?>
    <?= $view->insert('partials/google-button', ['label' => 'Sign up with Google', 'query' => ['type' => $t]]) ?>
    <div class="divider-text mt-3 mb-3"><span>or create a username</span></div>
  <?php endif; ?>
  <div class="grid grid-2" style="gap:0 1rem">
    <?= $view->insert('partials/field', ['name' => 'full_name', 'label' => 'Your name', 'required' => true, 'autocomplete' => 'name', 'placeholder' => 'Wanjiru Kamau']) ?>
    <?= $view->insert('partials/field', ['name' => 'username', 'label' => 'Username', 'required' => true, 'autocomplete' => 'username', 'placeholder' => 'wanjiru', 'attrs' => ' minlength="3" maxlength="32" pattern="@?[A-Za-z0-9][A-Za-z0-9._\-]{2,31}" autocapitalize="none" spellcheck="false"']) ?>
  </div>
  <?= $view->insert('partials/field', ['name' => 'phone', 'label' => 'Mobile number', 'type' => 'tel', 'required' => true, 'autocomplete' => 'tel', 'placeholder' => '0712 345 678',
      'hint' => $phoneVerification ? 'We text you a six-digit code to verify your account. Kenyan numbers can start with 07 or 01; others with +country code.' : 'Used for account recovery and M-Pesa payments.', 'attrs' => ' inputmode="tel"']) ?>
  <?= $view->insert('partials/field', ['name' => 'organization_name', 'label' => 'Institution or project name', 'hint' => 'Required for institutions. For apps it defaults to your name.', 'autocomplete' => 'organization', 'noOptional' => true]) ?>
  <?= $view->insert('partials/field', ['name' => 'email', 'label' => 'Email for invoices and notices', 'type' => 'email', 'required' => true, 'autocomplete' => 'email', 'hint' => 'Any provider works. You do not need to confirm it to get started.']) ?>
  <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'Password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password', 'hint' => 'At least 12 characters. A short sentence is easy to remember and hard to guess.', 'attrs' => ' minlength="12"']) ?>
  <label class="check"><input type="checkbox" name="terms" value="1"<?= checked((bool) old('terms')) ?>> <span class="small">I agree to the <a href="/legal/terms" target="_blank">terms</a> and the <a href="/legal/privacy" target="_blank">privacy notice</a>.</span></label>
  <?= error_text('terms') ?>
  <button class="btn btn-primary btn-block btn-lg mt-3" type="submit">Create account</button>
</form>
<p class="small muted mt-3" style="text-align:center">Already have an account? <a href="/login">Sign in</a></p>
