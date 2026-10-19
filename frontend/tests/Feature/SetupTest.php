<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class SetupTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
        $this->signInMailbox();
        $this->api->on('GET', '/v1/webmail/setup', 200, Fixtures::setup())
            ->on('POST', '/v1/webmail/setup/events', 202, ['ok' => true])
            ->on('GET', '/v1/webmail/setup/readiness', 200, [
                'checked_at' => '2026-10-13T08:00:00Z', 'overall' => 'ready',
                'mailbox' => ['status' => 'ready', 'message' => 'Your mailbox is active.'], 'domain' => ['status' => 'ready', 'message' => 'Domain verified.'],
                'imap' => ['status' => 'ready', 'code' => 'ok', 'host' => 'mail.riverside.example', 'port' => 993, 'message' => 'Incoming mail server answered securely.'],
                'smtp' => ['status' => 'ready', 'code' => 'ok', 'host' => 'mail.riverside.example', 'port' => 465, 'message' => 'Outgoing mail server answered securely.'],
                'authentication' => ['status' => 'ready', 'message' => 'x'], 'quota' => ['status' => 'nearly_full', 'message' => 'Your mailbox is almost full.'],
                'phone_connection_verified' => false, 'note' => 'These checks test the mail service from the platform side.',
            ]);
    }

    public function testWizardStagesAThroughE(): void
    {
        $this->assertContains('amina@riverside.example', $this->get('/mail/setup')->body());
        $device = $this->get('/mail/setup/device', ['user-agent' => 'Mozilla/5.0 (Linux; Android 14)'])->body();
        $this->assertContains('This device', $device);
        $this->assertRedirect('/mail/setup/device', $this->get('/mail/setup/app?device=toaster'));
        $apps = $this->get('/mail/setup/app?device=ios')->body();
        $this->assertContains('Apple Mail', $apps);
        $check = $this->get('/mail/setup/check?device=android&app=gmail')->body();
        $this->assertContains('Your mailbox is almost full.', $check);
        $this->assertContains('/mail/setup/guide/android-gmail', $check);
        $events = array_map(static fn ($c) => $c['options']['json']['event'], $this->api->callsTo('POST', '/v1/webmail/setup/events'));
        $this->assertSame(['guide_started', 'device_selected', 'client_selected'], $events);
    }

    public function testGuideStepsUseValidatedSettingsAndNeverAskForPasswords(): void
    {
        $catalog = require APP_ROOT . '/resources/guides/mail-clients.php';
        foreach ($catalog['guides'] as $id => $guide) {
            foreach (array_keys($guide['steps']) as $i) {
                $page = $this->get('/mail/setup/guide/' . $id . '?step=' . ($i + 1));
                $this->assertStatus(200, $page, $id . ' step ' . ($i + 1));
                $body = $page->body();
                $this->assertNotContains('{imap_host}', $body, 'placeholders are filled');
                $this->assertNotContains('{address}', $body);
                $this->assertNotContains('accept the certificate', strtolower($body));
            }
        }
        $page = $this->get('/mail/setup/guide/android-gmail?step=7')->body();
        $this->assertContains('mail.riverside.example', $page);
        $ios = $this->get('/mail/setup/guide/ios-apple_mail?step=1')->body();
        $this->assertContains('/mail/setup/profile.mobileconfig', $ios);
        $this->assertContains('does not contain your password', $ios);
        $this->assertStatus(404, $this->get('/mail/setup/guide/no-such-guide'));
    }

    public function testDoneRecordsCompletionAndOpensVerification(): void
    {
        $this->assertRedirect('/mail/setup/verify?guide=android-gmail', $this->get('/mail/setup/done/android-gmail'));
        $page = $this->get('/mail/setup/verify?guide=android-gmail')->body();
        $this->assertContains('Send me a code', $page);
        $this->assertContains('Finish the receiving test first', $page);
    }

    public function testVerificationFlow(): void
    {
        $this->api->on('POST', '/v1/webmail/setup/verify/incoming', 201, ['id' => 'x', 'direction' => 'incoming', 'expires_at' => '2026-10-13T08:30:00Z', 'subject_hint' => 'Your Mailvora connection code']);
        $this->assertRedirect('/mail/setup/verify?guide=android-gmail', $this->post('/mail/setup/verify/incoming', ['guide' => 'android-gmail', 'client' => 'gmail']));
        $this->assertSame('gmail', $this->api->callsTo('POST', '/v1/webmail/setup/verify/incoming')[0]['options']['json']['client']);
        $this->assertContains('Your Mailvora connection code', $this->get('/mail/setup/verify?guide=android-gmail')->body());

        $this->api->on('POST', '/v1/webmail/setup/verify/incoming/confirm', 409, ['error' => ['code' => 'wrong_code', 'message' => 'That code does not match.']]);
        $this->post('/mail/setup/verify/incoming/confirm', ['code' => 'AAAA', 'guide' => 'android-gmail']);
        $this->assertSame('That code does not match.', $this->flashed('errors')['code'] ?? null);

        $this->api->on('POST', '/v1/webmail/setup/verify/outgoing', 201, ['id' => 'y', 'code' => 'K7Q2', 'send_to' => 'amina@riverside.example', 'subject' => 'Setup check K7Q2', 'expires_at' => '2026-10-13T08:30:00Z']);
        $this->post('/mail/setup/verify/outgoing', ['guide' => 'android-gmail']);
        $this->assertContains('Setup check K7Q2', $this->get('/mail/setup/verify?guide=android-gmail')->body());
        $this->api->on('POST', '/v1/webmail/setup/verify/outgoing/check', 200, ['verified' => true, 'direction' => 'outgoing', 'message' => 'ok']);
        $json = $this->post('/mail/setup/verify/outgoing/check', [], ['accept' => 'application/json']);
        $this->assertSame(['verified' => true, 'message' => 'ok'], json_decode($json->body(), true));
    }

    public function testHelpSettingsProfileAndQr(): void
    {
        $this->assertContains('The app says my password is wrong', $this->get('/mail/setup/help')->body());
        $topic = $this->get('/mail/setup/help/wrong-password')->body();
        $this->assertContains('amina@riverside.example', $topic);
        $this->assertContains('IT staff will never ask for your password.', $topic);
        $settings = $this->get('/mail/setup/settings')->body();
        $this->assertContains('587 (STARTTLS)', $settings);
        $this->assertContains('We never show it here', $settings);
        $this->api->on('GET', '/v1/webmail/setup/apple-profile', 200, [], ['content-disposition' => ['attachment; filename="riverside.mobileconfig"']]);
        $profile = $this->get('/mail/setup/profile.mobileconfig');
        $this->assertSame('application/x-apple-aspen-config', $profile->headers()['Content-Type'] ?? null);
        $this->assertSame('no', $profile->headers()['X-Profile-Signed'] ?? null);
        $qr = $this->get('/mail/setup/qr.svg');
        $this->assertContains('<svg', $qr->body());
    }
}
