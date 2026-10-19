<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * "Continue with Google": OpenID Connect authorization-code flow with PKCE,
 * a one-time state (CSRF) and nonce (replay protection). The code is
 * exchanged by the API, which holds the client secret.
 */
final class GoogleController extends Controller
{
    private const AUTHORIZE = 'https://accounts.google.com/o/oauth2/v2/auth';

    public function start(Request $request): Response
    {
        $providers = $this->public('GET', '/v1/auth/providers');
        $clientId = $providers->ok() ? (string) ($providers->get('google.client_id') ?? '') : '';
        if ($clientId === '') {
            $this->flash('error', 'Google sign-in is not available right now. Use your username or email instead.');
            return $this->redirect('/login');
        }
        $verifier = self::b64url(random_bytes(48));
        $flow = [
            'state' => self::b64url(random_bytes(24)),
            'nonce' => self::b64url(random_bytes(24)),
            'verifier' => $verifier,
            'account_type' => in_array($request->query('type'), ['individual', 'institution'], true) ? $request->query('type') : null,
            'next' => (string) $request->query('next', ''),
            'started' => time(),
        ];
        $this->session()->put('google_flow', $flow);
        return Response::redirect(self::AUTHORIZE . '?' . http_build_query([
            'client_id' => $clientId,
            'redirect_uri' => $this->callbackUrl(),
            'response_type' => 'code',
            'scope' => 'openid email profile',
            'state' => $flow['state'],
            'nonce' => $flow['nonce'],
            'code_challenge' => self::b64url(hash('sha256', $verifier, true)),
            'code_challenge_method' => 'S256',
            'prompt' => 'select_account',
        ]), 302);
    }

    public function callback(Request $request): Response
    {
        $flow = $this->session()->pull('google_flow');
        if ($request->query('error')) {
            $this->flash('info', 'Google sign-in was cancelled.');
            return $this->redirect('/login');
        }
        if (!is_array($flow) || !hash_equals((string) $flow['state'], (string) $request->query('state', '')) || time() - (int) $flow['started'] > 600) {
            $this->flash('error', 'That Google sign-in link expired or was opened in another browser. Please try again.');
            return $this->redirect('/login');
        }
        $res = $this->console('POST', '/v1/auth/google', ['json' => array_filter([
            'code' => (string) $request->query('code', ''),
            'code_verifier' => $flow['verifier'],
            'redirect_uri' => $this->callbackUrl(),
            'nonce' => $flow['nonce'],
            'account_type' => $flow['account_type'],
        ], static fn ($v) => $v !== null)]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res, 'Google sign-in failed. Please try again.'));
            return $this->redirect('/login');
        }
        $this->session()->regenerate();
        if ($res->get('mfa_required')) {
            $this->session()->put('login_next', $flow['next']);
            return $this->redirect('/login/mfa');
        }
        $name = explode(' ', (string) $res->get('user.full_name', ''))[0];
        $this->flash('success', $res->get('created') ? 'Welcome to ' . $this->app->config->string('name') . ', ' . $name . '! Your account is ready.' : 'Welcome back, ' . $name . '.');
        return $this->redirectToLocal($flow['next'], '/console');
    }

    private function callbackUrl(): string
    {
        return rtrim((string) $this->app->config->get('url'), '/') . '/auth/google/callback';
    }

    private static function b64url(string $bytes): string
    {
        return rtrim(strtr(base64_encode($bytes), '+/', '-_'), '=');
    }
}
