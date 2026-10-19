<main class="mail-page" id="main">
  <div class="content">
    <?= $view->insert('partials/page-head', ['heading' => 'Directory', 'lede' => 'Everyone with a mailbox in ' . (($mail['mailbox']['organization'] ?? '') ?: 'your organization') . ', plus shared addresses.']) ?>
    <form method="get" class="toolbar mb-2"><input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Name, department or job title" aria-label="Search the directory" style="max-width:340px"><button class="btn" type="submit"><?= icon('search') ?> Search</button></form>
    <section class="card mb-3">
      <?php if ($people === []): ?><div class="card-body"><p class="muted mb-0">No one matches.</p></div><?php else: ?>
      <div class="table-wrap"><table class="table"><thead><tr><th>Name</th><th>Department</th><th>Email</th><th></th></tr></thead><tbody>
        <?php foreach ($people as $p): ?><tr><td><span class="cell-title"><?= e($p['name'] ?: $p['address']) ?></span><?= !empty($p['job_title']) ? '<div class="cell-sub">' . e($p['job_title']) . '</div>' : '' ?></td><td class="small"><?= e($p['department'] ?? '') ?></td><td class="small"><?= e($p['address']) ?></td><td><a class="btn btn-sm" href="/mail/compose?to=<?= e(rawurlencode($p['address'])) ?>"><?= icon('send') ?> Write</a></td></tr><?php endforeach; ?>
      </tbody></table></div>
      <?php endif; ?>
    </section>
    <?php if ($groups !== []): ?>
    <section class="card"><div class="card-head"><h2>Shared addresses</h2></div><div class="table-wrap"><table class="table table-compact"><tbody>
      <?php foreach ($groups as $g): $addr = is_array($g) ? ($g['address'] ?? '') : (string) $g; ?><tr><td><?= e($addr) ?></td><td><a class="btn btn-sm" href="/mail/compose?to=<?= e(rawurlencode($addr)) ?>"><?= icon('send') ?> Write</a></td></tr><?php endforeach; ?>
    </tbody></table></div></section>
    <?php endif; ?>
  </div>
</main>
