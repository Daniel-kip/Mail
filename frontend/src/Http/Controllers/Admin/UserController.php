<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Console user accounts across all tenants.
 */
final class UserController extends Controller
{
    public function index(Request $request): Response
    {
        $q = trim((string) $request->query('q', ''));
        $res = $this->must($this->console('GET', '/v1/admin/users', ['query' => array_filter(['q' => $q ?: null])]));
        return $this->render('admin/users', ['title' => 'Users', 'users' => $res->items(), 'q' => $q, 'area' => 'admin'], 'admin');
    }

    public function disable(Request $request): Response
    {
        $res = $this->console('POST', '/v1/admin/users/' . $request->param('id') . '/disable', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'User disabled and signed out everywhere.' : self::describe($res));
        return $this->redirect('/admin/users');
    }

    public function enable(Request $request): Response
    {
        $res = $this->console('POST', '/v1/admin/users/' . $request->param('id') . '/enable', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'User enabled.' : self::describe($res));
        return $this->redirect('/admin/users');
    }
}
