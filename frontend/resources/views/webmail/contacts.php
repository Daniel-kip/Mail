<main class="mail-page" id="main">
  <div class="content">
    <?= $view->insert('partials/page-head', ['heading' => 'Contacts', 'lede' => 'People you write to. Contacts appear as suggestions when you compose.']) ?>
    <div class="grid grid-2" style="grid-template-columns:1.6fr 1fr;align-items:start">
      <section class="card">
        <div class="card-head"><form method="get" class="grow"><input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Search contacts" aria-label="Search contacts"></form></div>
        <?php if ($contacts === []): ?><div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'contact', 'heading' => $q ? 'No contacts match' : 'No contacts yet', 'text' => 'Add people you write to often.']) ?></div><?php else: ?>
        <div class="table-wrap"><table class="table">
          <tbody><?php foreach ($contacts as $c): ?>
            <?php if ($editing === $c['id']): ?>
              <tr><td colspan="3"><form method="post" action="/mail/contacts/<?= e($c['id']) ?>" class="grid grid-2"><?= csrf_field() ?>
                <?php foreach (['name' => 'Name', 'email' => 'Email', 'phone' => 'Phone', 'organization' => 'Organization'] as $k => $label): ?><?= $view->insert('partials/field', ['name' => $k, 'id' => 'e-' . $k, 'label' => $label, 'value' => $c[$k] ?? '', 'required' => $k === 'email']) ?><?php endforeach; ?>
                <div class="span-2"><?= $view->insert('partials/field', ['name' => 'notes', 'id' => 'e-notes', 'label' => 'Notes', 'type' => 'textarea', 'rows' => 2, 'value' => $c['notes'] ?? '']) ?></div>
                <div class="form-actions"><button class="btn btn-primary btn-sm" type="submit">Save</button><a class="btn btn-ghost btn-sm" href="/mail/contacts">Cancel</a></div>
              </form></td></tr>
            <?php else: ?>
              <tr>
                <td><span class="avatar avatar-sm"><?= e(initials($c['name'] ?: $c['email'])) ?></span></td>
                <td><span class="cell-title"><?= e($c['name'] ?: $c['email']) ?></span><div class="cell-sub"><?= e($c['email']) ?><?= $c['phone'] ? ' · ' . e($c['phone']) : '' ?><?= $c['organization'] ? ' · ' . e($c['organization']) : '' ?></div></td>
                <td class="nowrap">
                  <a class="btn btn-sm" href="/mail/compose?to=<?= e(rawurlencode($c['email'])) ?>"><?= icon('send') ?><span class="sr-only">Write to <?= e($c['email']) ?></span></a>
                  <a class="btn btn-sm btn-ghost" href="/mail/contacts?edit=<?= e($c['id']) ?>">Edit</a>
                  <form method="post" action="/mail/contacts/<?= e($c['id']) ?>/delete" style="display:inline" data-confirm="Delete this contact?" data-confirm-label="Delete"><?= csrf_field() ?><button class="btn btn-sm btn-ghost" type="submit" aria-label="Delete"><?= icon('trash') ?></button></form>
                </td>
              </tr>
            <?php endif; ?>
          <?php endforeach; ?></tbody>
        </table></div>
        <?php endif; ?>
      </section>
      <form method="post" action="/mail/contacts" class="card card-body"><?= csrf_field() ?>
        <h2 class="section-title">Add a contact</h2>
        <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name']) ?>
        <?= $view->insert('partials/field', ['name' => 'email', 'label' => 'Email', 'type' => 'email', 'required' => true]) ?>
        <?= $view->insert('partials/field', ['name' => 'phone', 'label' => 'Phone', 'type' => 'tel']) ?>
        <?= $view->insert('partials/field', ['name' => 'organization', 'label' => 'Organization']) ?>
        <?= $view->insert('partials/field', ['name' => 'notes', 'label' => 'Notes', 'type' => 'textarea', 'rows' => 2]) ?>
        <button class="btn btn-primary" type="submit"><?= icon('plus') ?> Add contact</button>
      </form>
    </div>
  </div>
</main>
