<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Hosted work mailboxes: create (invite, handover link, set or generated
 * password), bulk import, lifecycle, support assistance and the employee
 * handout. Passwords are never shown except a generated one, exactly once.
 */
final class MailboxController extends Controller
{
    private const SETUP_METHODS = ['invite', 'link', 'password', 'generate'];
    private const IMPORT_LIMIT = 500;

    public function upgrade(Request $request): Response
    {
        if ($this->isInstitution($request)) {
            return $this->redirect('/console/mailboxes');
        }
        return $this->render('console/mailboxes/upgrade', ['title' => 'Hosted mailboxes']);
    }

    public function index(Request $request): Response
    {
        $all = $this->must($this->console('GET', '/v1/mailboxes'))->items();
        $q = strtolower(trim((string) $request->query('q', '')));
        $status = (string) $request->query('status', '');
        $list = array_values(array_filter($all, static function (array $m) use ($q, $status): bool {
            if ($status !== '' && $m['status'] !== $status && !($status === 'not_connected' && ($m['client_setup']['state'] ?? '') !== 'verified')) {
                return false;
            }
            if ($q === '') {
                return true;
            }
            return str_contains(strtolower($m['address'] . ' ' . ($m['display_name'] ?? '') . ' ' . ($m['department'] ?? '')), $q);
        }));
        return $this->render('console/mailboxes/index', ['title' => 'Mailboxes', 'mailboxes' => $list, 'total' => count($all), 'q' => $q, 'status' => $status]);
    }

    public function create(Request $request): Response
    {
        $domains = array_values(array_filter($this->must($this->console('GET', '/v1/domains'))->items(), static fn ($d) => !empty($d['mail_hosting_enabled'])));
        return $this->render('console/mailboxes/create', ['title' => 'New mailbox', 'domains' => $domains, 'crumbs' => [['Mailboxes', '/console/mailboxes'], ['New', null]]]);
    }

    public function store(Request $request): Response
    {
        $method = $request->string('setup_method', 'invite');
        $rules = [
            'domain_id' => 'required',
            'local_part' => 'required|localpart|max:64',
            'display_name' => 'required|max:200',
            'setup_method' => 'required|in:' . implode(',', self::SETUP_METHODS),
            'recovery_email' => ($method === 'invite' ? 'required|' : 'nullable|') . 'email',
            'department' => 'nullable|max:120',
            'job_title' => 'nullable|max:120',
            'quota_mb' => 'nullable|integer',
        ];
        if ($method === 'password') {
            $rules['password'] = 'required|min:10|max:256';
        }
        $errors = Validator::make($request->all(), $rules, [
            'local_part' => 'Address', 'display_name' => 'Full name', 'recovery_email' => 'Personal email', 'domain_id' => 'Domain', 'quota_mb' => 'Storage',
        ]);
        if ($errors !== []) {
            return $this->invalid($request, '/console/mailboxes/new', $errors);
        }
        $payload = array_filter([
            'domain_id' => $request->string('domain_id'),
            'local_part' => strtolower($request->string('local_part')),
            'display_name' => $request->string('display_name'),
            'setup_method' => $method,
            'recovery_email' => $request->string('recovery_email'),
            'department' => $request->string('department'),
            'job_title' => $request->string('job_title'),
            'password' => $method === 'password' ? (string) $request->input('password') : '',
        ], static fn ($v) => $v !== '');
        if ($request->string('quota_mb') !== '') {
            $payload['quota_bytes'] = (int) $request->string('quota_mb') * 1048576;
        }
        $res = $this->console('POST', '/v1/mailboxes', ['json' => $payload]);
        if (!$res->ok()) {
            if ($res->errorCode() === 'address_taken') {
                return $this->invalid($request, '/console/mailboxes/new', ['local_part' => 'This address is already in use.'], 'Choose another address.');
            }
            return $this->failed($request, '/console/mailboxes/new', $res);
        }
        $this->revealHandover($res->json());
        $this->flash('success', 'Mailbox ' . $res->get('address') . ' created.');
        return $this->redirect('/console/mailboxes/' . $res->get('id'));
    }

    public function importForm(Request $request): Response
    {
        $domains = array_values(array_filter($this->must($this->console('GET', '/v1/domains'))->items(), static fn ($d) => !empty($d['mail_hosting_enabled'])));
        return $this->render('console/mailboxes/import', ['title' => 'Import mailboxes', 'domains' => $domains, 'result' => $this->session()->flashed('import_result'), 'crumbs' => [['Mailboxes', '/console/mailboxes'], ['Import', null]]]);
    }

