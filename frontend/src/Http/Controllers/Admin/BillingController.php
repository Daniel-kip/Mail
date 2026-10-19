<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;

/**
 * Receivables (record payments, void invoices), the renewal run and the
 * plan catalogue.
 */
final class BillingController extends Controller
{
    private const PLAN_INTS = ['price_monthly_cents', 'price_per_mailbox_cents', 'annual_discount_percent', 'daily_send_limit', 'monthly_send_limit', 'max_domains', 'max_mailboxes', 'max_members', 'max_api_keys', 'max_webhooks', 'max_recipients_per_request', 'api_requests_per_minute', 'trial_days'];

    public function index(Request $request): Response
    {
        $status = in_array($request->query('status'), ['open', 'paid', 'void', 'overdue'], true) ? $request->query('status') : null;
        $res = $this->must($this->console('GET', '/v1/admin/invoices', ['query' => array_filter(['status' => $status])]));
        return $this->render('admin/billing', ['title' => 'Receivables', 'invoices' => $res->items(), 'summary' => (array) $res->get('summary', []), 'status' => $status, 'area' => 'admin'], 'admin');
    }

    public function invoice(Request $request): Response
    {
        $invoice = $this->must($this->console('GET', '/v1/admin/invoices/' . $request->param('id')))->json();
        return $this->render('admin/invoice', ['title' => 'Invoice ' . $invoice['number'], 'invoice' => $invoice, 'area' => 'admin', 'crumbs' => [['Receivables', '/admin/billing'], [$invoice['number'], null]]], 'admin');
    }

    public function markPaid(Request $request): Response
    {
        $id = $request->param('id');
        $payload = ['method' => $request->string('method', 'bank_transfer'), 'reference' => $request->string('reference') ?: null];
        if ($request->string('paid_at') !== '') {
            $payload['paid_at'] = (new \DateTimeImmutable($request->string('paid_at') . ' 12:00:00'))->format(DATE_ATOM);
        }
        $res = $this->console('POST', '/v1/admin/invoices/' . $id . '/mark-paid', ['json' => $payload]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Payment recorded.' : self::describe($res));
        return $this->redirect('/admin/billing/invoices/' . $id);
    }

    public function void(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/admin/invoices/' . $id . '/void', ['json' => ['reason' => $request->string('reason')]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Invoice voided.' : self::describe($res));
        return $this->redirect('/admin/billing/invoices/' . $id);
    }

    public function run(Request $request): Response
    {
        $res = $this->console('POST', '/v1/admin/billing/run', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Billing run finished: ' . json_encode($res->json(), JSON_UNESCAPED_SLASHES) : self::describe($res));
        return $this->redirect('/admin/billing');
    }

    public function plans(Request $request): Response
    {
        $plans = $this->must($this->console('GET', '/v1/admin/plans'))->items();
        return $this->render('admin/plans', ['title' => 'Plans', 'plans' => $plans, 'area' => 'admin'], 'admin');
    }

    public function updatePlan(Request $request): Response
    {
        $payload = ['name' => $request->string('name'), 'description' => $request->string('description'), 'is_public' => $request->bool('is_public'), 'self_service' => $request->bool('self_service')];
        foreach (self::PLAN_INTS as $field) {
            if ($request->string($field) !== '' && is_numeric($request->string($field))) {
                $payload[$field] = (int) $request->string($field);
            }
        }
        $res = $this->console('PATCH', '/v1/admin/plans/' . $request->param('plan'), ['json' => $payload]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Plan saved. New prices apply from the next invoice.' : self::describe($res));
        return $this->redirect('/admin/plans#plan-' . $request->param('plan'));
    }
}
