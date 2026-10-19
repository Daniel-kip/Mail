<?php $method = old('setup_method', 'invite'); ?>
<?= $view->insert('partials/page-head', ['heading' => 'New mailbox', 'lede' => 'Create a work address for a staff member or student.']) ?>
<?php if ($domains === []): ?>
  <section class="card"><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'globe', 'heading' => 'No domain is ready for mailboxes', 'text' => 'Verify a domain and turn on mail hosting first.', 'action' => '<a class="btn btn-primary btn-sm" href="/console/domains">Go to domains</a>']) ?></div></section>
<?php else: ?>
<form method="post" action="/console/mailboxes" class="card" style="max-width:820px">
  <?= csrf_field() ?>
  <div class="card-body">
    <h2 class="section-title">Person</h2>
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'display_name', 'label' => 'Full name', 'required' => true, 'placeholder' => 'Jane Wanjiru']) ?>
      <div class="field">
        <label for="f-local_part">Email address</label>
        <div class="input-group">
          <input class="input" id="f-local_part" name="local_part" value="<?= e(old('local_part')) ?>" required placeholder="jane.wanjiru" autocapitalize="none" spellcheck="false"<?= error_attrs('local_part') ?>>
          <select class="input" name="domain_id" aria-label="Domain"><?php foreach ($domains as $d): ?><option value="<?= e($d['id']) ?>"<?= selected(old('domain_id'), $d['id']) ?>>@<?= e($d['domain']) ?></option><?php endforeach; ?></select>
        </div>
        <?= error_text('local_part') ?>
      </div>
      <?= $view->insert('partials/field', ['name' => 'department', 'label' => 'Department', 'placeholder' => 'Finance']) ?>
      <?= $view->insert('partials/field', ['name' => 'job_title', 'label' => 'Job title', 'placeholder' => 'Accountant']) ?>
    </div>
    <h2 class="section-title mt-2">How will they get access?</h2>
    <div class="stack">
      <label class="choice"><input type="radio" name="setup_method" value="invite"<?= checked($method === 'invite') ?>><span><strong>Email an invitation</strong> <span class="badge badge-brand">Recommended</span><span class="small muted">We email a single-use link (valid 7 days) to their personal address; they choose their own password. Nobody else ever knows it.</span></span></label>
      <label class="choice"><input type="radio" name="setup_method" value="link"<?= checked($method === 'link') ?>><span><strong>Give them an activation link</strong><span class="small muted">You get a single-use link to hand over in person or by chat. They choose their own password.</span></span></label>
      <label class="choice"><input type="radio" name="setup_method" value="generate"<?= checked($method === 'generate') ?>><span><strong>Generate a password</strong><span class="small muted">A strong password is shown to you once. Share it securely and ask them to change it.</span></span></label>
      <label class="choice"><input type="radio" name="setup_method" value="password"<?= checked($method === 'password') ?>><span><strong>Set a password myself</strong><span class="small muted">Only when you must; prefer an invitation.</span></span></label>
    </div>
    <div class="grid grid-2 mt-2">
      <?= $view->insert('partials/field', ['name' => 'recovery_email', 'label' => 'Personal email', 'type' => 'email', 'hint' => 'Required for invitations; used for password resets. Must not be a hosted address.', 'noOptional' => true]) ?>
      <?= $view->insert('partials/field', ['name' => 'password', 'label' => 'Password (only for "set a password myself")', 'type' => 'password', 'autocomplete' => 'new-password', 'noOptional' => true, 'attrs' => ' minlength="10"']) ?>
      <?= $view->insert('partials/field', ['name' => 'quota_mb', 'label' => 'Storage (MB)', 'type' => 'number', 'hint' => 'Leave empty for the plan default.', 'attrs' => ' min="50" step="50"']) ?>
    </div>
  </div>
  <div class="card-foot form-actions"><button class="btn btn-primary" type="submit">Create mailbox</button><a class="btn btn-ghost" href="/console/mailboxes">Cancel</a></div>
</form>
<?php endif; ?>
