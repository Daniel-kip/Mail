<?php
$folderUrl = '/mail/folder/' . rawurlencode($currentFolder);
$pages = max(1, (int) ceil($list['total'] / max(1, $list['per_page'])));
$activeUid = $activeUid ?? null;
$qs = $q !== '' ? '?q=' . rawurlencode($q) : '';
?>
<section class="mail-list" aria-label="Messages">
  <div class="mail-list-head">
    <form method="get" action="<?= e($folderUrl) ?>" role="search"><input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Search mail" aria-label="Search mail" data-shortcut="search"></form>
    <a class="btn btn-ghost btn-icon" href="<?= e($folderUrl . $qs) ?>" aria-label="Refresh"><?= icon('refresh') ?></a>
  </div>
  <form method="post" action="/mail/actions" id="bulk-form" data-select-scope>
    <?= csrf_field() ?>
    <input type="hidden" name="folder" value="<?= e($currentFolder) ?>">
    <input type="hidden" name="q" value="<?= e($q) ?>">
    <input type="hidden" name="back" value="<?= e($folderUrl . $qs) ?>">
    <div class="mail-bulk">
      <label class="check mb-0"><input type="checkbox" data-select-all aria-label="Select all"></label>
      <span class="grow small muted" data-bulk-count><?= plural((int) $list['total'], 'message') ?></span>
      <span data-bulk-bar hidden class="row" style="gap:.2rem">
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="read" aria-label="Mark read" title="Mark read"><?= icon('mail') ?></button>
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="unread" aria-label="Mark unread" title="Mark unread"><?= icon('inbox') ?></button>
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="flag" aria-label="Flag" title="Flag"><?= icon('flag') ?></button>
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="archive" aria-label="Archive" title="Archive"><?= icon('archive') ?></button>
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="spam" aria-label="Report spam" title="Report spam"><?= icon('alert-triangle') ?></button>
        <button class="btn btn-ghost btn-sm btn-icon" name="action" value="delete" aria-label="Delete" title="Delete"><?= icon('trash') ?></button>
      </span>
    </div>
    <div class="mail-items">
      <?php if ($list['error']): ?>
        <div class="reader-empty"><div><?= icon('server', 'icon-xl') ?><p><?= e($list['error']) ?></p></div></div>
      <?php elseif ($list['data'] === []): ?>
        <div class="reader-empty"><div><?= icon('inbox', 'icon-xl') ?><p><?= $q !== '' ? 'No messages match your search.' : 'This folder is empty.' ?></p></div></div>
      <?php endif; ?>
      <?php foreach ($list['data'] as $msg): ?>
        <div class="mail-item<?= empty($msg['seen']) ? ' is-unread' : '' ?><?= (int) $msg['uid'] === $activeUid ? ' is-active' : '' ?>">
          <span class="mi-check"><input type="checkbox" name="uids[]" value="<?= (int) $msg['uid'] ?>" data-select-item aria-label="Select message from <?= e($msg['from'] ?? '') ?>"></span>
          <a href="<?= e($folderUrl . '/message/' . (int) $msg['uid'] . $qs) ?>" style="display:contents;color:inherit">
            <span class="mi-top"><span class="mi-from truncate"><?= e($msg['from'] ?: ($msg['from_address'] ?? '(unknown sender)')) ?></span><span class="mi-date"><?= !empty($msg['flagged']) ? icon('flag', 'icon-xs', 'Flagged') : '' ?><?= !empty($msg['has_attachments']) ? icon('paperclip', 'icon-xs', 'Has attachments') : '' ?> <?= e($msg['date'] ? ago($msg['date']) : '') ?></span></span>
            <span class="mi-subject truncate"><?= !empty($msg['answered']) ? icon('reply', 'icon-xs', 'Replied') . ' ' : '' ?><?= e($msg['subject'] ?: '(no subject)') ?></span>
            <?php if (!empty($msg['preview'])): ?><span class="mi-preview truncate small muted"><?= e($msg['preview']) ?></span><?php endif; ?>
          </a>
        </div>
      <?php endforeach; ?>
    </div>
  </form>
  <?php if ($pages > 1): ?>
    <nav class="pagination" style="padding:.5rem .9rem;border-top:1px solid var(--border)" aria-label="Pages">
      <?php if ($list['page'] > 1): ?><a class="btn btn-sm" href="<?= e($folderUrl . '?' . http_build_query(array_filter(['q' => $q ?: null, 'page' => $list['page'] - 1]))) ?>"><?= icon('chevron-left', 'icon-sm') ?> Newer</a><?php endif; ?>
      <span class="small muted grow" style="text-align:center">Page <?= (int) $list['page'] ?> of <?= $pages ?></span>
      <?php if ($list['page'] < $pages): ?><a class="btn btn-sm" href="<?= e($folderUrl . '?' . http_build_query(array_filter(['q' => $q ?: null, 'page' => $list['page'] + 1]))) ?>">Older <?= icon('chevron-right', 'icon-sm') ?></a><?php endif; ?>
    </nav>
  <?php endif; ?>
</section>
