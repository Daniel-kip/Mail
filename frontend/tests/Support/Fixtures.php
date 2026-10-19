<?php

declare(strict_types=1);

namespace Tests\Support;

/**
 * Canonical API payloads (shapes copied from the Rust handlers) used by the
 * feature tests.
 */
final class Fixtures
{
    public const ORG_ID = '0192a000-0000-7000-8000-000000000001';
    public const USER_ID = '0192a000-0000-7000-8000-000000000002';
    public const MAILBOX_ID = '0192a000-0000-7000-8000-000000000003';
    public const DOMAIN_ID = '0192a000-0000-7000-8000-000000000004';

    public static function permissions(string $role = 'owner'): array
    {
        $viewer = ['messages:read', 'templates:read', 'domains:read', 'mailboxes:read', 'suppressions:read', 'webhooks:read', 'usage:read', 'members:read'];
        $developer = ['messages:send', 'templates:write', 'suppressions:write', 'webhooks:write', 'api_keys:manage'];
        $admin = ['domains:write', 'mailboxes:write', 'mailboxes:assist', 'members:manage', 'organization:manage', 'audit:read', 'billing:read', 'billing:manage'];
        return match ($role) {
            'viewer' => $viewer,
            'support' => array_merge($viewer, ['mailboxes:assist']),
            'developer' => array_merge($viewer, $developer),
            default => array_merge($viewer, $developer, $admin),
        };
    }

    public static function consoleSession(string $role = 'owner', string $type = 'institution'): array
    {
        return [
            'user' => ['id' => self::USER_ID, 'email' => 'grace@riverside.example', 'full_name' => 'Grace Mutua', 'email_verified' => true, 'mfa_enabled' => false, 'is_platform_admin' => false],
            'organization' => ['id' => self::ORG_ID, 'name' => 'Riverside Academy', 'role' => $role, 'status' => 'active', 'plan' => 'free', 'account_type' => $type, 'permissions' => self::permissions($role)],
            'organizations' => [['id' => self::ORG_ID, 'name' => 'Riverside Academy', 'slug' => 'riverside-academy', 'role' => $role, 'status' => 'active', 'plan' => 'free', 'account_type' => $type]],
            'csrf_token' => 'backend-csrf',
            'expires_at' => '2030-01-01T00:00:00Z',
            'environment' => 'development',
        ];
    }

    public static function mailboxSession(): array
    {
        return [
            'mailbox' => ['id' => self::MAILBOX_ID, 'address' => 'amina@riverside.example', 'display_name' => 'Amina Odhiambo', 'quota_bytes' => 2147483648, 'organization' => 'Riverside Academy'],
            'csrf_token' => 'mail-csrf',
            'storage_configured' => true,
            'quota' => ['used_bytes' => 52428800, 'limit_bytes' => 2147483648],
        ];
    }

    public static function folders(): array
    {
        return ['data' => [
            ['name' => 'INBOX', 'display_name' => 'Inbox', 'role' => 'inbox', 'total' => 3, 'unseen' => 2],
            ['name' => 'Sent', 'display_name' => 'Sent', 'role' => 'sent', 'total' => 1, 'unseen' => 0],
            ['name' => 'Drafts', 'display_name' => 'Drafts', 'role' => 'drafts', 'total' => 0, 'unseen' => 0],
            ['name' => 'Trash', 'display_name' => 'Trash', 'role' => 'trash', 'total' => 0, 'unseen' => 0],
        ]];
    }

    public static function setup(): array
    {
        return [
            'mailbox' => ['address' => 'amina@riverside.example', 'display_name' => 'Amina Odhiambo', 'status' => 'active', 'quota_bytes' => 2147483648, 'organization' => 'Riverside Academy', 'domain' => 'riverside.example'],
            'domain' => ['status' => 'verified', 'mail_hosting_enabled' => true],
            'organization_status' => 'active',
            'settings' => [
                'username' => 'amina@riverside.example',
                'imap' => ['protocol' => 'IMAP', 'host' => 'mail.riverside.example', 'port' => 993, 'security' => 'SSL/TLS', 'security_code' => 'tls', 'authentication' => 'Normal password'],
                'smtp' => ['protocol' => 'SMTP', 'host' => 'mail.riverside.example', 'port' => 465, 'security' => 'SSL/TLS', 'security_code' => 'tls', 'authentication' => 'Normal password', 'requires_authentication' => true],
                'smtp_alternative' => ['port' => 587, 'security' => 'STARTTLS', 'security_code' => 'starttls'],
                'source' => 'platform',
                'version' => 1,
            ],
            'webmail_url' => 'https://app.example.com/mail',
            'support' => ['email' => 'ict@riverside.example', 'phone' => '+254 700 000 000', 'url' => null, 'hours' => 'Mon–Fri 8:00–17:00', 'setup_help_text' => 'Visit the ICT office in Block B for help.'],
            'verification' => ['incoming_verified_at' => null, 'outgoing_verified_at' => null, 'state' => 'not_verified'],
            'automation' => ['account_import' => false, 'apple_profile' => true, 'thunderbird_autoconfig' => true],
        ];
    }

    public static function plans(): array
    {
        $plan = static fn (string $id, string $name, string $audience, int $price, int $seat, bool $self = true, int $trial = 0) => [
            'id' => $id, 'name' => $name, 'audience' => $audience, 'description' => $name . ' plan', 'currency' => 'KES',
            'price_monthly_cents' => $price, 'price_per_mailbox_cents' => $seat, 'price_annual_cents' => $price * 10,
            'price_per_mailbox_annual_cents' => $seat * 10, 'annual_discount_percent' => 15, 'trial_days' => $trial,
            'self_service' => $self, 'is_public' => true,
            'limits' => ['daily_send_limit' => 200, 'monthly_send_limit' => 3000, 'max_domains' => 2, 'max_mailboxes' => $audience === 'institution' ? 5 : 0,
                'default_mailbox_quota_bytes' => 1073741824, 'max_mailbox_quota_bytes' => 2147483648, 'max_recipients_per_request' => 10,
                'api_requests_per_minute' => 60, 'max_members' => 3, 'max_api_keys' => 5, 'max_webhooks' => 2],
        ];
        return ['currency' => 'KES', 'tax' => ['rate_bps' => 1600, 'label' => 'VAT'], 'data' => [
            $plan('developer', 'Developer', 'individual', 0, 0),
            $plan('developer_plus', 'Starter', 'individual', 12000, 0),
            $plan('developer_scale', 'Standard', 'individual', 20000, 0),
            $plan('free', 'Institution Free', 'institution', 0, 0),
            $plan('pro', 'Premium', 'institution', 120000, 0, true, 14),
            $plan('enterprise', 'Enterprise', 'institution', 0, 0, false),
        ]];
    }
}
