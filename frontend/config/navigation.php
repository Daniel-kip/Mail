<?php

declare(strict_types=1);

/*
 * Sidebar navigation. An item is shown when the member's role grants
 * `can` (if set) and the account type matches `institution` (if set).
 */
return [
    'console' => [
        ['label' => null, 'items' => [
            ['Overview', '/console', 'home', 'exact' => true],
        ]],
        ['label' => 'Sending', 'items' => [
            ['Messages', '/console/messages', 'send', 'can' => 'messages:read'],
            ['Templates', '/console/templates', 'file-text', 'can' => 'templates:read'],
            ['Domains', '/console/domains', 'globe', 'can' => 'domains:read'],
            ['Suppressions', '/console/suppressions', 'ban', 'can' => 'suppressions:read'],
            ['Deliverability tools', '/tools', 'search'],
        ]],
        ['label' => 'Work email', 'institution' => true, 'items' => [
            ['Mailboxes', '/console/mailboxes', 'inbox', 'can' => 'mailboxes:read'],
            ['Shared addresses', '/console/aliases', 'users', 'can' => 'mailboxes:read'],
            ['Mail-app onboarding', '/console/mailboxes/onboarding', 'smartphone-check', 'can' => 'mailboxes:read'],
        ]],
        ['label' => 'Developers', 'items' => [
            ['API keys', '/console/api-keys', 'key', 'can' => 'api_keys:manage'],
            ['Webhooks', '/console/webhooks', 'webhook', 'can' => 'webhooks:read'],
            ['API quickstart', '/console/developers', 'code'],
        ]],
        ['label' => 'Organization', 'items' => [
            ['Team', '/console/team', 'user-plus', 'can' => 'members:read'],
            ['Usage', '/console/usage', 'chart', 'can' => 'usage:read'],
            ['Billing', '/console/billing', 'credit-card', 'can' => 'billing:read'],
            ['Audit log', '/console/audit', 'activity', 'can' => 'audit:read'],
            ['Settings', '/console/settings', 'settings'],
        ]],
    ],
    'admin' => [
        ['label' => 'Platform', 'items' => [
            ['Overview', '/admin', 'activity', 'exact' => true],
            ['Organizations', '/admin/organizations', 'building'],
            ['Users', '/admin/users', 'users'],
            ['Delivery queue', '/admin/queue', 'layers'],
            ['Security', '/admin/security', 'shield-check'],
            ['Documentation', '/admin/docs', 'book'],
            ['System infrastructure', '/admin/infrastructure', 'file-text'],
        ]],
        ['label' => 'Commercial', 'items' => [
            ['Invoices', '/admin/billing', 'receipt'],
            ['Plans & pricing', '/admin/plans', 'dollar'],
        ]],
        ['label' => 'Mail apps', 'items' => [
            ['Client settings', '/admin/mail-clients', 'server'],
            ['Onboarding analytics', '/admin/onboarding', 'smartphone-check'],
        ]],
    ],
];
