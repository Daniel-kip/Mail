<?php
$folderUrl = '/mail/folder/' . rawurlencode($currentFolder);
$qs = $q !== '' ? '?q=' . rawurlencode($q) : '';
$msgUrl = $folderUrl . '/message/' . (int) $m['uid'];
$from = $m['from'][0] ?? ['name' => '', 'email' => ''];
$fmt = static fn (array $list): string => implode(', ', array_map(static fn ($a) => ($a['name'] ?? '') !== '' ? $a['name'] . ' <' . $a['email'] . '>' : (string) $a['email'], $list));
$compose = static fn (string $mode): string => '/mail/compose?' . http_build_query(['mode' => $mode, 'folder' => $m['folder'], 'uid' => $m['uid']]);
$bodyUrl = $msgUrl . '/body' . ($remote ? '?remote=1' : '');
$inJunk = $roles['junk'] !== '' && $currentFolder === $roles['junk'];
$action = static function (string $value, string $icon, string $label, string $shortcut = '') use ($m, $currentFolder, $folderUrl, $msgUrl): string {
    return '<form method="post" action="/mail/actions" style="display:inline">' . csrf_field()
        . '<input type="hidden" name="folder" value="' . e($currentFolder) . '"><input type="hidden" name="uids[]" value="' . (int) $m['uid'] . '"><input type="hidden" name="back" value="' . e($msgUrl) . '">'
        . '<button class="btn btn-sm" type="submit" name="action" value="' . e($value) . '"' . ($shortcut ? ' data-shortcut="' . $shortcut . '"' : '') . '>' . icon($icon) . ' <span>' . e($label) . '</span></button></form>';
};
?>
<?= $view->insert('webmail/partials/list', ['list' => $list, 'currentFolder' => $currentFolder, 'q' => $q, 'activeUid' => (int) $m['uid']]) ?>
<article class="mail-reader" aria-label="Message">
  <div class="reader-actions">
    <a class="btn btn-sm btn-ghost mobile-only" href="<?= e($folderUrl . $qs) ?>" data-shortcut="back"><?= icon('arrow-left') ?> <span>Back</span></a>
    <a class="btn btn-sm btn-primary" href="<?= e($compose('reply')) ?>" data-shortcut="reply"><?= icon('reply') ?> <span>Reply</span></a>
    <a class="btn btn-sm" href="<?= e($compose('reply-all')) ?>" data-shortcut="reply-all"><?= icon('reply-all') ?> <span>Reply all</span></a>
    <a class="btn btn-sm" href="<?= e($compose('forward')) ?>" data-shortcut="forward"><?= icon('forward') ?> <span>Forward</span></a>
    <?php if ($roles['archive'] !== '' && $currentFolder !== $roles['archive']): ?><?= $action('archive', 'archive', 'Archive', 'archive') ?><?php endif; ?>
    <?= $inJunk ? $action('not_spam', 'inbox', 'Not spam') : $action('spam', 'alert-triangle', 'Spam') ?>
    <?= $action('unread', 'mail', 'Mark unread') ?>
    <?= $action('delete', 'trash', 'Delete', 'delete') ?>
    <span class="grow"></span>
    <?php if ($prevUid): ?><a class="btn btn-sm btn-ghost btn-icon" href="<?= e($folderUrl . '/message/' . $prevUid . $qs) ?>" aria-label="Newer message" data-shortcut="prev"><?= icon('chevron-up') ?></a><?php endif; ?>
    <?php if ($nextUid): ?><a class="btn btn-sm btn-ghost btn-icon" href="<?= e($folderUrl . '/message/' . $nextUid . $qs) ?>" aria-label="Older message" data-shortcut="next"><?= icon('chevron-down') ?></a><?php endif; ?>
  </div>
  <header class="reader-head">
    <h1><?= e($m['subject']) ?></h1>
    <div class="reader-meta">
      <span class="avatar"><?= e(initials($from['name'] ?: $from['email'])) ?></span>
      <div class="who">
        <strong><?= e($from['name'] ?: $from['email']) ?></strong>
        <span class="muted"><?= e($from['email']) ?></span>
        <div class="small text-2">To: <?= e($fmt($m['to'] ?? [])) ?><?= !empty($m['cc']) ? ' · Cc: ' . e($fmt($m['cc'])) : '' ?></div>
      </div>
      <span class="small muted nowrap"><?= e(datetime($m['date'] ?? null)) ?></span>
    </div>
  </header>
  <div class="reader-body">
    <?php if (!empty($m['spam_flag']) && stripos((string) $m['spam_flag'], 'yes') !== false && !$inJunk): ?>
      <div class="alert alert-warning mb-2"><?= icon('alert-triangle') ?><div>This message looks like spam. Do not open links or attachments unless you expected it.</div></div>
    <?php endif; ?>
    <?php if (!empty($m['has_remote_content']) && !$remote): ?>
      <div class="alert alert-info mb-2"><?= icon('eye-off') ?><div>Remote images are blocked to protect your privacy. <a href="<?= e($msgUrl . '?' . http_build_query(array_filter(['q' => $q ?: null, 'remote' => 1]))) ?>">Show images</a></div></div>
    <?php endif; ?>
    <iframe class="mail-frame" src="<?= e($bodyUrl) ?>" title="Message content" sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" loading="lazy"></iframe>
    <?php if (!empty($m['attachments'])): ?>
      <div class="attachments mt-2">
        <?php foreach ($m['attachments'] as $a): ?>
          <a class="attachment" href="<?= e($msgUrl . '/attachment/' . (int) $a['index']) ?>" download><?= icon('paperclip') ?> <span class="truncate"><?= e($a['filename']) ?></span> <span class="small muted"><?= bytes($a['size']) ?></span></a>
        <?php endforeach; ?>
      </div>
    <?php endif; ?>
  </div>
</article>
