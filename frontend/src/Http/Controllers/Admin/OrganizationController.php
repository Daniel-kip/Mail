<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Tenant directory: plans, suspension and activity.
 */
final class OrganizationController extends Controller
{
    public function index(Request $request): Response
    {
        $q = trim((string) $request->query('q', ''));
        $res = $this->must($this->console('GET', '/v1/admin/organizations', ['query' => array_filter(['q' => $q ?: null])]));
        return $this->render('admin/organizations', ['title' => 'Organizations', 'orgs' => $res->items(), 'q' => $q, 'area' => 'admin'], 'admin');
    }

    public function show(Request $request): Response
    {
        $org = $this->must($this->console('GET', '/v1/admin/organizations/' . $request->param('id')))->json();
        $plans = $this->console('GET', '/v1/admin/plans');
        return $this->render('admin/organization', [
            'title' => $org['name'], 'org' => $org, 'plans' => $plans->ok() ? $plans->items() : [], 'area' => 'admin',
            'crumbs' => [['Organizations', '/admin/organizations'], [$org['name'], null]],
        ], 'admin');
    }

    public function plan(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('PATCH', '/v1/admin/organizations/' . $id, ['json' => ['plan' => $request->string('plan')]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Plan changed. No invoice is issued for operator changes.' : self::describe($res));
        return $this->redirect('/admin/organizations/' . $id);
    }

    public function suspend(Request $request): Response
    {
        $id = $request->param('id');
        if (trim($request->string('reason')) === '') {
            $this->flash('error', 'Give a reason; the organization sees it.');
            return $this->redirect('/admin/organizations/' . $id);
        }
        $res = $this->console('POST', '/v1/admin/organizations/' . $id . '/suspend', ['json' => ['reason' => $request->string('reason')]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Organization suspended. Sending and changes are blocked.' : self::describe($res));
        return $this->redirect('/admin/organizations/' . $id);
    }

    public function unsuspend(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/admin/organizations/' . $id . '/unsuspend', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Organization reinstated.' : self::describe($res));
        return $this->redirect('/admin/organizations/' . $id);
    }
}
