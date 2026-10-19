<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Addresses that will not be sent to (bounces, complaints, unsubscribes, manual).
 */
final class SuppressionController extends Controller
{
    public function index(Request $request): Response
    {
        $query = array_filter(['q' => trim((string) $request->query('q', '')) ?: null, 'reason' => $request->query('reason') ?: null, 'cursor' => $request->query('cursor') ?: null, 'limit' => 50]);
        $res = $this->must($this->console('GET', '/v1/suppressions', ['query' => $query]));
        return $this->render('console/suppressions', ['title' => 'Suppressions', 'rows' => $res->items(), 'next' => $res->get('next_cursor'), 'filters' => $query]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['email' => 'required|email', 'details' => 'nullable|max:500']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/suppressions', $errors);
        }
        $res = $this->console('POST', '/v1/suppressions', ['json' => ['email' => $request->string('email'), 'reason' => 'manual', 'details' => $request->string('details') ?: null]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/suppressions', $res);
        }
        $this->flash('success', $request->string('email') . ' will no longer receive mail from this workspace.');
        return $this->redirect('/console/suppressions');
    }

    public function destroy(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/suppressions/' . rawurlencode($request->string('email')));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Removed. Mail to this address will be attempted again.' : self::describe($res));
        return $this->redirect('/console/suppressions');
    }
}
