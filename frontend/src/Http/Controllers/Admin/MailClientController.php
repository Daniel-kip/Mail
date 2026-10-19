<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Server settings shown to mail apps (platform default and per-domain
 * overrides, versioned) and the onboarding analytics.
 */
final class MailClientController extends Controller
{
    public function index(Request $request): Response
    {
        $settings = $this->must($this->console('GET', '/v1/admin/mail-client-settings'))->json();
        return $this->render('admin/mail-clients', ['title' => 'Mail app settings', 's' => $settings, 'area' => 'admin'], 'admin');
    }

    public function saveDefault(Request $request): Response
    {
        $res = $this->console('PUT', '/v1/admin/mail-client-settings', ['json' => self::profile($request)]);
        if (!$res->ok()) {
            return $this->failed($request, '/admin/mail-clients', $res);
        }
        $this->flash('success', 'Platform default saved as a new version. Guides, autoconfig and profiles use it immediately.');
        return $this->redirect('/admin/mail-clients');
    }

    public function saveDomain(Request $request): Response
    {
        $domainId = $request->string('domain_id');
        if (!preg_match('/^[0-9a-f-]{36}$/i', $domainId)) {
            return $this->invalid($request, '/admin/mail-clients', ['domain_id' => 'Enter the domain ID (shown on the domain page).']);
        }
        $res = $this->console('PUT', '/v1/admin/mail-client-settings/domains/' . $domainId, ['json' => self::profile($request)]);
        if (!$res->ok()) {
            return $this->failed($request, '/admin/mail-clients', $res);
        }
        $this->flash('success', 'Domain override saved.');
        return $this->redirect('/admin/mail-clients');
    }

    public function deleteDomain(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/admin/mail-client-settings/domains/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Override removed; the domain uses the platform default.' : self::describe($res));
        return $this->redirect('/admin/mail-clients');
    }

    public function onboarding(Request $request): Response
    {
        $m = $this->must($this->console('GET', '/v1/admin/onboarding', ['query' => ['days' => (int) $request->query('days', 30) ?: 30]]))->json();
        return $this->render('admin/onboarding', ['title' => 'Onboarding analytics', 'm' => $m, 'area' => 'admin'], 'admin');
    }

    private static function profile(Request $request): array
    {
        $alt = $request->string('smtp_alt_port');
        return [
            'imap_host' => strtolower($request->string('imap_host')),
            'imap_port' => (int) $request->string('imap_port'),
            'smtp_host' => strtolower($request->string('smtp_host')),
            'smtp_port' => (int) $request->string('smtp_port'),
            'smtp_security' => $request->string('smtp_security') === 'starttls' ? 'starttls' : 'tls',
            'smtp_alt_port' => $alt !== '' ? (int) $alt : null,
            'smtp_alt_security' => $alt !== '' ? ($request->string('smtp_alt_security') === 'tls' ? 'tls' : 'starttls') : null,
            'change_note' => $request->string('change_note') ?: null,
        ];
    }
}
