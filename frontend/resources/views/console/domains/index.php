<?= $view->insert('partials/page-head', ['heading' => 'Domains', 'lede' => 'Authenticate the domains you send from' . (is_institution() ? ' and host work mailboxes on.' : '.')]) ?>
<?php if (can('domains:write')): ?>
  <form method="post" action="/console/domains" class="card card-body mb-3 inline-form">
    <?= csrf_field() ?>
    <div class="field grow mb-0"><label for="domain">Add a domain</label><input class="input" id="domain" name="domain" placeholder="example.com" value="<?= e(old('domain')) ?>" required<?= error_attrs('domain') ?>><?= error_text('domain') ?></div>
    <button class="btn btn-primary" type="submit"><?= icon('plus') ?> Add domain</button>
  </form>
<?php endif; ?>
<section class="card">
  <?php if ($domains === []): ?>
    <div class="card-body"><?= $view->insert('partials/empty', ['icon' => 'globe', 'heading' => 'No domains yet', 'text' => 'Add a domain you control. You will get the exact DNS records to publish.']) ?></div>
  <?php else: ?>
    <div class="table-wrap"><table class="table">
      <thead><tr><th>Domain</th><th>Status</th><th>SPF</th><th>DKIM</th><th>DMARC</th><?php if (is_institution()): ?><th>Mail hosting</th><?php endif; ?><th>Last check</th></tr></thead>
      <tbody>
        <?php foreach ($domains as $d): $c = $d['checks'] ?? []; ?>
          <tr>
            <td><a class="cell-title" href="/console/domains/<?= e($d['id']) ?>"><?= e($d['domain']) ?></a></td>
            <td><?= badge($d['status']) ?></td>
            <?php foreach (['spf', 'dkim', 'dmarc'] as $k): ?><td><?= ($c[$k] ?? $d[$k] ?? null) === true || (is_array($c[$k] ?? null) && !empty($c[$k]['ok'])) ? icon('check-circle', 'icon-sm', 'Found') : icon('minus', 'icon-sm', 'Not found') ?></td><?php endforeach; ?>
            <?php if (is_institution()): ?><td><?= !empty($d['mail_hosting_enabled']) ? badge('active', number($d['mailbox_count'] ?? 0) . ' mailboxes') : '<span class="muted small">Off</span>' ?></td><?php endif; ?>
            <td class="nowrap muted small"><?= time_tag($d['last_checked_at']) ?: 'Never' ?></td>
          </tr>
        <?php endforeach; ?>
      </tbody>
    </table></div>
  <?php endif; ?>
</section>
