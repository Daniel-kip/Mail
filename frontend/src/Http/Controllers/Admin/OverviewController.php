<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Platform health at a glance and the dead-letter queue.
 */
final class OverviewController extends Controller
{
    public function index(Request $request): Response
    {
        $o = $this->must($this->console('GET', '/v1/admin/overview'))->json();
        return $this->render('admin/overview', ['title' => 'Operator overview', 'o' => $o, 'area' => 'admin'], 'admin');
    }

    public function security(Request $request): Response
    {
        $s = $this->must($this->console('GET', '/v1/admin/security'))->json();
        return $this->render('admin/security', ['title' => 'Security', 's' => $s, 'area' => 'admin'], 'admin');
    }

    public function queue(Request $request): Response
    {
        $jobs = $this->must($this->console('GET', '/v1/admin/jobs/dead'))->items();
        return $this->render('admin/queue', ['title' => 'Failed jobs', 'jobs' => $jobs, 'area' => 'admin'], 'admin');
    }

    public function requeue(Request $request): Response
    {
        $res = $this->console('POST', '/v1/admin/jobs/' . $request->param('id') . '/requeue', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Job requeued.' : self::describe($res));
        return $this->redirect('/admin/queue');
    }
}
