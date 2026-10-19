<?php if (!$chart || $chart['rows'] === []): ?>
  <p class="muted small">No sending activity in this period.</p>
<?php else: ?>
  <figure class="chart-figure" role="img" aria-label="Messages per day by delivery status">
    <?= $chart['svg'] ?>
    <figcaption class="legend">
      <?php foreach (\App\View\Charts::DELIVERY_SERIES as $key => $label): ?><span><i class="s-<?= e($key) ?>"></i><?= e(is_array($label) ? ($label['label'] ?? $key) : $label) ?></span><?php endforeach; ?>
    </figcaption>
  </figure>
  <details class="mt-2"><summary class="small">Show as table</summary>
    <div class="table-wrap"><table class="table table-compact">
      <thead><tr><th>Date</th><?php foreach (\App\View\Charts::DELIVERY_SERIES as $key => $label): ?><th class="num"><?= e(is_array($label) ? ($label['label'] ?? $key) : $label) ?></th><?php endforeach; ?><th class="num">Total</th></tr></thead>
      <tbody><?php foreach ($chart['rows'] as $row): ?><tr><td><?= e($row['date']) ?></td><?php foreach ($row['values'] as $v): ?><td class="num"><?= number($v) ?></td><?php endforeach; ?><td class="num"><?= number($row['total']) ?></td></tr><?php endforeach; ?></tbody>
    </table></div>
  </details>
<?php endif; ?>
