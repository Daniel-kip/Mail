<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Event webhooks with signed deliveries, test sends and retries.
 */
final class WebhookController extends Controller
{
    public function index(Request $request): Response
    {
        $hooks = $this->must($this->console('GET', '/v1/webhooks'))->items();
        return $this->render('console/webhooks/index', ['title' => 'Webhooks', 'hooks' => $hooks, 'events' => $this->eventTypes(), 'secret' => $this->revealed('webhook_secret')]);
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['url' => 'required|https|max:2000', 'description' => 'nullable|max:200'], ['url' => 'Endpoint URL']);
        $events = array_values(array_filter((array) $request->input('events', []), 'is_string'));
        if ($events === []) {
            $errors['events'] = 'Choose at least one event.';
        }
        if ($errors !== []) {
            return $this->invalid($request, '/console/webhooks', $errors);
        }
        $res = $this->console('POST', '/v1/webhooks', ['json' => ['url' => $request->string('url'), 'events' => $events, 'description' => $request->string('description') ?: null]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/webhooks', $res);
        }
        $this->reveal('webhook_secret', ['secret' => $res->get('secret'), 'url' => $res->get('url')]);
        return $this->redirect('/console/webhooks/' . $res->get('id'));
    }

    public function show(Request $request): Response
    {
        $id = $request->param('id');
        $hook = $this->must($this->console('GET', '/v1/webhooks/' . $id))->json();
        $deliveries = $this->console('GET', '/v1/webhooks/' . $id . '/deliveries', ['query' => ['limit' => 50]]);
        return $this->render('console/webhooks/show', [
            'title' => 'Webhook', 'hook' => $hook, 'deliveries' => $deliveries->ok() ? $deliveries->items() : [], 'events' => $this->eventTypes(),
            'secret' => $this->revealed('webhook_secret'), 'crumbs' => [['Webhooks', '/console/webhooks'], [parse_url((string) $hook['url'], PHP_URL_HOST) ?: 'Endpoint', null]],
        ]);
    }

    public function update(Request $request): Response
    {
        $id = $request->param('id');
        $payload = [];
        if ($request->input('url') !== null) {
            $payload = ['url' => $request->string('url'), 'description' => $request->string('description'), 'events' => array_values(array_filter((array) $request->input('events', []), 'is_string'))];
        }
        if (in_array($request->string('status'), ['active', 'disabled'], true)) {
            $payload['status'] = $request->string('status');
        }
        $res = $this->console('PATCH', '/v1/webhooks/' . $id, ['json' => $payload]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/webhooks/' . $id, $res);
        }
        $this->flash('success', 'Webhook updated.');
        return $this->redirect('/console/webhooks/' . $id);
    }

    public function test(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/webhooks/' . $id . '/test', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Test event queued. It appears in the delivery log within a few seconds.' : self::describe($res));
        return $this->redirect('/console/webhooks/' . $id);
    }

    public function rotate(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/webhooks/' . $id . '/rotate-secret', ['json' => new \stdClass()]);
        if ($res->ok()) {
            $this->reveal('webhook_secret', ['secret' => $res->get('secret')]);
        } else {
            $this->flash('error', self::describe($res));
        }
        return $this->redirect('/console/webhooks/' . $id);
    }

    public function destroy(Request $request): Response
    {
        $res = $this->console('DELETE', '/v1/webhooks/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Webhook deleted.' : self::describe($res));
        return $this->redirect('/console/webhooks');
    }

    public function retry(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/webhooks/' . $id . '/deliveries/' . $request->param('delivery') . '/retry', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Delivery queued for retry.' : self::describe($res));
        return $this->redirect('/console/webhooks/' . $id);
    }

    /** @return list<string> */
    private function eventTypes(): array
    {
        $res = $this->console('GET', '/v1/webhooks/events');
        $items = $res->ok() ? ($res->get('data') ?? $res->get('events') ?? []) : [];
        return array_values(array_map(static fn ($e) => is_array($e) ? (string) ($e['type'] ?? $e['name'] ?? '') : (string) $e, $items)) ?: ['message.delivered', 'message.bounced', 'message.failed', 'message.complained'];
    }
}
