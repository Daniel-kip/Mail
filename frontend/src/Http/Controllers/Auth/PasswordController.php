<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Console password recovery. Responses never reveal whether an account
 * exists for an address.
 */
final class PasswordController extends Controller
{
    public function forgot(Request $request): Response
    {
        return $this->render('auth/forgot', ['title' => 'Reset your password', 'sent' => (bool) $this->session()->flashed('reset_sent')], 'auth');
    }

    public function sendLink(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['email' => 'required|email']);
        if ($errors !== []) {
            return $this->invalid($request, '/forgot-password', $errors);
        }
        $res = $this->public('POST', '/v1/auth/password/forgot', ['json' => ['email' => $request->string('email')]]);
        if ($res->status === 429) {
            $this->flash('error', 'Too many reset requests. Wait an hour and try again, or contact support.');
            return $this->redirect('/forgot-password');
        }
        $this->session()->flash('reset_sent', true);
        return $this->redirect('/forgot-password');
    }

    public function reset(Request $request): Response
    {
        $token = (string) $request->query('token', '');
        return $this->render('auth/reset', ['title' => 'Choose a new password', 'token' => $token, 'valid' => preg_match('/^[A-Za-z0-9_-]{20,200}$/', $token) === 1], 'auth');
    }

    public function update(Request $request): Response
    {
        $token = $request->string('token');
        $back = '/reset-password?' . http_build_query(['token' => $token]);
        $errors = Validator::make($request->all(), ['password' => 'required|min:12|max:256', 'password_confirmation' => 'same:password'], ['password_confirmation' => 'Password confirmation']);
        if ($errors !== []) {
            return $this->invalid($request, $back, $errors);
        }
        $res = $this->public('POST', '/v1/auth/password/reset', ['json' => ['token' => $token, 'password' => (string) $request->input('password')]]);
        if (!$res->ok()) {
            if ($res->errorCode() === 'invalid_token') {
                $this->flash('error', 'This reset link is invalid or has expired. Request a new one.');
                return $this->redirect('/forgot-password');
            }
            return $this->invalid($request, $back, ['password' => self::describe($res)], 'The password was not changed.');
        }
        $this->app->api()->forget('console');
        $this->flash('success', 'Your password was changed and other sessions were signed out. Sign in with the new password.');
        return $this->redirect('/login');
    }
}
