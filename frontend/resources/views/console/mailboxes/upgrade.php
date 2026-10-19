<?= $view->insert('partials/page-head', ['heading' => 'Hosted work mailboxes', 'lede' => 'Available on institution workspaces.']) ?>
<section class="card" style="max-width:760px">
  <div class="card-body">
    <p class="text-2">Your workspace is an <strong>individual developer</strong> account: it relays transactional email from your applications. Institutions can also give every staff member or student a mailbox on their own domain, with:</p>
    <ul class="check-list">
      <li><?= icon('check') ?> Webmail with folders, contacts, a staff directory and out-of-office replies</li>
      <li><?= icon('check') ?> Guided phone setup for Gmail, Apple Mail and Outlook, with connection tests</li>
      <li><?= icon('check') ?> Emailed invitations so administrators never handle passwords</li>
      <li><?= icon('check') ?> An ICT support role that can unlock accounts and send reset links</li>
      <li><?= icon('check') ?> Shared addresses (aliases) such as info@ and admissions@</li>
    </ul>
    <?php if (auth()['organization']['role'] === 'owner'): ?>
      <form method="post" action="/console/settings/convert" data-confirm="Convert this workspace to an institution? Your keys, domains and history stay. This cannot be undone." data-confirm-label="Convert">
        <?= csrf_field() ?>
        <button class="btn btn-primary" type="submit"><?= icon('building') ?> Convert to an institution workspace</button>
      </form>
    <?php else: ?>
      <p class="small muted">Only the workspace owner can convert it to an institution.</p>
    <?php endif; ?>
    <p class="small muted mt-2 mb-0">Or <a href="/console/organizations/new">create a separate institution workspace</a>.</p>
  </div>
</section>
