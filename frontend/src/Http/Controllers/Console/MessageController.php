<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Message log, detail timeline, console compose, scheduling and export.
 */
final class MessageController extends Controller
{
    public const STATUSES = ['queued', 'scheduled', 'processing', 'accepted_by_mta', 'delivered', 'deferred', 'bounced', 'failed', 'complained', 'cancelled', 'suppressed'];

    public function index(Request $request): Response
    {
        $query = array_filter([
            'status' => in_array($request->query('status'), self::STATUSES, true) ? $request->query('status') : null,
            'q' => trim((string) $request->query('q', '')) ?: null,
            'tag' => trim((string) $request->query('tag', '')) ?: null,
            'cursor' => (string) $request->query('cursor', '') ?: null,
            'limit' => 50,
        ]);
        $res = $this->must($this->console('GET', '/v1/messages', ['query' => $query]));
        return $this->render('console/messages/index', ['title' => 'Messages', 'messages' => $res->items(), 'next' => $res->get('next_cursor'), 'filters' => $query]);
    }

    public function export(Request $request): Response
    {
        $res = $this->console('GET', '/v1/messages/export.csv', ['query' => array_filter(['status' => $request->query('status'), 'q' => $request->query('q'), 'tag' => $request->query('tag')])]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/console/messages');
        }
        return Response::download($res->body, 'messages-' . date('Y-m-d') . '.csv', 'text/csv; charset=utf-8');
    }

    public function show(Request $request): Response
    {
        $m = $this->must($this->console('GET', '/v1/messages/' . $request->param('id')))->json();
        return $this->render('console/messages/show', ['title' => $m['subject'] ?: 'Message', 'm' => $m, 'crumbs' => [['Messages', '/console/messages'], [$m['subject'] ?: '(no subject)', null]]]);
    }

    public function compose(Request $request): Response
    {
        $domains = array_values(array_filter($this->console('GET', '/v1/domains')->items(), static fn ($d) => $d['status'] === 'verified'));
        $templates = $this->can($request, 'templates:read') ? array_values(array_filter($this->console('GET', '/v1/templates')->items(), static fn ($t) => $t['status'] === 'active')) : [];
        return $this->render('console/messages/compose', ['title' => 'Send a message', 'domains' => $domains, 'templates' => $templates, 'crumbs' => [['Messages', '/console/messages'], ['Send', null]]]);
    }

    public function send(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['from' => 'required|email', 'to' => 'required', 'subject' => 'max:998', 'send_at' => 'nullable']);
        $to = array_values(array_filter(array_map('trim', preg_split('/[,;\n]+/', $request->string('to')) ?: [])));
        foreach ($to as $addr) {
            if (!filter_var($addr, FILTER_VALIDATE_EMAIL)) {
                $errors['to'] = '"' . $addr . '" is not a valid email address.';
                break;
            }
        }
        if (count($to) > 50) {
            $errors['to'] = 'At most 50 recipients per message.';
        }
        $template = $request->string('template');
        if ($template === '' && $request->string('subject') === '') {
            $errors['subject'] = 'Enter a subject or choose a template.';
        }
        if ($template === '' && trim($request->string('text')) === '' && trim((string) $request->input('html', '')) === '') {
            $errors['text'] = 'Write a message or choose a template.';
        }
        if ($errors !== []) {
            return $this->invalid($request, '/console/messages/compose', $errors);
        }
        $from = $request->string('from_name') !== '' ? $request->string('from_name') . ' <' . $request->string('from') . '>' : $request->string('from');
        $payload = ['from' => $from, 'to' => $to, 'tags' => array_values(array_filter(array_map('trim', explode(',', $request->string('tags'))))), 'unsubscribe' => $request->bool('unsubscribe')];
        if ($template !== '') {
            $payload['template'] = $template;
            $vars = json_decode((string) $request->input('variables', '{}'), true);
            if ($request->string('variables') !== '' && !is_array($vars)) {
                return $this->invalid($request, '/console/messages/compose', ['variables' => 'Variables must be a JSON object, for example {"name": "Jane"}.']);
            }
            $payload['variables'] = (object) ($vars ?: []);
        } else {
            $payload['subject'] = $request->string('subject');
            $payload['text'] = $request->string('text');
            if (trim((string) $request->input('html', '')) !== '') {
                $payload['html'] = (string) $request->input('html');
            }
        }
        if ($request->string('reply_to') !== '') {
            $payload['reply_to'] = $request->string('reply_to');
        }
        if ($request->string('send_at') !== '') {
            try {
                $tz = new \DateTimeZone(\App\Support\Env::get('APP_TIMEZONE', 'Africa/Nairobi'));
                $payload['send_at'] = (new \DateTimeImmutable($request->string('send_at'), $tz))->setTimezone(new \DateTimeZone('UTC'))->format('Y-m-d\TH:i:s\Z');
            } catch (\Exception) {
                return $this->invalid($request, '/console/messages/compose', ['send_at' => 'Enter a valid date and time.']);
            }
        }
        $res = $this->console('POST', '/v1/messages', ['json' => $payload, 'headers' => ['Idempotency-Key' => 'console-' . bin2hex(random_bytes(12))]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/messages/compose', $res);
        }
        $this->flash('success', isset($payload['send_at']) ? 'Message scheduled.' : 'Message queued for delivery.');
        // The API answers with one entry per recipient under `messages`.
        $sent = (array) $res->get('messages', []);
        $id = count($sent) === 1 ? (string) ($sent[0]['id'] ?? '') : '';
        return $this->redirect($id !== '' ? '/console/messages/' . rawurlencode($id) : '/console/messages');
    }

    public function cancel(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/messages/' . $id . '/cancel', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Scheduled message cancelled.' : self::describe($res));
        return $this->redirect('/console/messages/' . $id);
    }
}
