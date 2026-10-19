<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * The signed-in user's own profile, password, two-factor and sessions.
 */
final class AccountController extends Controller
{
    public function show(Request $request): Response
    {
        $sessions = $this->console('GET', '/v1/auth/sessions');
        return $this->render('console/account', [
            'title' => 'Your account',
            'sessions' => $sessions->ok() ? $sessions->items() : [],
            'mfaSetup' => $this->session()->get('mfa_setup'),
            'recoveryCodes' => $this->revealed('recovery_codes'),
        ]);
    }

    public function profile(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['full_name' => 'required|max:200'], ['full_name' => 'Name']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/account', $errors);
        }
        $res = $this->console('PATCH', '/v1/auth/profile', ['json' => ['full_name' => $request->string('full_name')]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/account', $res);
        }
        $this->flash('success', 'Profile updated.');
        return $this->redirect('/console/account');
    }

    public function password(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['current_password' => 'required', 'new_password' => 'required|min:12|max:256', 'password_confirmation' => 'same:new_password'], ['password_confirmation' => 'Confirmation']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/account#password', $errors);
        }
        $res = $this->console('POST', '/v1/auth/password', ['json' => ['current_password' => (string) $request->input('current_password'), 'new_password' => (string) $request->input('new_password')]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/console/account#password', [$res->errorCode() === 'invalid_credentials' ? 'current_password' : 'new_password' => self::describe($res)], 'The password was not changed.');
        }
        $this->flash('success', 'Password changed. Your other sessions were signed out.');
        return $this->redirect('/console/account');
    }

    public function resendVerification(Request $request): Response
    {
        $res = $this->console('POST', '/v1/auth/email/resend', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'A new confirmation link is on its way.' : self::describe($res));
        return $this->redirectToLocal($request->string('back'), '/console');
    }

    public function mfaSetup(Request $request): Response
    {
        $res = $this->console('POST', '/v1/auth/mfa/setup', ['json' => new \stdClass()]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } else {
            $this->session()->put('mfa_setup', ['secret' => $res->get('secret'), 'otpauth_url' => $res->get('otpauth_url')]);
        }
        return $this->redirect('/console/account#mfa');
    }

    public function mfaEnable(Request $request): Response
    {
        $code = preg_replace('/\s+/', '', $request->string('code')) ?? '';
        $res = $this->console('POST', '/v1/auth/mfa/enable', ['json' => ['code' => $code]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/console/account#mfa', ['code' => self::describe($res, 'That code is incorrect.')], 'Two-factor was not enabled.');
        }
        $this->session()->forget('mfa_setup');
        if (is_array($res->get('recovery_codes'))) {
            $this->reveal('recovery_codes', $res->get('recovery_codes'));
        }
        $this->flash('success', 'Two-factor authentication is on.');
        return $this->redirect('/console/account#mfa');
    }

    public function mfaDisable(Request $request): Response
    {
        $res = $this->console('POST', '/v1/auth/mfa/disable', ['json' => ['password' => (string) $request->input('password'), 'code' => preg_replace('/\s+/', '', $request->string('code'))]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/console/account#mfa', ['code' => self::describe($res)], 'Two-factor is still on.');
        }
        $this->flash('success', 'Two-factor authentication was turned off.');
        return $this->redirect('/console/account#mfa');
    }

    public function revokeSession(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/auth/sessions/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Session signed out.' : self::describe($res));
        return $this->redirect('/console/account#sessions');
    }
}
