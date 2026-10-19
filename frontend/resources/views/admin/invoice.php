<?= $view->insert('partials/page-head', ['heading' => 'Invoice ' . $invoice['number'], 'lede' => $invoice['organization'] ?? ($invoice['billing']['organization'] ?? '')]) ?>
<div class="grid grid-2" style="grid-template-columns:2fr 1fr;align-items:start">
  <?= $view->insert('console/billing/invoice', ['invoice' => $invoice, 'config' => $config]) ?>
  <div class="stack">
    <?php if ($invoice['status'] === 'open'): ?>
    <section class="card"><div class="card-head"><h2>Record payment</h2></div>
      <form method="post" action="/admin/billing/invoices/<?= e($invoice['id']) ?>/paid" class="card-body"><?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'method', 'label' => 'Method', 'type' => 'select', 'required' => true, 'options' => ['bank_transfer' => 'Bank transfer', 'mobile_money' => 'Mobile money (M-Pesa)', 'card' => 'Card', 'cash' => 'Cash', 'other' => 'Other']]) ?>
        <?= $view->insert('partials/field', ['name' => 'reference', 'label' => 'Transaction reference', 'placeholder' => 'QJK3XYZ12']) ?>
        <?= $view->insert('partials/field', ['name' => 'paid_at', 'label' => 'Date received', 'type' => 'date', 'hint' => 'Defaults to now.']) ?>
        <button class="btn btn-primary" type="submit">Mark as paid</button>
      </form>
    </section>
    <section class="card card-danger"><div class="card-head"><h2>Void</h2></div>
      <form method="post" action="/admin/billing/invoices/<?= e($invoice['id']) ?>/void" class="card-body" data-confirm="Void this invoice? It can no longer be paid." data-confirm-label="Void"><?= csrf_field() ?>
        <?= $view->insert('partials/field', ['name' => 'reason', 'label' => 'Reason', 'required' => true]) ?>
        <button class="btn btn-danger" type="submit">Void invoice</button>
      </form>
    </section>
    <?php endif; ?>
  </div>
</div>
