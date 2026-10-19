<?php

declare(strict_types=1);

namespace App\Http\Controllers\Console;

use App\Http\Controllers\Controller;
use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Subscription, plan changes, billing details and invoices.
 */
final class BillingController extends Controller
{
    public function index(Request $request): Response
    {
        $billing = $this->must($this->console('GET', '/v1/billing'))->json();
        return $this->render('console/billing/index', ['title' => 'Billing', 'b' => $billing]);
    }

    public function plans(Request $request): Response
    {
        $billing = $this->must($this->console('GET', '/v1/billing'))->json();
        $catalogue = $this->public('GET', '/v1/plans', ['query' => ['audience' => $billing['account_type']]]);
        return $this->render('console/billing/plans', [
            'title' => 'Plans',
            'b' => $billing,
            'catalogue' => $catalogue->ok() ? $catalogue->json() : ['data' => []],
            'cycle' => $request->query('cycle') === 'annual' ? 'annual' : ($billing['subscription']['billing_cycle'] ?? 'monthly'),
            'highlight' => (string) $request->query('highlight', ''),
            'crumbs' => [['Billing', '/console/billing'], ['Plans', null]],
        ]);
    }

    public function changePlan(Request $request): Response
    {
        $res = $this->console('POST', '/v1/billing/subscription', ['json' => ['plan_id' => $request->string('plan_id'), 'billing_cycle' => $request->string('billing_cycle') === 'annual' ? 'annual' : 'monthly']]);
        if (!$res->ok()) {
            $violations = $res->get('error.details.violations');
            $this->flash('error', is_array($violations) && $violations !== [] ? 'This plan is too small for your current usage: ' . implode('; ', array_map(static fn ($v) => is_array($v) ? ($v['message'] ?? json_encode($v)) : (string) $v, $violations)) . '.' : self::describe($res));
            return $this->redirect('/console/billing/plans');
        }
        $this->flash('success', match ($res->get('outcome')) {
            'trial_started' => 'Your free trial has started. No invoice is issued until it ends on ' . date_short($res->get('subscription.trial_ends_at') ?? $res->get('effective_at')) . '.',
            'scheduled' => 'The change takes effect on ' . date_short($res->get('effective_at')) . ', at the end of the paid period.',
            default => 'Plan changed.' . ($res->get('invoice') ? ' A prorated invoice was issued.' : ''),
        });
        return $this->redirect('/console/billing');
    }

    public function cancel(Request $request): Response
    {
        $res = $this->console('POST', '/v1/billing/subscription/cancel', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Your subscription will end at the close of the current period. You can resume before then.' : self::describe($res));
        return $this->redirect('/console/billing');
    }

    public function resume(Request $request): Response
    {
        $res = $this->console('POST', '/v1/billing/subscription/resume', ['json' => new \stdClass()]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Subscription resumed.' : self::describe($res));
        return $this->redirect('/console/billing');
    }

    public function details(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['billing_name' => 'nullable|max:200', 'billing_email' => 'nullable|email', 'billing_address' => 'nullable|max:500', 'tax_id' => 'nullable|max:60', 'country' => 'nullable|max:2'], ['billing_email' => 'Billing email', 'tax_id' => 'Tax ID / KRA PIN']);
        if ($errors !== []) {
            return $this->invalid($request, '/console/billing#details', $errors);
        }
        $res = $this->console('PATCH', '/v1/billing/details', ['json' => [
            'billing_name' => $request->string('billing_name'), 'billing_email' => $request->string('billing_email'), 'billing_address' => $request->string('billing_address'),
            'tax_id' => $request->string('tax_id'), 'country' => strtoupper($request->string('country')),
        ]]);
        if (!$res->ok()) {
            return $this->failed($request, '/console/billing#details', $res);
        }
        $this->flash('success', 'Billing details saved. They appear on future invoices.');
        return $this->redirect('/console/billing#details');
    }

    public function invoice(Request $request): Response
    {
        $invoice = $this->must($this->console('GET', '/v1/billing/invoices/' . $request->param('id')))->json();
        $providers = $this->public('GET', '/v1/auth/providers');
        return $this->render('console/billing/invoice', [
            'title' => 'Invoice ' . $invoice['number'], 'invoice' => $invoice, 'backUrl' => '/console/billing',
            'mpesa' => $providers->ok() && $providers->get('payments.mpesa') === true && $invoice['currency'] === 'KES',
            'canPay' => $this->can($request, 'billing:manage'),
            'payerPhone' => (string) ($this->auth($request)['user']['phone'] ?? ''),
        ], 'print');
    }

    /** Start an M-Pesa STK push for an open invoice. */
    public function pay(Request $request): Response
    {
        $id = $request->param('id');
        $res = $this->console('POST', '/v1/billing/invoices/' . $id . '/pay', ['json' => ['phone' => $request->string('phone')]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/console/billing/invoices/' . $id, ['phone' => self::describe($res)], 'The M-Pesa prompt was not sent.');
        }
        return $this->redirect('/console/billing/payments/' . $res->get('id'));
    }

    /** Waiting page while the payer approves the prompt; refreshes itself until final. */
    public function payment(Request $request): Response
    {
        $payment = $this->must($this->console('GET', '/v1/billing/payments/' . $request->param('id')))->json();
        return $this->render('console/billing/payment', [
            'title' => 'M-Pesa payment', 'payment' => $payment, 'narrow' => true,
            'refresh' => $payment['final'] ? null : 4,
            'crumbs' => [['Billing', '/console/billing'], ['Payment', null]],
        ]);
    }
}
