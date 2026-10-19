<?php
$currency = $plans['currency'] ?? 'KES';
$byAudience = ['individual' => [], 'institution' => []];
foreach ($plans['data'] ?? [] as $p) {
    $byAudience[$p['audience']][] = $p;
}
$fromPrice = static function (array $list) use ($currency): string {
    $paid = array_filter($list, static fn ($p) => $p['price_monthly_cents'] > 0 && $p['self_service']);
    if ($paid === []) {
        return 'Free to start';
    }
    $min = min(array_map(static fn ($p) => $p['price_monthly_cents'], $paid));
    return 'Free plan, paid from ' . money($min, $currency) . '/mo';
};
?>
<section class="hero hero-dark">
  <div class="hero-glow" aria-hidden="true"></div>
  <div class="container hero-grid">
    <div class="hero-network">
      <video class="hero-network-video" src="<?= e(asset('video/mailvora-global-network-loop.mp4')) ?>" poster="<?= e(asset('video/mailvora-global-network-poster.jpg')) ?>" autoplay muted loop playsinline preload="auto" disablepictureinpicture disableremoteplayback draggable="false" tabindex="-1" aria-hidden="true"></video>
    </div>
    <div class="hero-copy">
      <span class="eyebrow eyebrow-glow"><?= icon('zap', 'icon-sm') ?> Business email. Built for trust.</span>
      <h1>Every email <em>delivered</em>.<br>Every inbox <em>yours</em>.</h1>
      <p class="lead"><?= e($appName) ?> sends your app's invoices, receipts and sign-in codes through one simple API, and gives your institution professional mailboxes on its own domain, with M-Pesa billing and setup that works on every phone.</p>
      <div class="hero-actions">
        <a class="btn btn-primary btn-lg btn-glow" href="/register">Start free <?= icon('arrow-right') ?></a>
        <a class="btn btn-lg btn-ghost-light" href="/tools"><?= icon('search') ?> Check your domain</a>
      </div>
      <div class="trust-row">
        <span><?= icon('shield-check') ?> SPF · DKIM · DMARC</span>
        <span><?= icon('lock') ?> TLS everywhere</span>
        <span><?= icon('smartphone-check') ?> Pay with M-Pesa</span>
      </div>
    </div>

  </div>
</section>


