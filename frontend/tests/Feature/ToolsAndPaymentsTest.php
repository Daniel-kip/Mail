<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class ToolsAndPaymentsTest extends TestCase
{
    private const INV = '0192a000-0000-7000-8000-0000000000e1';
    private const PAY = '0192a000-0000-7000-8000-0000000000f1';

    public function setUp(): void
    {
        $this->boot();
    }

    public function testLandingPageHasTheSendSimulation(): void
    {
        $this->api->on('GET', '/v1/plans', 200, Fixtures::plans());
        $page = $this->get('/')->body();
        $this->assertContains('data-sim', $page);
        $this->assertContains('/assets/js/sim.js', $page);
        $this->assertContains('video/mailvora-global-network-loop.mp4', $page);
        $this->assertTrue(preg_match('~<video[^>]*\sautoplay\s+muted\s+loop\s+playsinline\s~', $page) === 1, 'hero video autoplays muted in a loop');
        $this->assertTrue(preg_match('~<video[^>]*\scontrols[\s>=]~', $page) === 0 && !str_contains($page, 'data-video-toggle'), 'no video controls');
        $this->assertContains('Powered by <strong>TECHCORE SOLUTIONS</strong>', $page);
        $this->assertContains('sent in this demo', $page);
        $this->assertContains('brand/mailvora-mark.svg', $page);
    }

    public function testDomainReportRenders(): void
    {
        $finding = static fn ($level, $msg) => ['level' => $level, 'message' => $msg];
        $this->api->on('GET', '/v1/tools/domain', 200, [
            'domain' => 'example.co.ke', 'score' => 73, 'grade' => 'C', 'checked_at' => '2026-10-14T08:00:00Z',
            'mx' => ['status' => 'pass', 'findings' => [$finding('pass', '1 mail server found')], 'records' => [['preference' => 10, 'host' => 'mx.example.co.ke', 'addresses' => [['ip' => '203.0.113.5', 'ptr' => ['mx.example.co.ke'], 'fcrdns' => true, 'listed_on' => []]]]]],
            'spf' => ['status' => 'warn', 'record' => 'v=spf1 include:_spf.google.com ~all', 'lookups' => 8, 'findings' => [$finding('warn', 'SPF uses 8 of the 10 allowed DNS lookups')]],
            'dmarc' => ['status' => 'fail', 'record' => null, 'tags' => [], 'findings' => [$finding('fail', 'No DMARC record')]],
            'dkim' => ['status' => 'pass', 'keys' => [['selector' => 'google', 'record' => 'v=DKIM1; k=rsa; p=MIIB']], 'findings' => [$finding('pass', 'DKIM key found: google')], 'selectors_checked' => 23],
            'mta_sts' => ['status' => 'info', 'record' => null, 'policy' => null, 'findings' => [$finding('info', 'No MTA-STS')]],
            'tls_rpt' => ['status' => 'info', 'records' => [], 'findings' => [$finding('info', 'No TLS-RPT record')]],
            'bimi' => ['status' => 'info', 'records' => [], 'findings' => [$finding('info', 'No BIMI record')]],
            'caa' => [],
        ]);
        $this->assertRedirect('/tools/domain?domain=example.co.ke', $this->get('/tools?domain=example.co.ke'));
        $page = $this->get('/tools/domain?domain=example.co.ke')->body();
        $this->assertContains('73/100', $page);
        $this->assertContains('No DMARC record', $page);
        $this->assertContains('8/10', $page);
        $this->assertContains('mx.example.co.ke', $page);
    }

    public function testDnsLookupAndHeaderAnalysis(): void
    {
        $this->api->on('GET', '/v1/tools/dns', 200, ['name' => 'gmail.com', 'type' => 'MX', 'answers' => ['5 gmail-smtp-in.l.google.com'], 'elapsed_ms' => 21, 'resolver' => '1.1.1.1 (Cloudflare)']);
        $this->assertContains('gmail-smtp-in.l.google.com', $this->get('/tools/dns?name=gmail.com&type=MX')->body());
        $this->api->on('POST', '/v1/tools/headers', 200, [
            'summary' => ['from' => 'A <a@a.example>', 'subject' => 'Hi', 'to' => null, 'date' => null, 'message_id' => null, 'return_path' => null, 'spam_status' => null, 'list_unsubscribe' => null],
            'authentication' => ['spf' => 'pass', 'dkim' => 'pass', 'dmarc' => 'fail', 'raw' => 'x'],
            'hops' => [['hop' => 1, 'from' => 'app.a.example', 'by' => 'mx.b.example', 'with' => 'ESMTPS', 'date' => null, 'delay_seconds' => null, 'tls' => true]],
            'total_delay_seconds' => 0, 'header_count' => 5, 'findings' => [['level' => 'fail', 'message' => 'DMARC result: fail']],
        ]);
        $this->assertRedirect('/tools/headers#result', $this->post('/tools/headers', ['headers' => "From: a@a.example\nSubject: Hi"]));
        $page = $this->get('/tools/headers')->body();
        $this->assertContains('DMARC result: fail', $page);
        $this->assertContains('mx.b.example', $page);
    }

    public function testInvoiceCanBePaidWithMpesa(): void
    {
        $this->signInConsole(['user' => ['phone' => '+254712345678']]);
        $invoice = ['id' => self::INV, 'number' => 'MV-2026-0007', 'kind' => 'subscription', 'status' => 'open', 'overdue' => false, 'currency' => 'KES',
            'period_start' => '2026-10-14T00:00:00Z', 'period_end' => '2026-11-14T00:00:00Z', 'subtotal_cents' => 120000, 'tax_rate_bps' => 0, 'tax_cents' => 0,
            'total_cents' => 120000, 'issued_at' => '2026-10-14T00:00:00Z', 'due_at' => '2026-10-28T00:00:00Z', 'paid_at' => null, 'voided_at' => null,
            'lines' => [['description' => 'Premium (monthly)', 'quantity' => 1, 'unit_cents' => 120000, 'amount_cents' => 120000]], 'billing' => []];
        $this->api->on('GET', '/v1/billing/invoices/' . self::INV, 200, $invoice)
            ->on('GET', '/v1/auth/providers', 200, ['payments' => ['mpesa' => true]]);
        $page = $this->get('/console/billing/invoices/' . self::INV)->body();
        $this->assertContains('with M-Pesa', $page);
        $this->assertContains('+254712345678', $page);

        $this->api->on('POST', '/v1/billing/invoices/' . self::INV . '/pay', 202, ['id' => self::PAY, 'status' => 'processing']);
        $this->assertRedirect('/console/billing/payments/' . self::PAY, $this->post('/console/billing/invoices/' . self::INV . '/pay', ['phone' => '0712345678']));
        $payment = ['id' => self::PAY, 'invoice_id' => self::INV, 'reference' => 'MV-MV-2026-0007-AB12CD', 'phone' => '+254712•••678', 'amount_cents' => 120000, 'currency' => 'KES',
            'status' => 'processing', 'receipt' => null, 'failure_reason' => null, 'created_at' => '2026-10-14T08:00:00Z', 'completed_at' => null, 'final' => false];
        $this->api->on('GET', '/v1/billing/payments/' . self::PAY, 200, $payment);
        $waiting = $this->get('/console/billing/payments/' . self::PAY)->body();
        $this->assertContains('Check your phone', $waiting);
        $this->assertContains('http-equiv="refresh"', $waiting);
        $this->api->on('GET', '/v1/billing/payments/' . self::PAY, 200, ['status' => 'succeeded', 'receipt' => 'TGR7ABC123', 'final' => true, 'completed_at' => '2026-10-14T08:00:20Z'] + $payment);
        $done = $this->get('/console/billing/payments/' . self::PAY)->body();
        $this->assertContains('Payment received', $done);
        $this->assertContains('TGR7ABC123', $done);
        $this->assertNotContains('http-equiv="refresh"', $done);
    }

    public function testToolsHubGeneratorAndSecurityTxt(): void
    {
        $hub = $this->get('/tools')->body();
        foreach (['Domain health check', 'DNS record generator', 'Blocklist checker', 'Mail port checker', 'Email header analyzer'] as $name) {
            $this->assertContains($name, $hub);
        }
        $this->assertStatus(404, $this->get('/tools/temp-mail'));
        $gen = $this->get('/tools/generator?domain=example.co.ke&providers[]=google&mx=1&ips=203.0.113.10&all=-all&policy=quarantine&rua=dmarc@example.co.ke&sts_mode=testing&mx_hosts=mail.example.co.ke&tlsrpt=tls@example.co.ke')->body();
        $this->assertContains('v=spf1 mx include:_spf.google.com ip4:203.0.113.10 -all', $gen);
        $this->assertContains('v=DMARC1; p=quarantine; rua=mailto:dmarc@example.co.ke', $gen);
        $this->assertContains('_mta-sts.example.co.ke', $gen);
        $this->assertContains('mode: testing', $gen);
        $this->assertContains('v=TLSRPTv1; rua=mailto:tls@example.co.ke', $gen);
        $this->assertContains('Enter a domain', $this->get('/tools/generator?domain=not_a_domain')->body());

        $this->api->on('GET', '/v1/tools/blocklist', 200, ['target' => '203.0.113.5', 'addresses' => [['ip' => '203.0.113.5', 'host' => '203.0.113.5', 'ptr' => [], 'fcrdns' => false, 'listed_on' => ['SpamCop']]],
            'lists_checked' => [['zone' => 'bl.spamcop.net', 'name' => 'SpamCop'], ['zone' => 'psbl.surriel.com', 'name' => 'PSBL']], 'listed_count' => 1, 'status' => 'fail', 'checked_at' => '2026-10-14T08:00:00Z']);
        $this->assertContains('1 address(es) are listed', $this->get('/tools/blocklist?target=203.0.113.5')->body());

        $txt = $this->get('/.well-known/security.txt');
        $this->assertContains('Expires: ', $txt->body());
        $this->assertContains('Canonical: ', $txt->body());
    }

    public function testOperatorSecurityPage(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $this->api->on('GET', '/v1/admin/security', 200, [
            'counts' => ['failed_logins_24h' => 7, 'failed_mfa_24h' => 1, 'password_resets_24h' => 0, 'mfa_changes_7d' => 2, 'role_changes_7d' => 1, 'keys_created_24h' => 0,
                'locked_users' => 1, 'locked_mailboxes' => 0, 'admins_total' => 1, 'admins_without_mfa' => 1, 'owners_without_mfa' => 2, 'unverified_users' => 3],
            'controls' => ['breached_password_screening' => true, 'operator_mfa_enforced' => false, 'secure_cookies' => false, 'sms_verification' => false, 'google_sign_in' => false, 'mpesa_payments' => true, 'content_retention_days' => 30],
            'findings' => [['level' => 'fail', 'message' => '1 platform operator(s) without two-factor authentication.']],
            'top_failing_accounts' => [['account' => 'admin@example.com', 'count' => 7]], 'top_failing_ips' => [['ip' => '198.51.100.7', 'count' => 7]],
            'operators' => [['id' => 'x', 'email' => 'admin@example.com', 'full_name' => 'Admin', 'mfa_enabled' => false, 'last_login_at' => null]],
            'privileged_activity' => [['action' => 'member.role_changed', 'actor' => 'admin@example.com', 'ip' => '127.0.0.1', 'at' => '2026-10-14T08:00:00Z']],
        ]);
        $page = $this->get('/admin/security')->body();
        $this->assertContains('without two-factor authentication', $page);
        $this->assertContains('198.51.100.7', $page);
        $this->assertContains('member.role_changed', $page);
    }
}
