<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;

/**
 * Require a signed-in mailbox user (webmail and the phone setup guide).
 * Disabling a mailbox, a password reset or a tenant suspension ends the
 * session at the API, which this check observes on the next request.
 */
final class RequireMailboxSession extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        $api = $this->app->api();
        if (!$api->hasSession('webmail')) {
            return $this->toLogin($request);
        }
        $res = $api->request('GET', '/v1/webmail/session', ['kind' => 'webmail']);
        if ($res->status === 401) {
            $api->forget('webmail');
            $this->app->session()->flash('info', 'Your mail session ended. Sign in again to continue.');
            return $this->toLogin($request);
        }
        if (!$res->ok()) {
            throw new \RuntimeException('Could not load the mailbox session (HTTP ' . $res->status . ')');
        }
        $mailbox = $res->json();
        $this->app->view()->share('mail', $mailbox);
        return $next($request->withAttribute('mail', $mailbox));
    }

    private function toLogin(Request $request): Response
    {
        $next = $request->isSafe() ? $request->fullPath() : '/mail';
        return Response::redirect('/mail/login?' . http_build_query(['next' => $next]));
    }
}
