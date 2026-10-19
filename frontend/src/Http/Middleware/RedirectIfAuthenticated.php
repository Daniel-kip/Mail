<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;

/**
 * Keep signed-in users away from sign-in and sign-up pages.
 * `guest` checks the console session, `guest:mailbox` the webmail session.
 */
final class RedirectIfAuthenticated extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        $kind = $this->argument === 'mailbox' ? 'webmail' : 'console';
        if ($this->app->api()->hasSession($kind)) {
            return Response::redirect($kind === 'webmail' ? '/mail' : '/console');
        }
        return $next($request);
    }
}
