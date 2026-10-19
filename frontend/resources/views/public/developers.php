<?php
$endpoints = [
    ['POST', '/v1/messages', 'messages:send', 'Send one message to one or more recipients (one message per recipient).'],
    ['GET', '/v1/messages', 'messages:read', 'List messages, newest first. Filters: status, q, tag, batch_id; cursor pagination.'],
    ['GET', '/v1/messages/{id}', 'messages:read', 'A message with its delivery timeline.'],
    ['POST', '/v1/messages/{id}/cancel', 'messages:send', 'Cancel a scheduled message before it is sent.'],
    ['GET', '/v1/messages/export.csv', 'messages:read', 'Export message history as CSV.'],
    ['GET', '/v1/templates', 'templates:read', 'List templates.'],
    ['POST', '/v1/templates', 'templates:write', 'Create a template (subject, html, text, variables).'],
    ['POST', '/v1/templates/{id}/preview', 'templates:read', 'Render a saved template with sample variables.'],
    ['GET', '/v1/suppressions', 'suppressions:read', 'Addresses that will not be sent to (bounces, complaints, unsubscribes).'],
    ['POST', '/v1/suppressions', 'suppressions:write', 'Add an address to the suppression list.'],
    ['DELETE', '/v1/suppressions/{email}', 'suppressions:write', 'Remove an address from the suppression list.'],
    ['GET', '/v1/domains', 'domains:read', 'Sending domains and their DNS verification status.'],
    ['GET', '/v1/webhooks', 'webhooks:read', 'Webhook endpoints.'],
    ['POST', '/v1/webhooks', 'webhooks:write', 'Register an HTTPS endpoint for delivery events.'],
    ['GET', '/v1/usage', 'usage:read', 'Daily and monthly usage against your plan limits.'],
    ['GET', '/v1/mailboxes', 'mailboxes:read', 'Hosted mailboxes (institutional accounts).'],
];
$errors = [
    ['401', 'invalid_api_key / api_key_revoked / api_key_expired', 'The key is wrong, revoked or expired.'],
    ['403', 'insufficient_permissions', 'The key lacks the scope this endpoint needs.'],
    ['403', 'api_key_ip_not_allowed', 'The request came from an IP address outside the key\'s allowlist.'],
    ['403', 'sender_domain_not_allowed', 'The key is restricted to other sending domains.'],
    ['403', 'domain_not_verified (details.reason)', 'The From domain is not verified for your account.'],
    ['409', 'idempotency_in_progress', 'A request with the same Idempotency-Key is still being processed.'],
    ['422', 'validation_failed / idempotency_key_reused', 'The request is invalid, or the key was reused with a different body.'],
    ['429', 'rate_limited / quota_exceeded', 'Too many requests per minute, or the daily/monthly plan limit is reached.'],
    ['5xx', 'internal_error / service_unavailable', 'Retry with the same Idempotency-Key and exponential backoff.'],
];
?>
<section class="section">
  <div class="container" style="display:grid;grid-template-columns:220px minmax(0,1fr);gap:2.5rem">
    <nav aria-label="On this page" class="no-print" style="position:sticky;top:90px;align-self:start">
      <p class="section-title mt-0">On this page</p>
      <?php foreach (['start' => 'Quick start', 'auth' => 'Authentication', 'send' => 'Sending a message', 'templates' => 'Templates', 'idempotency' => 'Idempotency', 'webhooks' => 'Webhooks', 'errors' => 'Errors', 'endpoints' => 'Endpoints', 'limits' => 'Limits'] as $id => $label): ?>
        <a class="folder" href="#<?= e($id) ?>"><?= e($label) ?></a>
      <?php endforeach; ?>
    </nav>
    <div class="prose" style="max-width:none">
      <h1 id="start">API reference</h1>
      <p>Base URL: <code><?= e($apiUrl) ?></code>. All requests and responses use JSON over HTTPS. Times are ISO 8601 in UTC.</p>
      <ol>
        <li><a href="/register">Create an account</a> and confirm your email address.</li>
        <li>Add and verify your sending domain in <strong>Domains</strong> (publish the DNS records shown).</li>
        <li>Create an API key in <strong>API keys</strong>. Copy it: it is shown only once.</li>
        <li>Send a request as shown below. A test key (<code>mv_test_…</code>) processes the message without delivering it.</li>
      </ol>

      <h2 id="auth">Authentication</h2>
      <p>Send your key in the <code>Authorization</code> header. Keys carry scopes (for example <code>messages:send</code>) and can be restricted to IP ranges and sending domains. Keep keys on your server; never put them in a browser or mobile app.</p>
      <pre class="code">Authorization: Bearer mv_live_xxxxxxxxxxxx_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</pre>

      <h2 id="send">Sending a message</h2>
      <pre class="code"><?= e("POST /v1/messages\nIdempotency-Key: 6d1f9c3e-invoice-4821\n\n{\n  \"from\": \"Acme Billing <billing@acme.co.ke>\",\n  \"to\": [\"customer@example.com\"],\n  \"subject\": \"Invoice 4821\",\n  \"html\": \"<p>Your invoice is attached.</p>\",\n  \"text\": \"Your invoice is attached.\",\n  \"reply_to\": \"accounts@acme.co.ke\",\n  \"tags\": [\"invoice\"],\n  \"metadata\": { \"invoice_id\": \"4821\" },\n  \"attachments\": [{ \"filename\": \"invoice-4821.pdf\", \"content\": \"<base64>\", \"content_type\": \"application/pdf\" }],\n  \"send_at\": \"2026-10-20T07:00:00Z\",\n  \"unsubscribe\": false\n}") ?></pre>
      <p>Response <code>202 Accepted</code>:</p>
      <pre class="code"><?= e("{\n  \"batch_id\": \"0192…\",\n  \"status\": \"accepted\",\n  \"sandbox\": false,\n  \"messages\": [{ \"id\": \"0192…\", \"to\": \"customer@example.com\", \"status\": \"queued\" }]\n}") ?></pre>
      <p><strong>202 means accepted for delivery, not delivered.</strong> Track progress with <code>GET /v1/messages/{id}</code> or webhooks. Statuses: <code>scheduled</code>, <code>queued</code>, <code>processing</code>, <code>accepted_by_mta</code> (handed to the receiving server), <code>delivered</code>, <code>deferred</code> (temporary failure, retrying), <code>bounced</code>, <code>failed</code>, <code>suppressed</code>, <code>sandboxed</code>, <code>cancelled</code>.</p>
      <p>Set <code>"unsubscribe": true</code> for newsletters and other bulk mail: the message gets RFC 8058 one-click <code>List-Unsubscribe</code> headers (required by Gmail and Yahoo for bulk senders) and recipients who use it are added to your suppression list.</p>

      <h2 id="templates">Templates</h2>
      <p>Store content once and send with variables. Variables are HTML-escaped; undefined variables are errors, so a typo never sends a broken message.</p>
      <pre class="code"><?= e("{\n  \"from\": \"admissions@school.ac.ke\",\n  \"to\": \"parent@example.com\",\n  \"template\": \"admission-confirmation\",\n  \"variables\": { \"first_name\": \"Amina\", \"programme\": \"Form One\" }\n}") ?></pre>

      <h2 id="idempotency">Idempotency</h2>
      <p>Send an <code>Idempotency-Key</code> header (any unique string up to 255 characters) so a retried request never sends twice. Repeating the same key and body within 24 hours returns the original response with <code>Idempotent-Replayed: true</code>; reusing it with a different body returns <code>422 idempotency_key_reused</code>.</p>

      <h2 id="webhooks">Webhooks</h2>
      <p>Register an HTTPS endpoint to receive events such as <code>message.delivered</code>, <code>message.bounced</code>, <code>message.complained</code>, <code>message.unsubscribed</code> and <code>domain.verified</code>. Each request carries <code>Mailvora-Signature: t=&lt;unix&gt;,v1=&lt;hex&gt;</code> plus <code>Mailvora-Event-Id</code>, <code>Mailvora-Event-Type</code> and <code>Mailvora-Delivery-Id</code> headers, an HMAC-SHA256 of <code>t + "." + body</code> with your endpoint secret. Reject signatures older than five minutes. Failed deliveries are retried with backoff.</p>
      <pre class="code"><?= e("\$payload = file_get_contents('php://input');\nparse_str(str_replace(',', '&', \$_SERVER['HTTP_EH_SIGNATURE'] ?? ''), \$sig);\n\$expected = hash_hmac('sha256', \$sig['t'] . '.' . \$payload, getenv('WEBHOOK_SECRET'));\nif (!hash_equals(\$expected, \$sig['v1'] ?? '') || abs(time() - (int) \$sig['t']) > 300) {\n    http_response_code(401); exit;\n}") ?></pre>

      <h2 id="errors">Errors</h2>
      <p>Errors share one shape. <code>code</code> is stable for programs; <code>message</code> is safe to show; <code>request_id</code> helps support find the request.</p>
      <pre class="code"><?= e("{ \"error\": { \"code\": \"quota_exceeded\", \"message\": \"Daily sending limit reached (200/200).\", \"request_id\": \"req_…\" } }") ?></pre>
      <div class="table-wrap card mt-2">
        <table class="table">
          <thead><tr><th>HTTP</th><th>Code</th><th>Meaning</th></tr></thead>
          <tbody><?php foreach ($errors as [$s, $c, $m]): ?><tr><td class="mono"><?= e($s) ?></td><td><code><?= e($c) ?></code></td><td><?= e($m) ?></td></tr><?php endforeach; ?></tbody>
        </table>
      </div>

      <h2 id="endpoints">Endpoints</h2>
      <div class="table-wrap card">
        <table class="table">
          <thead><tr><th>Method</th><th>Path</th><th>Scope</th><th>Description</th></tr></thead>
          <tbody><?php foreach ($endpoints as [$m, $p, $s, $d]): ?><tr><td><span class="tag"><?= e($m) ?></span></td><td class="mono nowrap"><?= e($p) ?></td><td><code><?= e($s) ?></code></td><td><?= e($d) ?></td></tr><?php endforeach; ?></tbody>
        </table>
      </div>

      <h2 id="limits">Limits</h2>
      <ul>
        <li>Requests per minute per key, recipients per request, and daily/monthly sends depend on your plan (see <a href="/pricing">pricing</a>).</li>
        <li>Message size up to 10 MB; attachments up to 8 MB in total, at most 10 per message. Executable file types are refused.</li>
        <li>Up to 10 tags, 20 metadata keys (4 KB) and 10 custom <code>X-</code> headers per message.</li>
      </ul>
    </div>
  </div>
</section>
