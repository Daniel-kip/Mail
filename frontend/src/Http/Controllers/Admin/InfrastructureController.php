<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;

/**
 * The System Infrastructure document (architecture, system flows, security
 * flows) for platform operators: an overview page, the PDF download and its
 * preview images. Built by frontend/scripts/build_infrastructure_pdf.py.
 */
final class InfrastructureController extends Controller
{
    private const PREVIEWS = ['cover', 'deployment', 'security'];

    public function show(Request $request): Response
    {
        $manifest = $this->manifest();
        return $this->render('admin/infrastructure', [
            'title' => 'System infrastructure',
            'doc' => $manifest,
            'available' => $manifest !== null && is_file($this->dir() . '/' . $manifest['file']),
            'area' => 'admin',
        ], 'admin');
    }

    public function download(Request $request): Response
    {
        $manifest = $this->manifest();
        $path = $manifest !== null ? $this->dir() . '/' . $manifest['file'] : '';
        if ($path === '' || !is_file($path)) {
            throw HttpException::notFound();
        }
        $this->logger()->info('infrastructure_document_downloaded', ['user' => $request->attribute('auth')['user']['email'] ?? null, 'version' => $manifest['version']]);
        return Response::download((string) file_get_contents($path), 'Mailvora-System-Infrastructure-v' . $manifest['version'] . '.pdf', 'application/pdf');
    }

    public function preview(Request $request): Response
    {
        $name = $request->param('name');
        $path = $this->dir() . '/preview-' . $name . '.png';
        if (!in_array($name, self::PREVIEWS, true) || !is_file($path)) {
            throw HttpException::notFound();
        }
        return new Response((string) file_get_contents($path), 200, ['Content-Type' => 'image/png', 'Cache-Control' => 'private, max-age=3600', 'X-Content-Type-Options' => 'nosniff']);
    }

    /** @return array<string, mixed>|null */
    private function manifest(): ?array
    {
        $file = $this->dir() . '/manifest.json';
        $data = is_file($file) ? json_decode((string) file_get_contents($file), true) : null;
        if (!is_array($data) || !isset($data['file']) || !preg_match('/^[A-Za-z0-9._-]+\.pdf$/', (string) $data['file'])) {
            return null;
        }
        return $data;
    }

    private function dir(): string
    {
        return $this->app->root . '/resources/infrastructure';
    }

    private function logger(): \App\Support\Logger
    {
        return $this->app->logger();
    }
}
