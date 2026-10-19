<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;

/**
 * Synchronizer-token CSRF protection for every state-changing request.
 * Forms send `_token`; scripts send the `X-CSRF-Token` header.
 *
 * The only exemption is RFC 8058 one-click unsubscribe: mailbox providers
 * POST to the List-Unsubscribe URL without any session, and the URL itself
 * carries an unforgeable signed token.
 */
final class VerifyCsrfToken extends Middleware
{
    private const EXEMPT = ['~^/u/[A-Za-z0-9_-]+$~'];

    public function handle(Request $request, \Closure $next): Response
    {
        if ($request->isSafe()) {
            return $next($request);
        }
        foreach (self::EXEMPT as $pattern) {
            if (preg_match($pattern, $request->path())) {
                return $next($request);
            }
        }
        $expected = $this->app->session()->csrfToken();
        $provided = (string) ($request->input('_token') ?? '');
        if ($provided === '') {
            $provided = $request->header('x-csrf-token');
        }
        if ($provided === '' || !hash_equals($expected, $provided)) {
            throw new HttpException(419, 'Your page was open for too long or the form was submitted twice. Go back, reload the page and try again.');
        }
        return $next($request);
    }
}
