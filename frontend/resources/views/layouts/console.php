<?php
$noindex = true;
$auth = $auth ?? [];
$user = $auth['user'] ?? [];
$org = $auth['organization'] ?? null;
$area = $area ?? 'console';
$nav = (require app()->root . '/config/navigation.php')[$area];
$permissions = $org['permissions'] ?? [];
$institution = ($org['account_type'] ?? 'institution') === 'institution';
$planNames = ['free' => 'Free', 'starter' => 'Legacy', 'pro' => 'Premium', 'enterprise' => 'Enterprise',
    'developer' => 'Free', 'developer_plus' => 'Starter', 'developer_scale' => 'Standard'];
?>
<?= $view->insert('partials/document-start', get_defined_vars()) ?>
<div class="shell">
  <aside class="sidebar" aria-label="Sidebar">
    <?= $view->insert('partials/brand', ['href' => $area === 'admin' ? '/admin' : '/console', 'subtitle' => $area === 'admin' ? 'OPERATOR CONSOLE' : 'EMAIL PLATFORM']) ?>
    <?php if ($area === 'console' && $org): ?>
      <details class="dropdown org-switch">
        <summary aria-label="Switch organization">
          <span class="avatar avatar-sm"><?= e(initials($org['name'])) ?></span>
          <span class="org-meta">
            <span class="org-name"><?= e($org['name']) ?></span>
            <span class="org-plan"><?= e(($institution ? 'Institution' : 'Individual') . ' · ' . ($planNames[$org['plan']] ?? $org['plan'])) ?></span>
          </span>
          <?= icon('chevron-down', 'icon-sm') ?>
        </summary>
        <div class="dropdown-menu">
          <div class="head">Switch workspace</div>
          <?php foreach ($auth['organizations'] ?? [] as $o): ?>
            <form method="post" action="/console/switch">
              <?= csrf_field() ?>
              <input type="hidden" name="organization_id" value="<?= e($o['id']) ?>">
              <button type="submit"<?= $o['id'] === $org['id'] ? ' aria-current="true"' : '' ?>>
                <span class="avatar avatar-sm"><?= e(initials($o['name'])) ?></span>
                <span class="grow truncate"><?= e($o['name']) ?><br><span class="muted small"><?= e(ucfirst($o['role'])) ?> · <?= e($o['account_type'] === 'individual' ? 'Individual' : 'Institution') ?></span></span>
                <?php if ($o['id'] === $org['id']): ?><?= icon('check', 'icon-sm') ?><?php endif; ?>
              </button>
            </form>
          <?php endforeach; ?>
          <div class="divider"></div>
          <a href="/console/organizations/new"><?= icon('plus') ?> New workspace</a>
        </div>
      </details>
    <?php endif; ?>
    <nav aria-label="<?= $area === 'admin' ? 'Operator' : 'Console' ?>">
      <?php foreach ($nav as $group): ?>
        <?php if (!empty($group['institution']) && !$institution) {
            continue;
        } ?>
        <?php $items = array_filter($group['items'], static fn ($i) => empty($i['can']) || in_array($i['can'], $permissions, true) || $area === 'admin'); ?>
        <?php if ($items === []) {
            continue;
        } ?>
        <div class="nav-group">
          <?php if ($group['label']): ?><div class="nav-label"><?= e($group['label']) ?></div><?php endif; ?>
          <?php foreach ($items as $item): ?>
            <a class="nav-link<?= nav_active($item[1], !empty($item['exact'])) ?>" href="<?= e($item[1]) ?>"><?= icon($item[2]) ?><span><?= e($item[0]) ?></span></a>
          <?php endforeach; ?>
        </div>
      <?php endforeach; ?>
      <?php if ($area === 'console' && !$institution): ?>
        <div class="nav-group">
          <div class="nav-label">Work email</div>
          <a class="nav-link<?= nav_active('/console/mailboxes') ?>" href="/console/mailboxes/upgrade"><?= icon('inbox') ?><span>Hosted mailboxes</span><span class="count">Institution</span></a>
        </div>
      <?php endif; ?>
    </nav>
    <div class="sidebar-foot">
      <?php if ($area === 'admin'): ?>
        <a class="nav-link" href="/console"><?= icon('arrow-left') ?><span>Back to console</span></a>
      <?php elseif (!empty($user['is_platform_admin'])): ?>
        <a class="nav-link<?= nav_active('/admin') ?>" href="/admin"><?= icon('shield') ?><span>Operator console</span></a>
      <?php endif; ?>
      <a class="nav-link" href="/mail" ><?= icon('mail') ?><span>Open webmail</span></a>
      <?php if ($area === 'console' && $org && in_array($org['plan'] ?? '', ['free', 'developer'], true) && in_array('billing:read', $permissions, true)): ?>
        <a class="upgrade-cta" href="/console/billing/plans"><?= icon('sparkles', 'icon-sm') ?><span><b>Upgrade plan</b><small><?= $institution ? 'Premium: KES 1,200/month' : 'From KES 120/month' ?></small></span></a>
      <?php endif; ?>
    </div>
  </aside>
  <div class="main">
    <header class="topbar">
      <button type="button" class="btn btn-ghost btn-icon menu-toggle" data-nav-toggle aria-label="Open menu" aria-expanded="false"><?= icon('menu') ?></button>
      <nav class="crumbs truncate" aria-label="Breadcrumb">
        <?php foreach ($crumbs ?? [] as $i => $crumb): ?>
          <?php if ($i > 0): ?><?= icon('chevron-right', 'icon-xs') ?><?php endif; ?>
          <?php if (!empty($crumb[1])): ?><a href="<?= e($crumb[1]) ?>"><?= e($crumb[0]) ?></a><?php else: ?><span><?= e($crumb[0]) ?></span><?php endif; ?>
        <?php endforeach; ?>
      </nav>
      <span class="spacer"></span>
      <?php if (($auth['environment'] ?? '') === 'development'): ?><span class="env-pill" title="Development mode: mail is caught locally">Dev</span><?php endif; ?>
      <?= $view->insert('partials/theme-switch') ?>
      <a class="btn btn-ghost btn-icon" href="/tools" title="Deliverability tools" aria-label="Deliverability tools"><?= icon('search') ?></a>
      <details class="dropdown">
        <summary class="btn btn-ghost" aria-label="Account menu">
          <span class="avatar avatar-sm"><?= e(initials($user['full_name'] ?? $user['email'] ?? '')) ?></span>
          <span class="truncate" style="max-width:160px"><?= e($user['full_name'] ?? '') ?></span>
          <?= icon('chevron-down', 'icon-sm') ?>
        </summary>
        <div class="dropdown-menu">
          <div class="head"><?= e($user['email'] ?? '') ?><?php if ($org): ?><br><?= e(ucfirst($org['role'])) ?> at <?= e($org['name']) ?><?php endif; ?></div>
          <a href="/console/account"><?= icon('user') ?> Account &amp; security</a>
          <?php if (!empty($user['is_platform_admin']) && $area !== 'admin'): ?><a href="/admin"><?= icon('shield') ?> Operator console</a><?php endif; ?>
          <a href="/developers"><?= icon('book') ?> API reference</a>
          <div class="divider"></div>
          <form method="post" action="/logout"><?= csrf_field() ?><button type="submit"><?= icon('log-out') ?> Sign out</button></form>
        </div>
      </details>
    </header>
    <?php if ($area === 'console'): ?>
      <?php if ($user && empty($user['email_verified'])): ?>
        <div class="banner banner-warning" role="status">
          <?= icon(!empty($user['phone']) ? 'smartphone-check' : 'mail') ?>
          <?php if (!empty($user['phone'])): ?>
            <span class="grow">Verify your account to add domains and create live API keys. We text a six-digit code to <strong><?= e($user['phone']) ?></strong>.</span>
            <a class="btn btn-sm btn-primary" href="/console/verify-phone">Verify phone</a>
          <?php else: ?>
            <span class="grow">Verify your account to add domains and create live API keys: confirm the link sent to <strong><?= e($user['email']) ?></strong>, or add a phone number.</span>
            <form method="post" action="/console/account/email/resend" class="inline-form"><?= csrf_field() ?><button class="btn btn-sm" type="submit">Resend link</button></form>
            <a class="btn btn-sm" href="/console/verify-phone">Use my phone</a>
          <?php endif; ?>
        </div>
      <?php endif; ?>
      <?php if ($org && ($org['status'] ?? '') === 'suspended'): ?>
        <div class="banner banner-danger" role="alert"><?= icon('ban') ?><span>This organization is suspended. Sending, API keys and mailboxes are stopped. Contact platform support to restore service.</span></div>
      <?php endif; ?>
    <?php endif; ?>
    <main id="main" class="content<?= !empty($narrow) ? ' content-narrow' : '' ?>">
      <?= $content ?>
    </main>
  </div>
</div>
<?= $view->insert('partials/help-widget', get_defined_vars()) ?>
<?= $view->insert('partials/document-end', get_defined_vars()) ?>
