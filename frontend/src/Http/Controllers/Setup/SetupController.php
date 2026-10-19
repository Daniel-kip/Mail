<?php

declare(strict_types=1);

namespace App\Http\Controllers\Setup;

use App\Http\Controllers\Controller;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;
use App\Mail\AppleProfileSigner;
use App\Mail\GuideCatalog;
use App\View\QrCode;

/**
 * "Connect my work email to my phone": a guided, stateless wizard.
 *
 * A. welcome → B. device → C. app → D. readiness check → E. guide steps
 * → F. connection test (VerifyController) → G. help topics.
 *
 * Settings always come from the API's validated client profile; nothing
 * the user types is ever used as a server name. Passwords are never
 * asked for, shown or logged here.
 */
final class SetupController extends Controller
{
    public function welcome(Request $request): Response
    {
        $setup = $this->setup();
        return $this->render('setup/welcome', ['title' => 'Connect your work email', 'stage' => 'welcome', 'setup' => $setup], 'setup');
    }

    public function device(Request $request): Response
    {
        $this->track('guide_started');
        return $this->render('setup/device', ['title' => 'Which device?', 'stage' => 'device', 'devices' => $this->catalog()->devices(), 'suggested' => self::sniffDevice($request)], 'setup');
    }

    public function app(Request $request): Response
    {
        $device = (string) $request->query('device', '');
        if (!$this->catalog()->hasDevice($device)) {
            return $this->redirect('/mail/setup/device');
        }
        $this->track('device_selected', $device);
        return $this->render('setup/app', ['title' => 'Which email app?', 'stage' => 'app', 'device' => $device, 'deviceInfo' => $this->catalog()->devices()[$device], 'apps' => $this->catalog()->appsFor($device)], 'setup');
    }

    public function check(Request $request): Response
    {
        $device = (string) $request->query('device', '');
        $app = (string) $request->query('app', '');
        $guide = $this->catalog()->guideFor($device, $app);
        if ($guide === null) {
            return $this->redirect('/mail/setup/device');
        }
        $this->track('client_selected', $device, $app);
        $readiness = $this->mailbox('GET', '/v1/webmail/setup/readiness');
        return $this->render('setup/check', [
            'title' => 'Checking your account', 'stage' => 'check', 'device' => $device, 'app' => $app, 'guide' => $guide,
            'readiness' => $readiness->ok() ? $readiness->json() : null,
            'readinessError' => $readiness->ok() ? null : self::describe($readiness),
            'setup' => $this->setup(),
        ], 'setup');
    }

    public function guide(Request $request): Response
    {
        $guide = $this->catalog()->guide($request->param('guide'));
        if ($guide === null) {
            throw HttpException::notFound();
        }
        $setup = $this->setup();
        $steps = $guide['steps'];
        $step = max(1, min(count($steps), (int) $request->query('step', 1)));
        return $this->render('setup/guide', [
            'title' => $guide['title'], 'stage' => 'guide', 'guide' => $guide, 'step' => $step, 'total' => count($steps),
            's' => $steps[$step - 1], 'setup' => $setup, 'placeholders' => GuideCatalog::placeholders($setup), 'fields' => GuideCatalog::fields($setup),
            'helpTopics' => $this->catalog()->helpTopics(),
        ], 'setup');
    }

    public function done(Request $request): Response
    {
        $guide = $this->catalog()->guide($request->param('guide'));
        if ($guide === null) {
            throw HttpException::notFound();
        }
        $this->track('guide_completed', $guide['device'] ?? null, $guide['app'] ?? null);
        return $this->redirect('/mail/setup/verify?' . http_build_query(['guide' => $guide['id']]));
    }

    public function settings(Request $request): Response
    {
        $setup = $this->setup();
        return $this->render('setup/settings', ['title' => 'Server settings', 'setup' => $setup, 'fields' => GuideCatalog::fields($setup)], 'setup');
    }

    public function help(Request $request): Response
    {
        return $this->render('setup/help', ['title' => 'Help with email setup', 'topics' => $this->catalog()->helpTopics(), 'setup' => $this->setup(), 'reviewed' => $this->catalog()->reviewed()], 'setup');
    }

    public function helpTopic(Request $request): Response
    {
        $slug = $request->param('topic');
        $topic = $this->catalog()->help($slug);
        if ($topic === null) {
            throw HttpException::notFound();
        }
        $setup = $this->setup();
        $this->track('help_viewed', null, null, $slug);
        return $this->render('setup/help-topic', ['title' => $topic['title'], 'topic' => $topic, 'slug' => $slug, 'setup' => $setup, 'placeholders' => GuideCatalog::placeholders($setup)], 'setup');
    }

    /** Apple configuration profile (no password inside), optionally signed. */
    public function profile(Request $request): Response
    {
        $res = $this->mailbox('GET', '/v1/webmail/setup/apple-profile');
        $this->must($res);
        [$body, $signed] = $this->app->get(AppleProfileSigner::class)->sign($res->body);
        $name = preg_match('/filename="([^"]+)"/', (string) $res->header('content-disposition'), $m) ? $m[1] : 'work-email.mobileconfig';
        return (new Response($body, 200, [
            'Content-Type' => 'application/x-apple-aspen-config',
            'Content-Disposition' => 'attachment; filename="' . str_replace('"', '', $name) . '"',
            'X-Profile-Signed' => $signed ? 'yes' : 'no',
            'Cache-Control' => 'no-store',
        ]));
    }

    /** QR code that opens this setup guide on a phone (no secrets in it). */
    public function qr(Request $request): Response
    {
        $url = rtrim((string) $this->app->config->get('url'), '/') . '/mail/setup';
        return (new Response(QrCode::svg($url, 6, 'Scan to open the setup guide on your phone'), 200, ['Content-Type' => 'image/svg+xml', 'Cache-Control' => 'private, max-age=300']));
    }

    /** Analytics beacon from the guide pages (JSON, CSRF-protected). */
    public function event(Request $request): Response
    {
        $ok = $this->track($request->string('event'), $request->string('device') ?: null, $request->string('client') ?: null, $request->string('detail') ?: null);
        // A lost analytics event is never the visitor's problem: always 202.
        return Response::json(['ok' => $ok], 202);
    }

    /** Record a setup step; analytics never block the guide. */
    private function track(string $event, ?string $device = null, ?string $client = null, ?string $detail = null): bool
    {
        $detail = $detail !== null ? substr(preg_replace('/[^a-z0-9_-]/i', '', $detail) ?? '', 0, 60) : null;
        $payload = array_filter(['event' => $event, 'device' => $device, 'client' => $client, 'detail' => $detail], static fn ($v) => $v !== null && $v !== '');
        try {
            return $this->mailbox('POST', '/v1/webmail/setup/events', ['json' => $payload, 'timeout' => 3])->ok();
        } catch (\App\Api\ApiException) {
            return false;
        }
    }

    private function setup(): array
    {
        return $this->must($this->mailbox('GET', '/v1/webmail/setup'))->json();
    }

    private function catalog(): GuideCatalog
    {
        return $this->app->get(GuideCatalog::class);
    }

    private static function sniffDevice(Request $request): ?string
    {
        $ua = $request->header('User-Agent');
        return match (true) {
            (bool) preg_match('/iPhone|iPad|iPod/i', $ua) => 'ios',
            (bool) preg_match('/Android/i', $ua) => 'android',
            (bool) preg_match('/Windows|Macintosh|Linux|CrOS/i', $ua) => 'computer',
            default => null,
        };
    }
}
