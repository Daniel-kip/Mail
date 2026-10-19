<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Email confirmation. The link opens a page with a button rather than
 * confirming on GET, so security scanners that prefetch links in mail
 * cannot use up the single-use token.
 */
final class VerificationController extends Controller
{
    public function show(Request $request): Response
    {
        return $this->render('auth/verify', ['title' => 'Confirm your email', 'token' => (string) $request->query('token', ''), 'done' => false], 'minimal');
    }

    public function verify(Request $request): Response
    {
        $res = $this->public('POST', '/v1/auth/email/verify', ['json' => ['token' => $request->string('token')]]);
        if (!$res->ok()) {
            $this->flash('error', $res->errorCode() === 'invalid_token' ? 'This confirmation link is invalid or has already been used. Sign in and request a new link from the banner.' : self::describe($res));
            return $this->redirect('/verify-email?' . http_build_query(['token' => $request->string('token')]));
        }
        return $this->render('auth/verify', ['title' => 'Email confirmed', 'token' => '', 'done' => true], 'minimal');
    }
}
