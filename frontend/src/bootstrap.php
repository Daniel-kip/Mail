<?php

declare(strict_types=1);

/*
 * Application bootstrap shared by the web front controller, the CLI tools
 * and the test runner. The project has no runtime third-party dependencies,
 * so a small PSR-4 autoloader is registered here; Composer's autoloader is
 * used instead when `composer install` has been run.
 */

define('APP_ROOT', dirname(__DIR__));

$composer = APP_ROOT . '/vendor/autoload.php';
if (is_file($composer)) {
    require $composer;
} else {
    spl_autoload_register(static function (string $class): void {
        foreach (['App\\' => APP_ROOT . '/src/', 'Tests\\' => APP_ROOT . '/tests/'] as $prefix => $dir) {
            if (str_starts_with($class, $prefix)) {
                $file = $dir . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
                if (is_file($file)) {
                    require $file;
                }
                return;
            }
        }
    });
    require APP_ROOT . '/src/View/helpers.php';
}

App\Support\Env::load(APP_ROOT . '/.env');

return App\Kernel\Application::create(APP_ROOT);
