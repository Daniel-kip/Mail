<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\TestCase;

final class InfrastructureTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
    }

    public function testOperatorsSeeTheDocumentPage(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $res = $this->get('/admin/infrastructure');
        $this->assertStatus(200, $res);
        $manifest = json_decode((string) file_get_contents(APP_ROOT . '/resources/infrastructure/manifest.json'), true);
        $this->assertContains('Download PDF', $res->body());
        $this->assertContains($manifest['sha256'], $res->body(), 'the checksum is shown');
        $this->assertContains('/admin/infrastructure/preview/deployment', $res->body());
    }

    public function testOperatorsDownloadTheVectorPdf(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $res = $this->get('/admin/infrastructure/download');
        $this->assertStatus(200, $res);
        $this->assertSame('application/pdf', $res->header('Content-Type'));
        $this->assertContains('attachment; filename="Mailvora-System-Infrastructure-v', (string) $res->header('Content-Disposition'));
        $this->assertTrue(str_starts_with($res->body(), '%PDF-'), 'a real PDF file');
        $manifest = json_decode((string) file_get_contents(APP_ROOT . '/resources/infrastructure/manifest.json'), true);
        $this->assertSame($manifest['sha256'], hash('sha256', $res->body()), 'the download matches the manifest checksum');
    }

    public function testPreviewsAreWhitelisted(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => true]]);
        $png = $this->get('/admin/infrastructure/preview/cover');
        $this->assertStatus(200, $png);
        $this->assertSame('image/png', $png->header('Content-Type'));
        $this->assertStatus(404, $this->get('/admin/infrastructure/preview/manifest'));
    }

    public function testOthersCannotReachIt(): void
    {
        $this->signInConsole(['user' => ['is_platform_admin' => false]]);
        $this->assertStatus(404, $this->get('/admin/infrastructure'));
        $this->assertStatus(404, $this->get('/admin/infrastructure/download'));
    }
}