<section class="showcase" aria-labelledby="showcase-title">
  <div class="container showcase-grid">
    <div class="sim" data-sim data-api-url="<?= e($apiUrl) ?>" aria-label="Animated demonstration: an email travelling from an application to a phone">
      <div class="sim-tabs" role="tablist" aria-label="Choose a demonstration">
        <button type="button" role="tab" class="sim-tab is-active" data-sim-tab="0" aria-selected="true"><?= icon('code', 'icon-xs') ?> App invoice</button>
        <button type="button" role="tab" class="sim-tab" data-sim-tab="1" aria-selected="false"><?= icon('building', 'icon-xs') ?> Staff email</button>
        <button type="button" role="tab" class="sim-tab" data-sim-tab="2" aria-selected="false"><?= icon('key', 'icon-xs') ?> Sign-in code</button>
      </div>

      <div class="sim-stage">
        <div class="sim-composer">
          <div class="sim-composer-bar"><i></i><i></i><i></i><span data-sim-file>POST /v1/messages</span></div>
          <pre class="sim-code" data-sim-code aria-hidden="true"></pre>
          <button type="button" class="sim-send" data-sim-send tabindex="-1" aria-hidden="true"><?= icon('send', 'icon-xs') ?> <span>Send</span></button>
        </div>

        <div class="sim-flight">
          <ol class="sim-track" data-sim-track>
            <li data-step="0"><span class="sim-node"><?= icon('check', 'icon-xs') ?></span><b>Accepted</b><small data-sim-ms>&nbsp;</small></li>
            <li data-step="1"><span class="sim-node"><?= icon('check', 'icon-xs') ?></span><b>DKIM signed</b><small data-sim-ms>&nbsp;</small></li>
            <li data-step="2"><span class="sim-node"><?= icon('check', 'icon-xs') ?></span><b>TLS relay</b><small data-sim-ms>&nbsp;</small></li>
            <li data-step="3"><span class="sim-node"><?= icon('check', 'icon-xs') ?></span><b>Delivered</b><small data-sim-ms>&nbsp;</small></li>
          </ol>
          <div class="sim-envelope" data-sim-envelope aria-hidden="true">
            <img src="<?= e(asset('brand/mailvora-mark.svg')) ?>" alt="" width="34" height="34">
          </div>
          <div class="sim-log" data-sim-log aria-live="off"></div>
        </div>

        <div class="sim-phone" aria-hidden="true">
          <div class="sim-phone-notch"></div>
          <div class="sim-phone-status"><span data-sim-clock>9:41</span><span>5G ▮▮▮</span></div>
          <div class="sim-toast" data-sim-toast>
            <img src="<?= e(asset('brand/mailvora-mark.svg')) ?>" alt="" width="22" height="22">
            <div><b data-sim-toast-from>Acme Store</b><span data-sim-toast-subject>Invoice INV-1042</span></div>
          </div>
          <div class="sim-inbox">
            <div class="sim-inbox-head">Inbox</div>
            <div class="sim-mail is-new" data-sim-newmail>
              <span class="sim-avatar" data-sim-avatar>A</span>
              <div><b data-sim-from>Acme Store</b><span data-sim-subject>Invoice INV-1042</span><small data-sim-preview>KES 1,200 paid via M-Pesa. Thank you!</small>
                <em class="sim-auth"><?= icon('shield-check', 'icon-xs') ?> SPF · DKIM · DMARC pass</em></div>
            </div>
            <div class="sim-mail"><span class="sim-avatar">R</span><div><b>Riverside ICT</b><span>Your mailbox is ready</span><small>Connect your phone in 3 steps</small></div></div>
            <div class="sim-mail"><span class="sim-avatar">B</span><div><b>Board secretary</b><span>Agenda for Friday</span><small>Budget, new laboratory, staffing</small></div></div>
          </div>
        </div>
      </div>

      <div class="sim-stats">
        <div><b data-sim-count="sent">0</b><span>sent in this demo</span></div>
        <div><b data-sim-count="latency">&mdash;</b><span>simulated time to inbox</span></div>
        <div><b>4</b><span>checks on every message</span></div>
        <button type="button" class="sim-replay" data-sim-replay><?= icon('refresh', 'icon-xs') ?> Replay</button>
      </div>
    </div>
    <div class="showcase-copy">
      <span class="eyebrow"><?= icon('activity', 'icon-sm') ?> Watch a message travel</span>
      <h2 id="showcase-title">See exactly what happens after you hit send</h2>
      <p class="text-2">Every message is authenticated with your own DKIM key, sent over TLS, and tracked from acceptance to delivery. Bounces and complaints suppress automatically, so your sender reputation keeps working for you.</p>
      <ul class="showcase-points">
        <li><?= icon('shield-check') ?><span><b>Authenticated by default</b> SPF, DKIM and DMARC guidance for every domain you add.</span></li>
        <li><?= icon('activity') ?><span><b>Traceable to the last hop</b> A timeline for every message and signed webhooks for your app.</span></li>
        <li><?= icon('smartphone-check') ?><span><b>Works on every phone</b> Guided Gmail, Apple Mail and Outlook setup with live connection tests.</span></li>
      </ul>
    </div>
  </div>
</section>

<section class="section" id="product">
  <div class="container">
    <div class="section-head">
      <h2>One platform, two ways to use it</h2>
      <p>Start as a single developer relaying mail from one app, or run email for an entire institution. Converting later keeps everything you set up.</p>
    </div>
    <div class="grid-2">
      <article class="card audience" id="developers">
        <div class="stat-icon"><?= icon('code') ?></div>
        <h3 class="mt-2">For individual developers</h3>
        <p class="text-2">Send invoices, receipts, password resets and product updates from your application with an API key.</p>
        <ul>
          <li><?= icon('check-circle') ?> REST API with idempotency keys and per-recipient status</li>
          <li><?= icon('check-circle') ?> Versioned templates with variables and live preview</li>
          <li><?= icon('check-circle') ?> Signed webhooks for delivery, bounce and complaint events</li>
          <li><?= icon('check-circle') ?> Keys restricted by scope, IP range and sending domain</li>
        </ul>
        <p class="small muted"><?= e($fromPrice($byAudience['individual'])) ?></p>
        <a class="btn btn-primary" href="/register?type=individual">Start as a developer</a>
      </article>
      <article class="card audience" id="institutions">
        <div class="stat-icon"><?= icon('building') ?></div>
        <h3 class="mt-2">For institutions</h3>
        <p class="text-2">Give staff and students professional mailboxes on your domain, and relay system mail from your own applications.</p>
        <ul>
          <li><?= icon('check-circle') ?> Work mailboxes with webmail, shared addresses and quotas</li>
          <li><?= icon('check-circle') ?> Employees choose their own password through a secure activation link</li>
          <li><?= icon('check-circle') ?> Guided phone setup for Gmail, Apple Mail and Outlook</li>
          <li><?= icon('check-circle') ?> Roles for owners, administrators, developers and ICT support</li>
        </ul>
        <p class="small muted"><?= e($fromPrice($byAudience['institution'])) ?></p>
        <a class="btn btn-primary" href="/register?type=institution">Set up my institution</a>
      </article>
    </div>
  </div>
