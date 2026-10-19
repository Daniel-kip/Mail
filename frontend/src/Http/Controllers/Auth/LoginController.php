<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Console sign-in, two-factor step and sign-out.
 */
final class LoginController extends Controller
{
    public function show(Request $request): Response
    {
        return $this->render('auth/login', [
            'title' => 'Sign in',
            'next' => (string) $request->query('next', ''),
            'signupEnabled' => $this->app->config->bool('signup_enabled'),
            'google' => $this->googleEnabled(),
        ], 'auth');
    }

    public function login(Request $request): Response
    {
        $next = $request->string('next');
        $back = '/login' . ($next !== '' ? '?' . http_build_query(['next' => $next]) : '');
        $errors = Validator::make($request->all(), ['email' => 'required|max:254', 'password' => 'required'], ['email' => 'Email, username or phone']);
        if ($errors !== []) {
            return $this->invalid($request, $back, $errors);
        }
        $res = $this->console('POST', '/v1/auth/login', ['json' => [
            'email' => $request->string('email'),
            'password' => (string) $request->input('password'),
        ]]);
        if (!$res->ok()) {
            $message = match ($res->errorCode()) {
                'invalid_credentials' => 'Those details do not match an account. Check your email, username or phone and password.',
                'account_locked' => $res->errorMessage(),
                'rate_limited' => 'Too many sign-in attempts. Wait a few minutes and try again.',
                default => self::describe($res),
            };
            $this->session()->flash('old', ['email' => $request->string('email')]);
            $this->flash('error', $message);
            return $this->redirect($back);
        }
        $this->session()->regenerate();
        if ($res->get('mfa_required')) {
            $this->session()->put('login_next', $next);
            return $this->redirect('/login/mfa');
        }
        return $this->afterLogin($res->json(), $next);
    }

    public function mfa(Request $request): Response
    {
        if (!$this->app->api()->hasSession('console')) {
            return $this->redirect('/login');
        }
        return $this->render('auth/mfa', ['title' => 'Two-factor authentication'], 'auth');
    }

    public function verifyMfa(Request $request): Response
    {
        $code = preg_replace('/\s+/', '', $request->string('code')) ?? '';
        if (!preg_match('/^\d{6}$/', $code)) {
            return $this->invalid($request, '/login/mfa', ['code' => 'Enter the 6-digit code from your authenticator app.']);
        }
        $res = $this->console('POST', '/v1/auth/mfa/verify', ['json' => ['code' => $code]]);
        if ($res->status === 401 && $res->errorCode() !== 'invalid_mfa_code') {
            $this->flash('error', 'Your sign-in attempt expired. Sign in again.');
            return $this->redirect('/login');
        }
        if (!$res->ok()) {
            return $this->invalid($request, '/login/mfa', ['code' => $res->errorCode() === 'invalid_mfa_code' ? 'That code is incorrect or expired. Try the current code.' : self::describe($res)], 'The code was not accepted.');
        }
        $this->session()->regenerate();
        return $this->afterLogin($res->json(), (string) $this->session()->pull('login_next', ''));
    }

    public function logout(Request $request): Response
    {
        if ($this->app->api()->hasSession('console')) {
            $this->console('POST', '/v1/auth/logout', ['json' => new \stdClass()]);
        }
        $this->app->api()->forget('console');
        $this->session()->regenerate();
        $this->flash('success', 'You have been signed out.');
        return $this->redirect('/login');
    }

    private function googleEnabled(): bool
    {
        $res = $this->public('GET', '/v1/auth/providers');
        return $res->ok() && $res->get('google.client_id') !== null;
    }

    private function afterLogin(array $session, string $next): Response
    {
        $name = $session['user']['full_name'] ?? '';
        $this->flash('success', 'Welcome back' . ($name !== '' ? ', ' . explode(' ', $name)[0] : '') . '.');
        if (empty($session['organization'])) {
            return $this->redirect('/console/organizations/new');
        }
        return $this->redirectToLocal($next, '/console');
    }
}
