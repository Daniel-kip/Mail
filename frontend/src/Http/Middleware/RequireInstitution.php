<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;

/**
 * Hosted work mailboxes and aliases belong to institutional accounts.
 * Individual (developer) accounts are sent to the explanation page.
 */
final class RequireInstitution extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        $type = $request->attribute('auth')['organization']['account_type'] ?? 'institution';
        if ($type !== 'institution') {
            return Response::redirect('/console/mailboxes/upgrade');
        }
        return $next($request);
    }
}
