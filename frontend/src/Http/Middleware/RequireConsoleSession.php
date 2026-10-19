<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;

/**
 * Require a signed-in console user. The authoritative session state (user,
 * active organization, role, permissions) is loaded from the API on every
 * request, so role changes and suspensions apply immediately.
 *
 * `console` also requires an active organization; `console:any` allows
 * pages that work without one (create organization, account settings).
 */
final class RequireConsoleSession extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        $api = $this->app->api();
        if (!$api->hasSession('console')) {
            return $this->toLogin($request);
        }
        $res = $api->request('GET', '/v1/auth/session', ['kind' => 'console']);
        if ($res->status === 401) {
            if ($res->errorCode() === 'mfa_required') {
                return Response::redirect('/login/mfa');
            }
            $api->forget('console');
            $this->app->session()->flash('info', 'Your session ended. Sign in again to continue.');
            return $this->toLogin($request);
        }
        if (!$res->ok()) {
            throw new \RuntimeException('Could not load the console session (HTTP ' . $res->status . ')');
        }
        $auth = $res->json();
        if ($this->argument !== 'any' && empty($auth['organization'])) {
            return Response::redirect('/console/organizations/new');
        }
        $view = $this->app->view();
        $view->share('auth', $auth);
        return $next($request->withAttribute('auth', $auth));
    }

    private function toLogin(Request $request): Response
    {
        $next = $request->isSafe() ? $request->fullPath() : '/console';
        return Response::redirect('/login?' . http_build_query(['next' => $next]));
    }
}
