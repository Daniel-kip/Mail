<?php /** Public documentation landing: the customer guides, grouped as in the wiki sidebar. */ ?>
<section class="hero hero-dark hero-compact">
  <div class="container" style="text-align:center">
    <span class="eyebrow eyebrow-glow"><?= icon('book', 'icon-sm') ?> Documentation</span>
    <h1>Guides for every <em>role</em></h1>
    <p class="lead" style="margin-inline:auto">Set up your domain, send from your app, connect phones, pay with M-Pesa and help your colleagues, step by step.</p>
  </div>
</section>
<section class="section">
  <div class="container">
    <?php foreach ($sections as $section): ?>
      <h2 class="docs-group"><?= e($section['title']) ?></h2>
      <div class="docs-cards">
        <?php foreach ($section['pages'] as $p): ?>
          <a class="docs-card" href="<?= e($base . '/' . $p['slug']) ?>">
            <strong><?= e($p['title']) ?></strong>
            <span><?= e($p['lead']) ?></span>
            <span class="docs-card-go">Read <?= icon('arrow-right', 'icon-xs') ?></span>
          </a>
        <?php endforeach; ?>
      </div>
    <?php endforeach; ?>
    <div class="cta-band mt-3">
      <div>
        <h2>Building an integration?</h2>
        <p>The API quickstart has copy-ready examples in Node, Python and PHP.</p>
      </div>
      <a class="btn btn-primary btn-lg" href="/developers">API quickstart <?= icon('arrow-right') ?></a>
    </div>
  </div>
</section>
