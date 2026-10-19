<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Account verification by SMS: a six-digit code sent to the phone number on
 * the account (or a new number entered here).
 */
final class PhoneController extends Controller
{
    public function show(Request $request): Response
    {
        $user = $this->auth($request)['user'] ?? [];
        if (!empty($user['phone_verified'])) {
            return $this->redirectToLocal((string) $request->query('next', ''), '/console');
        }
        return $this->render('auth/verify-phone', [
            'title' => 'Verify your phone',
            'phone' => (string) ($user['phone'] ?? ''),
            'sentTo' => $this->session()->flashed('code_sent_to'),
            'next' => (string) $request->query('next', ''),
            'artTitle' => 'One code, and you are in.',
            'artText' => 'We text a six-digit code to confirm the number belongs to you. It keeps your account and your organization safe.',
        ], 'auth');
    }

    public function send(Request $request): Response
    {
        $phone = trim($request->string('phone'));
        $res = $this->console('POST', '/v1/auth/phone/send', ['json' => $phone !== '' ? ['phone' => $phone] : new \stdClass()]);
        if (!$res->ok()) {
            $field = in_array($res->errorCode(), ['validation_failed', 'phone_taken'], true) ? 'phone' : 'code';
            return $this->invalid($request, '/console/verify-phone', [$field => self::describe($res)], 'The code was not sent.');
        }
        $this->session()->flash('code_sent_to', $res->get('sent_to'));
        $this->flash('success', 'We sent a code to ' . $res->get('sent_to') . '.');
        return $this->redirect('/console/verify-phone');
    }

    public function verify(Request $request): Response
    {
        $res = $this->console('POST', '/v1/auth/phone/verify', ['json' => ['code' => $request->string('code')]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/console/verify-phone', ['code' => self::describe($res)], 'The code was not accepted.');
        }
        $this->flash('success', 'Your phone number is verified. Your account is fully active.');
        return $this->redirectToLocal($request->string('next'), '/console');
    }
}
