<div class="card"><div class="card-body">
<?php if (!$invitation): ?>
  <?= $view->insert('partials/empty', ['icon' => 'link', 'heading' => 'This invitation is not valid', 'text' => 'It may have expired, been revoked or already used. Ask the person who invited you for a new one.']) ?>
<?php else: ?>
  <h1 style="font-size:1.4rem">Join <?= e($invitation['organization_name'] ?? 'the organization') ?></h1>
  <p class="text-2">You were invited as <strong><?= e(ucfirst($invitation['role'] ?? 'member')) ?></strong> using <strong><?= e($invitation['email'] ?? '') ?></strong>.</p>
  <?php if ($signedIn): ?>
    <?php if (strcasecmp((string) ($signedInEmail ?? ''), (string) ($invitation['email'] ?? '')) !== 0): ?>
      <div class="alert alert-warning"><?= icon('alert-triangle') ?><div>You are signed in as <?= e($signedInEmail) ?>. Sign in with the invited address to accept.</div></div>
    <?php endif; ?>
    <form method="post" action="/invitations/<?= e($token) ?>"><?= csrf_field() ?><button class="btn btn-primary btn-block btn-lg" type="submit">Accept invitation</button></form>
  <?php else: ?>
    <a class="btn btn-primary btn-block btn-lg" href="/login?next=<?= e(rawurlencode('/invitations/' . $token)) ?>">Sign in to accept</a>
    <a class="btn btn-block mt-2" href="/register?type=institution">Create an account with <?= e($invitation['email'] ?? 'that address') ?></a>
  <?php endif; ?>
<?php endif; ?>
</div></div>
