<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;
use App\Kernel\Application;

/**
 * Middleware receive the request and the next handler. Route definitions
 * refer to them by alias, optionally with an argument (`can:domains:write`).
 */
abstract class Middleware
{
    public function __construct(protected readonly Application $app, protected readonly ?string $argument = null)
    {
    }

    abstract public function handle(Request $request, \Closure $next): Response;

    public static function resolve(string $alias, ?string $argument, Application $app): self
    {
        $class = match ($alias) {
            'headers' => SecurityHeaders::class,
            'session' => StartSession::class,
            'csrf' => VerifyCsrfToken::class,
            'guest' => RedirectIfAuthenticated::class,
            'console' => RequireConsoleSession::class,
            'can' => RequirePermission::class,
            'institution' => RequireInstitution::class,
            'admin' => RequirePlatformAdmin::class,
            'mailbox' => RequireMailboxSession::class,
            default => throw new \InvalidArgumentException("Unknown middleware {$alias}"),
        };
        return new $class($app, $argument);
    }
}
