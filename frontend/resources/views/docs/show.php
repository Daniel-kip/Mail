<?php
/** One wiki page: sidebar, article, "On this page". Shared by /docs and /admin/docs. */
$operator = !empty($operator);
?>
<div class="<?= $operator ? '' : 'container section-tight ' ?>docs-shell">
  <aside class="docs-nav" aria-label="Documentation">
    <a class="docs-nav-home<?= $slug === 'home' ? ' is-active' : '' ?>" href="<?= e($base) ?>"><?= icon('book', 'icon-sm') ?> <?= $operator ? 'Wiki home' : 'All guides' ?></a>
    <?php foreach ($sections as $section): ?>
      <p class="docs-nav-title"><?= e($section['title']) ?></p>
      <?php foreach ($section['pages'] as $p): ?>
        <a href="<?= e($base . '/' . $p['slug']) ?>"<?= $p['slug'] === $slug ? ' class="is-active" aria-current="page"' : '' ?>><?= e($p['title']) ?></a>
      <?php endforeach; ?>
    <?php endforeach; ?>
  </aside>
  <article class="docs-article">
    <?php if ($slug !== 'home'): ?>
      <nav class="docs-crumbs" aria-label="Breadcrumb"><a href="<?= e($base) ?>">Documentation</a> <?= icon('chevron-right', 'icon-xs') ?> <span><?= e($title) ?></span></nav>
    <?php endif; ?>
    <h1><?= e($title) ?></h1>
    <?php if ($operator && $slug === 'home'): ?>
      <a class="docs-download" href="/admin/infrastructure"><?= icon('file-text') ?><span><b>System infrastructure (PDF)</b><small>Architecture, system flows and security flows with vector diagrams</small></span><?= icon('arrow-right', 'icon-sm') ?></a>
    <?php endif; ?>
    <div class="docs-body"><?= $page['html'] ?></div>
    <?php if (trim(strip_tags($page['footer'])) !== ''): ?><footer class="docs-footer"><?= $page['footer'] ?></footer><?php endif; ?>
  </article>
  <?php if (count($page['toc']) > 2): ?>
    <nav class="docs-toc" aria-label="On this page">
      <p class="docs-nav-title">On this page</p>
      <?php foreach ($page['toc'] as $h): ?><a href="#<?= e($h['id']) ?>"><?= e($h['text']) ?></a><?php endforeach; ?>
    </nav>
  <?php endif; ?>
</div>
