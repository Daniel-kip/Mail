<?php

declare(strict_types=1);

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Joining an organization from an emailed invitation. Accepting requires
 * being signed in with the invited address.
 */
final class InvitationController extends Controller
{
    public function show(Request $request): Response
    {
        $token = $request->param('token');
        $res = $this->public('GET', '/v1/invitations/' . $token);
        if (!$res->ok()) {
            return $this->render('auth/invitation', ['title' => 'Invitation', 'invitation' => null, 'token' => $token, 'signedIn' => false], 'minimal', $res->status === 404 ? 404 : 200);
        }
        $signedIn = false;
        $email = null;
        if ($this->app->api()->hasSession('console')) {
            $session = $this->console('GET', '/v1/auth/session');
            if ($session->ok()) {
                $signedIn = true;
                $email = $session->get('user.email');
            }
        }
        return $this->render('auth/invitation', [
            'title' => 'Join ' . $res->get('organization_name'),
            'invitation' => $res->json(),
            'token' => $token,
            'signedIn' => $signedIn,
            'signedInEmail' => $email,
        ], 'minimal');
    }

    public function accept(Request $request): Response
    {
        $token = $request->param('token');
        $res = $this->console('POST', '/v1/invitations/' . $token . '/accept', ['json' => new \stdClass()]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res, 'The invitation could not be accepted.'));
            return $this->redirect('/invitations/' . $token);
        }
        $this->flash('success', 'You joined the organization as ' . $res->get('role') . '.');
        return $this->redirect('/console');
    }
}
