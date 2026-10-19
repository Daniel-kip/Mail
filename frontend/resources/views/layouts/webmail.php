<?php
$noindex = true;
$shortcuts = true;
$mailbox = $mail['mailbox'] ?? [];
$quota = $mail['quota'] ?? null;
$folders = $folders ?? [];
$current = $currentFolder ?? null;
$roleIcons = ['inbox' => 'inbox', 'sent' => 'send', 'drafts' => 'edit', 'trash' => 'trash', 'junk' => 'alert-triangle', 'archive' => 'archive'];
$order = ['inbox' => 0, 'drafts' => 1, 'sent' => 2, 'archive' => 3, 'junk' => 4, 'trash' => 5];
usort($folders, static fn ($a, $b) => [($order[$a['role'] ?? ''] ?? 9), $a['display_name']] <=> [($order[$b['role'] ?? ''] ?? 9), $b['display_name']]);
$shellClass = 'mail-shell' . (($mailLayout ?? 'list') === 'page' ? ' no-reader' : '') . (!empty($reading) ? ' reading' : '');
?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<div class="<?= e($shellClass) ?>">
  <aside class="mail-side" aria-label="Mailbox navigation">
    <?= $view->insert('partials/brand', ['href' => '/mail', 'subtitle' => 'WORK EMAIL']) ?>
    <a class="btn btn-primary btn-block compose-btn" href="/mail/compose" data-shortcut="compose"><?= icon('edit') ?> Compose</a>
    <nav aria-label="Folders">
      <?php foreach ($folders as $f): ?>
        <a class="folder<?= $current === $f['name'] ? ' is-active' : '' ?>" href="<?= e('/mail/folder/' . rawurlencode($f['name'])) ?>"<?= $current === $f['name'] ? ' aria-current="page"' : '' ?>>
          <?= icon($roleIcons[$f['role'] ?? ''] ?? 'layers') ?>
          <span class="truncate"><?= e($f['display_name']) ?></span>
          <?php if (($f['unseen'] ?? 0) > 0 && ($f['role'] ?? '') !== 'sent' && ($f['role'] ?? '') !== 'trash'): ?><span class="count" aria-label="<?= e($f['unseen']) ?> unread"><?= e($f['unseen']) ?></span><?php endif; ?>
        </a>
      <?php endforeach; ?>
      <?php if ($folders === []): ?><p class="small muted" style="padding:.5rem .7rem">Folders are unavailable right now.</p><?php endif; ?>
    </nav>
    <div class="nav-label" style="color:var(--text-3);margin-top:1rem">Tools</div>
    <a class="folder<?= nav_active('/mail/contacts') ?>" href="/mail/contacts"><?= icon('contact') ?><span>Contacts</span></a>
    <a class="folder<?= nav_active('/mail/directory') ?>" href="/mail/directory"><?= icon('building') ?><span>Directory</span></a>
    <a class="folder<?= nav_active('/mail/settings') ?>" href="/mail/settings"><?= icon('settings') ?><span>Settings</span></a>
    <div class="connect-card mt-3">
      <strong><?= icon('phone', 'icon-sm') ?> Work email on your phone</strong>
      Add this mailbox to Gmail, Apple Mail or Outlook in a few guided steps.
      <a class="btn btn-sm btn-block" href="/mail/setup">Connect my phone</a>
    </div>
    <?php if (is_array($quota) && ($quota['limit_bytes'] ?? 0) > 0): ?>
      <div class="quota">
        <?= \App\View\Charts::meter('Storage', (int) round($quota['used_bytes'] / 1048576), (int) round($quota['limit_bytes'] / 1048576), 'MB') ?>
      </div>
    <?php endif; ?>
    <details class="dropdown mt-2">
      <summary class="btn btn-ghost btn-block" style="justify-content:flex-start">
        <span class="avatar avatar-sm"><?= e(initials($mailbox['display_name'] ?? $mailbox['address'] ?? '')) ?></span>
        <span class="truncate grow" style="text-align:left"><?= e($mailbox['address'] ?? '') ?></span>
      </summary>
      <div class="dropdown-menu left" style="bottom:calc(100% + 6px);top:auto">
        <div class="head"><?= e($mailbox['organization'] ?? '') ?></div>
        <a href="/mail/settings"><?= icon('settings') ?> Settings</a>
        <a href="/mail/setup"><?= icon('phone') ?> Connect a phone</a>
        <button type="button" data-theme-toggle><?= icon('moon') ?> Toggle dark mode</button>
        <div class="divider"></div>
        <form method="post" action="/mail/logout"><?= csrf_field() ?><button type="submit"><?= icon('log-out') ?> Sign out</button></form>
      </div>
    </details>
  </aside>
  <?= $content ?>
</div>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
