<?php

declare(strict_types=1);

namespace App\Http\Controllers\PublicSite;

use App\Api\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Public pages: product, pricing (live plan catalogue), developer
 * reference, security and legal pages, and the health check.
 */
final class HomeController extends Controller
{
    public function index(Request $request): Response
    {
        return $this->render('public/home', [
            'title' => '',
            'plans' => $this->plans(),
            'apiUrl' => $this->app->config->string('api.public_url'),
            'scripts' => ['js/home-video.js', 'js/sim.js'],
            'bodyClass' => 'page-home',
        ], 'public');
    }

    public function pricing(Request $request): Response
    {
        $audience = $request->query('for') === 'developers' ? 'individual' : 'institution';
        return $this->render('public/pricing', [
            'title' => 'Pricing',
            'description' => 'Plans for individual developers and for institutions hosting work email.',
            'audience' => $audience,
            'catalogue' => $this->plans(),
        ], 'public');
    }

    public function developers(Request $request): Response
    {
        return $this->render('public/developers', [
            'title' => 'API reference',
            'description' => 'Send transactional email with a REST API: authentication, sending, templates, webhooks, errors.',
            'apiUrl' => $this->app->config->string('api.public_url'),
        ], 'public');
    }

    public function security(Request $request): Response
    {
        return $this->render('public/security', ['title' => 'Security'], 'public');
    }

    public function privacy(Request $request): Response
    {
        return $this->render('public/privacy', ['title' => 'Privacy notice'], 'public');
    }

    public function terms(Request $request): Response
    {
        return $this->render('public/terms', ['title' => 'Terms of service'], 'public');
    }

    /** Liveness of this web tier plus reachability of the platform API. */
    /** RFC 9116 security contact, valid for one year from each request. */
    public function securityTxt(Request $request): Response
    {
        $email = (string) $this->app->config->get('support_email');
        $url = rtrim((string) $this->app->config->get('url'), '/');
        $lines = array_filter([
            $email !== '' ? 'Contact: mailto:' . $email : null,
            'Expires: ' . gmdate('Y-m-d\TH:i:s\Z', time() + 365 * 86400),
            'Preferred-Languages: en, sw',
            'Canonical: ' . $url . '/.well-known/security.txt',
            'Policy: ' . $url . '/security',
        ]);
        return Response::text(implode("\n", $lines) . "\n")->withHeader('Cache-Control', 'public, max-age=86400');
    }

    public function health(Request $request): Response
    {
        try {
            $api = $this->public('GET', '/readyz', ['timeout' => 5]);
            $apiOk = $api->ok();
        } catch (ApiException) {
            $apiOk = false;
        }
        return Response::json(['status' => $apiOk ? 'ok' : 'degraded', 'web' => 'ok', 'api' => $apiOk ? 'ok' : 'unreachable'], $apiOk ? 200 : 503);
    }

    /** @return array{currency: string, tax: array, data: list<array>} */
    private function plans(): array
    {
        try {
            $res = $this->public('GET', '/v1/plans', ['timeout' => 8]);
            if ($res->ok()) {
                return $res->json();
            }
        } catch (ApiException) {
            // The marketing pages still render without live prices.
        }
        return ['currency' => 'KES', 'tax' => ['rate_bps' => 0, 'label' => 'Tax'], 'data' => []];
    }
}