    /**
     * CSV columns: address or local_part, display_name, recovery_email,
     * department, job_title. Every row becomes an emailed invitation (or a
     * handover link when no personal email is given), so no passwords
     * travel in spreadsheets.
     */
    public function import(Request $request): Response
    {
        $file = $request->files()['csv'] ?? null;
        $domainId = $request->string('domain_id');
        if (!is_array($file) || ($file['error'] ?? UPLOAD_ERR_NO_FILE) !== UPLOAD_ERR_OK || $domainId === '') {
            return $this->invalid($request, '/console/mailboxes/import', ['csv' => 'Choose a CSV file and a domain.']);
        }
        if (($file['size'] ?? 0) > 1048576) {
            return $this->invalid($request, '/console/mailboxes/import', ['csv' => 'The file is larger than 1 MB.']);
        }
        $rows = self::parseCsv((string) file_get_contents($file['tmp_name']));
        if ($rows === []) {
            return $this->invalid($request, '/console/mailboxes/import', ['csv' => 'No rows found. The first line must be a header with at least local_part (or address) and display_name.']);
        }
        if (count($rows) > self::IMPORT_LIMIT) {
            return $this->invalid($request, '/console/mailboxes/import', ['csv' => 'At most ' . self::IMPORT_LIMIT . ' rows per import.']);
        }
        $results = [];
        foreach ($rows as $i => $row) {
            $local = strtolower(trim($row['local_part'] ?? strstr((string) ($row['address'] ?? ''), '@', true) ?: ($row['address'] ?? '')));
            $recovery = trim((string) ($row['recovery_email'] ?? $row['personal_email'] ?? ''));
            $payload = array_filter([
                'domain_id' => $domainId,
                'local_part' => $local,
                'display_name' => trim((string) ($row['display_name'] ?? $row['name'] ?? '')),
                'setup_method' => $recovery !== '' ? 'invite' : 'link',
                'recovery_email' => $recovery,
                'department' => trim((string) ($row['department'] ?? '')),
                'job_title' => trim((string) ($row['job_title'] ?? '')),
            ], static fn ($v) => $v !== '');
            $res = $this->console('POST', '/v1/mailboxes', ['json' => $payload]);
            $results[] = [
                'line' => $i + 2,
                'address' => $res->ok() ? $res->get('address') : $local,
                'ok' => $res->ok(),
                'message' => $res->ok() ? ($recovery !== '' ? 'Invitation emailed to ' . $recovery : 'Created; share the activation link from the mailbox page') : self::describe($res),
                'id' => $res->ok() ? $res->get('id') : null,
            ];
            if ($res->status === 429 || $res->status >= 500) {
                break;
            }
        }
        $this->session()->flash('import_result', $results);
        $ok = count(array_filter($results, static fn ($r) => $r['ok']));
        $this->flash($ok === count($rows) ? 'success' : 'warning', $ok . ' of ' . count($rows) . ' mailboxes created.');
        return $this->redirect('/console/mailboxes/import');
    }

    /** @return list<array<string, string>> */
    public static function parseCsv(string $data): array
    {
        $data = preg_replace('/^\xEF\xBB\xBF/', '', $data) ?? $data;
        $lines = preg_split('/\r\n|\n|\r/', trim($data)) ?: [];
        if (count($lines) < 2) {
            return [];
        }
        $delimiter = substr_count($lines[0], ';') > substr_count($lines[0], ',') ? ';' : ',';
        $header = array_map(static fn ($h) => strtolower(str_replace([' ', '-'], '_', trim((string) $h))), str_getcsv($lines[0], $delimiter, '"', ''));
        $rows = [];
        foreach (array_slice($lines, 1) as $line) {
            if (trim($line) === '') {
                continue;
            }
            $cells = str_getcsv($line, $delimiter, '"', '');
            $rows[] = array_combine($header, array_pad(array_slice($cells, 0, count($header)), count($header), ''));
        }
        return $rows;
    }

    public function onboarding(Request $request): Response
    {
        $report = $this->must($this->console('GET', '/v1/mailboxes/onboarding'))->json();
        $mailboxes = $this->console('GET', '/v1/mailboxes');
        return $this->render('console/mailboxes/onboarding', ['title' => 'Phone setup report', 'report' => $report, 'mailboxes' => $mailboxes->ok() ? $mailboxes->items() : [], 'crumbs' => [['Mailboxes', '/console/mailboxes'], ['Phone setup report', null]]]);
    }

    public function show(Request $request): Response
    {
        $m = $this->must($this->console('GET', '/v1/mailboxes/' . $request->param('id')))->json();
        return $this->render('console/mailboxes/show', [
            'title' => $m['address'],
            'm' => $m,
            'handover' => $this->revealed('handover'),
            'crumbs' => [['Mailboxes', '/console/mailboxes'], [$m['address'], null]],
        ]);
    }

    public function update(Request $request): Response
    {
        $id = $request->param('id');
        $errors = Validator::make($request->all(), ['display_name' => 'required|max:200', 'recovery_email' => 'nullable|email', 'quota_mb' => 'nullable|integer', 'department' => 'nullable|max:120', 'job_title' => 'nullable|max:120'], ['recovery_email' => 'Personal email', 'quota_mb' => 'Storage']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/mailboxes/' . $id, $errors);
        }
        $payload = [
            'display_name' => $request->string('display_name'),
            'recovery_email' => $request->string('recovery_email'),
            'department' => $request->string('department'),
            'job_title' => $request->string('job_title'),
        ];
        if ($request->string('quota_mb') !== '') {
            $payload['quota_bytes'] = (int) $request->string('quota_mb') * 1048576;
        }
        $res = $this->console('PATCH', '/v1/mailboxes/' . $id, ['json' => $payload]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/mailboxes/' . $id, $res);
        }
        $this->flash('success', 'Mailbox updated.');
        return $this->redirect('/console/mailboxes/' . $id);
    }

