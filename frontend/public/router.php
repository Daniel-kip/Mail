<?php

declare(strict_types=1);

/*
 * Router for PHP's built-in development server
 * (`php -S 127.0.0.1:8080 -t public public/router.php`): serve existing
 * static files directly, send everything else to the front controller.
 * Production uses Nginx/Apache rewrites to index.php instead.
 */
$path = parse_url((string) ($_SERVER['REQUEST_URI'] ?? '/'), PHP_URL_PATH) ?: '/';
$root = realpath(__DIR__);
$file = realpath(__DIR__ . DIRECTORY_SEPARATOR . ltrim(rawurldecode($path), '/\\'));
if ($path !== '/' && $root !== false && $file !== false && str_starts_with($file, $root . DIRECTORY_SEPARATOR)
    && is_file($file) && !str_ends_with($file, '.php')) {
    return false;
}
require __DIR__ . '/index.php';
