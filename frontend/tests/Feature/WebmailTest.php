<?php

declare(strict_types=1);

namespace Tests\Feature;

use App\Api\ApiResponse;
use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class WebmailTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
    }

    private function stubMail(): void
    {
        $this->signInMailbox();
        $this->api
            ->on('GET', '/v1/webmail/preferences', 200, ['signature' => "Amina\nFinance", 'messages_per_page' => 50, 'auto_reply' => ['enabled' => false, 'active_now' => false, 'subject' => null, 'body' => null, 'starts_on' => null, 'ends_on' => null, 'synced_at' => null, 'sync_error' => null, 'server_supported' => true]])
            ->on('GET', '/v1/webmail/folders/INBOX/messages', 200, ['data' => [
                ['uid' => 7, 'from' => 'Principal', 'from_address' => 'head@riverside.example', 'to' => ['amina@riverside.example'], 'subject' => 'Staff meeting', 'date' => '2026-10-13T07:00:00Z', 'size' => 1200, 'seen' => false, 'flagged' => true, 'answered' => false, 'has_attachments' => true],
                ['uid' => 6, 'from' => null, 'from_address' => 'x@example.com', 'to' => [], 'subject' => '', 'date' => null, 'size' => 10, 'seen' => true, 'flagged' => false, 'answered' => true, 'has_attachments' => false],
            ], 'page' => 1, 'per_page' => 50, 'total' => 2])
            ->on('GET', '/v1/webmail/folders/INBOX/messages/7', 200, ['uid' => 7, 'folder' => 'INBOX', 'subject' => 'Staff meeting', 'from' => [['name' => 'Principal', 'email' => 'head@riverside.example']],
                'to' => [['name' => 'Amina', 'email' => 'amina@riverside.example']], 'cc' => [['name' => '', 'email' => 'deputy@riverside.example']], 'reply_to' => [], 'date' => '2026-10-13T07:00:00Z',
                'message_id' => 'abc@riverside.example', 'references' => null, 'html_document' => '<!doctype html><html><head><meta charset="utf-8"></head><body><p>Agenda</p><script>bad()</script></body></html>',
                'text' => "Agenda\nItem 1", 'has_remote_content' => true, 'remote_content_loaded' => false,
                'attachments' => [['index' => 0, 'filename' => 'agenda.pdf', 'content_type' => 'application/pdf', 'size' => 5000]], 'authentication_results' => null, 'spam_flag' => null]);
    }

    public function testLoginAndRedirectToInbox(): void
    {
        $this->assertContains('Work email address', $this->get('/mail/login')->body());
        $this->api->onCallback('POST', '/v1/webmail/login', function () {
            $this->api->signIn('webmail');
            return ApiResponse::fake(200, Fixtures::mailboxSession());
        });
        $this->assertRedirect('/mail', $this->post('/mail/login', ['address' => 'Amina@Riverside.example', 'password' => 'pw-secret-1']));
        $this->assertSame('amina@riverside.example', $this->api->callsTo('POST', '/v1/webmail/login')[0]['options']['json']['address']);
    }

    public function testUnauthenticatedWebmailRedirectsToLogin(): void
    {
        $this->assertRedirect('/mail/login?next=%2Fmail%2Fcontacts', $this->get('/mail/contacts'));
    }

    public function testFolderAndReadingPane(): void
    {
        $this->stubMail();
        $this->assertRedirect('/mail/folder/INBOX', $this->get('/mail'));
        $list = $this->get('/mail/folder/INBOX')->body();
        $this->assertContains('Staff meeting', $list);
        $this->assertContains('(no subject)', $list);
        $page = $this->get('/mail/folder/INBOX/message/7')->body();
        $this->assertContains('sandbox="allow-popups allow-popups-to-escape-sandbox"', $page);
        $this->assertContains('/mail/folder/INBOX/message/7/body', $page);
        $this->assertContains('Show images', $page);
        $this->assertContains('agenda.pdf', $page);
        $this->assertNotContains('bad()', $page, 'the HTML body is never inlined into the app page');
    }

    public function testMessageBodyIsServedWithAStrictPolicy(): void
    {
        $this->stubMail();
        $res = $this->get('/mail/folder/INBOX/message/7/body');
        $csp = (string) ($res->headers()['Content-Security-Policy'] ?? '');
        $this->assertContains("default-src 'none'", $csp);
        $this->assertContains("frame-ancestors 'self'", $csp);
        $this->assertNotContains('script-src', $csp);
        $this->assertContains('<base target="_blank">', $res->body());
        $remote = $this->get('/mail/folder/INBOX/message/7/body?remote=1');
        $this->assertContains('img-src data: https:', (string) $remote->headers()['Content-Security-Policy']);
    }

    public function testBulkActionsMapToTheApi(): void
    {
        $this->stubMail();
        $this->api->on('POST', '/v1/webmail/folders/INBOX/move', 200, ['ok' => true])->on('POST', '/v1/webmail/folders/INBOX/flags', 200, ['ok' => true]);
        $this->assertRedirect('/mail/folder/INBOX', $this->post('/mail/actions', ['folder' => 'INBOX', 'action' => 'archive', 'uids' => ['7', '6', 'x']]));
        $this->assertSame(['uids' => [7, 6], 'destination' => 'Archive'], $this->api->callsTo('POST', '/v1/webmail/folders/INBOX/move')[0]['options']['json']);
        $this->post('/mail/actions', ['folder' => 'INBOX', 'action' => 'unread', 'uids' => ['7'], 'back' => '/mail/folder/INBOX/message/7']);
        $this->assertSame(false, $this->api->callsTo('POST', '/v1/webmail/folders/INBOX/flags')[0]['options']['json']['seen']);
    }

    public function testReplyAllPrefillsRecipientsAndThreadHeaders(): void
    {
        $this->stubMail();
        $page = $this->get('/mail/compose?mode=reply-all&folder=INBOX&uid=7')->body();
        $this->assertContains('Re: Staff meeting', $page);
        $this->assertContains('Principal &lt;head@riverside.example&gt;', $page);
        $this->assertContains('deputy@riverside.example', $page);
        $this->assertContains('&lt;abc@riverside.example&gt;', $page);
        $this->assertContains('&gt; Agenda', $page);
        $this->assertContains("-- \nAmina", $page, 'signature is appended');
    }

    public function testSendBuildsMultipartPayload(): void
    {
        $this->stubMail();
        $this->api->on('POST', '/v1/webmail/send', 200, ['ok' => true]);
        $this->assertRedirect('/mail/compose', $this->post('/mail/compose', ['to' => 'not-an-email', 'subject' => 'x', 'intent' => 'send']));
        $res = $this->post('/mail/compose', ['to' => 'Head <head@riverside.example>; b@example.com', 'subject' => 'Hello', 'text' => 'Body', 'intent' => 'send']);
        $this->assertRedirect('/mail', $res);
        $payload = json_decode($this->api->callsTo('POST', '/v1/webmail/send')[0]['options']['multipart']['fields']['payload'], true);
        $this->assertSame(['Head <head@riverside.example>', 'b@example.com'], $payload['to']);
        $this->assertSame('Hello', $payload['subject']);
    }

    public function testActivationSignsInAndContinuesToPhoneSetup(): void
    {
        $token = str_repeat('t', 40);
        $this->api->on('GET', '/v1/webmail/activation', 200, ['purpose' => 'activate', 'address' => 'jane@riverside.example', 'display_name' => 'Jane', 'organization' => 'Riverside Academy', 'expires_at' => '2026-10-20T00:00:00Z']);
        $this->assertContains('jane@riverside.example', $this->get('/mail/activate?token=' . $token)->body());
        $this->api->on('POST', '/v1/webmail/activate', 200, ['ok' => true, 'address' => 'jane@riverside.example']);
        $this->api->on('POST', '/v1/webmail/login', 200, Fixtures::mailboxSession());
        $this->assertRedirect('/mail/activate?token=' . $token, $this->post('/mail/activate', ['token' => $token, 'password' => 'short', 'password_confirmation' => 'short']));
        $this->assertRedirect('/mail/setup', $this->post('/mail/activate', ['token' => $token, 'password' => 'a long new password', 'password_confirmation' => 'a long new password']));
    }

    public function testSettingsContactsAndDirectoryRender(): void
    {
        $this->stubMail();
        $this->api->on('GET', '/v1/webmail/contacts', 200, ['data' => [['id' => Fixtures::DOMAIN_ID, 'name' => 'Peter', 'email' => 'peter@example.com', 'phone' => null, 'organization' => null, 'notes' => null, 'created_at' => 'x', 'updated_at' => 'x']]])
            ->on('GET', '/v1/webmail/directory', 200, ['people' => [['address' => 'head@riverside.example', 'name' => 'Principal', 'department' => 'Admin', 'job_title' => 'Head']], 'groups' => [['address' => 'info@riverside.example']]]);
        $this->assertContains('Out-of-office reply', $this->get('/mail/settings')->body());
        $this->assertContains('peter@example.com', $this->get('/mail/contacts')->body());
        $dir = $this->get('/mail/directory')->body();
        $this->assertContains('Principal', $dir);
        $this->assertContains('info@riverside.example', $dir);
        $this->api->on('PUT', '/v1/webmail/preferences', 200, ['auto_reply' => ['sync_error' => null]]);
        $this->assertRedirect('/mail/settings#auto-reply', $this->post('/mail/settings/auto-reply', ['auto_reply_enabled' => '1']));
        $this->assertSame('Message is required.', $this->flashed('errors')['auto_reply_body'] ?? null);
    }
}