</section>

<section class="section section-alt">
  <div class="container">
    <div class="section-head">
      <h2>Built for deliverability and accountability</h2>
      <p>Everything a production email platform needs, with the mail itself handled by proven open-source servers.</p>
    </div>
    <div class="grid-3">
      <?php foreach ([
          ['shield-check', 'Authenticated by default', 'Each domain gets its own 2048-bit DKIM key. The console shows the exact SPF, DKIM, DMARC and MX records to publish and checks them.'],
          ['activity', 'Traceable delivery', 'Every message has a timeline from queued to handed over, delivered or bounced. A 202 response means accepted, never "delivered".'],
          ['ban', 'Automatic suppression', 'Hard bounces, complaints and one-click unsubscribes (RFC 8058) stop future sends to that address.'],
          ['clock', 'Scheduling & idempotency', 'Schedule messages up to 30 days ahead, cancel before they leave, and retry safely with idempotency keys.'],
          ['users', 'Least-privilege roles', 'Owners, administrators, developers, ICT support and viewers. Support staff can help employees without ever seeing a password.'],
          ['file-text', 'Audit everything', 'Security-relevant changes are recorded with actor, time and address, and can be exported for reviews.'],
      ] as [$i, $t, $d]): ?>
        <div class="card feature"><div class="stat-icon"><?= icon($i) ?></div><h3><?= e($t) ?></h3><p><?= e($d) ?></p></div>
      <?php endforeach; ?>
    </div>
  </div>
</section>

<section class="section">
  <div class="container grid-2" style="align-items:center;gap:3rem">
    <div>
      <span class="eyebrow"><?= icon('terminal', 'icon-sm') ?> Developer friendly</span>
      <h2 style="font-size:2rem;letter-spacing:-.02em">Your first email in one request</h2>
      <p class="text-2">Create a key in the console, verify a domain, and call the API from any language. Responses use stable error codes you can program against.</p>
      <ul class="audience" style="padding:0">
        <li class="row"><?= icon('check-circle') ?> Test keys process mail without delivering it</li>
        <li class="row"><?= icon('check-circle') ?> Attachments, tags, metadata and custom headers</li>
        <li class="row"><?= icon('check-circle') ?> HMAC-signed webhooks with retries</li>
      </ul>
      <a class="btn" href="/developers">Full API reference <?= icon('arrow-right') ?></a>
    </div>
    <div class="code-panel" data-tabs="home-code">
      <div class="tabs" role="tablist">
        <button class="tab is-active" role="tab" id="hc-curl" aria-controls="hc-curl-p" aria-selected="true">cURL</button>
        <button class="tab" role="tab" id="hc-php" aria-controls="hc-php-p" aria-selected="false">PHP</button>
        <button class="tab" role="tab" id="hc-node" aria-controls="hc-node-p" aria-selected="false">Node.js</button>
      </div>
      <pre class="code" id="hc-curl-p" role="tabpanel"><?= e("curl -X POST {$apiUrl}/v1/messages \\\n  -H \"Authorization: Bearer \$API_KEY\" \\\n  -H \"Idempotency-Key: order-4821\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\n    \"from\": \"Billing <billing@yourapp.com>\",\n    \"to\": \"customer@example.com\",\n    \"subject\": \"Your invoice #4821\",\n    \"text\": \"Thanks for your order.\"\n  }'") ?></pre>
      <pre class="code" id="hc-php-p" role="tabpanel" hidden><?= e("\$ch = curl_init('{$apiUrl}/v1/messages');\ncurl_setopt_array(\$ch, [\n  CURLOPT_POST => true,\n  CURLOPT_RETURNTRANSFER => true,\n  CURLOPT_HTTPHEADER => [\n    'Authorization: Bearer ' . getenv('API_KEY'),\n    'Content-Type: application/json',\n  ],\n  CURLOPT_POSTFIELDS => json_encode([\n    'from' => 'billing@yourapp.com',\n    'to' => 'customer@example.com',\n    'template' => 'invoice',\n    'variables' => ['number' => 4821],\n  ]),\n]);\n\$result = json_decode(curl_exec(\$ch), true);") ?></pre>
      <pre class="code" id="hc-node-p" role="tabpanel" hidden><?= e("const res = await fetch('{$apiUrl}/v1/messages', {\n  method: 'POST',\n  headers: {\n    Authorization: `Bearer \${process.env.API_KEY}`,\n    'Content-Type': 'application/json',\n  },\n  body: JSON.stringify({\n    from: 'billing@yourapp.com',\n    to: 'customer@example.com',\n    subject: 'Your invoice #4821',\n    html: '<p>Thanks for your order.</p>',\n  }),\n});\nconst { messages } = await res.json();") ?></pre>
    </div>
  </div>
