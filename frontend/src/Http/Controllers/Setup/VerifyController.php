<?php

declare(strict_types=1);

namespace App\Http\Controllers\Setup;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Stage F: prove the phone really connected.
 *
 * Incoming: the platform delivers a message with a short code to the
 * mailbox; the person reads it in the phone app and types the code.
 * Outgoing: the person sends a message from the phone app with a given
 * subject; the platform looks for it arriving through submission.
 */
final class VerifyController extends Controller
{
    public function show(Request $request): Response
    {
        $setup = $this->must($this->mailbox('GET', '/v1/webmail/setup'))->json();
        $guide = preg_match('/^[a-z_-]{3,40}$/', (string) $request->query('guide', '')) ? (string) $request->query('guide') : '';
        return $this->render('setup/verify', [
            'title' => 'Test your connection', 'stage' => 'verify', 'setup' => $setup, 'guide' => $guide,
            'verification' => $setup['verification'] ?? [],
            'incoming' => $this->session()->get('setup_incoming'),
            'outgoing' => $this->session()->get('setup_outgoing'),
        ], 'setup');
    }

    public function startIncoming(Request $request): Response
    {
        $res = $this->mailbox('POST', '/v1/webmail/setup/verify/incoming', ['json' => ['client' => self::client($request)]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } else {
            $this->session()->put('setup_incoming', ['expires_at' => $res->get('expires_at'), 'subject' => $res->get('subject_hint')]);
            $this->flash('success', 'A message with your code is on its way to your inbox.');
        }
        return $this->back($request);
    }

    public function confirmIncoming(Request $request): Response
    {
        $res = $this->mailbox('POST', '/v1/webmail/setup/verify/incoming/confirm', ['json' => ['code' => $request->string('code')]]);
        if (!$res->ok()) {
            return $this->invalid($request, $this->backUrl($request), ['code' => self::describe($res)], 'The code did not match.');
        }
        $this->session()->forget('setup_incoming');
        $this->flash('success', 'Receiving works: your phone app is showing your work email.');
        return $this->back($request);
    }

    public function startOutgoing(Request $request): Response
    {
        $res = $this->mailbox('POST', '/v1/webmail/setup/verify/outgoing', ['json' => ['client' => self::client($request)]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } else {
            $this->session()->put('setup_outgoing', ['subject' => $res->get('subject'), 'send_to' => $res->get('send_to'), 'expires_at' => $res->get('expires_at')]);
        }
        return $this->back($request);
    }

    /** Polled by the page (JSON) or submitted by the "Check again" button. */
    public function checkOutgoing(Request $request): Response
    {
        $res = $this->mailbox('POST', '/v1/webmail/setup/verify/outgoing/check', ['json' => new \stdClass()]);
        $verified = $res->ok() && (bool) $res->get('verified');
        if ($verified) {
            $this->session()->forget('setup_outgoing');
        }
        if ($request->wantsJson()) {
            return Response::json(['verified' => $verified, 'message' => $res->ok() ? $res->get('message') : self::describe($res)]);
        }
        if ($verified) {
            $this->flash('success', 'Sending works: your phone app sent mail through the work server.');
        } else {
            $this->flash($res->ok() ? 'info' : 'error', $res->ok() ? (string) $res->get('message') : self::describe($res));
        }
        return $this->back($request);
    }

    private static function client(Request $request): ?string
    {
        $c = $request->string('client');
        return in_array($c, ['gmail', 'apple_mail', 'outlook', 'other', 'webmail'], true) ? $c : null;
    }

    private function backUrl(Request $request): string
    {
        $guide = $request->string('guide');
        return '/mail/setup/verify' . ($guide !== '' ? '?' . http_build_query(['guide' => $guide]) : '');
    }

    private function back(Request $request): Response
    {
        return $this->redirect($this->backUrl($request));
    }
}
