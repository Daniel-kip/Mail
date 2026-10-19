<?php $content = $m['content'] ?? []; ?>
<?= $view->insert('partials/page-head', [
    'heading' => $m['subject'] ?: '(no subject)',
    'lede' => 'Message ' . $m['id'],
    'actions' => $m['status'] === 'scheduled' && can('messages:send') ? '<form method="post" action="/console/messages/' . e($m['id']) . '/cancel" data-confirm="Cancel this scheduled message?" data-confirm-label="Cancel message">' . csrf_field() . '<button class="btn btn-danger" type="submit">' . icon('x') . ' Cancel scheduled send</button></form>' : '',
]) ?>
<div class="grid grid-2" style="grid-template-columns:1fr 1.3fr;align-items:start">
  <div class="stack">
    <section class="card"><div class="card-head"><h2>Details</h2><?= badge($m['status']) ?></div><div class="card-body"><dl class="dl">
      <dt>From</dt><dd><?= e(trim(($m['from_name'] ? $m['from_name'] . ' ' : '') . '<' . $m['from'] . '>')) ?></dd>
      <dt>To</dt><dd><?= e(implode(', ', (array) $m['to'])) ?></dd>
      <?php if ($m['reply_to']): ?><dt>Reply-To</dt><dd><?= e($m['reply_to']) ?></dd><?php endif; ?>
      <dt>Source</dt><dd><?= e(\App\View\Format::statusLabel((string) $m['source'])) ?><?= $m['sandbox'] ? ' (test, not delivered)' : '' ?></dd>
      <?php if ($m['template_id']): ?><dt>Template</dt><dd><a href="/console/templates/<?= e($m['template_id']) ?>">v<?= e($m['template_version']) ?></a></dd><?php endif; ?>
      <?php if ($m['scheduled_at']): ?><dt>Scheduled for</dt><dd><?= e(datetime($m['scheduled_at'])) ?></dd><?php endif; ?>
      <dt>Attempts</dt><dd><?= number($m['attempts']) ?></dd>
      <?php if ($m['last_error']): ?><dt>Last error</dt><dd class="small"><?= e(($m['last_smtp_code'] ? $m['last_smtp_code'] . ' ' : '') . $m['last_error']) ?></dd><?php endif; ?>
      <?php if ($m['smtp_message_id']): ?><dt>Message-ID</dt><dd class="small break"><code><?= e($m['smtp_message_id']) ?></code></dd><?php endif; ?>
      <dt>Size</dt><dd><?= bytes($m['size_bytes']) ?></dd>
      <dt>One-click unsubscribe</dt><dd><?= $m['list_unsubscribe'] ? 'Included (RFC 8058)' : 'No' ?></dd>
      <?php if ($m['tags']): ?><dt>Tags</dt><dd><?php foreach ($m['tags'] as $t): ?><span class="tag"><?= e($t) ?></span> <?php endforeach; ?></dd><?php endif; ?>
      <?php if ($m['metadata']): ?><dt>Metadata</dt><dd><pre class="small"><?= e(json_encode($m['metadata'], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES)) ?></pre></dd><?php endif; ?>
    </dl></div></section>
    <section class="card"><div class="card-head"><h2>Timeline</h2></div>
      <ol class="timeline card-body">
        <li><span class="dot"></span><strong>Created</strong> <span class="muted small"><?= e(datetime($m['created_at'])) ?></span></li>
        <?php foreach ($m['events'] ?? [] as $ev): ?>
          <li><span class="dot"></span><strong><?= e(\App\View\Format::statusLabel((string) $ev['type'])) ?></strong> <span class="muted small"><?= e(datetime($ev['occurred_at'])) ?></span>
            <?php if (!empty($ev['details'])): ?><div class="small text-2 break"><?= e(is_array($ev['details']) ? implode(' · ', array_map(static fn ($k, $v) => $k . ': ' . (is_scalar($v) ? $v : json_encode($v)), array_keys($ev['details']), $ev['details'])) : (string) $ev['details']) ?></div><?php endif; ?></li>
        <?php endforeach; ?>
      </ol>
    </section>
  </div>
  <section class="card">
    <div class="card-head"><h2>Content</h2></div>
    <div class="card-body">
      <?php if (!empty($content['purged_at'])): ?><p class="muted">Content was removed on <?= e(date_short($content['purged_at'])) ?> under the retention policy. Delivery records are kept.</p>
      <?php elseif (!empty($content['html'])): ?><iframe class="preview-frame" title="Message content" sandbox="" srcdoc="<?= e($content['html']) ?>"></iframe>
        <?php if (!empty($content['text'])): ?><details class="mt-2"><summary class="small">Plain-text version</summary><pre class="small"><?= e($content['text']) ?></pre></details><?php endif; ?>
      <?php elseif (!empty($content['text'])): ?><pre class="mail-text"><?= e($content['text']) ?></pre>
      <?php else: ?><p class="muted">No content stored.</p><?php endif; ?>
    </div>
  </section>
</div>