</section>

<section class="section section-alt">
  <div class="container">
    <div class="section-head">
      <h2>Work email on every phone, without IT tickets</h2>
      <p>Employees follow a plain-language guide for their phone and app. The platform checks the servers, then proves the connection works from the phone itself.</p>
    </div>
    <div class="steps-row">
      <div class="card"><h3>Activate</h3><p class="text-2 mb-0">The employee opens a single-use link and chooses their own password.</p></div>
      <div class="card"><h3>Choose a device</h3><p class="text-2 mb-0">Android, iPhone or computer — Gmail, Apple Mail, Outlook or another app.</p></div>
      <div class="card"><h3>Follow the steps</h3><p class="text-2 mb-0">One instruction at a time with the exact settings and copy buttons.</p></div>
      <div class="card"><h3>Verify</h3><p class="text-2 mb-0">A code arrives on the phone and a test message is sent from it.</p></div>
    </div>
  </div>
</section>

<?php $paid = array_values(array_filter($plans['data'] ?? [], static fn ($p) => $p['self_service'] && $p['price_monthly_cents'] > 0)); ?>
<?php if ($paid !== []): ?>
<section class="section" id="pricing">
  <div class="container">
    <div class="section-head">
      <h2>One flat monthly price. Pay with M-Pesa.</h2>
      <p>No per-message or per-mailbox surprises. Start free, upgrade when you are ready, cancel any time.</p>
    </div>
    <div class="plans">
      <?php foreach ($paid as $plan): $inst = $plan['audience'] === 'institution'; ?>
        <?= $view->insert('partials/plan-card', ['plan' => $plan, 'currency' => $currency, 'featured' => $inst,
            'cta' => '<a class="btn ' . ($inst ? 'btn-primary' : '') . ' btn-block" href="/register?type=' . ($inst ? 'institution' : 'individual') . '&amp;plan=' . e($plan['id']) . '">Choose ' . e($plan['name']) . '</a>']) ?>
      <?php endforeach; ?>
    </div>
    <p class="mt-3" style="text-align:center"><a href="/pricing">See free plans and full limits <?= icon('arrow-right', 'icon-sm') ?></a></p>
  </div>
</section>
<?php endif; ?>

<section class="section section-alt">
  <div class="container grid-2" style="align-items:center;gap:3rem">
    <div>
      <span class="eyebrow"><?= icon('search', 'icon-sm') ?> Free deliverability tools</span>
      <h2 style="font-size:2rem;letter-spacing:-.02em">Is your domain ready to send?</h2>
      <p class="text-2">Check MX, SPF (with the 10-lookup limit), DMARC, DKIM, MTA-STS, TLS reporting, BIMI, reverse DNS and blocklists in one click. Paste message headers to see every hop and why a message landed in spam.</p>
      <a class="btn btn-primary" href="/tools">Open the tools <?= icon('arrow-right') ?></a>
    </div>
    <form class="card card-body tool-teaser" method="get" action="/tools/domain">
      <label class="label" for="teaser-domain">Domain</label>
      <div class="input-group"><input class="input" id="teaser-domain" name="domain" placeholder="yourcompany.co.ke" autocomplete="off" required><button class="btn btn-primary" type="submit"><?= icon('search') ?> Check</button></div>
      <p class="small muted mb-0 mt-1">No account needed. Results in a few seconds.</p>
    </form>
  </div>
</section>

<section class="section">
  <div class="container">
    <div class="cta-band">
      <div>
        <h2>Ready to send your first message?</h2>
        <p>Create an account in a minute with a username and your phone, or continue with Google.</p>
      </div>
      <a class="btn btn-primary btn-lg" href="/register">Get started free</a>
    </div>
  </div>
</section>
