<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;

/**
 * Operator console. Non-operators get a 404 so the area is not advertised.
 */
final class RequirePlatformAdmin extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        if (empty($request->attribute('auth')['user']['is_platform_admin'])) {
            throw HttpException::notFound();
        }
        return $next($request);
    }
}
