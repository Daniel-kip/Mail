<?php

declare(strict_types=1);

namespace App\Http\Middleware;

use App\Http\Request;
use App\Http\Response;

/**
 * Start the session and share the CSRF token and flash messages with the
 * views. Static health checks skip the session entirely.
 *
 * Flash data lives for exactly one rendered page: it is kept across
 * redirects and background requests.
 */
final class StartSession extends Middleware
{
    public function handle(Request $request, \Closure $next): Response
    {
        if ($request->path() === '/healthz') {
            return $next($request);
        }
        $session = $this->app->session();
        $session->start();
        $this->app->links()->ensureKey();
        $view = $this->app->view();
        $view->share('csrfToken', $session->csrfToken());
        $view->share('flashes', array_filter([
            'success' => $session->flashed('success'),
            'error' => $session->flashed('error'),
            'warning' => $session->flashed('warning'),
            'info' => $session->flashed('info'),
        ]));
        $view->share('errors', (array) $session->flashed('errors', []));
        $view->share('old', (array) $session->flashed('old', []));
        $view->share('request', $request);
        $view->share('appName', $this->app->config->string('name'));
        $view->share('config', $this->app->config);
        $response = $next($request);
        // Flash messages are for the next page a person sees. A redirect or a
        // background fetch (live preview, polling, autocomplete) shows
        // nothing, so it must not use them up.
        if ($response->status() >= 300 && $response->status() < 400 || $request->header('X-Requested-With') === 'fetch') {
            $session->reflash();
        }
        return $response;
    }
}
