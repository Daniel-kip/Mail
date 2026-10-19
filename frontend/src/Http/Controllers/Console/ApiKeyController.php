<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * API keys. The secret is shown once after creation or rotation and is
 * never stored by the frontend beyond that single flash.
 */
final class ApiKeyController extends Controller
{
    public function index(Request $request): Response
    {
        $keys = $this->must($this->console('GET', '/v1/api-keys'))->items();
        $scopes = $this->console('GET', '/v1/api-keys/scopes');
        $domains = $this->console('GET', '/v1/domains');
        $all = $scopes->ok() ? (array) $scopes->get('scopes', []) : ['messages:send', 'messages:read'];
        if (!$this->isInstitution($request)) {
            $all = array_values(array_filter($all, static fn ($s) => !str_starts_with($s, 'mailboxes:')));
        }
        return $this->render('console/api-keys', [
            'title' => 'API keys',
            'keys' => $keys,
            'scopes' => $all,
            'defaultScopes' => $scopes->ok() ? (array) $scopes->get('default', []) : ['messages:send'],
            'domains' => $domains->ok() ? array_column($domains->items(), 'domain') : [],
            'newKey' => $this->revealed('api_key'),
            'editing' => (string) $request->query('edit', ''),
        ]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['name' => 'required|max:100', 'environment' => 'required|in:live,test', 'description' => 'nullable|max:500', 'expires_at' => 'nullable|date'], ['expires_at' => 'Expiry date']);
        $scopes = array_values(array_filter((array) $request->input('scopes', []), 'is_string'));
        if ($scopes === []) {
            $errors['scopes'] = 'Choose at least one permission.';
        }
        if ($errors !== []) {
            return $this->invalid($request, '/console/api-keys', $errors);
        }
        $payload = [
            'name' => $request->string('name'),
            'environment' => $request->string('environment'),
            'scopes' => $scopes,
            'description' => $request->string('description') ?: null,
            'allowed_ips' => self::list($request->string('allowed_ips')),
            'allowed_sender_domains' => self::list(strtolower($request->string('allowed_sender_domains'))),
        ];
        if ($request->string('expires_at') !== '') {
            $payload['expires_at'] = (new \DateTimeImmutable($request->string('expires_at') . ' 23:59:59'))->format(DATE_ATOM);
        }
        $res = $this->console('POST', '/v1/api-keys', ['json' => $payload]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/api-keys', $res);
        }
        $this->reveal('api_key', ['name' => $res->get('name'), 'key' => $res->get('key')]);
        return $this->redirect('/console/api-keys');
    }

    public function update(Request $request): Response
    {
        $res = $this->console('PATCH', '/v1/api-keys/' . $request->param('id'), ['json' => [
            'name' => $request->string('name'),
            'description' => $request->string('description'),
            'allowed_ips' => self::list($request->string('allowed_ips')),
            'allowed_sender_domains' => self::list(strtolower($request->string('allowed_sender_domains'))),
        ]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/api-keys?edit=' . $request->param('id'), $res);
        }
        $this->flash('success', 'Key updated. Restrictions apply to the next request.');
        return $this->redirect('/console/api-keys');
    }

    public function rotate(Request $request): Response
    {
        $res = $this->console('POST', '/v1/api-keys/' . $request->param('id') . '/rotate', ['json' => new \stdClass()]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/api-keys');
        }
        $this->reveal('api_key', ['name' => $res->get('name'), 'key' => $res->get('key'), 'rotated' => true]);
        return $this->redirect('/console/api-keys');
    }

    public function revoke(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/api-keys/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Key revoked. Requests using it now fail with 401.' : self::describe($res));
        return $this->redirect('/console/api-keys');
    }

    /** @return list<string> */
    private static function list(string $value): array
    {
        return array_values(array_unique(array_filter(array_map('trim', preg_split('/[\s,;]+/', $value) ?: []))));
    }
}