    public function status(Request $request): Response
    {
        $id = $request->param('id');
        $status = $request->string('status');
        if (!in_array($status, ['active', 'suspended'], true)) {
            return $this->redirect('/console/mailboxes/' . $id);
        }
        $res = $this->console('PATCH', '/v1/mailboxes/' . $id, ['json' => ['status' => $status]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? ($status === 'suspended' ? 'Mailbox suspended. Sign-in is blocked; mail is still received.' : 'Mailbox reactivated.') : self::describe($res));
        return $this->redirect('/console/mailboxes/' . $id);
    }

    /** Administrator-set or generated password (write permission). */
    public function resetPassword(Request $request): Response
    {
        $id = $request->param('id');
        $set = $request->string('mode') === 'set';
        if ($set) {
            $errors = Validator::make($request->all(), ['password' => 'required|min:10|max:256']);
            if ($errors !== []) {
                return $this->invalid($request, '/console/mailboxes/' . $id . '#access', $errors);
            }
        }
        $res = $this->console('POST', '/v1/mailboxes/' . $id . '/reset-password', ['json' => $set ? ['password' => (string) $request->input('password')] : new \stdClass()]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/mailboxes/' . $id . '#access', $res);
        }
        if ($res->get('password')) {
            $this->reveal('handover', ['password' => $res->get('password')]);
        }
        $this->flash('success', 'Password changed. The user was signed out of webmail and must update the password in their phone app.');
        return $this->redirect('/console/mailboxes/' . $id . '#access');
    }

    /** Resend an invitation or create a handover activation link (support role). */
    public function activation(Request $request): Response
    {
        $id = $request->param('id');
        $method = $request->string('method') === 'link' ? 'link' : 'invite';
        $res = $this->console('POST', '/v1/mailboxes/' . $id . '/activation', ['json' => ['method' => $method]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } else {
            $this->revealHandover($res->json());
            $this->flash('success', $method === 'link' ? 'A new activation link was created. Earlier links no longer work.' : 'A new invitation was emailed to ' . ($res->get('activation.sent_to') ?? 'the personal address') . '.');
        }
        return $this->redirect('/console/mailboxes/' . $id . '#access');
    }

    /** Password reset without seeing or choosing the password (support role). */
    public function sendReset(Request $request): Response
    {
        $id = $request->param('id');
        $method = $request->string('method') === 'link' ? 'link' : 'email';
        $res = $this->console('POST', '/v1/mailboxes/' . $id . '/send-reset', ['json' => ['method' => $method]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
        } else {
            if ($res->get('reset.url')) {
                $this->reveal('handover', ['reset_url' => $res->get('reset.url'), 'expires_at' => $res->get('reset.expires_at')]);
            }
            $this->flash('success', $method === 'link' ? 'Reset link created. It works once and expires in one hour.' : 'A reset link was sent to ' . ($res->get('reset.sent_to') ?? 'the personal address') . '.');
        }
        return $this->redirect('/console/mailboxes/' . $id . '#access');
    }

    public function unlock(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/mailboxes/' . $id . '/unlock', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Sign-in unlocked. Ask the user to try again with the correct password.' : self::describe($res));
        return $this->redirect('/console/mailboxes/' . $id . '#access');
    }

    public function destroy(Request $request): Response
    {
        $id = $request->param('id');
        if ($request->string('confirm_address') !== $request->string('address')) {
            $this->flash('error', 'Type the full address to confirm deletion.');
            return $this->redirect('/console/mailboxes/' . $id);
        }
        $res = $this->console('DELETE', '/v1/mailboxes/' . $id);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/mailboxes/' . $id);
        }
        $this->flash('success', 'Mailbox deleted.');
        return $this->redirect('/console/mailboxes');
    }

    /** Printable welcome sheet: settings, QR to the setup page, no password. */
    public function handout(Request $request): Response
    {
        $m = $this->must($this->console('GET', '/v1/mailboxes/' . $request->param('id')))->json();
        $support = $this->console('GET', '/v1/organization/support');
        return $this->render('console/mailboxes/handout', [
            'title' => 'Welcome sheet · ' . $m['address'],
            'm' => $m,
            'support' => $support->ok() ? $support->json() : [],
            'org' => $this->auth($request)['organization']['name'] ?? '',
            'appUrl' => rtrim((string) $this->app->config->get('url'), '/'),
            'backUrl' => '/console/mailboxes/' . $m['id'],
        ], 'print');
    }

    private function revealHandover(array $body): void
    {
        $handover = array_filter([
            'password' => $body['password'] ?? null,
            'activation_url' => $body['activation']['url'] ?? null,
            'sent_to' => $body['activation']['sent_to'] ?? null,
            'expires_at' => $body['activation']['expires_at'] ?? null,
        ]);
        if ($handover !== []) {
            $this->reveal('handover', $handover);
        }
    }
}
