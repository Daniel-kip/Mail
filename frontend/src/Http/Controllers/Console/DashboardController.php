<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Overview, usage and the developer quickstart.
 */
final class DashboardController extends Controller
{
    public function index(Request $request): Response
    {
        $usage = $this->can($request, 'usage:read') ? $this->console('GET', '/v1/usage') : null;
        $domains = $this->can($request, 'domains:read') ? $this->console('GET', '/v1/domains') : null;
        $messages = $this->can($request, 'messages:read') ? $this->console('GET', '/v1/messages', ['query' => ['limit' => 6]]) : null;
        $keys = $this->can($request, 'api_keys:manage') ? $this->console('GET', '/v1/api-keys') : null;
        $mailboxes = $this->isInstitution($request) && $this->can($request, 'mailboxes:read') ? $this->console('GET', '/v1/mailboxes/onboarding') : null;
        $u = $usage?->ok() ? $usage->json() : null;
        $domainList = $domains?->ok() ? $domains->items() : [];
        $checklist = [
            ['Add a sending domain', $domainList !== [], '/console/domains', 'domains:read'],
            ['Verify DNS (SPF, DKIM, DMARC)', count(array_filter($domainList, static fn ($d) => $d['status'] === 'verified')) > 0, '/console/domains', 'domains:read'],
            ['Create an API key', $keys?->ok() && $keys->items() !== [], '/console/api-keys', 'api_keys:manage'],
            ['Send your first message', ($u['last_7_days']['sent'] ?? 0) > 0 || ($messages?->ok() && $messages->items() !== []), '/console/messages/compose', 'messages:send'],
        ];
        if ($this->isInstitution($request)) {
            $checklist[] = ['Create work mailboxes for your people', ($u['totals']['mailboxes'] ?? 0) > 0, '/console/mailboxes/new', 'mailboxes:write'];
        }
        return $this->render('console/dashboard', [
            'title' => 'Overview',
            'usage' => $u,
            'domains' => $domainList,
            'messages' => $messages?->ok() ? $messages->items() : [],
            'onboarding' => $mailboxes?->ok() ? $mailboxes->json() : null,
            'checklist' => array_values(array_filter($checklist, fn ($c) => $this->can($request, $c[3]))),
        ]);
    }

    public function usage(Request $request): Response
    {
        $usage = $this->must($this->console('GET', '/v1/usage'))->json();
        return $this->render('console/usage', ['title' => 'Usage', 'usage' => $usage]);
    }

    public function developers(Request $request): Response
    {
        $domains = $this->can($request, 'domains:read') ? $this->console('GET', '/v1/domains') : null;
        $verified = array_values(array_filter($domains?->ok() ? $domains->items() : [], static fn ($d) => $d['status'] === 'verified'));
        return $this->render('console/developers', [
            'title' => 'Developer quickstart',
            'apiUrl' => rtrim((string) ($this->app->config->get('api.public_url') ?: $this->app->config->get('api.url')), '/'),
            'domain' => $verified[0]['domain'] ?? 'yourdomain.com',
        ]);
    }
}
