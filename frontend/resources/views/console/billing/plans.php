<?php
$current = $b['subscription']['plan']['id'];
$cur = $catalogue['currency'] ?? $b['currency'];
?>
<?= $view->insert('partials/page-head', ['heading' => 'Choose a plan', 'lede' => 'Upgrades apply immediately with a prorated invoice. Downgrades apply at the end of the paid period.']) ?>
<div style="text-align:center" class="mb-3">
  <div class="plan-toggle" role="tablist" aria-label="Billing cycle">
    <a role="tab" href="?cycle=monthly" class="<?= $cycle === 'monthly' ? 'is-active' : '' ?>" aria-selected="<?= $cycle === 'monthly' ? 'true' : 'false' ?>">Monthly</a>
    <a role="tab" href="?cycle=annual" class="<?= $cycle === 'annual' ? 'is-active' : '' ?>" aria-selected="<?= $cycle === 'annual' ? 'true' : 'false' ?>">Annual (save up to <?= e(max(array_merge([0], array_map(static fn ($p) => $p['annual_discount_percent'], $catalogue['data'] ?? [])))) ?>%)</a>
  </div>
</div>
<div class="plans">
  <?php foreach ($catalogue['data'] ?? [] as $plan): ?>
    <?php
    $isCurrent = $plan['id'] === $current && $b['subscription']['billing_cycle'] === $cycle;
    if (!$plan['self_service']) {
        $cta = '<a class="btn btn-block" href="mailto:' . e($config->get('support_email')) . '">Contact sales</a>';
    } elseif ($isCurrent) {
        $cta = '<button class="btn btn-block" type="button" disabled>Current plan</button>';
    } elseif (!can('billing:manage')) {
        $cta = '<p class="small muted mb-0">Ask an owner or admin to change the plan.</p>';
    } else {
        $cta = '<form method="post" action="/console/billing/plan">' . csrf_field() . '<input type="hidden" name="plan_id" value="' . e($plan['id']) . '"><input type="hidden" name="billing_cycle" value="' . e($cycle) . '"><button class="btn btn-block' . ($plan['id'] === $highlight ? ' btn-primary' : '') . '" type="submit">' . ($plan['id'] === $current ? 'Switch to ' . e($cycle) : 'Choose ' . e($plan['name'])) . '</button></form>';
    }
    ?>
    <?= $view->insert('partials/plan-card', ['plan' => $plan, 'currency' => $cur, 'cycle' => $cycle, 'featured' => $plan['id'] === ($highlight ?: $current), 'cta' => $cta]) ?>
  <?php endforeach; ?>
</div>
<?php if (($catalogue['tax']['rate_bps'] ?? 0) > 0): ?><p class="small muted mt-3" style="text-align:center">Prices exclude <?= e($catalogue['tax']['label']) ?> at <?= e(rtrim(rtrim(number_format($catalogue['tax']['rate_bps'] / 100, 2), '0'), '.')) ?>%.</p><?php endif; ?>
