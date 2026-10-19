<?php
/** @var string $appName */
$pageTitle = isset($title) && $title !== '' ? $title . ' · ' . $appName : $appName . ' · ' . $config->string('tagline');
?><!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="light dark">
<meta name="theme-color" content="#0b7a5c">
<?php if (!empty($refresh)): ?><meta http-equiv="refresh" content="<?= (int) $refresh ?>">
<?php endif; ?>
<meta name="csrf-token" content="<?= e($csrfToken ?? '') ?>">
<meta name="description" content="<?= e($description ?? $config->string('tagline')) ?>">
<?php if (!empty($noindex)): ?><meta name="robots" content="noindex, nofollow">
<?php endif; ?>
<title><?= e($pageTitle) ?></title>
<link rel="icon" href="<?= e(asset('brand/mailvora-mark.svg')) ?>" type="image/svg+xml">
<link rel="icon" href="<?= e(asset('brand/favicon-32.png')) ?>" sizes="32x32" type="image/png">
<link rel="apple-touch-icon" href="<?= e(asset('brand/apple-touch-icon.png')) ?>">
<link rel="manifest" href="/manifest.webmanifest">
<meta property="og:site_name" content="<?= e($appName) ?>">
<meta property="og:title" content="<?= e(($title ?? '') !== '' ? $title . ' · ' . $appName : $appName) ?>">
<meta property="og:description" content="<?= e($description ?? 'Business email. Built for trust.') ?>">
<meta property="og:image" content="<?= e(rtrim((string) $config->get('url'), '/') . '/assets/brand/og-image.png') ?>">
<meta name="twitter:card" content="summary_large_image">
<link rel="stylesheet" href="<?= e(asset('css/app.css')) ?>">
<script src="<?= e(asset('js/theme.js')) ?>"></script>
<script src="<?= e(asset('js/app.js')) ?>" defer></script>
<?php foreach ($scripts ?? [] as $script): ?><script src="<?= e(asset($script)) ?>" defer></script>
<?php endforeach; ?>
</head>
<body<?= !empty($bodyClass) ? ' class="' . e($bodyClass) . '"' : '' ?><?= !empty($shortcuts) ? ' data-shortcuts' : '' ?>>
<a class="skip-link" href="#main">Skip to content</a>
