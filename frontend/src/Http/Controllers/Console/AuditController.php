<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * The workspace audit log and its CSV export.
 */
final class AuditController extends Controller
{
    public function index(Request $request): Response
    {
        $query = array_filter(['action' => trim((string) $request->query('action', '')) ?: null, 'before_id' => ctype_digit((string) $request->query('before_id', '')) ? $request->query('before_id') : null, 'limit' => 50]);
        $res = $this->must($this->console('GET', '/v1/audit-logs', ['query' => $query]));
        return $this->render('console/audit', ['title' => 'Audit log', 'rows' => $res->items(), 'next' => $res->get('next_before_id'), 'action' => $query['action'] ?? '']);
    }

    public function export(Request $request): Response
    {
        $res = $this->console('GET', '/v1/audit-logs/export.csv', ['query' => array_filter(['action' => $request->query('action')])]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/audit');
        }
        return Response::download($res->body, 'audit-log-' . date('Y-m-d') . '.csv', 'text/csv; charset=utf-8');
    }
}
