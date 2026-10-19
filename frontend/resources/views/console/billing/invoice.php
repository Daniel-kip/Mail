<?php if (($mpesa ?? false) && ($canPay ?? false) && $invoice['status'] === 'open'): ?>
<section class="card pay-panel no-print mb-3">
  <form method="post" action="/console/billing/invoices/<?= e($invoice['id']) ?>/pay" class="card-body">
    <?= csrf_field() ?>
    <div class="row-between">
      <div><h2 class="mb-0"><?= icon('smartphone-check') ?> Pay <?= e(money($invoice['total_cents'], $invoice['currency'])) ?> with M-Pesa</h2>
        <p class="small text-2 mb-0">We send a prompt to your phone. Enter your M-Pesa PIN to approve; the invoice is marked paid instantly.</p></div>
      <span class="mpesa-badge">M-PESA</span>
    </div>
    <div class="input-group mt-2">
      <input class="input input-lg" type="tel" name="phone" value="<?= e(old('phone', $payerPhone ?? '')) ?>" placeholder="0712 345 678" inputmode="tel" autocomplete="tel" required aria-label="Safaricom number"<?= error_attrs('phone') ?>>
      <button class="btn btn-primary btn-lg" type="submit">Send prompt</button>
    </div>
    <?= error_text('phone') ?>
    <p class="small muted mb-0 mt-1">Payments are processed by PesaTide. Prefer another method? Use the instructions on the invoice below.</p>
  </form>
</section>
<?php endif; ?>
<?php
$i = $invoice;
$bill = is_array($i['billing'] ?? null) ? $i['billing'] : [];
$issuer = $bill['issuer'] ?? $config->get('company.name');
$cur = $i['currency'];
?>
<article class="invoice card card-body">
  <header class="invoice-head row-between">
    <div><?= $view->insert('partials/brand', ['href' => '#']) ?><p class="small text-2 mt-1 mb-0" style="white-space:pre-line"><?= e(is_array($issuer) ? implode("\n", array_filter($issuer)) : (string) $issuer) ?><?= $config->get('company.address') ? "\n" . e($config->get('company.address')) : '' ?></p></div>
    <div style="text-align:right">
      <h1 style="margin:0">Invoice</h1>
      <p class="mb-0"><strong><?= e($i['number']) ?></strong></p>
      <p class="mb-0"><?= badge(!empty($i['overdue']) ? 'overdue' : $i['status'], !empty($i['overdue']) ? 'Overdue' : null) ?></p>
    </div>
  </header>
  <div class="grid grid-2 mt-3">
    <div><div class="stat-label">Billed to</div><p style="white-space:pre-line" class="mb-0"><?= e(implode("\n", array_filter([$bill['billing_name'] ?? $bill['organization'] ?? '', $bill['billing_address'] ?? '', isset($bill['tax_id']) ? 'Tax ID: ' . $bill['tax_id'] : '', $bill['billing_email'] ?? '']))) ?></p></div>
    <div><dl class="dl"><dt>Issued</dt><dd><?= e(date_short($i['issued_at'])) ?></dd><dt>Due</dt><dd><?= e(date_short($i['due_at'])) ?></dd><dt>Period</dt><dd><?= e(date_short($i['period_start'])) ?> – <?= e(date_short($i['period_end'])) ?></dd><?php if ($i['paid_at']): ?><dt>Paid</dt><dd><?= e(date_short($i['paid_at'])) ?><?= !empty($i['payment_method']) ? ' · ' . e($i['payment_method']) : '' ?><?= !empty($i['payment_reference']) ? ' (' . e($i['payment_reference']) . ')' : '' ?></dd><?php endif; ?></dl></div>
  </div>
  <table class="table mt-3">
    <thead><tr><th>Description</th><th class="num">Qty</th><th class="num">Unit price</th><th class="num">Amount</th></tr></thead>
    <tbody><?php foreach ($i['lines'] as $l): ?><tr><td><?= e($l['description']) ?></td><td class="num"><?= number($l['quantity']) ?></td><td class="num"><?= e(money($l['unit_cents'], $cur)) ?></td><td class="num"><?= e(money($l['amount_cents'] ?? $l['quantity'] * $l['unit_cents'], $cur)) ?></td></tr><?php endforeach; ?></tbody>
    <tfoot class="invoice-total">
      <tr><td colspan="3" class="num">Subtotal</td><td class="num"><?= e(money($i['subtotal_cents'], $cur)) ?></td></tr>
      <tr><td colspan="3" class="num"><?= e($bill['tax_label'] ?? 'Tax') ?> (<?= e(rtrim(rtrim(number_format($i['tax_rate_bps'] / 100, 2), '0'), '.')) ?>%)</td><td class="num"><?= e(money($i['tax_cents'], $cur)) ?></td></tr>
      <tr><td colspan="3" class="num"><strong>Total</strong></td><td class="num"><strong><?= e(money($i['total_cents'], $cur)) ?></strong></td></tr>
    </tfoot>
  </table>
  <?php if ($i['status'] === 'open' && !empty($bill['payment_instructions'])): ?><section class="callout mt-3"><strong>How to pay</strong><p class="mb-0" style="white-space:pre-line"><?= e($bill['payment_instructions']) ?>
Reference: <?= e($i['number']) ?></p></section><?php endif; ?>
  <?php if ($i['status'] === 'void'): ?><p class="alert alert-info mt-3">This invoice was voided and is not payable.</p><?php endif; ?>
</article>
