<?= $view->insert('partials/page-head', ['heading' => 'Organizations', 'lede' => 'All tenant workspaces.']) ?>
<form class="toolbar mb-2" method="get"><input class="input" type="search" name="q" value="<?= e($q) ?>" placeholder="Name or slug" aria-label="Search" style="max-width:300px"><button class="btn" type="submit"><?= icon('search') ?> Search</button></form>
<section class="card">
  <div class="table-wrap"><table class="table">
    <thead><tr><th>Organization</th><th>Plan</th><th>Status</th><th class="num">Members</th><th class="num">Domains</th><th class="num">Mailboxes</th><th class="num">Sent 7 d</th><th class="num">Bounced</th><th>Created</th></tr></thead>
    <tbody><?php foreach ($orgs as $o): ?>
      <tr><td><a class="cell-title" href="/admin/organizations/<?= e($o['id']) ?>"><?= e($o['name']) ?></a><div class="cell-sub"><?= e($o['slug']) ?></div></td><td class="small"><?= e(is_array($o['plan']) ? ($o['plan']['name'] ?? '') : $o['plan']) ?></td><td><?= badge($o['status']) ?></td><td class="num"><?= number($o['members']) ?></td><td class="num"><?= number($o['domains']) ?></td><td class="num"><?= number($o['mailboxes'] ?? 0) ?></td><td class="num"><?= number($o['messages_7d']) ?></td><td class="num"><?= number($o['bounced_7d']) ?></td><td class="small muted nowrap"><?= e(date_short($o['created_at'])) ?></td></tr>
    <?php endforeach; ?></tbody>
  </table></div>
</section>
