<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Members, roles and invitations.
 */
final class TeamController extends Controller
{
    public const ROLES = ['viewer', 'support', 'developer', 'admin', 'owner'];

    public function index(Request $request): Response
    {
        $res = $this->must($this->console('GET', '/v1/members'));
        return $this->render('console/team', ['title' => 'Team', 'members' => $res->items(), 'invitations' => (array) $res->get('invitations', [])]);
    }

    public function invite(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['email' => 'required|email', 'role' => 'required|in:' . implode(',', self::ROLES)]);
        if ($errors !== []) {
            return $this->invalid($request, '/console/team', $errors);
        }
        $res = $this->console('POST', '/v1/members/invitations', ['json' => ['email' => $request->string('email'), 'role' => $request->string('role')]]);
        if (!$res->ok()) {
            if ($res->status === 429 && $res->get('error.details.limit') === 'members') {
                $this->flash('error', 'Your plan\'s member limit is reached. Upgrade the plan or remove a member first.');
                return $this->redirect('/console/team');
            }
            return $this->failed($request, '/console/team', $res);
        }
        $this->flash('success', 'Invitation sent to ' . $request->string('email') . '. It expires in 7 days.');
        return $this->redirect('/console/team');
    }

    public function revokeInvitation(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/members/invitations/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Invitation revoked.' : self::describe($res));
        return $this->redirect('/console/team');
    }

    public function role(Request $request): Response
    {
        $res = $this->console('PATCH', '/v1/members/' . $request->param('user'), ['json' => ['role' => $request->string('role')]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Role changed.' : self::describe($res));
        return $this->redirect('/console/team');
    }

    public function remove(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/members/' . $request->param('user'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Member removed. Their sessions for this workspace ended.' : self::describe($res));
        return $this->redirect('/console/team');
    }
}
