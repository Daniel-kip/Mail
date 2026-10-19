<?php
/** @var array $plan @var string $currency */
$l = $plan['limits'];
$cycle = $cycle ?? 'monthly';
$base = $cycle === 'annual' ? $plan['price_annual_cents'] : $plan['price_monthly_cents'];
$seat = $cycle === 'annual' ? $plan['price_per_mailbox_annual_cents'] : $plan['price_per_mailbox_cents'];
$custom = !$plan['self_service'];
$institution = $plan['audience'] === 'institution';
$features = [];
$features[] = number($l['monthly_send_limit']) . ' emails / month (' . number($l['daily_send_limit']) . ' / day)';
$features[] = plural($l['max_domains'], 'sending domain');
if ($institution) {
    $features[] = 'Up to ' . plural($l['max_mailboxes'], 'work mailbox', 'work mailboxes') . ', ' . bytes($l['default_mailbox_quota_bytes']) . ' each';
}
$features[] = plural($l['max_members'], 'team member');
$features[] = plural($l['max_api_keys'], 'API key') . ', ' . plural($l['max_webhooks'], 'webhook');
$features[] = number($l['api_requests_per_minute']) . ' API requests / minute';
?>
<article class="card plan<?= !empty($featured) ? ' featured' : '' ?>">
  <?php if (!empty($featured)): ?><span class="ribbon">Most popular</span><?php endif; ?>
  <h3><?= e($plan['name']) ?></h3>
  <p class="muted small mb-0" style="min-height:2.6em"><?= e($plan['description']) ?></p>
  <?php if ($custom): ?>
    <div class="price">Custom</div>
    <div class="per-seat">Contracted limits and support</div>
  <?php elseif ($base === 0 && $seat === 0): ?>
    <div class="price">Free</div>
    <div class="per-seat">No card or invoice</div>
  <?php else: ?>
    <div class="price"><?= e(money($base, $currency)) ?><small> / <?= $cycle === 'annual' ? 'year' : 'month' ?></small></div>
    <div class="per-seat"><?= $seat > 0 ? '+ ' . e(money($seat, $currency)) . ' per mailbox' : '&nbsp;' ?></div>
  <?php endif; ?>
  <?php if (($plan['trial_days'] ?? 0) > 0 && !$custom): ?><p class="small mb-0"><span class="badge badge-info"><?= icon('sparkles', 'icon-xs') ?><span><?= e($plan['trial_days']) ?>-day free trial</span></span></p><?php endif; ?>
  <ul>
    <?php foreach ($features as $f): ?><li><?= icon('check', 'icon-sm') ?><span><?= e($f) ?></span></li><?php endforeach; ?>
  </ul>
  <?= $cta ?? '' ?>
</article>
