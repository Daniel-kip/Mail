<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class AdminTest extends TestCase
{
    private const INV = '0192a000-0000-7000-8000-0000000000e1';

    public function setUp(): void
    {
        $this->boot();
        $now = '2026-10-13T08:00:00Z';
        $plans = Fixtures::plans()['data'];
        $invoice = ['id' => self::INV, 'number' => 'MV-2026-0001', 'kind' => 'renewal', 'status' => 'open', 'overdue' => true, 'currency' => 'KES', 'organization' => 'Riverside Academy', 'period_start' => $now, 'period_end' => $now,
            'subtotal_cents' => 120000, 'tax_rate_bps' => 1600, 'tax_cents' => 19200, 'total_cents' => 139200, 'issued_at' => $now, 'due_at' => $now, 'paid_at' => null, 'voided_at' => null,
            'lines' => [['description' => 'Plan', 'quantity' => 1, 'unit_cents' => 1000, 'amount_cents' => 1000]], 'billing' => ['organization' => 'Riverside Academy']];
        $this->api
            ->on('GET', '/v1/admin/overview', 200, ['organizations' => 3, 'suspended_organizations' => 0, 'users' => 4, 'mailboxes' => 9, 'messages_24h' => 120, 'bounced_24h' => 1, 'failed_24h' => 0,
                'queue' => ['pending' => 2, 'processing' => 0, 'dead' => 1, 'oldest_pending_seconds' => 12], 'webhook_failures_24h' => 0, 'smtp_relay' => true, 'mail_storage_configured' => true, 'auto_reply_configured' => false,
                'billing' => ['currency' => 'KES', 'open_invoices' => 1, 'open_total_cents' => 139200, 'overdue_invoices' => 1, 'paid_last_30_days_cents' => 0, 'past_due_subscriptions' => 0, 'trialing' => 1],
                'organizations_by_plan' => [['plan' => 'free', 'name' => 'Institution Free', 'organizations' => 2]]])
            ->on('GET', '/v1/admin/organizations', 200, ['data' => [['id' => Fixtures::ORG_ID, 'name' => 'Riverside Academy', 'slug' => 'riverside', 'status' => 'active', 'suspended_reason' => null, 'plan' => 'pro', 'created_at' => $now, 'members' => 2, 'domains' => 1, 'mailboxes' => 9, 'messages_7d' => 50, 'bounced_7d' => 1]]])
            ->on('GET', '/v1/admin/organizations/' . Fixtures::ORG_ID, 200, ['id' => Fixtures::ORG_ID, 'name' => 'Riverside Academy', 'slug' => 'riverside', 'status' => 'active', 'suspended_reason' => null, 'account_type' => 'institution', 'plan' => ['id' => 'pro', 'name' => 'Premium'],
                'billing_email' => null, 'created_at' => $now, 'subscription' => ['status' => 'active', 'billing_cycle' => 'monthly', 'current_period_end' => $now, 'pending_plan' => null],
                'members' => [['user_id' => Fixtures::USER_ID, 'email' => 'grace@riverside.example', 'full_name' => 'Grace', 'role' => 'owner', 'mfa_enabled' => true, 'last_login_at' => $now]],
                'domains' => [['domain' => 'riverside.example', 'status' => 'verified', 'mail_hosting_enabled' => true]], 'mailboxes' => 9, 'active_api_keys' => 1, 'messages_30d' => 200, 'recent_activity' => [['action' => 'mailbox.created', 'actor' => 'grace', 'at' => $now]]])
            ->on('GET', '/v1/admin/plans', 200, ['data' => $plans])
            ->on('GET', '/v1/admin/users', 200, ['data' => [['id' => Fixtures::USER_ID, 'email' => 'grace@riverside.example', 'full_name' => 'Grace', 'status' => 'active', 'is_platform_admin' => false, 'mfa_enabled' => true, 'email_verified' => true, 'last_login_at' => $now, 'created_at' => $now, 'organizations' => [['name' => 'Riverside Academy', 'role' => 'owner']]]]])
            ->on('GET', '/v1/admin/invoices', 200, ['summary' => ['open' => 1, 'open_total_cents' => 139200, 'overdue' => 1, 'currency' => 'KES'], 'data' => [$invoice]])
            ->on('GET', '/v1/admin/invoices/' . self::INV, 200, $invoice)
            ->on('GET', '/v1/admin/jobs/dead', 200, ['data' => [['id' => 41, 'kind' => 'send_message', 'organization_id' => Fixtures::ORG_ID, 'message_id' => null, 'attempts' => 8, 'last_error' => 'relay refused', 'updated_at' => $now, 'status' => 'dead', 'email' => null]]])
            ->on('GET', '/v1/admin/mail-client-settings', 200, ['default' => null, 'environment' => ['imap_host' => 'mail.example.com', 'imap_port' => 993, 'smtp_host' => 'mail.example.com', 'smtp_port' => 465, 'smtp_security' => 'tls', 'smtp_alt_port' => 587, 'smtp_alt_security' => 'starttls'],
                'overrides' => [['domain_id' => Fixtures::DOMAIN_ID, 'domain' => 'riverside.example', 'organization' => 'Riverside Academy', 'imap_host' => 'imap.riverside.example', 'imap_port' => 993, 'smtp_host' => 'smtp.riverside.example', 'smtp_port' => 465, 'smtp_security' => 'tls', 'smtp_alt_port' => null, 'smtp_alt_security' => null, 'version' => 2, 'updated_at' => $now]],
                'history' => [['id' => 1, 'scope' => 'riverside.example', 'action' => 'updated', 'settings' => [], 'change_note' => 'New IMAP host', 'changed_by' => 'ops', 'at' => $now]]])
            ->on('GET', '/v1/admin/onboarding', 200, ['window_days' => 30, 'funnel' => [['event' => 'guide_started', 'mailboxes' => 5], ['event' => 'guide_completed', 'mailboxes' => 3]], 'by_client' => [['client' => 'gmail', 'mailboxes' => 3, 'verified' => 2]], 'checks' => [['direction' => 'incoming', 'result' => 'verified', 'count' => 2]], 'help_topics' => [['topic' => 'cannot-send', 'count' => 4]]]);
    }

    public function testOperatorPagesRenderForPlatformAdmins(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $pages = ['/admin' => 'Platform overview', '/admin/organizations' => 'Riverside Academy', '/admin/organizations/' . Fixtures::ORG_ID => 'Change plan', '/admin/users' => 'grace@riverside.example',
            '/admin/billing' => 'MV-2026-0001', '/admin/billing/invoices/' . self::INV => 'Record payment', '/admin/plans' => 'Premium', '/admin/queue' => 'relay refused',
            '/admin/mail-clients' => 'imap.riverside.example', '/admin/onboarding' => 'cannot send'];
        foreach ($pages as $path => $needle) {
            $res = $this->get($path);
            $this->assertStatus(200, $res, $path);
            $this->assertContains($needle, strtolower($res->body()) === $res->body() ? $res->body() : $res->body() . strtolower($res->body()), $path);
        }
    }

    public function testNonAdminsGetNotFound(): void
    {
        $this->signInConsole();
        $this->assertStatus(404, $this->get('/admin'));
    }

    public function testMailClientDefaultIsNormalised(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $this->api->on('PUT', '/v1/admin/mail-client-settings', 200, ['ok' => true]);
        $this->post('/admin/mail-clients/default', ['imap_host' => 'Mail.Example.com', 'imap_port' => '993', 'smtp_host' => 'mail.example.com', 'smtp_port' => '465', 'smtp_security' => 'tls', 'smtp_alt_port' => '', 'change_note' => '']);
        $sent = $this->api->callsTo('PUT', '/v1/admin/mail-client-settings')[0]['options']['json'];
        $this->assertSame('mail.example.com', $sent['imap_host']);
        $this->assertSame(null, $sent['smtp_alt_port']);
        $this->assertSame(null, $sent['smtp_alt_security']);
    }
}
