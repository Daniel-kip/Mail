<?php

declare(strict_types=1);

/*
 * Test runner:  php tests/run.php [filter]
 *
 * Discovers tests/Unit and tests/Feature classes ending in "Test" and runs
 * every public method starting with "test". Exits non-zero on failure.
 */

putenv('APP_ENV=testing');
putenv('SESSION_SECURE=false');
error_reporting(E_ALL);
set_error_handler(static function (int $severity, string $message, string $file, int $line): bool {
    throw new ErrorException($message, 0, $severity, $file, $line);
});

require dirname(__DIR__) . '/src/bootstrap.php';

$filter = $argv[1] ?? '';
$classes = [];
foreach (['Unit', 'Feature'] as $suite) {
    $dir = __DIR__ . '/' . $suite;
    if (!is_dir($dir)) {
        continue;
    }
    $iterator = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($dir, FilesystemIterator::SKIP_DOTS));
    foreach ($iterator as $file) {
        if ($file->isFile() && str_ends_with($file->getFilename(), 'Test.php')) {
            $relative = substr($file->getPathname(), strlen(__DIR__) + 1, -4);
            $classes[] = 'Tests\\' . str_replace(['/', '\\'], '\\', $relative);
        }
    }
}
sort($classes);

$passed = 0;
$failed = [];
$assertions = 0;
$started = microtime(true);
foreach ($classes as $class) {
    foreach ((new ReflectionClass($class))->getMethods(ReflectionMethod::IS_PUBLIC) as $method) {
        if (!str_starts_with($method->getName(), 'test')) {
            continue;
        }
        $name = $class . '::' . $method->getName();
        if ($filter !== '' && stripos($name, $filter) === false) {
            continue;
        }
        $test = new $class();
        try {
            $test->setUp();
            $test->{$method->getName()}();
            $test->tearDown();
            $passed++;
            echo '.';
        } catch (Throwable $e) {
            $failed[] = [$name, $e];
            echo 'F';
        }
        $assertions += $test->assertions;
    }
}
$elapsed = round(microtime(true) - $started, 2);
echo "\n\n";
foreach ($failed as [$name, $e]) {
    echo "FAIL {$name}\n  " . get_class($e) . ': ' . $e->getMessage() . "\n  at " . basename($e->getFile()) . ':' . $e->getLine() . "\n";
    foreach (array_slice($e->getTrace(), 0, 3) as $frame) {
        if (isset($frame['file'])) {
            echo '     ' . basename($frame['file']) . ':' . ($frame['line'] ?? '?') . "\n";
        }
    }
    echo "\n";
}
printf("%d passed, %d failed, %d assertions in %ss\n", $passed, count($failed), $assertions, $elapsed);
exit($failed === [] ? 0 : 1);
