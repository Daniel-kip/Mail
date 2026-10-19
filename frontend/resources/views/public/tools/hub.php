<section class="hero hero-dark hero-compact tools-hero">
  <div class="container" style="text-align:center">
    <span class="eyebrow eyebrow-glow"><?= icon('zap', 'icon-sm') ?> Free · instant · no account needed</span>
    <h1>Email &amp; DNS <em>power tools</em></h1>
    <p class="lead" style="margin-inline:auto">Professional diagnostics for developers, sysadmins and ICT teams. Every check follows the published standards, and results come back in seconds.</p>
    <div class="tools-badges"><span><?= icon('check', 'icon-xs') ?> <?= count($tools) ?> tools</span><span><?= icon('check', 'icon-xs') ?> RFC-accurate checks</span><span><?= icon('check', 'icon-xs') ?> Nothing stored</span></div>
  </div>
</section>
<section class="section tools-section">
  <div class="container">
    <div class="tools-search">
      <?= icon('search') ?>
      <input class="input input-lg" type="search" placeholder="Search tools: spf, mx, headers, blacklist, ports…" aria-label="Search tools" data-tool-filter autocomplete="off">
    </div>
    <div class="tools-grid">
      <?php foreach ($tools as $slug => [$ic, $name, $cat, $summary, $keywords, $tone]): ?>
        <a class="tool-card tone-<?= e($tone) ?>" href="/tools/<?= e($slug) ?>" data-tool-card="<?= e($slug . ' ' . $keywords) ?>">
          <span class="tool-icon"><?= icon($ic) ?></span>
          <span class="tool-cat"><?= e($cat) ?></span>
          <strong><?= e($name) ?></strong>
          <span class="tool-sum"><?= e($summary) ?></span>
          <span class="tool-go">Open tool <?= icon('arrow-right', 'icon-xs') ?></span>
        </a>
      <?php endforeach; ?>
    </div>
    <p class="muted" data-tool-empty hidden style="text-align:center">No tool matches that search.</p>
    <div class="cta-band mt-3">
      <div>
        <h2>Need professional email for your domain?</h2>
        <p>Flat monthly pricing in Kenyan shillings: from KES 120 for apps, KES 1,200 for a whole institution. Pay with M-Pesa.</p>
      </div>
      <a class="btn btn-primary btn-lg" href="/pricing">View plans <?= icon('arrow-right') ?></a>
    </div>
  </div>
</section>
