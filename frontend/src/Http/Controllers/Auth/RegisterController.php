<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Sign-up. The account type decides the starting plan and features:
 * individual developers relay transactional mail; institutions also host
 * work mailboxes.
 */
final class RegisterController extends Controller
{
    private function providers(): array
    {
        $res = $this->public('GET', '/v1/auth/providers');
        return $res->ok() ? $res->json() : [];
    }

    public function show(Request $request): Response
    {
        if (!$this->app->config->bool('signup_enabled')) {
            return $this->render('auth/signup-closed', ['title' => 'Sign-up'], 'auth');
        }
        $type = in_array($request->query('type'), ['individual', 'institution'], true) ? $request->query('type') : null;
        return $this->render('auth/register', [
            'title' => 'Create your account',
            'type' => $type,
            'plan' => preg_match('/^[a-z_]{2,40}$/', (string) $request->query('plan', '')) ? $request->query('plan') : '',
            'providers' => $this->providers(),
        ], 'auth');
    }

    public function register(Request $request): Response
    {
        if (!$this->app->config->bool('signup_enabled')) {
            return $this->redirect('/register');
        }
        $type = $request->string('account_type');
        $rules = [
            'account_type' => 'required|in:individual,institution',
            'full_name' => 'required|max:200',
            'username' => 'required|min:3|max:32',
            'phone' => 'required|max:20',
            'email' => 'required|email|max:254',
            'password' => 'required|min:12|max:256',
            'terms' => 'required',
        ];
        if ($type === 'institution') {
            $rules['organization_name'] = 'required|max:200';
        }
        $errors = Validator::make($request->all(), $rules, [
            'account_type' => 'Account type', 'full_name' => 'Your name', 'phone' => 'Phone number', 'organization_name' => 'Institution name', 'terms' => 'Acceptance of the terms',
        ]);
        $back = '/register' . ($type !== '' ? '?type=' . urlencode($type) : '');
        if ($errors !== []) {
            return $this->invalid($request, $back, $errors);
        }
        $payload = [
            'account_type' => $type,
            'full_name' => $request->string('full_name'),
            'username' => strtolower(ltrim($request->string('username'), '@')),
            'phone' => $request->string('phone'),
            'email' => $request->string('email'),
            'password' => (string) $request->input('password'),
        ];
        if ($request->string('organization_name') !== '') {
            $payload['organization_name'] = $request->string('organization_name');
        }
        $res = $this->console('POST', '/v1/auth/register', ['json' => $payload]);
        if (!$res->ok()) {
            if (in_array($res->errorCode(), ['username_taken', 'phone_taken'], true)) {
                $field = $res->errorCode() === 'username_taken' ? 'username' : 'phone';
                return $this->invalid($request, $back, [$field => $res->errorMessage()], $res->errorMessage() . '.');
            }
            if ($res->errorCode() === 'validation_failed' && preg_match('/username|phone/i', $res->errorMessage(), $m)) {
                return $this->invalid($request, $back, [strtolower($m[0]) => $res->errorMessage()]);
            }
            if ($res->errorCode() === 'email_taken') {
                return $this->invalid($request, $back, ['email' => 'An account with this email already exists. Sign in instead, or reset your password.'], 'This email is already registered.');
            }
            if ($res->errorCode() === 'validation_failed' && str_contains(strtolower($res->errorMessage()), 'password')) {
                return $this->invalid($request, $back, ['password' => $res->errorMessage()], 'Choose a stronger password.');
            }
            return $this->failed($request, $back, $res);
        }
        $this->session()->regenerate();
        $plan = $request->string('plan');
        $next = $plan !== '' && !in_array($plan, ['free', 'developer'], true) ? '/console/billing/plans?' . http_build_query(['highlight' => $plan]) : '/console';
        if ($res->get('verification.channel') === 'phone') {
            if ($res->get('verification.error')) {
                $this->flash('warning', 'Your account is ready, but the text message could not be sent yet. Request a new code below.');
            } else {
                $this->flash('success', 'Your account is ready. Enter the code we texted to ' . $res->get('verification.sent_to') . '.');
            }
            return $this->redirect('/console/verify-phone?' . http_build_query(['next' => $next]));
        }
        $this->flash('success', 'Your account is ready. We sent a confirmation link to ' . $payload['email'] . '.');
        return $this->redirect($next);
    }
}
