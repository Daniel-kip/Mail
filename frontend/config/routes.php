<?php

declare(strict_types=1);

use App\Http\Controllers\Admin;
use App\Http\Controllers\Auth;
use App\Http\Controllers\Console;
use App\Http\Controllers\PublicSite;
use App\Http\Controllers\Setup;
use App\Http\Controllers\Webmail;
use App\Http\Router;

/*
 * Route table. Middleware aliases:
 *   guest            only when not signed in to the console (guest:mailbox for webmail)
 *   console          signed-in console user with an active organization (console:any: none needed)
 *   can:<permission> the member's role grants the permission
 *   institution      institutional account (hosted mailboxes)
 *   admin            platform operator
 *   mailbox          signed-in mailbox user (webmail and phone setup)
 */
return static function (Router $r): void {
    // ── public site ─────────────────────────────────────────────────────
    $r->get('/', [PublicSite\HomeController::class, 'index'], 'home');
    $r->get('/pricing', [PublicSite\HomeController::class, 'pricing'], 'pricing');
    $r->get('/developers', [PublicSite\HomeController::class, 'developers'], 'developers');
    $r->get('/security', [PublicSite\HomeController::class, 'security'], 'security');
    $r->get('/legal/privacy', [PublicSite\HomeController::class, 'privacy'], 'privacy');
    $r->get('/legal/terms', [PublicSite\HomeController::class, 'terms'], 'terms');
    $r->get('/healthz', [PublicSite\HomeController::class, 'health'], 'health');
    $r->get('/tools', [PublicSite\ToolsController::class, 'index'], 'tools');
    $r->get('/tools/{tool:slug}', [PublicSite\ToolsController::class, 'show'], 'tools.show');
    $r->post('/tools/headers', [PublicSite\ToolsController::class, 'headers'], 'tools.headers');
    $r->get('/docs', [PublicSite\DocsController::class, 'index'], 'docs');
    $r->get('/docs/{page:slug}', [PublicSite\DocsController::class, 'show'], 'docs.show');
    $r->get('/.well-known/security.txt', [PublicSite\HomeController::class, 'securityTxt'], 'security.txt');
    $r->get('/u/{token:token}', [PublicSite\UnsubscribeController::class, 'show'], 'unsubscribe');
    $r->post('/u/{token:token}', [PublicSite\UnsubscribeController::class, 'confirm']);

    // ── console authentication ──────────────────────────────────────────
    $r->group(['middleware' => ['guest']], static function (Router $r): void {
        $r->get('/login', [Auth\LoginController::class, 'show'], 'login');
        $r->post('/login', [Auth\LoginController::class, 'login']);
        $r->get('/register', [Auth\RegisterController::class, 'show'], 'register');
        $r->post('/register', [Auth\RegisterController::class, 'register']);
        $r->get('/forgot-password', [Auth\PasswordController::class, 'forgot'], 'password.forgot');
        $r->post('/forgot-password', [Auth\PasswordController::class, 'sendLink']);
    });
    $r->get('/auth/google', [Auth\GoogleController::class, 'start'], 'google');
    $r->get('/auth/google/callback', [Auth\GoogleController::class, 'callback'], 'google.callback');
    $r->get('/login/mfa', [Auth\LoginController::class, 'mfa'], 'login.mfa');
    $r->post('/login/mfa', [Auth\LoginController::class, 'verifyMfa']);
    $r->post('/logout', [Auth\LoginController::class, 'logout'], 'logout');
    $r->get('/reset-password', [Auth\PasswordController::class, 'reset'], 'password.reset');
    $r->post('/reset-password', [Auth\PasswordController::class, 'update']);
    $r->get('/verify-email', [Auth\VerificationController::class, 'show'], 'verify');
    $r->post('/verify-email', [Auth\VerificationController::class, 'verify']);
    $r->get('/invitations/{token:token}', [Auth\InvitationController::class, 'show'], 'invitation');
    $r->post('/invitations/{token:token}', [Auth\InvitationController::class, 'accept'], null, ['console:any']);

    // ── organization console ────────────────────────────────────────────
    $r->group(['prefix' => '/console', 'middleware' => ['console:any']], static function (Router $r): void {
        $r->get('/organizations/new', [Console\OrganizationController::class, 'create'], 'org.create');
        $r->post('/organizations', [Console\OrganizationController::class, 'store']);
        $r->post('/switch', [Console\OrganizationController::class, 'switch'], 'org.switch');
        $r->get('/account', [Console\AccountController::class, 'show'], 'account');
        $r->post('/account/profile', [Console\AccountController::class, 'profile']);
        $r->post('/account/password', [Console\AccountController::class, 'password']);
        $r->post('/account/email/resend', [Console\AccountController::class, 'resendVerification']);
        $r->post('/account/mfa/setup', [Console\AccountController::class, 'mfaSetup']);
        $r->post('/account/mfa/enable', [Console\AccountController::class, 'mfaEnable']);
        $r->post('/account/mfa/disable', [Console\AccountController::class, 'mfaDisable']);
        $r->post('/account/sessions/{id:uuid}/revoke', [Console\AccountController::class, 'revokeSession']);
        $r->get('/verify-phone', [Auth\PhoneController::class, 'show'], 'phone.verify');
        $r->post('/verify-phone', [Auth\PhoneController::class, 'verify']);
        $r->post('/verify-phone/send', [Auth\PhoneController::class, 'send'], 'phone.send');
    });

    $r->group(['prefix' => '/console', 'middleware' => ['console']], static function (Router $r): void {
        $r->get('/', [Console\DashboardController::class, 'index'], 'dashboard');
        $r->get('/developers', [Console\DashboardController::class, 'developers'], 'console.developers');

        $r->get('/domains', [Console\DomainController::class, 'index'], 'domains', ['can:domains:read']);
        $r->post('/domains', [Console\DomainController::class, 'store'], null, ['can:domains:write']);
        $r->get('/domains/{id:uuid}', [Console\DomainController::class, 'show'], 'domains.show', ['can:domains:read']);
        $r->post('/domains/{id:uuid}/verify', [Console\DomainController::class, 'verify'], null, ['can:domains:write']);
        $r->post('/domains/{id:uuid}/hosting', [Console\DomainController::class, 'hosting'], null, ['can:domains:write']);
        $r->post('/domains/{id:uuid}/delete', [Console\DomainController::class, 'destroy'], null, ['can:domains:write']);

        $r->get('/mailboxes/upgrade', [Console\MailboxController::class, 'upgrade'], 'mailboxes.upgrade');
        $r->group(['middleware' => ['institution']], static function (Router $r): void {
            $r->get('/mailboxes', [Console\MailboxController::class, 'index'], 'mailboxes', ['can:mailboxes:read']);
            $r->get('/mailboxes/new', [Console\MailboxController::class, 'create'], 'mailboxes.create', ['can:mailboxes:write']);
            $r->post('/mailboxes', [Console\MailboxController::class, 'store'], null, ['can:mailboxes:write']);
            $r->get('/mailboxes/import', [Console\MailboxController::class, 'importForm'], 'mailboxes.import', ['can:mailboxes:write']);
            $r->post('/mailboxes/import', [Console\MailboxController::class, 'import'], null, ['can:mailboxes:write']);
            $r->get('/mailboxes/onboarding', [Console\MailboxController::class, 'onboarding'], 'mailboxes.onboarding', ['can:mailboxes:read']);
            $r->get('/mailboxes/{id:uuid}', [Console\MailboxController::class, 'show'], 'mailboxes.show', ['can:mailboxes:read']);
            $r->post('/mailboxes/{id:uuid}', [Console\MailboxController::class, 'update'], null, ['can:mailboxes:write']);
            $r->get('/mailboxes/{id:uuid}/handout', [Console\MailboxController::class, 'handout'], 'mailboxes.handout', ['can:mailboxes:read']);
            $r->post('/mailboxes/{id:uuid}/status', [Console\MailboxController::class, 'status'], null, ['can:mailboxes:write']);
            $r->post('/mailboxes/{id:uuid}/password', [Console\MailboxController::class, 'resetPassword'], null, ['can:mailboxes:write']);
            $r->post('/mailboxes/{id:uuid}/activation', [Console\MailboxController::class, 'activation'], null, ['can:mailboxes:assist']);
            $r->post('/mailboxes/{id:uuid}/send-reset', [Console\MailboxController::class, 'sendReset'], null, ['can:mailboxes:assist']);
            $r->post('/mailboxes/{id:uuid}/unlock', [Console\MailboxController::class, 'unlock'], null, ['can:mailboxes:assist']);
            $r->post('/mailboxes/{id:uuid}/delete', [Console\MailboxController::class, 'destroy'], null, ['can:mailboxes:write']);
            $r->get('/aliases', [Console\AliasController::class, 'index'], 'aliases', ['can:mailboxes:read']);
            $r->post('/aliases', [Console\AliasController::class, 'store'], null, ['can:mailboxes:write']);
            $r->post('/aliases/{id:uuid}', [Console\AliasController::class, 'update'], null, ['can:mailboxes:write']);
            $r->post('/aliases/{id:uuid}/delete', [Console\AliasController::class, 'destroy'], null, ['can:mailboxes:write']);
        });

        $r->get('/api-keys', [Console\ApiKeyController::class, 'index'], 'keys', ['can:api_keys:manage']);
        $r->post('/api-keys', [Console\ApiKeyController::class, 'store'], null, ['can:api_keys:manage']);
        $r->post('/api-keys/{id:uuid}', [Console\ApiKeyController::class, 'update'], null, ['can:api_keys:manage']);
        $r->post('/api-keys/{id:uuid}/rotate', [Console\ApiKeyController::class, 'rotate'], null, ['can:api_keys:manage']);
        $r->post('/api-keys/{id:uuid}/revoke', [Console\ApiKeyController::class, 'revoke'], null, ['can:api_keys:manage']);

        $r->get('/templates', [Console\TemplateController::class, 'index'], 'templates', ['can:templates:read']);
        $r->get('/templates/new', [Console\TemplateController::class, 'create'], 'templates.create', ['can:templates:write']);
        $r->post('/templates', [Console\TemplateController::class, 'store'], null, ['can:templates:write']);
        $r->post('/templates/preview', [Console\TemplateController::class, 'preview'], 'templates.preview', ['can:templates:read']);
        $r->get('/templates/{id:uuid}', [Console\TemplateController::class, 'edit'], 'templates.edit', ['can:templates:read']);
        $r->post('/templates/{id:uuid}', [Console\TemplateController::class, 'saveVersion'], null, ['can:templates:write']);
        $r->post('/templates/{id:uuid}/activate', [Console\TemplateController::class, 'activate'], null, ['can:templates:write']);
        $r->post('/templates/{id:uuid}/status', [Console\TemplateController::class, 'status'], null, ['can:templates:write']);

        $r->get('/messages', [Console\MessageController::class, 'index'], 'messages', ['can:messages:read']);
        $r->get('/messages/compose', [Console\MessageController::class, 'compose'], 'messages.compose', ['can:messages:send']);
        $r->post('/messages', [Console\MessageController::class, 'send'], null, ['can:messages:send']);
        $r->get('/messages/export', [Console\MessageController::class, 'export'], 'messages.export', ['can:messages:read']);
        $r->get('/messages/{id:uuid}', [Console\MessageController::class, 'show'], 'messages.show', ['can:messages:read']);
        $r->post('/messages/{id:uuid}/cancel', [Console\MessageController::class, 'cancel'], null, ['can:messages:send']);

        $r->get('/suppressions', [Console\SuppressionController::class, 'index'], 'suppressions', ['can:suppressions:read']);
        $r->post('/suppressions', [Console\SuppressionController::class, 'store'], null, ['can:suppressions:write']);
        $r->post('/suppressions/delete', [Console\SuppressionController::class, 'destroy'], null, ['can:suppressions:write']);

        $r->get('/webhooks', [Console\WebhookController::class, 'index'], 'webhooks', ['can:webhooks:read']);
        $r->post('/webhooks', [Console\WebhookController::class, 'store'], null, ['can:webhooks:write']);
        $r->get('/webhooks/{id:uuid}', [Console\WebhookController::class, 'show'], 'webhooks.show', ['can:webhooks:read']);
        $r->post('/webhooks/{id:uuid}', [Console\WebhookController::class, 'update'], null, ['can:webhooks:write']);
        $r->post('/webhooks/{id:uuid}/test', [Console\WebhookController::class, 'test'], null, ['can:webhooks:write']);
        $r->post('/webhooks/{id:uuid}/rotate', [Console\WebhookController::class, 'rotate'], null, ['can:webhooks:write']);
        $r->post('/webhooks/{id:uuid}/delete', [Console\WebhookController::class, 'destroy'], null, ['can:webhooks:write']);
        $r->post('/webhooks/{id:uuid}/deliveries/{delivery:uuid}/retry', [Console\WebhookController::class, 'retry'], null, ['can:webhooks:write']);

        $r->get('/team', [Console\TeamController::class, 'index'], 'team', ['can:members:read']);
        $r->post('/team/invitations', [Console\TeamController::class, 'invite'], null, ['can:members:manage']);
        $r->post('/team/invitations/{id:uuid}/revoke', [Console\TeamController::class, 'revokeInvitation'], null, ['can:members:manage']);
        $r->post('/team/{user:uuid}/role', [Console\TeamController::class, 'role'], null, ['can:members:manage']);
        $r->post('/team/{user:uuid}/remove', [Console\TeamController::class, 'remove']);

        $r->get('/usage', [Console\DashboardController::class, 'usage'], 'usage', ['can:usage:read']);
        $r->get('/billing', [Console\BillingController::class, 'index'], 'billing', ['can:billing:read']);
        $r->get('/billing/plans', [Console\BillingController::class, 'plans'], 'billing.plans', ['can:billing:read']);
        $r->post('/billing/plan', [Console\BillingController::class, 'changePlan'], null, ['can:billing:manage']);
        $r->post('/billing/cancel', [Console\BillingController::class, 'cancel'], null, ['can:billing:manage']);
        $r->post('/billing/resume', [Console\BillingController::class, 'resume'], null, ['can:billing:manage']);
        $r->post('/billing/details', [Console\BillingController::class, 'details'], null, ['can:billing:manage']);
        $r->get('/billing/invoices/{id:uuid}', [Console\BillingController::class, 'invoice'], 'billing.invoice', ['can:billing:read']);
        $r->post('/billing/invoices/{id:uuid}/pay', [Console\BillingController::class, 'pay'], 'billing.pay', ['can:billing:manage']);
        $r->get('/billing/payments/{id:uuid}', [Console\BillingController::class, 'payment'], 'billing.payment', ['can:billing:read']);

        $r->get('/audit', [Console\AuditController::class, 'index'], 'audit', ['can:audit:read']);
        $r->get('/audit/export', [Console\AuditController::class, 'export'], 'audit.export', ['can:audit:read']);

        $r->get('/settings', [Console\SettingsController::class, 'index'], 'settings');
        $r->post('/settings/organization', [Console\SettingsController::class, 'organization'], null, ['can:organization:manage']);
        $r->post('/settings/support', [Console\SettingsController::class, 'support'], null, ['can:organization:manage']);
        $r->post('/settings/convert', [Console\SettingsController::class, 'convert'], null, ['can:organization:manage']);
        $r->post('/settings/leave', [Console\SettingsController::class, 'leave']);
    });

    // ── platform operator ───────────────────────────────────────────────
    $r->group(['prefix' => '/admin', 'middleware' => ['console:any', 'admin']], static function (Router $r): void {
        $r->get('/', [Admin\OverviewController::class, 'index'], 'admin');
        $r->get('/organizations', [Admin\OrganizationController::class, 'index'], 'admin.orgs');
        $r->get('/organizations/{id:uuid}', [Admin\OrganizationController::class, 'show'], 'admin.orgs.show');
        $r->post('/organizations/{id:uuid}/plan', [Admin\OrganizationController::class, 'plan']);
        $r->post('/organizations/{id:uuid}/suspend', [Admin\OrganizationController::class, 'suspend']);
        $r->post('/organizations/{id:uuid}/unsuspend', [Admin\OrganizationController::class, 'unsuspend']);
        $r->get('/users', [Admin\UserController::class, 'index'], 'admin.users');
        $r->post('/users/{id:uuid}/disable', [Admin\UserController::class, 'disable']);
        $r->post('/users/{id:uuid}/enable', [Admin\UserController::class, 'enable']);
        $r->get('/billing', [Admin\BillingController::class, 'index'], 'admin.billing');
        $r->get('/billing/invoices/{id:uuid}', [Admin\BillingController::class, 'invoice'], 'admin.invoice');
        $r->post('/billing/invoices/{id:uuid}/paid', [Admin\BillingController::class, 'markPaid']);
        $r->post('/billing/invoices/{id:uuid}/void', [Admin\BillingController::class, 'void']);
        $r->post('/billing/run', [Admin\BillingController::class, 'run']);
        $r->get('/plans', [Admin\BillingController::class, 'plans'], 'admin.plans');
        $r->post('/plans/{plan:slug}', [Admin\BillingController::class, 'updatePlan']);
        $r->get('/mail-clients', [Admin\MailClientController::class, 'index'], 'admin.mailclients');
        $r->post('/mail-clients/default', [Admin\MailClientController::class, 'saveDefault']);
        $r->post('/mail-clients/domains', [Admin\MailClientController::class, 'saveDomain']);
        $r->post('/mail-clients/domains/{id:uuid}/delete', [Admin\MailClientController::class, 'deleteDomain']);
        $r->get('/onboarding', [Admin\MailClientController::class, 'onboarding'], 'admin.onboarding');
        $r->get('/security', [Admin\OverviewController::class, 'security'], 'admin.security');
        $r->get('/infrastructure', [Admin\InfrastructureController::class, 'show'], 'admin.infrastructure');
        $r->get('/infrastructure/download', [Admin\InfrastructureController::class, 'download'], 'admin.infrastructure.download');
        $r->get('/infrastructure/preview/{name:slug}', [Admin\InfrastructureController::class, 'preview'], 'admin.infrastructure.preview');
        $r->get('/docs', [Admin\DocsController::class, 'index'], 'admin.docs');
        $r->get('/docs/{page:slug}', [Admin\DocsController::class, 'show'], 'admin.docs.show');
        $r->get('/queue', [Admin\OverviewController::class, 'queue'], 'admin.queue');
        $r->post('/queue/{id:int}/requeue', [Admin\OverviewController::class, 'requeue']);
    });

    // ── webmail ─────────────────────────────────────────────────────────
    $r->group(['prefix' => '/mail', 'middleware' => ['guest:mailbox']], static function (Router $r): void {
        $r->get('/login', [Webmail\AuthController::class, 'show'], 'mail.login');
        $r->post('/login', [Webmail\AuthController::class, 'login']);
    });
    $r->get('/mail/forgot-password', [Webmail\AuthController::class, 'forgot'], 'mail.forgot');
    $r->post('/mail/forgot-password', [Webmail\AuthController::class, 'sendReset']);
    $r->get('/mail/reset-password', [Webmail\AuthController::class, 'resetForm'], 'mail.reset');
    $r->post('/mail/reset-password', [Webmail\AuthController::class, 'reset']);
    $r->get('/mail/activate', [Webmail\AuthController::class, 'activateForm'], 'mail.activate');
    $r->post('/mail/activate', [Webmail\AuthController::class, 'activate']);
    $r->post('/mail/logout', [Webmail\AuthController::class, 'logout'], 'mail.logout');

    $r->group(['prefix' => '/mail', 'middleware' => ['mailbox']], static function (Router $r): void {
        $r->get('/', [Webmail\MailController::class, 'home'], 'mail');
        $r->get('/folder/{folder:any}/message/{uid:int}', [Webmail\MailController::class, 'read'], 'mail.read');
        $r->get('/folder/{folder:any}/message/{uid:int}/attachment/{index:int}', [Webmail\MailController::class, 'attachment'], 'mail.attachment');
        $r->get('/folder/{folder:any}/message/{uid:int}/body', [Webmail\MailController::class, 'body'], 'mail.body');
        $r->get('/folder/{folder:any}', [Webmail\MailController::class, 'folder'], 'mail.folder');
        $r->post('/actions', [Webmail\MailController::class, 'action'], 'mail.action');
        $r->get('/compose', [Webmail\ComposeController::class, 'show'], 'mail.compose');
        $r->post('/compose', [Webmail\ComposeController::class, 'submit']);
        $r->get('/suggest', [Webmail\ComposeController::class, 'suggest'], 'mail.suggest');
        $r->get('/contacts', [Webmail\ContactController::class, 'index'], 'mail.contacts');
        $r->post('/contacts', [Webmail\ContactController::class, 'store']);
        $r->post('/contacts/{id:uuid}', [Webmail\ContactController::class, 'update']);
        $r->post('/contacts/{id:uuid}/delete', [Webmail\ContactController::class, 'destroy']);
        $r->get('/directory', [Webmail\ContactController::class, 'directory'], 'mail.directory');
        $r->get('/settings', [Webmail\SettingsController::class, 'show'], 'mail.settings');
        $r->post('/settings/password', [Webmail\SettingsController::class, 'password']);
        $r->post('/settings/preferences', [Webmail\SettingsController::class, 'preferences']);
        $r->post('/settings/auto-reply', [Webmail\SettingsController::class, 'autoReply']);

        // "Connect my work email to my phone"
        $r->get('/setup', [Setup\SetupController::class, 'welcome'], 'setup');
        $r->get('/setup/device', [Setup\SetupController::class, 'device'], 'setup.device');
        $r->get('/setup/app', [Setup\SetupController::class, 'app'], 'setup.app');
        $r->get('/setup/check', [Setup\SetupController::class, 'check'], 'setup.check');
        $r->get('/setup/guide/{guide:slug}', [Setup\SetupController::class, 'guide'], 'setup.guide');
        $r->get('/setup/done/{guide:slug}', [Setup\SetupController::class, 'done'], 'setup.done');
        $r->get('/setup/verify', [Setup\VerifyController::class, 'show'], 'setup.verify');
        $r->post('/setup/verify/incoming', [Setup\VerifyController::class, 'startIncoming']);
        $r->post('/setup/verify/incoming/confirm', [Setup\VerifyController::class, 'confirmIncoming']);
        $r->post('/setup/verify/outgoing', [Setup\VerifyController::class, 'startOutgoing']);
        $r->post('/setup/verify/outgoing/check', [Setup\VerifyController::class, 'checkOutgoing']);
        $r->get('/setup/settings', [Setup\SetupController::class, 'settings'], 'setup.settings');
        $r->get('/setup/help', [Setup\SetupController::class, 'help'], 'setup.help');
        $r->get('/setup/help/{topic:slug}', [Setup\SetupController::class, 'helpTopic'], 'setup.help.topic');
        $r->get('/setup/profile.mobileconfig', [Setup\SetupController::class, 'profile'], 'setup.profile');
        $r->get('/setup/qr.svg', [Setup\SetupController::class, 'qr'], 'setup.qr');
        $r->post('/setup/event', [Setup\SetupController::class, 'event'], 'setup.event');
    });
};
