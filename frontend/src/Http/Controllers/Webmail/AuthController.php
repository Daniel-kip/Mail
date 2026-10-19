<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Mailbox sign-in, activation (first password) and self-service reset.
 */
final class AuthController extends Controller
{
    public function show(Request $request): Response
    {
        return $this->render('webmail/login', ['title' => 'Sign in to webmail', 'next' => (string) $request->query('next', ''), 'artTitle' => 'Your work email, anywhere.', 'artText' => 'Read and send mail in the browser, then connect your phone in a few guided steps.'], 'auth');
    }

    public function login(Request $request): Response
    {
        $next = $request->string('next');
        $back = '/mail/login' . ($next !== '' ? '?' . http_build_query(['next' => $next]) : '');
        $errors = Validator::make($request->all(), ['address' => 'required|email', 'password' => 'required'], ['address' => 'Email address']);
        if ($errors !== []) {
            return $this->invalid($request, $back, $errors);
        }
        $res = $this->signIn($request->string('address'), (string) $request->input('password'));
        if (!$res->ok()) {
            $this->session()->flash('old', ['address' => $request->string('address')]);
            $this->flash('error', match ($res->errorCode()) {
                'invalid_credentials' => 'The email address or password is incorrect. Use your full work email address.',
                'account_locked' => $res->errorMessage('Too many failed attempts. Try again later or ask ICT support to unlock your account.'),
                'mailbox_pending', 'activation_required' => 'This mailbox has not been activated yet. Open the activation link you received, or ask ICT support for a new one.',
                'mailbox_suspended' => 'This mailbox is suspended. Contact your ICT support.',
                default => self::describe($res),
            });
            return $this->redirect($back);
        }
        return $this->redirectToLocal(str_starts_with($next, '/mail') ? $next : '', '/mail');
    }

    public function logout(Request $request): Response
    {
        if ($this->app->api()->hasSession('webmail')) {
            $this->mailbox('POST', '/v1/webmail/logout', ['json' => new \stdClass()]);
        }
        $this->app->api()->forget('webmail');
        $this->session()->regenerate();
        $this->flash('success', 'You have signed out of webmail.');
        return $this->redirect('/mail/login');
    }

    public function forgot(Request $request): Response
    {
        return $this->render('webmail/forgot', ['title' => 'Reset your mailbox password', 'sent' => (bool) $this->session()->flashed('mail_reset_sent')], 'auth');
    }

    public function sendReset(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['address' => 'required|email'], ['address' => 'Work email address']);
        if ($errors !== []) {
            return $this->invalid($request, '/mail/forgot-password', $errors);
        }
        $res = $this->public('POST', '/v1/webmail/password/forgot', ['json' => ['address' => $request->string('address')]]);
        if ($res->status === 429) {
            $this->flash('error', 'Too many requests. Wait an hour and try again, or contact ICT support.');
            return $this->redirect('/mail/forgot-password');
        }
        $this->session()->flash('mail_reset_sent', true);
        return $this->redirect('/mail/forgot-password');
    }

    public function resetForm(Request $request): Response
    {
        return $this->tokenForm($request, 'reset');
    }

    public function activateForm(Request $request): Response
    {
        return $this->tokenForm($request, 'activate');
    }

    public function reset(Request $request): Response
    {
        return $this->setPassword($request, 'reset');
    }

    public function activate(Request $request): Response
    {
        return $this->setPassword($request, 'activate');
    }

    private function tokenForm(Request $request, string $purpose): Response
    {
        $token = (string) $request->query('token', '');
        $info = null;
        if (preg_match('/^[A-Za-z0-9_-]{20,200}$/', $token)) {
            $res = $this->public('GET', '/v1/webmail/activation', ['query' => ['token' => $token]]);
            $info = $res->ok() ? $res->json() : null;
        }
        $activate = $purpose === 'activate';
        return $this->render('webmail/set-password', [
            'title' => $activate ? 'Activate your work email' : 'Choose a new password',
            'purpose' => $purpose,
            'token' => $token,
            'info' => $info,
        ], 'auth');
    }

    private function setPassword(Request $request, string $purpose): Response
    {
        $token = $request->string('token');
        $page = ($purpose === 'activate' ? '/mail/activate' : '/mail/reset-password') . '?' . http_build_query(['token' => $token]);
        $errors = Validator::make($request->all(), ['password' => 'required|min:10|max:256', 'password_confirmation' => 'same:password'], ['password_confirmation' => 'Password confirmation']);
        if ($errors !== []) {
            return $this->invalid($request, $page, $errors);
        }
        $password = (string) $request->input('password');
        $res = $this->public('POST', $purpose === 'activate' ? '/v1/webmail/activate' : '/v1/webmail/password/reset', ['json' => ['token' => $token, 'password' => $password]]);
        if (!$res->ok()) {
            if ($res->errorCode() === 'invalid_token') {
                $this->flash('error', 'This link is invalid, already used or expired. Ask ICT support for a new one.');
                return $this->redirect($page);
            }
            return $this->invalid($request, $page, ['password' => self::describe($res)], 'The password was not set.');
        }
        $address = (string) ($res->get('address') ?? '');
        if ($address !== '' && $this->signIn($address, $password)->ok()) {
            $this->flash('success', $purpose === 'activate' ? 'Your mailbox is active. Next, connect it to your phone.' : 'Password changed. Remember to update it in the mail app on your phone.');
            return $this->redirect($purpose === 'activate' ? '/mail/setup' : '/mail');
        }
        $this->flash('success', 'Password set. Sign in with your work email address and new password.');
        return $this->redirect('/mail/login');
    }

    private function signIn(string $address, string $password): \App\Api\ApiResponse
    {
        $res = $this->mailbox('POST', '/v1/webmail/login', ['json' => ['address' => strtolower(trim($address)), 'password' => $password]]);
        if ($res->ok()) {
            $this->session()->regenerate();
        }
        return $res;
    }
}
