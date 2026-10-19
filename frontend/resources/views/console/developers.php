<?php
$curl = "curl -X POST {$apiUrl}/v1/messages \\\n  -H \"Authorization: Bearer \$MAILVORA_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -H \"Idempotency-Key: invoice-1042\" \\\n  -d '{\n    \"from\": \"billing@{$domain}\",\n    \"to\": [\"customer@example.com\"],\n    \"subject\": \"Your invoice #1042\",\n    \"html\": \"<p>Thanks for your order.</p>\",\n    \"text\": \"Thanks for your order.\",\n    \"tags\": [\"invoice\"]\n  }'";
$php = "<?php\n\$ch = curl_init('{$apiUrl}/v1/messages');\ncurl_setopt_array(\$ch, [\n    CURLOPT_POST => true,\n    CURLOPT_RETURNTRANSFER => true,\n    CURLOPT_HTTPHEADER => [\n        'Authorization: Bearer ' . getenv('MAILVORA_API_KEY'),\n        'Content-Type: application/json',\n        'Idempotency-Key: invoice-' . \$invoice->id,\n    ],\n    CURLOPT_POSTFIELDS => json_encode([\n        'from' => 'billing@{$domain}',\n        'to' => [\$customer->email],\n        'template_id' => 'TEMPLATE_UUID',\n        'variables' => ['name' => \$customer->name, 'amount' => \$invoice->total],\n    ]),\n]);\n\$response = json_decode(curl_exec(\$ch), true);";
$node = "const res = await fetch('{$apiUrl}/v1/messages', {\n  method: 'POST',\n  headers: {\n    Authorization: `Bearer \${process.env.MAILVORA_API_KEY}`,\n    'Content-Type': 'application/json',\n    'Idempotency-Key': `order-\${order.id}`,\n  },\n  body: JSON.stringify({\n    from: 'updates@{$domain}',\n    to: [user.email],\n    subject: 'Your order shipped',\n    text: 'Good news: your order is on its way.',\n    unsubscribe: true,\n  }),\n});\nconst message = await res.json(); // { id, status: 'queued' }";
?>
<?= $view->insert('partials/page-head', ['heading' => 'Developer quickstart', 'lede' => 'Send invoices, receipts and notifications from your application in three steps.']) ?>
<div class="grid grid-3 mb-3">
  <a class="card card-body" href="/console/domains"><strong><?= icon('globe') ?> 1. Verify a domain</strong><p class="small muted mb-0">Publish SPF, DKIM and DMARC so mail is trusted.</p></a>
  <a class="card card-body" href="/console/api-keys"><strong><?= icon('key') ?> 2. Create an API key</strong><p class="small muted mb-0">Scope it to messages:send and restrict it by IP.</p></a>
  <a class="card card-body" href="/console/webhooks"><strong><?= icon('webhook') ?> 3. Receive events</strong><p class="small muted mb-0">Get delivered, bounced and complained events, HMAC-signed.</p></a>
</div>
<section class="card">
  <div class="card-head"><h2>Send a message</h2><span class="muted small">POST <?= e($apiUrl) ?>/v1/messages</span></div>
  <div class="card-body" data-tabs>
    <div class="tabs" role="tablist"><button class="tab" role="tab" type="button" aria-selected="true" aria-controls="ex-curl">cURL</button><button class="tab" role="tab" type="button" aria-selected="false" aria-controls="ex-php">PHP</button><button class="tab" role="tab" type="button" aria-selected="false" aria-controls="ex-node">Node.js</button></div>
    <?php foreach (['curl' => $curl, 'php' => $php, 'node' => $node] as $id => $code): ?>
      <div class="code-panel" id="ex-<?= $id ?>" role="tabpanel"<?= $id === 'curl' ? '' : ' hidden' ?>>
        <button class="btn btn-sm copy-btn" type="button" data-copy-target="code-<?= $id ?>"><?= icon('copy') ?><span class="copy-label">Copy</span></button>
        <pre><code id="code-<?= $id ?>"><?= e($code) ?></code></pre>
      </div>
    <?php endforeach; ?>
    <ul class="small text-2 mt-2">
      <li>Send the same <code>Idempotency-Key</code> when retrying; the message is never sent twice.</li>
      <li>Add <code>"send_at"</code> (ISO-8601) to schedule, and <code>"unsubscribe": true</code> for one-click unsubscribe headers (RFC 8058) on bulk updates.</li>
      <li>Errors are JSON with a stable <code>error.code</code> and a <code>request_id</code> to quote to support. Full reference on the <a href="/developers">developer page</a>.</li>
    </ul>
  </div>
</section>
