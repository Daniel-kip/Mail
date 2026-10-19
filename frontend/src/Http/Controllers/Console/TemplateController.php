<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Versioned email templates with live preview.
 */
final class TemplateController extends Controller
{
    public function index(Request $request): Response
    {
        $templates = $this->must($this->console('GET', '/v1/templates'))->items();
        return $this->render('console/templates/index', ['title' => 'Templates', 'templates' => $templates]);
    }

    public function create(Request $request): Response
    {
        return $this->render('console/templates/edit', [
            'title' => 'New template',
            'template' => null,
            'version' => ['subject' => 'Your invoice {{invoice_number}}', 'html_body' => self::STARTER_HTML, 'text_body' => "Hello {{name}},\n\nInvoice {{invoice_number}} for {{amount}} is ready.\n\nThank you.", 'variables' => [['name' => 'name', 'required' => true, 'sample' => 'Jane'], ['name' => 'invoice_number', 'required' => true, 'sample' => 'INV-1042'], ['name' => 'amount', 'required' => true, 'sample' => 'KES 1,200']]],
            'versions' => [],
            'crumbs' => [['Templates', '/console/templates'], ['New', null]],
        ]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['name' => 'required|max:120', 'subject' => 'required|max:998']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/templates/new', $errors);
        }
        $res = $this->console('POST', '/v1/templates', ['json' => [
            'name' => $request->string('name'),
            'description' => $request->string('description') ?: null,
        ] + self::content($request)]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/templates/new', $res);
        }
        $this->flash('success', 'Template created as version 1.');
        return $this->redirect('/console/templates/' . $res->get('id'));
    }

    public function preview(Request $request): Response
    {
        $data = [];
        foreach (self::variables($request) as $v) {
            if (array_key_exists('sample', $v)) {
                $data[$v['name']] = $v['sample'];
            }
        }
        $res = $this->console('POST', '/v1/templates/preview', ['json' => self::content($request) + ['data' => (object) $data]]);
        return Response::json($res->json() ?? ['error' => ['message' => 'Preview unavailable']], $res->ok() ? 200 : 422);
    }

    public function edit(Request $request): Response
    {
        $id = $request->param('id');
        $template = $this->must($this->console('GET', '/v1/templates/' . $id))->json();
        $versions = $this->console('GET', '/v1/templates/' . $id . '/versions');
        $wanted = (int) $request->query('version', $template['latest_version'] ?? 1);
        $version = $this->console('GET', '/v1/templates/' . $id . '/versions/' . $wanted);
        return $this->render('console/templates/edit', [
            'title' => $template['name'],
            'template' => $template,
            'version' => $version->ok() ? $version->json() : ($template['version'] ?? []),
            'versions' => $versions->ok() ? $versions->items() : [],
            'crumbs' => [['Templates', '/console/templates'], [$template['name'], null]],
        ]);
    }

    public function saveVersion(Request $request): Response
    {
        $id = $request->param('id');
        $errors = Validator::make($request->all(), ['subject' => 'required|max:998']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/templates/' . $id, $errors);
        }
        if ($request->string('name') !== '') {
            $this->console('PATCH', '/v1/templates/' . $id, ['json' => ['name' => $request->string('name'), 'description' => $request->string('description')]]);
        }
        $res = $this->console('POST', '/v1/templates/' . $id . '/versions', ['json' => self::content($request) + ['activate' => $request->bool('activate')]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/templates/' . $id, $res);
        }
        $this->flash('success', 'Saved version ' . ($res->get('version') ?? $res->get('latest_version')) . ($request->bool('activate') ? ' and made it active.' : '. The active version is unchanged until you activate it.'));
        return $this->redirect('/console/templates/' . $id);
    }

    public function activate(Request $request): Response
    {
        $id = $request->param('id');
        $version = (int) $request->string('version');
        $res = $this->console('POST', '/v1/templates/' . $id . '/versions/' . $version . '/activate', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Version ' . $version . ' is now active.' : self::describe($res));
        return $this->redirect('/console/templates/' . $id . '?version=' . $version);
    }

    public function status(Request $request): Response
    {
        $id = $request->param('id');
        $status = $request->string('status') === 'archived' ? 'archived' : 'active';
        $res = $this->console('PATCH', '/v1/templates/' . $id, ['json' => ['status' => $status]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? ($status === 'archived' ? 'Template archived; sends using it are rejected.' : 'Template restored.') : self::describe($res));
        return $this->redirect('/console/templates/' . $id);
    }

    private static function content(Request $request): array
    {
        return [
            'subject' => $request->string('subject'),
            'html_body' => (string) $request->input('html_body', '') ?: null,
            'text_body' => (string) $request->input('text_body', '') ?: null,
            'variables' => self::variables($request),
        ];
    }

    /** Variables from "name | sample | required" lines. */
    private static function variables(Request $request): array
    {
        $out = [];
        foreach (preg_split('/\r\n|\n/', (string) $request->input('variables', '')) ?: [] as $line) {
            $parts = array_map('trim', explode('|', $line));
            if (!preg_match('/^[A-Za-z_][A-Za-z0-9_]{0,63}$/', $parts[0] ?? '')) {
                continue;
            }
            $v = ['name' => $parts[0], 'required' => strtolower($parts[2] ?? '') === 'required'];
            if (($parts[1] ?? '') !== '') {
                $v['sample'] = $parts[1];
            }
            $out[] = $v;
        }
        return $out;
    }

    private const STARTER_HTML = <<<'HTML'
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#f6f7f6;padding:24px 0;font-family:Arial,Helvetica,sans-serif">
  <tr><td align="center">
    <table role="presentation" width="560" cellpadding="0" cellspacing="0" style="background:#ffffff;border-radius:8px;overflow:hidden">
      <tr><td style="background:#0b7a5c;color:#ffffff;padding:20px 28px;font-size:18px;font-weight:bold">Your invoice</td></tr>
      <tr><td style="padding:28px;color:#111816;font-size:15px;line-height:1.6">
        <p>Hello {{name}},</p>
        <p>Invoice <strong>{{invoice_number}}</strong> for <strong>{{amount}}</strong> is ready.</p>
        <p>Thank you for your business.</p>
      </td></tr>
    </table>
  </td></tr>
</table>
HTML;
}
