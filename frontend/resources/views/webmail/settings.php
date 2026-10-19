<?php $ar = $prefs['auto_reply'] ?? []; ?>
<main class="mail-page" id="main">
  <div class="content stack" style="max-width:760px">
    <?= $view->insert('partials/page-head', ['heading' => 'Settings', 'lede' => $mail['mailbox']['address'] ?? '']) ?>
    <section class="card"><div class="card-head"><h2>Phone and mail apps</h2></div><div class="card-body row-between">
      <p class="text-2 mb-0">Add this mailbox to Gmail, Apple Mail or Outlook with step-by-step help and a connection test.</p>
      <a class="btn btn-primary" href="/mail/setup"><?= icon('phone') ?> Connect my phone</a>
    </div></section>

    <section class="card" id="auto-reply"><div class="card-head"><h2>Out-of-office reply</h2><?= !empty($ar['active_now']) ? badge('active', 'On now') : (!empty($ar['enabled']) ? badge('pending', 'Scheduled') : badge('disabled', 'Off')) ?></div>
      <form method="post" action="/mail/settings/auto-reply" class="card-body"><?= csrf_field() ?>
        <?php if (empty($ar['server_supported'])): ?><div class="alert alert-info mb-2"><?= icon('info') ?><div>Automatic replies are not available on this server yet. Your settings are saved and apply once your administrator enables them.</div></div><?php endif; ?>
        <label class="check"><input type="checkbox" name="auto_reply_enabled" value="1"<?= checked((bool) old('auto_reply_enabled', $ar['enabled'] ?? false)) ?>> Send an automatic reply to people who email me</label>
        <div class="grid grid-2 mt-2">
          <?= $view->insert('partials/field', ['name' => 'auto_reply_starts_on', 'label' => 'From', 'type' => 'date', 'default' => $ar['starts_on'] ?? '']) ?>
          <?= $view->insert('partials/field', ['name' => 'auto_reply_ends_on', 'label' => 'Until', 'type' => 'date', 'default' => $ar['ends_on'] ?? '']) ?>
        </div>
        <?= $view->insert('partials/field', ['name' => 'auto_reply_subject', 'label' => 'Subject', 'default' => $ar['subject'] ?? '', 'placeholder' => 'Out of office']) ?>
        <?= $view->insert('partials/field', ['name' => 'auto_reply_body', 'label' => 'Message', 'type' => 'textarea', 'rows' => 5, 'default' => $ar['body'] ?? '', 'noOptional' => true, 'placeholder' => 'I am away until Monday 14 October with limited access to email. For urgent matters contact ...']) ?>
        <p class="hint">Each sender gets at most one reply every few days. Mailing lists and automated mail are not answered.</p>
        <?php if (!empty($ar['sync_error'])): ?><p class="field-error"><?= icon('alert-circle', 'icon-sm') ?> Not applied yet: <?= e($ar['sync_error']) ?></p><?php endif; ?>
        <button class="btn btn-primary" type="submit">Save out-of-office</button>
      </form>
    </section>

    <section class="card"><div class="card-head"><h2>Reading and writing</h2></div>
      <form method="post" action="/mail/settings/preferences" class="card-body"><?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'signature', 'label' => 'Signature', 'type' => 'textarea', 'rows' => 4, 'default' => $prefs['signature'] ?? '', 'placeholder' => "Jane Wanjiru\nAccountant, Finance\n+254 700 000 000"]) ?>
        <?= $view->insert('partials/field', ['name' => 'messages_per_page', 'label' => 'Messages per page', 'type' => 'select', 'required' => true, 'default' => (string) ($prefs['messages_per_page'] ?? 50), 'options' => ['25' => '25', '50' => '50', '100' => '100']]) ?>
        <button class="btn btn-primary" type="submit">Save preferences</button>
      </form>
    </section>

    <section class="card" id="password"><div class="card-head"><h2>Password</h2></div>
      <form method="post" action="/mail/settings/password" class="card-body"><?= csrf_field() ?>
        <input type="text" name="username" value="<?= e($mail['mailbox']['address'] ?? '') ?>" autocomplete="username" hidden>
        <?= $view->insert('partials/field', ['name' => 'current_password', 'label' => 'Current password', 'type' => 'password', 'required' => true, 'autocomplete' => 'current-password']) ?>
        <?= $view->insert('partials/field', ['name' => 'new_password', 'label' => 'New password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password', 'hint' => 'At least 10 characters. Afterwards, update it in your phone\'s mail app.', 'attrs' => ' minlength="10"']) ?>
        <?= $view->insert('partials/field', ['name' => 'password_confirmation', 'label' => 'Confirm new password', 'type' => 'password', 'required' => true, 'autocomplete' => 'new-password']) ?>
        <button class="btn btn-primary" type="submit">Change password</button>
      </form>
    </section>
  </div>
</main>
