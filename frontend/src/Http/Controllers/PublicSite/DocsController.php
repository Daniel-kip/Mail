<?php

declare(strict_types=1);

namespace App\Http\Controllers\PublicSite;

use App\Http\Controllers\Controller;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;
use App\Support\Wiki;

/** Public documentation: the customer guides from the wiki, at /docs. */
final class DocsController extends Controller
{
    public function index(Request $request): Response
    {
        $wiki = $this->wiki();
        return $this->render('docs/index', [
            'title' => 'Documentation',
            'description' => 'Guides for Mailvora administrators, developers, ICT support and staff.',
            'sections' => $wiki->sections(false),
            'base' => '/docs',
        ], 'public');
    }

    public function show(Request $request): Response
    {
        $wiki = $this->wiki();
        $slug = strtolower($request->param('page'));
        if (!$wiki->isPublic($slug)) {
            throw HttpException::notFound();
        }
        return $this->render('docs/show', [
            'title' => $wiki->title($slug),
            'description' => \App\View\Markdown::lead($wiki->source($slug)),
            'page' => $wiki->render($slug, '/docs', false),
            'slug' => $slug,
            'sections' => $wiki->sections(false),
            'base' => '/docs',
        ], 'public');
    }

    private function wiki(): Wiki
    {
        return new Wiki($this->app->root . '/resources/wiki');
    }
}
