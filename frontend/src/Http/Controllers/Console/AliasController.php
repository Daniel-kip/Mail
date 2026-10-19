<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Shared addresses (info@, admissions@) that deliver to one or more mailboxes.
 */
final class AliasController extends Controller
{
    public function index(Request $request): Response
    {
        $aliases = $this->must($this->console('GET', '/v1/aliases'))->items();
        $domains = array_values(array_filter($this->console('GET', '/v1/domains')->items(), static fn ($d) => !empty($d['mail_hosting_enabled'])));
        $mailboxes = $this->console('GET', '/v1/mailboxes')->items();
        return $this->render('console/aliases', ['title' => 'Aliases', 'aliases' => $aliases, 'domains' => $domains, 'mailboxes' => $mailboxes]);
    }

    public function store(Request $request): Response
    {
        $destinations = self::destinations($request);
        $errors = Validator::make($request->all(), ['local_part' => 'required|localpart', 'domain_id' => 'required'], ['local_part' => 'Address']);
        if ($destinations === []) {
            $errors['destinations'] = 'Choose at least one mailbox to deliver to.';
        }
        if ($errors !== []) {
            return $this->invalid($request, '/console/aliases', $errors);
        }
        $res = $this->console('POST', '/v1/aliases', ['json' => ['domain_id' => $request->string('domain_id'), 'local_part' => strtolower($request->string('local_part')), 'destinations' => $destinations]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/aliases', $res);
        }
        $this->flash('success', 'Alias ' . $res->get('address') . ' created.');
        return $this->redirect('/console/aliases');
    }

    public function update(Request $request): Response
    {
        $payload = [];
        if ($request->input('destinations') !== null) {
            $payload['destinations'] = self::destinations($request);
        }
        if (in_array($request->string('status'), ['active', 'disabled'], true)) {
            $payload['status'] = $request->string('status');
        }
        $res = $this->console('PATCH', '/v1/aliases/' . $request->param('id'), ['json' => $payload]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Alias updated.' : self::describe($res));
        return $this->redirect('/console/aliases');
    }

    public function destroy(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/aliases/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Alias deleted.' : self::describe($res));
        return $this->redirect('/console/aliases');
    }

    /** @return list<string> */
    private static function destinations(Request $request): array
    {
        $raw = $request->input('destinations', []);
        $list = is_array($raw) ? $raw : preg_split('/[\s,;]+/', (string) $raw);
        return array_values(array_unique(array_filter(array_map(static fn ($v) => strtolower(trim((string) $v)), $list ?: []))));
    }
}
