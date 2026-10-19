<?php

declare(strict_types=1);

namespace App\Http\Controllers\PublicSite;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * One-click unsubscribe (RFC 8058).
 *
 * Mailbox providers POST `List-Unsubscribe=One-Click` to this URL without a
 * session; the URL carries a signed token, so CSRF does not apply here.
 * A plain GET (a person clicking the link, or a link scanner) only shows a
 * confirmation page and never unsubscribes by itself.
 */
final class UnsubscribeController extends Controller
{
    public function show(Request $request): Response
    {
        $res = $this->public('GET', '/v1/unsubscribe/' . $request->param('token'));
        if ($res->status === 404) {
            return $this->render('public/unsubscribe', ['title' => 'Unsubscribe', 'state' => 'invalid', 'noindex' => true], 'minimal', 404);
        }
        $this->must($res);
        return $this->render('public/unsubscribe', [
            'title' => 'Unsubscribe',
            'noindex' => true,
            'state' => $res->get('unsubscribed') ? 'done' : 'confirm',
            'info' => $res->json(),
            'token' => $request->param('token'),
        ], 'minimal');
    }

    public function confirm(Request $request): Response
    {
        $res = $this->public('POST', '/v1/unsubscribe/' . $request->param('token'));
        $oneClick = trim($request->rawBody()) === 'List-Unsubscribe=One-Click' || $request->input('List-Unsubscribe') === 'One-Click';
        if ($oneClick) {
            // Machine request from a mailbox provider: a bare status is enough.
            return Response::text($res->ok() ? 'Unsubscribed' : 'Not found', $res->ok() ? 200 : 404);
        }
        if ($res->status === 404) {
            return $this->render('public/unsubscribe', ['title' => 'Unsubscribe', 'state' => 'invalid', 'noindex' => true], 'minimal', 404);
        }
        $this->must($res);
        return $this->render('public/unsubscribe', ['title' => 'Unsubscribed', 'state' => 'done', 'info' => $res->json(), 'noindex' => true], 'minimal');
    }
}
