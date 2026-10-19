<?php

declare(strict_types=1);

namespace Tests\Feature;

use App\View\Markdown;
use Tests\Support\TestCase;

final class DocsTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
    }

    public function testPublicDocsListOnlyCustomerGuides(): void
    {
        $res = $this->get('/docs');
        $this->assertStatus(200, $res);
        foreach (['Administrator guide', 'Sending API', 'Billing and payments', 'Phone setup guide', 'Troubleshooting'] as $guide) {
            $this->assertContains($guide, $res->body());
        }
        foreach (['Operations', 'VPS deployment', 'Security readiness', 'Decision log'] as $internal) {
            $this->assertNotContains('>' . $internal . '<', $res->body(), "{$internal} is for operators only");
        }
    }

    public function testPublicPageRendersMarkdown(): void
    {
        $res = $this->get('/docs/sending-api');
        $this->assertStatus(200, $res);
        $html = $res->body();
        $this->assertContains('<h1>Sending API</h1>', $html);
        $this->assertContains('<table class="table">', $html);
        $this->assertContains('<code>Mailvora-Signature</code>', $html);
        $this->assertContains('<pre class="doc-code" data-lang="bash">', $html);
        $this->assertContains('href="/docs/administrator-guide"', $html, 'wiki links point at rendered pages');
        $this->assertContains('<span class="doc-ref">Security model</span>', $html, 'operator pages are not linked for the public');
    }

    public function testOperatorPagesAndUnknownPagesAreHidden(): void
    {
        $this->assertStatus(404, $this->get('/docs/operations'));
        $this->assertStatus(404, $this->get('/docs/home'));
        $this->assertStatus(404, $this->get('/docs/does-not-exist'));
        $this->assertStatus(404, $this->get('/docs/_Sidebar'));
        $this->assertStatus(404, $this->get('/docs/..%2F..%2Fconfig%2Fapp'));
    }

    public function testOperatorsSeeTheWholeWiki(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $home = $this->get('/admin/docs');
        $this->assertStatus(200, $home);
        $this->assertContains('VPS deployment', $home->body());
        $ops = $this->get('/admin/docs/operations');
        $this->assertStatus(200, $ops);
        $this->assertContains('href="/admin/docs/security-model"', $ops->body());

        $this->signInConsole(['user' => ['is_platform_admin' => false]]);
        $this->assertStatus(404, $this->get('/admin/docs/operations')); // the operator console does not reveal itself
    }

    public function testMarkdownEscapesEverything(): void
    {
        $md = new Markdown(static fn (string $t): ?string => str_starts_with($t, 'javascript:') ? null : $t);
        $html = $md->render("<script>alert(1)</script>\n\n[click](javascript:alert(1)) **<b>bold</b>** `<i>`\n\n| a | b |\n|---|---|\n| <img src=x> | `x\\|y` |");
        $this->assertNotContains('<script>', $html);
        $this->assertNotContains('<img', $html);
        $this->assertNotContains('javascript:', $html);
        $this->assertContains('&lt;script&gt;', $html);
        $this->assertContains('<strong>&lt;b&gt;bold&lt;/b&gt;</strong>', $html);
        $this->assertContains('<code>&lt;i&gt;</code>', $html);
        $this->assertContains('<span class="doc-ref">click</span>', $html);
    }
}
