<?php
$currency = $catalogue['currency'] ?? 'KES';
$tax = $catalogue['tax'] ?? ['rate_bps' => 0, 'label' => 'Tax'];
$list = array_values(array_filter($catalogue['data'] ?? [], static fn ($p) => $p['audience'] === $audience));
?>
<section class="section">
  <div class="container">
    <div class="section-head">
      <h1 style="font-size:clamp(1.9rem,4vw,2.6rem);letter-spacing:-.02em">Plans that grow with you</h1>
      <p>Pick the account type that fits. Individual developers relay mail from their applications; institutions also host work mailboxes for their people.</p>
    </div>
    <div style="text-align:center">
      <div class="plan-toggle" role="tablist" aria-label="Account type">
        <a role="tab" href="/pricing" class="<?= $audience === 'institution' ? 'is-active' : '' ?>" aria-selected="<?= $audience === 'institution' ? 'true' : 'false' ?>">Institutions</a>
        <a role="tab" href="/pricing?for=developers" class="<?= $audience === 'individual' ? 'is-active' : '' ?>" aria-selected="<?= $audience === 'individual' ? 'true' : 'false' ?>">Individual developers</a>
      </div>
    </div>
    <?php if ($list === []): ?>
      <div class="card"><?= $view->insert('partials/empty', ['icon' => 'server', 'heading' => 'Prices are temporarily unavailable', 'text' => 'The plan catalogue could not be loaded. Please refresh in a moment.']) ?></div>
    <?php else: ?>
      <div class="plans">
        <?php foreach ($list as $i => $plan): ?>
          <?php
          $featured = $audience === 'institution' ? $plan['id'] === 'pro' : $plan['id'] === 'developer_scale';
          $type = $audience === 'institution' ? 'institution' : 'individual';
          $cta = $plan['self_service']
              ? '<a class="btn ' . ($featured ? 'btn-primary' : '') . ' btn-block" href="/register?type=' . $type . '&amp;plan=' . e($plan['id']) . '">' . ($plan['price_monthly_cents'] > 0 ? 'Start with ' . e($plan['name']) : 'Create free account') . '</a>'
              : '<a class="btn btn-block" href="' . e($config->get('support_email') ? 'mailto:' . $config->get('support_email') : '/security') . '">Contact us</a>';
          ?>
          <?= $view->insert('partials/plan-card', ['plan' => $plan, 'currency' => $currency, 'featured' => $featured, 'cta' => $cta]) ?>
        <?php endforeach; ?>
      </div>
      <p class="small muted mt-3" style="text-align:center">
        Prices in <?= e($currency) ?><?= $tax['rate_bps'] > 0 ? ', excluding ' . e($tax['label']) . ' (' . e(rtrim(rtrim(number_format($tax['rate_bps'] / 100, 2), '0'), '.')) . '%)' : '' ?>.
        Annual billing saves up to <?= e(max(array_map(static fn ($p) => $p['annual_discount_percent'], $list))) ?>%. Change or cancel your plan at any time from the console.
      </p>
    <?php endif; ?>
  </div>
</section>
<section class="section section-alt">
  <div class="container" style="max-width:860px">
    <h2>Frequently asked questions</h2>
    <?php foreach ([
        ['What is the difference between an individual and an institutional account?', 'An individual (developer) account relays transactional email from your applications: invoices, receipts, password resets and notifications. An institutional account adds hosted work mailboxes with webmail and phone setup for your staff or students, shared addresses, and roles such as ICT support. You can convert an individual workspace to an institution at any time.'],
        ['How are plans billed?', 'One flat monthly price in Kenyan shillings, however many mailboxes or messages you use within the plan limits: Starter KES 120 and Standard KES 200 for apps, Premium KES 1,200 for a whole institution. Upgrades take effect immediately with a prorated invoice; downgrades take effect at the end of the paid period.'],
        ['How do I pay?', 'With M-Pesa: open the invoice in the console, enter your Safaricom number and approve the prompt on your phone. The invoice is marked paid the moment M-Pesa confirms, and you get the receipt number on screen. Payments are processed by PesaTide.'],
        ['What happens if I exceed a limit?', 'Sending pauses with a clear quota error until the next day or month, or until you upgrade. Nothing already accepted is lost.'],
        ['Is there a free trial of paid institution plans?', 'Yes. Paid institution plans include a 14-day free trial the first time you choose one. No invoice is issued until the trial ends.'],
    ] as [$q, $a]): ?>
      <details class="cant-find" style="margin-top:.75rem">
        <summary><?= icon('help') ?> <?= e($q) ?></summary>
        <div><?= e($a) ?></div>
      </details>
    <?php endforeach; ?>
  </div>
</section>
