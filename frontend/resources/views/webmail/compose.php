<?php $source = $draft['source']; ?>
<main class="mail-page" id="main">
  <div class="content">
    <?= $view->insert('partials/page-head', ['heading' => $title]) ?>
    <form method="post" action="/mail/compose" enctype="multipart/form-data" class="card compose-form">
      <?= csrf_field() ?>
      <input type="hidden" name="in_reply_to" value="<?= e(old('in_reply_to', $draft['in_reply_to'])) ?>">
      <input type="hidden" name="references" value="<?= e(old('references', $draft['references'])) ?>">
      <?php if ($source): ?><input type="hidden" name="source_folder" value="<?= e($source['folder']) ?>"><input type="hidden" name="source_uid" value="<?= (int) $source['uid'] ?>"><?php endif; ?>
      <div class="card-body">
        <div class="field"><label for="to">To</label><input class="input" id="to" name="to" value="<?= e(old('to', $draft['to'])) ?>" list="recipient-options" data-suggest="/mail/suggest" autocomplete="off" multiple<?= error_attrs('to') ?><?= $draft['to'] === '' ? ' autofocus' : '' ?>><?= error_text('to') ?></div>
        <datalist id="recipient-options"></datalist>
        <details class="mb-2"<?= old('cc', $draft['cc']) !== '' || old('bcc') !== '' ? ' open' : '' ?>><summary class="small">Cc and Bcc</summary>
          <div class="grid grid-2 mt-1">
            <div class="field"><label for="cc">Cc</label><input class="input" id="cc" name="cc" value="<?= e(old('cc', $draft['cc'])) ?>" list="recipient-options" data-suggest="/mail/suggest" autocomplete="off"></div>
            <div class="field"><label for="bcc">Bcc</label><input class="input" id="bcc" name="bcc" value="<?= e(old('bcc')) ?>" list="recipient-options" data-suggest="/mail/suggest" autocomplete="off"></div>
          </div>
        </details>
        <div class="field"><label for="subject">Subject</label><input class="input" id="subject" name="subject" value="<?= e(old('subject', $draft['subject'])) ?>"></div>
        <div class="field"><label for="text">Message</label><textarea class="input" id="text" name="text" rows="16"<?= $draft['to'] !== '' ? ' autofocus' : '' ?>><?= e(old('text', $draft['text'])) ?></textarea></div>
        <?php if ($source && !empty($source['attachments'])): ?>
          <fieldset class="field" style="border:0;padding:0"><legend class="label">Include original attachments</legend>
            <?php foreach ($source['attachments'] as $a): ?><label class="check"><input type="checkbox" name="forward_attachments[]" value="<?= (int) $a['index'] ?>" checked> <?= e($a['filename']) ?> <span class="small muted"><?= bytes($a['size']) ?></span></label><?php endforeach; ?>
          </fieldset>
        <?php endif; ?>
        <div class="field"><label for="attachments">Attach files</label><input class="input" type="file" id="attachments" name="attachments[]" multiple<?= error_attrs('attachments') ?>><p class="hint">Up to 20 files, 20 MB in total.</p><?= error_text('attachments') ?></div>
      </div>
      <div class="card-foot form-actions">
        <button class="btn btn-primary" type="submit" name="intent" value="send"><?= icon('send') ?> Send</button>
        <button class="btn" type="submit" name="intent" value="draft" formnovalidate><?= icon('edit') ?> Save draft</button>
        <a class="btn btn-ghost" href="/mail">Discard</a>
      </div>
    </form>
  </div>
</main>
