<?php

declare(strict_types=1);

/** @var App\Kernel\Application $app */
$app = require dirname(__DIR__) . '/src/bootstrap.php';
$app->run();
