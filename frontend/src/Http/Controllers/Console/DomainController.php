<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Sending domains: DNS records, verification and mail hosting.
 */
final class DomainController extends Controller
{
    public function index(Request $request): Response
    {
        $res = $this->must($this->console('GET', '/v1/domains'));
        return $this->render('console/domains/index', ['title' => 'Domains', 'domains' => $res->items()]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['domain' => 'required|domain'], ['domain' => 'Domain']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/domains', $errors);
        }
        $res = $this->console('POST', '/v1/domains', ['json' => ['domain' => strtolower($request->string('domain'))]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/domains', $res);
        }
        $this->flash('success', 'Domain added. Publish the DNS records below, then select "Check DNS".');
        return $this->redirect('/console/domains/' . $res->get('id'));
    }

    public function show(Request $request): Response
    {
        $domain = $this->must($this->console('GET', '/v1/domains/' . $request->param('id')))->json();
        return $this->render('console/domains/show', ['title' => $domain['domain'], 'domain' => $domain, 'crumbs' => [['Domains', '/console/domains'], [$domain['domain'], null]]]);
    }

    public function verify(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/domains/' . $id . '/verification-check', ['json' => new \stdClass()]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } elseif ($res->get('status') === 'verified') {
            $this->flash('success', 'All required records were found. The domain is verified.');
        } else {
            $this->flash('warning', 'Some records were not found yet. DNS changes can take up to an hour to appear; check again shortly.');
        }
        return $this->redirect('/console/domains/' . $id);
    }

    public function hosting(Request $request): Response
    {
        $id = $request->param('id');
        $on = $request->bool('enabled');
        $res = $this->console('PATCH', '/v1/domains/' . $id, ['json' => ['mail_hosting_enabled' => $on]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? ($on ? 'Mail hosting enabled. Publish the MX record so mail reaches your mailboxes.' : 'Mail hosting disabled.') : self::describe($res));
        return $this->redirect('/console/domains/' . $id);
    }

    public function destroy(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/domains/' . $request->param('id'));
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/domains/' . $request->param('id'));
        }
        $this->flash('success', 'Domain removed.');
        return $this->redirect('/console/domains');
    }
}
