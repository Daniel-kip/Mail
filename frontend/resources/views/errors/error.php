<?php
$icons = [403 => 'lock', 404 => 'search', 419 => 'clock', 429 => 'clock', 503 => 'server'];
$defaults = [
    403 => 'You do not have access to this page.',
    404 => 'The page you are looking for does not exist, or you do not have access to it.',
    405 => 'This address does not accept that kind of request.',
    419 => 'Your page expired. Go back, reload and try again.',
    429 => 'Too many requests in a short time. Wait a moment and try again.',
    500 => 'An unexpected error occurred. The team has been notified through the logs.',
    503 => 'The service is temporarily unavailable. Please try again in a moment.',
];
?>
<div class="card">
  <div class="card-body empty">
    <div class="empty-icon"><?= icon($icons[$status] ?? 'alert-triangle') ?></div>
    <p class="muted mb-1">Error <?= e($status) ?></p>
    <h1><?= e($title) ?></h1>
    <p><?= e($message !== '' ? $message : ($defaults[$status] ?? '')) ?></p>
    <?php if (!empty($reference)): ?>
      <p class="small muted">Reference: <code><?= e($reference) ?></code> — include it if you contact support.</p>
    <?php endif; ?>
    <?php
    $path = isset($request) ? $request->path() : '/';
    [$backHref, $backLabel] = match (true) {
        str_starts_with($path, '/admin') => ['/admin', 'Operator console'],
        str_starts_with($path, '/console') => ['/console', 'Console'],
        str_starts_with($path, '/mail') => ['/mail', 'Mailbox'],
        default => ['/', 'Home'],
    };
    ?>
    <div class="row" style="justify-content:center">
      <?php if ($status === 401 || $status === 419): ?>
        <a class="btn" href="<?= e($path === '/' ? '/' : $path) ?>"><?= icon('refresh') ?> Reload</a>
      <?php endif; ?>
      <a class="btn btn-primary" href="<?= e($backHref) ?>"><?= icon('arrow-left') ?> <?= e($backLabel) ?></a>
    </div>
  </div>
</div>
