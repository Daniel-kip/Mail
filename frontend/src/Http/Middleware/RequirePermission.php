<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;

/**
 * `can:<permission>`: the member's role in the active organization must
 * grant the permission. The API enforces the same rule; checking here gives
 * a clear page instead of a failed form.
 */
final class RequirePermission extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        $permissions = $request->attribute('auth')['organization']['permissions'] ?? [];
        if (!in_array($this->argument, $permissions, true)) {
            throw HttpException::forbidden('Your role in this organization does not include access to this page. Ask an owner or administrator if you need it.');
        }
        return $next($request);
    }
}
