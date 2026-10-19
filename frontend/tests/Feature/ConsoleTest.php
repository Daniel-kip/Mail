<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class ConsoleTest extends TestCase
{
    private const MSG_ID = '0192a000-0000-7000-8000-0000000000a1';
    private const KEY_ID = '0192a000-0000-7000-8000-0000000000b1';
    private const HOOK_ID = '0192a000-0000-7000-8000-0000000000c1';
    private const TPL_ID = '0192a000-0000-7000-8000-0000000000d1';
    private const INV_ID = '0192a000-0000-7000-8000-0000000000e1';

    public function setUp(): void
    {
        $this->boot();
        $this->signInConsole();
        $plans = Fixtures::plans();
        $plan = $plans['data'][3];
        $now = '2026-10-13T08:00:00Z';
        $domain = ['id' => Fixtures::DOMAIN_ID, 'domain' => 'riverside.example', 'status' => 'verified', 'sending_enabled' => true, 'mail_hosting_enabled' => true, 'verified_at' => $now,
            'last_checked_at' => $now, 'check_count' => 2, 'checks' => ['spf' => true, 'dkim' => true, 'dmarc' => false, 'mx' => true], 'dkim_selector' => 'mv1', 'mailbox_count' => 1, 'created_at' => $now,
            'dns_records' => [
                ['type' => 'TXT', 'host' => '_mailvora.riverside.example', 'value' => 'mailvora-verify=abc', 'purpose' => 'Proves you control this domain', 'required' => true, 'status' => 'verified'],
                ['type' => 'TXT', 'host' => '_dmarc.riverside.example', 'value' => 'v=DMARC1; p=none', 'purpose' => 'Reporting', 'required' => false, 'status' => 'missing'],
            ],
            'client_discovery_records' => [['type' => 'SRV', 'host' => '_imaps._tcp.riverside.example', 'value' => '0 1 993 mail.example.com.', 'purpose' => 'RFC 6186', 'required' => false, 'status' => 'unchecked']]];
        $mailbox = ['id' => Fixtures::MAILBOX_ID, 'domain_id' => Fixtures::DOMAIN_ID, 'domain' => 'riverside.example', 'local_part' => 'amina', 'address' => 'amina@riverside.example', 'display_name' => 'Amina Odhiambo',
            'status' => 'active', 'quota_bytes' => 1073741824, 'owner' => null, 'recovery_email' => 'a***@gmail.com', 'has_recovery_email' => true, 'department' => 'Finance', 'job_title' => 'Accountant', 'locked' => true,
            'last_login_at' => $now, 'activated_at' => $now, 'password_changed_at' => $now, 'client_setup' => ['state' => 'guide_completed', 'last_client' => 'gmail', 'incoming_verified_at' => null, 'outgoing_verified_at' => null],
            'created_at' => $now, 'updated_at' => $now];
        $message = ['id' => self::MSG_ID, 'batch_id' => null, 'source' => 'api', 'from' => 'billing@riverside.example', 'from_name' => 'Billing', 'to' => ['parent@example.com'], 'reply_to' => null, 'reply_mode' => null,
            'subject' => 'Invoice INV-1', 'status' => 'scheduled', 'sandbox' => false, 'attempts' => 0, 'last_error' => null, 'last_smtp_code' => null, 'tags' => ['invoice'], 'metadata' => ['order' => 7], 'template_id' => null,
            'template_version' => null, 'smtp_message_id' => null, 'size_bytes' => 2048, 'accepted_at' => null, 'bounced_at' => null, 'scheduled_at' => '2026-10-14T08:00:00Z', 'list_unsubscribe' => true, 'created_at' => $now, 'updated_at' => $now];
        $invoice = ['id' => self::INV_ID, 'number' => 'MV-2026-0001', 'kind' => 'renewal', 'status' => 'open', 'overdue' => false, 'currency' => 'KES', 'period_start' => $now, 'period_end' => '2026-11-13T08:00:00Z',
            'subtotal_cents' => 120000, 'tax_rate_bps' => 1600, 'tax_cents' => 19200, 'total_cents' => 139200, 'issued_at' => $now, 'due_at' => '2026-10-27T08:00:00Z', 'paid_at' => null, 'voided_at' => null,
            'lines' => [['description' => 'Premium (monthly)', 'quantity' => 1, 'unit_cents' => 120000, 'amount_cents' => 120000]],
            'billing' => ['organization' => 'Riverside Academy', 'billing_name' => 'Riverside Academy Ltd', 'tax_label' => 'VAT', 'payment_instructions' => 'Pay by M-Pesa paybill 123456.']];

        $this->api
            ->on('GET', '/v1/usage', 200, ['plan' => 'Premium', 'quota' => ['daily' => ['used' => 5, 'limit' => 200], 'monthly' => ['used' => 40, 'limit' => 3000]],
                'daily' => [['date' => '2026-10-12', 'total' => 4, 'by_status' => ['delivered' => 3, 'bounced' => 1]], ['date' => '2026-10-13', 'total' => 1, 'by_status' => ['queued' => 1]]],
                'totals' => ['domains' => 1, 'verified_domains' => 1, 'mailboxes' => 1, 'active_api_keys' => 1, 'suppressions' => 0, 'active_webhooks' => 1, 'in_flight' => 1],
                'last_7_days' => ['sent' => 4, 'bounced' => 1, 'failed' => 0, 'bounce_rate' => 0.25]])
            ->on('GET', '/v1/domains', 200, ['data' => [$domain]])
            ->on('GET', '/v1/domains/' . Fixtures::DOMAIN_ID, 200, $domain)
            ->on('GET', '/v1/messages', 200, ['data' => [$message], 'next_cursor' => 'abc'])
            ->on('GET', '/v1/messages/' . self::MSG_ID, 200, $message + ['events' => [['type' => 'queued', 'details' => ['by' => 'api'], 'occurred_at' => $now]], 'content' => ['html' => '<p>Hi</p>', 'text' => 'Hi']])
            ->on('GET', '/v1/api-keys', 200, ['data' => [['id' => self::KEY_ID, 'name' => 'Billing app', 'prefix' => 'mv_live_ab12', 'environment' => 'live', 'scopes' => ['messages:send'], 'status' => 'active', 'created_by' => 'grace',
                'expires_at' => null, 'revoked_at' => null, 'last_used_at' => $now, 'last_used_ip' => '203.0.113.5', 'created_at' => $now, 'description' => 'Invoices', 'allowed_ips' => ['203.0.113.0/24'], 'allowed_sender_domains' => ['riverside.example'], 'messages_30d' => 12]]])
            ->on('GET', '/v1/api-keys/scopes', 200, ['scopes' => ['messages:send', 'messages:read', 'mailboxes:read'], 'default' => ['messages:send']])
            ->on('GET', '/v1/mailboxes', 200, ['data' => [$mailbox]])
            ->on('GET', '/v1/mailboxes/onboarding', 200, ['totals' => ['mailboxes' => 1, 'pending_activation' => 0, 'guide_started' => 1, 'guide_completed' => 1, 'incoming_verified' => 0, 'outgoing_verified' => 0, 'fully_verified' => 0],
                'by_client' => [['client' => 'gmail', 'mailboxes' => 1]], 'by_device' => [['device' => 'android', 'mailboxes' => 1]], 'help_topics' => [['client' => 'gmail', 'topic' => 'wrong-password', 'count' => 2]]])
            ->on('GET', '/v1/mailboxes/' . Fixtures::MAILBOX_ID, 200, $mailbox + ['client_settings' => Fixtures::setup()['settings'], 'usage' => ['used_bytes' => 1000, 'limit_bytes' => 1073741824], 'aliases' => [], 'pending_activation' => null,
                'setup_events' => [['event' => 'guide_started', 'device' => 'android', 'client' => 'gmail', 'at' => $now]]])
            ->on('GET', '/v1/organization/support', 200, Fixtures::setup()['support'])
            ->on('GET', '/v1/aliases', 200, ['data' => [['id' => self::KEY_ID, 'domain_id' => Fixtures::DOMAIN_ID, 'address' => 'info@riverside.example', 'destinations' => ['amina@riverside.example'], 'status' => 'active', 'created_at' => $now]]])
            ->on('GET', '/v1/templates', 200, ['data' => [['id' => self::TPL_ID, 'name' => 'Invoice', 'slug' => 'invoice', 'description' => null, 'status' => 'active', 'active_version' => 1, 'latest_version' => 2, 'created_at' => $now, 'updated_at' => $now]]])
            ->on('GET', '/v1/templates/' . self::TPL_ID, 200, ['id' => self::TPL_ID, 'name' => 'Invoice', 'slug' => 'invoice', 'description' => null, 'status' => 'active', 'active_version' => 1, 'latest_version' => 2, 'created_at' => $now, 'updated_at' => $now])
            ->on('GET', '/v1/templates/' . self::TPL_ID . '/versions', 200, ['data' => [['version' => 2, 'subject' => 'Invoice {{n}}', 'created_at' => $now], ['version' => 1, 'subject' => 'Invoice', 'created_at' => $now]]])
            ->on('GET', '/v1/templates/' . self::TPL_ID . '/versions/2', 200, ['version' => 2, 'subject' => 'Invoice {{n}}', 'html_body' => '<p>{{n}}</p>', 'text_body' => '{{n}}', 'variables' => [['name' => 'n', 'required' => true, 'sample' => '42']], 'created_at' => $now])
            ->on('GET', '/v1/suppressions', 200, ['data' => [['id' => self::KEY_ID, 'email' => 'gone@example.com', 'reason' => 'hard_bounce', 'source_message_id' => self::MSG_ID, 'details' => '550 user unknown', 'created_at' => $now]], 'next_cursor' => null])
            ->on('GET', '/v1/webhooks', 200, ['data' => [['id' => self::HOOK_ID, 'url' => 'https://app.example.com/hook', 'description' => 'App', 'events' => ['message.delivered'], 'status' => 'active', 'consecutive_failures' => 0, 'created_at' => $now, 'updated_at' => $now]]])
            ->on('GET', '/v1/webhooks/events', 200, ['data' => ['message.delivered', 'message.bounced']])
            ->on('GET', '/v1/webhooks/' . self::HOOK_ID, 200, ['id' => self::HOOK_ID, 'url' => 'https://app.example.com/hook', 'description' => 'App', 'events' => ['message.delivered'], 'status' => 'active', 'consecutive_failures' => 0, 'created_at' => $now, 'updated_at' => $now])
            ->on('GET', '/v1/webhooks/' . self::HOOK_ID . '/deliveries', 200, ['data' => [['id' => self::KEY_ID, 'event_id' => 'e1', 'event_type' => 'message.delivered', 'status' => 'failed', 'attempts' => 3, 'response_status' => 500, 'response_excerpt' => 'err', 'last_error' => 'HTTP 500', 'payload' => [], 'created_at' => $now, 'completed_at' => null]]])
            ->on('GET', '/v1/members', 200, ['data' => [['user_id' => Fixtures::USER_ID, 'email' => 'grace@riverside.example', 'full_name' => 'Grace Mutua', 'role' => 'owner', 'mfa_enabled' => true, 'last_login_at' => $now, 'joined_at' => $now, 'is_you' => true],
                ['user_id' => self::KEY_ID, 'email' => 'tech@riverside.example', 'full_name' => 'Tech Person', 'role' => 'support', 'mfa_enabled' => false, 'last_login_at' => null, 'joined_at' => $now, 'is_you' => false]],
                'invitations' => [['id' => self::HOOK_ID, 'email' => 'new@riverside.example', 'role' => 'viewer', 'invited_by' => 'grace', 'expires_at' => '2026-10-20T00:00:00Z', 'created_at' => $now]]])
            ->on('GET', '/v1/billing', 200, ['account_type' => 'institution', 'currency' => 'KES', 'subscription' => ['status' => 'trialing', 'billing_cycle' => 'monthly', 'current_period_start' => $now, 'current_period_end' => '2026-10-27T08:00:00Z',
                'trial_ends_at' => '2026-10-27T08:00:00Z', 'trial_used' => true, 'cancel_at_period_end' => false, 'plan' => $plan, 'pending_plan' => null], 'seats' => 1,
                'next_invoice' => ['date' => '2026-10-27T08:00:00Z', 'subtotal_cents' => 120000, 'tax_cents' => 19200, 'total_cents' => 139200, 'lines' => $invoice['lines']], 'open_balance_cents' => 139200,
                'details' => ['organization' => 'Riverside Academy', 'billing_name' => null, 'billing_email' => null, 'billing_address' => null, 'tax_id' => null, 'country' => 'KE'],
                'payment_instructions' => 'Pay by M-Pesa paybill 123456.', 'invoices' => [$invoice]])
            ->on('GET', '/v1/plans', 200, $plans)
            ->on('GET', '/v1/billing/invoices/' . self::INV_ID, 200, $invoice)
            ->on('GET', '/v1/audit-logs', 200, ['data' => [['id' => 9, 'actor_type' => 'user', 'actor_id' => Fixtures::USER_ID, 'actor' => 'grace@riverside.example', 'action' => 'mailbox.created', 'target_type' => 'mailbox', 'target_id' => null, 'ip' => '127.0.0.1', 'user_agent' => 'x', 'details' => ['address' => 'amina@riverside.example'], 'created_at' => $now]], 'next_before_id' => null])
            ->on('GET', '/v1/organization', 200, ['id' => Fixtures::ORG_ID, 'name' => 'Riverside Academy', 'slug' => 'riverside', 'status' => 'active', 'account_type' => 'institution', 'suspended_reason' => null, 'created_at' => $now,
                'plan' => ['id' => 'pro', 'name' => 'Premium', 'audience' => 'institution', 'daily_send_limit' => 200, 'monthly_send_limit' => 3000, 'max_domains' => 2, 'max_mailboxes' => 5, 'default_mailbox_quota_bytes' => 1073741824,
                    'max_mailbox_quota_bytes' => 2147483648, 'max_recipients_per_request' => 10, 'api_requests_per_minute' => 60, 'max_members' => 3, 'max_api_keys' => 5, 'max_webhooks' => 2]])
            ->on('GET', '/v1/auth/sessions', 200, ['data' => [['id' => self::KEY_ID, 'user_agent' => 'Firefox', 'ip' => '127.0.0.1', 'created_at' => $now, 'last_seen_at' => $now, 'current' => true]]]);
    }

    public function testEveryConsolePageRenders(): void
    {
        $pages = [
            '/console' => 'Welcome back, Grace', '/console/usage' => 'Messages per day', '/console/developers' => 'Idempotency-Key',
            '/console/domains' => 'riverside.example', '/console/domains/' . Fixtures::DOMAIN_ID => '_imaps._tcp.riverside.example',
            '/console/mailboxes' => 'Amina Odhiambo', '/console/mailboxes/new' => 'Email an invitation', '/console/mailboxes/import' => 'local_part,display_name',
            '/console/mailboxes/onboarding' => 'Phone setup report', '/console/mailboxes/' . Fixtures::MAILBOX_ID => 'Unlock sign-in',
            '/console/mailboxes/' . Fixtures::MAILBOX_ID . '/handout' => 'Welcome to your work email', '/console/aliases' => 'info@riverside.example',
            '/console/api-keys' => 'mv_live_ab12', '/console/templates' => 'invoice', '/console/templates/new' => 'Create template',
            '/console/templates/' . self::TPL_ID . '?version=2' => 'Save and activate', '/console/messages' => 'Invoice INV-1',
            '/console/messages/compose' => 'Send later', '/console/messages/' . self::MSG_ID => 'Cancel scheduled send',
            '/console/suppressions' => 'gone@example.com', '/console/webhooks' => 'https://app.example.com/hook', '/console/webhooks/' . self::HOOK_ID => 'Retry',
            '/console/team' => 'ICT support', '/console/billing' => 'Free trial until', '/console/billing/plans' => 'KES 1,200',
            '/console/billing/invoices/' . self::INV_ID => 'MV-2026-0001', '/console/audit' => 'mailbox.created', '/console/settings' => 'Help for employees',
            '/console/account' => 'Set up two-factor', '/console/organizations/new' => 'Create a workspace',
        ];
        foreach ($pages as $path => $needle) {
            $res = $this->get($path);
            $this->assertStatus(200, $res, $path);
            $this->assertContains($needle, $res->body(), $path);
        }
    }

    public function testIndividualAccountsAreSentToTheUpgradePage(): void
    {
        $this->signInConsole(['organization' => ['account_type' => 'individual']]);
        $this->assertRedirect('/console/mailboxes/upgrade', $this->get('/console/mailboxes'));
        $page = $this->get('/console/mailboxes/upgrade')->body();
        $this->assertContains('Convert to an institution workspace', $page);
        $this->assertNotContains('mailboxes:read', $this->get('/console/api-keys')->body(), 'individual keys cannot get mailbox scopes');
    }

    public function testViewersCannotReachManagementPages(): void
    {
        $this->signInConsole(['organization' => ['role' => 'viewer', 'permissions' => Fixtures::permissions('viewer')]]);
        $this->assertStatus(403, $this->get('/console/api-keys'));
        $this->assertStatus(403, $this->post('/console/domains', ['domain' => 'x.example']));
    }

    public function testNewApiKeyIsShownExactlyOnce(): void
    {
        $this->api->on('POST', '/v1/api-keys', 201, ['id' => self::KEY_ID, 'name' => 'CI', 'key' => 'mv_live_SECRET_VALUE']);
        $this->assertRedirect('/console/api-keys', $this->post('/console/api-keys', ['name' => 'CI', 'environment' => 'live', 'scopes' => ['messages:send'], 'allowed_ips' => '203.0.113.9, 10.0.0.0/8', 'allowed_sender_domains' => 'Riverside.example']));
        $sent = $this->api->callsTo('POST', '/v1/api-keys')[0]['options']['json'];
        $this->assertSame(['203.0.113.9', '10.0.0.0/8'], $sent['allowed_ips']);
        $this->assertSame(['riverside.example'], $sent['allowed_sender_domains']);
        $this->assertContains('mv_live_SECRET_VALUE', $this->get('/console/api-keys')->body());
        $this->assertNotContains('mv_live_SECRET_VALUE', $this->get('/console/api-keys')->body());
    }

    public function testMailboxInvitationAndSupportActions(): void
    {
        $this->assertRedirect('/console/mailboxes/new', $this->post('/console/mailboxes', ['domain_id' => Fixtures::DOMAIN_ID, 'local_part' => 'jane', 'display_name' => 'Jane', 'setup_method' => 'invite']));
        $this->assertSame('Personal email is required.', $this->flashed('errors')['recovery_email'] ?? null);

        $this->api->on('POST', '/v1/mailboxes', 201, ['id' => Fixtures::MAILBOX_ID, 'address' => 'jane@riverside.example', 'activation' => ['method' => 'link', 'url' => 'https://app.example.com/mail/activate?token=TOKEN123', 'expires_at' => '2026-10-20T00:00:00Z']]);
        $this->assertRedirect('/console/mailboxes/' . Fixtures::MAILBOX_ID, $this->post('/console/mailboxes', ['domain_id' => Fixtures::DOMAIN_ID, 'local_part' => 'Jane', 'display_name' => 'Jane', 'setup_method' => 'link', 'quota_mb' => '500']));
        $sent = $this->api->callsTo('POST', '/v1/mailboxes')[0]['options']['json'];
        $this->assertSame('jane', $sent['local_part']);
        $this->assertSame(500 * 1048576, $sent['quota_bytes']);
        $this->assertTrue(!isset($sent['password']), 'no password for link activation');
        $this->assertContains('activate?token=TOKEN123', $this->get('/console/mailboxes/' . Fixtures::MAILBOX_ID)->body());

        $this->signInConsole(['organization' => ['role' => 'support', 'permissions' => Fixtures::permissions('support')]]);
        $this->api->on('POST', '/v1/mailboxes/' . Fixtures::MAILBOX_ID . '/send-reset', 200, ['id' => Fixtures::MAILBOX_ID, 'reset' => ['method' => 'email', 'sent_to' => 'a***@gmail.com', 'expires_at' => '2026-10-13T09:00:00Z']]);
        $this->assertRedirect('/console/mailboxes/' . Fixtures::MAILBOX_ID . '#access', $this->post('/console/mailboxes/' . Fixtures::MAILBOX_ID . '/send-reset', ['method' => 'email']));
        $this->assertStatus(403, $this->post('/console/mailboxes/' . Fixtures::MAILBOX_ID . '/password', ['mode' => 'generate']));
        $page = $this->get('/console/mailboxes/' . Fixtures::MAILBOX_ID)->body();
        $this->assertNotContains('Set or generate a password', $page, 'support staff never set passwords');
    }

    public function testCsvImportParsesHeadersAndCreatesInvitations(): void
    {
        $rows = \App\Http\Controllers\Console\MailboxController::parseCsv("\xEF\xBB\xBFLocal Part;Display Name;Recovery Email\njane;Jane W;jane@gmail.com\n\npeter;\"Otieno, Peter\";\n");
        $this->assertSame(2, count($rows));
        $this->assertSame('Otieno, Peter', $rows[1]['display_name']);
        $this->assertSame('jane@gmail.com', $rows[0]['recovery_email']);
    }

    public function testComposeSchedulesInUtcAndSendsIdempotencyKey(): void
    {
        $this->api->on('POST', '/v1/messages', 202, ['batch_id' => self::MSG_ID, 'status' => 'accepted', 'messages' => [['id' => self::MSG_ID, 'to' => 'parent@example.com', 'status' => 'scheduled']]]);
        $res = $this->post('/console/messages', ['from' => 'billing@riverside.example', 'to' => 'a@example.com, b@example.com', 'subject' => 'Hi', 'text' => 'Body', 'send_at' => '2026-10-20T09:30', 'unsubscribe' => '1']);
        $this->assertRedirect('/console/messages/' . self::MSG_ID, $res);
        $call = $this->api->callsTo('POST', '/v1/messages')[0]['options'];
        $this->assertSame('2026-10-20T06:30:00Z', $call['json']['send_at']);
        $this->assertSame(['a@example.com', 'b@example.com'], $call['json']['to']);
        $this->assertTrue($call['json']['unsubscribe']);
        $this->assertTrue(str_starts_with($call['headers']['Idempotency-Key'] ?? '', 'console-'));
    }

    public function testTemplatePreviewProxiesJson(): void
    {
        $this->api->on('POST', '/v1/templates/preview', 200, ['subject' => 'Invoice 42', 'html' => '<p>42</p>', 'text' => '42']);
        $res = $this->post('/console/templates/preview', ['subject' => 'Invoice {{n}}', 'html_body' => '<p>{{n}}</p>', 'variables' => "n | 42 | required"]);
        $this->assertStatus(200, $res);
        $this->assertContains('Invoice 42', $res->body());
        $sent = $this->api->callsTo('POST', '/v1/templates/preview')[0]['options']['json'];
        $this->assertSame('42', $sent['data']->n);
    }

    public function testPlanChangeOutcomes(): void
    {
        $this->api->on('POST', '/v1/billing/subscription', 200, ['outcome' => 'scheduled', 'effective_at' => '2026-11-13T08:00:00Z']);
        $this->assertRedirect('/console/billing', $this->post('/console/billing/plan', ['plan_id' => 'free', 'billing_cycle' => 'monthly']));
        $this->assertContains('at the end of the paid period', (string) $this->flashed('success'));
    }

    public function testExportsAreProxiedAsDownloads(): void
    {
        $this->api->on('GET', '/v1/messages/export.csv', 200, []);
        $res = $this->get('/console/messages/export?status=delivered');
        $this->assertStatus(200, $res);
        $this->assertContains('attachment', (string) ($res->headers()['Content-Disposition'] ?? ''));
    }
}
