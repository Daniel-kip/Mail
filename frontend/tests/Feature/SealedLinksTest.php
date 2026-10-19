<?php

declare(strict_types=1);

namespace Tests\Feature;

use App\Http\Response;
use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class SealedLinksTest extends TestCase
{
    private const OTHER_ID = '0192a000-0000-7000-8000-0000000000ff';

    public function setUp(): void
    {
        $this->boot(['sealed_links' => true]);
        $this->signInConsole();
        $this->api->on('GET', '/v1/domains', 200, ['data' => [[
            'id' => Fixtures::DOMAIN_ID, 'domain' => 'riverside.example', 'status' => 'verified', 'mail_hosting_enabled' => true,
            'mailbox_count' => 3, 'last_checked_at' => '2026-10-13T08:00:00Z', 'checks' => ['spf' => true, 'dkim' => true, 'dmarc' => true, 'mx' => true],
        ]]]);
    }

    public function testMenuAddressRedirectsToItsSealedForm(): void
    {
        $res = $this->get('/console/domains?sort=name');
        $this->assertStatus(303, $res);
        $location = (string) $res->header('Location');
        $this->assertTrue(str_starts_with($location, '/app?ref='), "sealed redirect, got {$location}");
        $this->assertNotContains('domains', $location);
        $this->assertSame('/console/domains?sort=name', $this->app->links()->open(substr($location, 9)));
    }

    public function testSealedPagesContainNoReadablePrivateAddresses(): void
    {
        $page = $this->follow($this->get('/console/domains'));
        $this->assertStatus(200, $page);
        $html = $page->body();
        $this->assertContains('riverside.example', $html);
        $this->assertTrue(preg_match('~(href|action|src)="/(console|admin|mail)\b~', $html) === 0, 'no private address may remain in links or forms');
        $this->assertContains('href="/app?ref=', $html);
        $this->assertContains('action="/app?ref=', $html, 'POST forms are sealed');
        // Public addresses stay readable.
        $this->assertContains('href="/tools"', $html);
    }

    public function testRecordAddressesCannotBeTypedOrEdited(): void
    {
        $typed = $this->get('/console/domains/' . self::OTHER_ID);
        $this->assertStatus(303, $typed);
        $this->assertTrue(str_starts_with((string) $typed->header('Location'), '/app?ref='));
        $this->assertContains('only be opened from inside the app', (string) $this->flashed('warning'));

        $sealed = $this->app->links()->seal('/console/domains/' . Fixtures::DOMAIN_ID);
        $ref = substr($sealed, 9);
        $tampered = substr($ref, 0, -2) . (str_ends_with($ref, 'A') ? 'BB' : 'AA');
        $res = $this->get('/app?ref=' . $tampered);
        $this->assertStatus(303, $res);
        $this->assertContains('expired or was changed', (string) $this->flashed('warning'));

        $this->assertStatus(404, $this->post('/console/domains', ['domain' => 'evil.example']));
    }

    public function testLinksDieWithTheSessionAndCannotBeShared(): void
    {
        $sealed = $this->app->links()->seal('/console/domains');
        $this->session->destroy();
        $this->session->save();
        $this->app->links()->ensureKey();
        $res = $this->get($sealed);
        $this->assertStatus(303, $res);
        $this->assertContains('expired or was changed', (string) $this->flashed('warning'));
    }

    public function testSealedFormsAndFiltersWork(): void
    {
        $this->api->on('POST', '/v1/domains', 201, ['id' => Fixtures::DOMAIN_ID]);
        $form = $this->app->links()->seal('/console/domains');
        $res = $this->post($form, ['domain' => 'new.example']);
        $this->assertStatus(303, $res);
        $this->assertTrue(str_starts_with((string) $res->header('Location'), '/app?ref='), 'redirects after a POST are sealed too');

        // A GET form carries the token in a hidden field and adds its own fields;
        // sealed parameters cannot be overridden.
        $html = $this->app->links()->rewriteHtml('<form method="get" action="/console/messages?status=failed"><input name="q"></form>');
        $this->assertContains('action="/app"', $html);
        preg_match('~name="ref" value="([^"]+)"~', $html, $m);
        $this->assertSame('/console/messages?status=failed', $this->app->links()->open($m[1]));
    }

    public function testPublicAndEmailLinkPagesKeepReadableAddresses(): void
    {
        foreach (['/', '/pricing', '/tools/domain', '/login', '/register', '/mail/login', '/mail/activate', '/reset-password', '/.well-known/security.txt'] as $path) {
            $this->assertFalse($this->app->links()->isPrivate($path), "{$path} stays public");
        }
        foreach (['/console', '/console/domains', '/admin/security', '/mail', '/mail/compose', '/mail/setup'] as $path) {
            $this->assertTrue($this->app->links()->isPrivate($path), "{$path} is sealed");
        }
    }

    private function follow(Response $response): Response
    {
        for ($i = 0; $i < 5 && $response->status() >= 300 && $response->status() < 400; $i++) {
            $response = $this->get((string) $response->header('Location'));
        }
        return $response;
    }
}
