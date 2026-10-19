<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class PublicPagesTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
        $this->api->on('GET', '/v1/plans', 200, Fixtures::plans());
    }

    public function testHomePageRendersBothAudiencesAndLivePrices(): void
    {
        $res = $this->get('/');
        $this->assertStatus(200, $res);
        $this->assertContains('For individual developers', $res->body());
        $this->assertContains('For institutions', $res->body());
        $this->assertContains('Premium', $res->body());
        $this->assertContains('KES 1,200', $res->body());
        $this->assertNotContains('USD', $res->body());
    }

    public function testPricingSwitchesAudience(): void
    {
        $institutions = $this->get('/pricing');
        $this->assertContains('Premium', $institutions->body());
        $this->assertNotContains('plan=developer_plus', $institutions->body());
        $developers = $this->get('/pricing?for=developers');
        $this->assertContains('Standard', $developers->body());
        $this->assertContains('excluding VAT (16%)', $developers->body());
        $this->assertContains('Contact us', $institutions->body(), 'non-self-service plans are not sold online');
    }

    public function testPricingDegradesGracefullyWithoutTheApi(): void
    {
        $this->boot();
        $res = $this->get('/pricing');
        $this->assertStatus(200, $res);
        $this->assertContains('Prices are temporarily unavailable', $res->body());
    }

    public function testStaticPagesRender(): void
    {
        foreach (['/developers' => 'API reference', '/security' => 'Tenant isolation', '/legal/privacy' => 'Privacy notice', '/legal/terms' => 'Terms of service'] as $path => $text) {
            $res = $this->get($path);
            $this->assertStatus(200, $res);
            $this->assertContains($text, $res->body(), $path);
        }
    }

    public function testSecurityHeadersAreSentOnEveryPage(): void
    {
        $res = $this->get('/');
        $this->assertContains("script-src 'self'", (string) $res->header('Content-Security-Policy'));
        $this->assertSame('DENY', $res->header('X-Frame-Options'));
        $this->assertSame('nosniff', $res->header('X-Content-Type-Options'));
        $this->assertNotContains('<script>', $res->body(), 'no inline scripts');
    }

    public function testUnknownPagesAre404WithFriendlyPage(): void
    {
        $res = $this->get('/no/such/page');
        $this->assertStatus(404, $res);
        $this->assertContains('Page not found', $res->body());
    }

    public function testUnsubscribeGetOnlyShowsConfirmation(): void
    {
        $token = str_repeat('A', 43);
        $this->api->on('GET', '/v1/unsubscribe/' . $token, 200, ['organization' => 'Club', 'sender' => 'news@club.example', 'email' => 'r*****@example.org', 'unsubscribed' => false]);
        $res = $this->get('/u/' . $token);
        $this->assertStatus(200, $res);
        $this->assertContains('Unsubscribe from Club?', $res->body());
        $this->assertSame([], $this->api->callsTo('POST', '/v1/unsubscribe/' . $token), 'GET must not unsubscribe (link scanners)');
    }

    public function testOneClickUnsubscribeWorksWithoutSessionOrCsrf(): void
    {
        $token = str_repeat('B', 43);
        $this->api->on('POST', '/v1/unsubscribe/' . $token, 200, ['ok' => true, 'organization' => 'Club', 'email' => 'r***@example.org']);
        $res = $this->call('POST', '/u/' . $token, ['_token' => ''], ['content-type' => 'application/x-www-form-urlencoded'], 'List-Unsubscribe=One-Click');
        $this->assertStatus(200, $res);
        $this->assertSame('Unsubscribed', $res->body());
    }

    public function testFormsWithoutCsrfTokenAreRejected(): void
    {
        $res = $this->call('POST', '/login', ['_token' => 'wrong', 'email' => 'a@b.c', 'password' => 'x']);
        $this->assertStatus(419, $res);
    }
}
