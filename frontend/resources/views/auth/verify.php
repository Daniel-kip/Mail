<div class="card"><div class="card-body">
<?php if ($done): ?>
  <div class="empty"><div class="empty-icon"><?= icon('check-circle') ?></div><h1 style="font-size:1.4rem">Email confirmed</h1><p>Thank you. Your address is verified.</p><a class="btn btn-primary" href="/console">Go to the console</a></div>
<?php else: ?>
  <h1 style="font-size:1.4rem">Confirm your email address</h1>
  <p class="text-2">Select the button to finish confirming your address.</p>
  <form method="post" action="/verify-email"><?= csrf_field() ?><input type="hidden" name="token" value="<?= e($token) ?>"><button class="btn btn-primary btn-block btn-lg" type="submit"<?= $token === '' ? ' disabled' : '' ?>>Confirm email</button></form>
<?php endif; ?>
</div></div>
