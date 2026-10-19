<?php $t = old('account_type', 'institution'); ?>
<?= $view->insert('partials/page-head', ['heading' => 'Create a workspace', 'lede' => 'A workspace holds its own domains, keys, mailboxes, team and billing.']) ?>
<form method="post" action="/console/organizations" class="card card-body">
  <?= csrf_field() ?>
  <fieldset class="field" style="border:0;padding:0">
    <legend class="label">Type</legend>
    <div class="choice-grid" style="grid-template-columns:1fr 1fr">
      <label class="choice"><input type="radio" name="account_type" value="institution"<?= checked($t === 'institution') ?>><span><strong>Institution</strong><span class="small muted">Hosted work mailboxes and sending</span></span></label>
      <label class="choice"><input type="radio" name="account_type" value="individual"<?= checked($t === 'individual') ?>><span><strong>Individual</strong><span class="small muted">Transactional sending only</span></span></label>
    </div>
  </fieldset>
  <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name', 'required' => true, 'placeholder' => 'Acme University', 'attrs' => ' autofocus']) ?>
  <div class="form-actions"><button class="btn btn-primary" type="submit">Create workspace</button><a class="btn btn-ghost" href="/console">Cancel</a></div>
</form>
