<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Workspace settings: name, plan limits, employee support contacts,
 * conversion to an institution and leaving the workspace.
 */
final class SettingsController extends Controller
{
    public function index(Request $request): Response
    {
        $org = $this->must($this->console('GET', '/v1/organization'))->json();
        $support = $this->console('GET', '/v1/organization/support');
        return $this->render('console/settings', ['title' => 'Settings', 'org' => $org, 'support' => $support->ok() ? $support->json() : []]);
    }

    public function organization(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['name' => 'required|max:200']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/settings', $errors);
        }
        $res = $this->console('PATCH', '/v1/organization', ['json' => ['name' => $request->string('name')]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/settings', $res);
        }
        $this->flash('success', 'Workspace name saved.');
        return $this->redirect('/console/settings');
    }

    public function support(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['email' => 'nullable|email', 'phone' => 'nullable|max:40', 'url' => 'nullable|https', 'hours' => 'nullable|max:120', 'setup_help_text' => 'nullable|max:1000'], ['url' => 'Help page', 'setup_help_text' => 'Message']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/settings#support', $errors);
        }
        $res = $this->console('PUT', '/v1/organization/support', ['json' => $request->only('email', 'phone', 'url', 'hours', 'setup_help_text')]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/settings#support', $res);
        }
        $this->flash('success', 'Support contacts saved. Employees see them in the phone setup guide and on welcome sheets.');
        return $this->redirect('/console/settings#support');
    }

    public function convert(Request $request): Response
    {
        $res = $this->console('POST', '/v1/organization/convert', ['json' => new \stdClass()]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/settings');
        }
        $this->flash('success', 'This workspace is now an institution. Verify a domain and turn on mail hosting to create mailboxes.');
        return $this->redirect('/console/domains');
    }

    public function leave(Request $request): Response
    {
        $userId = $this->auth($request)['user']['id'] ?? '';
        $res = $this->console('DELETE', '/v1/members/' . $userId);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res, 'You could not leave this workspace.'));
            return $this->redirect('/console/settings');
        }
        $this->flash('success', 'You left the workspace.');
        return $this->redirect('/console');
    }
}
