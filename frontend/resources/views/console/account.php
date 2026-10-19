<?php $user = auth()['user'] ?? []; ?>
<?= $view->insert('partials/page-head', ['heading' => 'Your account', 'lede' => $user['email'] ?? '']) ?>
<div class="stack" style="max-width:760px">
  <section class="card">
    <div class="card-head"><h2>Profile</h2></div>
    <form method="post" action="/console/account/profile" class="card-body">
      <?= csrf_field() ?>
      <?= $view->insert('partials/field', ['name' => 'full_name', 'label' => 'Name', 'required' => true, 'default' => $user['full_name'] ?? '', 'autocomplete' => 'name']) ?>
      <div class="field"><span class="label">Username</span><div class="row"><code>@<?= e($user['username'] ?? '') ?></code><?= !empty($user['google_linked']) ? badge('active', 'Google linked') : '' ?></div></div>
      <div class="field"><span class="label">Email</span><div class="row"><span><?= e($user['email'] ?? '') ?></span><?= !empty($user['email_confirmed']) ? badge('verified', 'Confirmed') : badge('neutral', 'Not confirmed') ?></div></div>
      <div class="field"><span class="label">Mobile number</span><div class="row"><span><?= e(($user['phone'] ?? '') ?: 'Not added') ?></span><?= !empty($user['phone_verified']) ? badge('verified', 'Verified') : '<a class="btn btn-sm" href="/console/verify-phone">' . (empty($user['phone']) ? 'Add and verify' : 'Verify now') . '</a>' ?></div></div>
      <div class="form-actions"><button class="btn btn-primary" type="submit">Save profile</button></div>
    </form>
    <?php if (empty($user['email_verified'])): ?>
      <form method="post" action="/console/account/email/resend" class="card-foot"><?= csrf_field() ?><input type="hidden" name="back" value="/console/account"><button class="btn btn-sm" type="submit">Resend confirmation email</button></form>
    <?php endif; ?>
  </section>

  <section class="card" id="password">
    <div class="card-head"><h2>Password</h2></div>
    <form method="post" action="/console/account/password" class="card-body">
      <?= csrf_field() ?>
      <?= $view->insert('partials/field', ['name' => 'current_password', 'label' => 'Current password', 'type' => 'password', 'required' => true, 'autocomplete' => 'current-password']) ?>
      <?= $view->insert('partials/field', ['name' => 'new_password', 'label' => 'New password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password', 'hint' => 'At least 12 characters.', 'attrs' => ' minlength="12"']) ?>
      <?= $view->insert('partials/field', ['name' => 'password_confirmation', 'label' => 'Confirm new password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password']) ?>
      <div class="form-actions"><button class="btn btn-primary" type="submit">Change password</button></div>
    </form>
  </section>

  <section class="card" id="mfa">
    <div class="card-head"><h2>Two-factor authentication</h2><?= !empty($user['mfa_enabled']) ? badge('active', 'On') : badge('disabled', 'Off') ?></div>
    <div class="card-body">
      <?php if ($recoveryCodes): ?>
        <div class="secret-box mb-3"><strong>Save your recovery codes</strong><p class="small">Each code works once if you lose your phone. They are shown only now.</p><pre id="recovery"><?= e(implode("\n", $recoveryCodes)) ?></pre><button class="btn btn-sm" type="button" data-copy-target="recovery"><?= icon('copy') ?><span class="copy-label">Copy codes</span></button></div>
      <?php endif; ?>
      <?php if (!empty($user['mfa_enabled'])): ?>
        <p class="text-2">Sign-in requires a code from your authenticator app.</p>
        <form method="post" action="/console/account/mfa/disable" class="grid grid-2">
          <?= csrf_field() ?>
          <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'Password', 'type' => 'password', 'required' => true, 'id' => 'mfa-pass', 'autocomplete' => 'current-password']) ?>
          <?= $view->insert('partials/field', ['name' => 'code', 'label' => 'Current code', 'required' => true, 'attrs' => ' inputmode="numeric" autocomplete="one-time-code"']) ?>
          <div><button class="btn btn-danger" type="submit">Turn off two-factor</button></div>
        </form>
      <?php elseif ($mfaSetup): ?>
        <div class="grid grid-2">
          <div class="qr-box"><?= \App\View\QrCode::svg((string) $mfaSetup['otpauth_url'], 4, 'Authenticator setup QR code') ?></div>
          <div>
            <ol class="small text-2"><li>Scan the code with an authenticator app (Google Authenticator, Microsoft Authenticator, 1Password, Aegis).</li><li>Or enter this key manually:</li></ol>
            <div class="copy-field"><span class="value" id="mfa-secret"><?= e(trim(chunk_split((string) $mfaSetup['secret'], 4, ' '))) ?></span><button class="btn btn-sm btn-ghost" type="button" data-copy-target="mfa-secret"><?= icon('copy') ?><span class="sr-only copy-label">Copy</span></button></div>
            <form method="post" action="/console/account/mfa/enable" class="mt-2"><?= csrf_field() ?>
              <?= $view->insert('partials/field', ['name' => 'code', 'label' => '6-digit code from the app', 'required' => true, 'attrs' => ' inputmode="numeric" autocomplete="one-time-code" maxlength="7"']) ?>
              <button class="btn btn-primary" type="submit">Turn on two-factor</button>
            </form>
          </div>
        </div>
      <?php else: ?>
        <p class="text-2">Protect your account with a time-based code from an authenticator app.</p>
        <form method="post" action="/console/account/mfa/setup"><?= csrf_field() ?><button class="btn btn-primary" type="submit"><?= icon('shield-check') ?> Set up two-factor</button></form>
      <?php endif; ?>
    </div>
  </section>

  <section class="card" id="sessions">
    <div class="card-head"><h2>Active sessions</h2></div>
    <div class="table-wrap"><table class="table">
      <thead><tr><th>Device</th><th>IP address</th><th>Last active</th><th></th></tr></thead>
      <tbody>
        <?php foreach ($sessions as $s): ?>
          <tr>
            <td><span class="truncate" style="max-width:320px;display:inline-block"><?= e($s['user_agent'] ?? 'Unknown device') ?></span><?= !empty($s['current']) ? ' ' . badge('active', 'This session') : '' ?></td>
            <td class="nowrap"><?= e($s['ip'] ?? $s['ip_address'] ?? '—') ?></td>
            <td class="nowrap"><?= time_tag($s['last_seen_at'] ?? $s['created_at'] ?? null) ?></td>
            <td><?php if (empty($s['current'])): ?><form method="post" action="/console/account/sessions/<?= e($s['id']) ?>/revoke"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Sign out</button></form><?php endif; ?></td>
          </tr>
        <?php endforeach; ?>
      </tbody>
    </table></div>
  </section>
</div>
