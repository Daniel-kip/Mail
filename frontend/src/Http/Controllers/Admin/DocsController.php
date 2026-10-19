<?php

declare(strict_types=1);

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;
use App\Support\Wiki;

/** The complete wiki, including operations and security internals, for platform operators. */
final class DocsController extends Controller
{
    public function index(Request $request): Response
    {
        return $this->page('home');
    }

    public function show(Request $request): Response
    {
        $slug = strtolower($request->param('page'));
        if ($slug === 'home') {
            return $this->redirect('/admin/docs');
        }
        return $this->page($slug);
    }

    private function page(string $slug): Response
    {
        $wiki = new Wiki($this->app->root . '/resources/wiki');
        if (!$wiki->exists($slug)) {
            throw HttpException::notFound();
        }
        return $this->render('docs/show', [
            'title' => $slug === 'home' ? 'Documentation' : $wiki->title($slug),
            'page' => $wiki->render($slug, '/admin/docs', true),
            'slug' => $slug,
            'sections' => $wiki->sections(true),
            'base' => '/admin/docs',
            'operator' => true,
            'area' => 'admin',
        ], 'admin');
    }
}
