<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Creating and switching workspaces (organizations).
 */
final class OrganizationController extends Controller
{
    public function create(Request $request): Response
    {
        return $this->render('console/organization-new', ['title' => 'New workspace', 'narrow' => true]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['name' => 'required|max:200', 'account_type' => 'required|in:individual,institution'], ['name' => 'Name', 'account_type' => 'Account type']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/organizations/new', $errors);
        }
        $res = $this->console('POST', '/v1/organizations', ['json' => ['name' => $request->string('name'), 'account_type' => $request->string('account_type')]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/organizations/new', $res);
        }
        $this->console('POST', '/v1/auth/switch-organization', ['json' => ['organization_id' => $res->get('id')]]);
        $this->flash('success', 'Workspace "' . $request->string('name') . '" created.');
        return $this->redirect('/console');
    }

    public function switch(Request $request): Response
    {
        $res = $this->console('POST', '/v1/auth/switch-organization', ['json' => ['organization_id' => $request->string('organization_id')]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        }
        return $this->redirect('/console');
    }
}
