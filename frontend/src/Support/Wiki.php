<?php

declare(strict_types=1);

namespace App\Support;

use App\View\Markdown;

/**
 * The documentation wiki in `resources/wiki` (GitHub wiki layout: Home.md,
 * _Sidebar.md, _Footer.md, one Page-Name.md per topic).
 *
 * Customer guides are public at /docs; the full wiki, including operations
 * and security internals, is shown to platform operators at /admin/docs.
 * Pages are looked up by slug in the directory listing, never by building a
 * path from the request.
 */
final class Wiki
{
    /** Pages for platform operators only (deployment, internals, security design). */
    private const OPERATOR_PAGES = [
        'quick-start', 'architecture', 'web-application', 'configuration', 'vps-deployment', 'operations',
        'release-checklist', 'mobile-mail-setup', 'security-model', 'security-readiness', 'sealed-links',
        'decision-log', 'project-status', 'research-notes',
    ];

    /** @var array<string, string> slug => file name */
    private array $files = [];

    public function __construct(private readonly string $dir)
    {
        foreach (glob($dir . '/*.md') ?: [] as $path) {
            $name = basename($path, '.md');
            if ($name[0] !== '_') {
                $this->files[strtolower($name)] = $name;
            }
        }
    }

    public function exists(string $slug): bool
    {
        return isset($this->files[strtolower($slug)]);
    }

    public function isPublic(string $slug): bool
    {
        $slug = strtolower($slug);
        return isset($this->files[$slug]) && $slug !== 'home' && !in_array($slug, self::OPERATOR_PAGES, true);
    }

    public function title(string $slug): string
    {
        foreach ($this->sections(true) as $section) {
            foreach ($section['pages'] as $page) {
                if ($page['slug'] === strtolower($slug)) {
                    return $page['title'];
                }
            }
        }
        return str_replace('-', ' ', $this->files[strtolower($slug)] ?? 'Documentation');
    }

    public function source(string $slug): string
    {
        $file = $this->files[strtolower($slug)] ?? null;
        return $file === null ? '' : (string) file_get_contents($this->dir . '/' . $file . '.md');
    }

    public function footer(): string
    {
        return is_file($this->dir . '/_Footer.md') ? (string) file_get_contents($this->dir . '/_Footer.md') : '';
    }

    /**
     * Sidebar sections from _Sidebar.md (bold lines are headings, list items
     * are pages). Public readers see only public pages.
     *
     * @return list<array{title: string, pages: list<array{slug: string, title: string, lead: string}>}>
     */
    public function sections(bool $operator): array
    {
        $sections = [];
        $current = null;
        foreach (preg_split('/\R/', (string) @file_get_contents($this->dir . '/_Sidebar.md')) ?: [] as $line) {
            if (preg_match('/^\*\*([^*\[]+)\*\*\s*$/', trim($line), $m)) {
                $sections[] = ['title' => trim($m[1]), 'pages' => []];
                $current = count($sections) - 1;
            } elseif ($current !== null && preg_match('/^[-*]\s+\[([^\]]+)\]\(([A-Za-z0-9_-]+)\.md\)/', trim($line), $m)) {
                $slug = strtolower($m[2]);
                if ($this->exists($slug) && ($operator || $this->isPublic($slug))) {
                    $sections[$current]['pages'][] = ['slug' => $slug, 'title' => $m[1], 'lead' => Markdown::lead($this->source($slug))];
                }
            }
        }
        return array_values(array_filter($sections, static fn (array $s) => $s['pages'] !== []));
    }

    /**
     * Render a page. Wiki links become /docs/<slug> (or the operator base);
     * links to pages the reader may not open, and to source files, render as
     * plain text.
     *
     * @return array{html: string, toc: list<array{id: string, text: string}>, footer: string}
     */
    public function render(string $slug, string $base, bool $operator): array
    {
        $md = new Markdown(function (string $target) use ($base, $operator): ?string {
            if (preg_match('#^(https?://|mailto:)#i', $target)) {
                return $target;
            }
            if (str_starts_with($target, '#')) {
                return $target;
            }
            if (preg_match('/^([A-Za-z0-9_-]+)\.md(#[A-Za-z0-9_-]+)?$/', $target, $m)) {
                $page = strtolower($m[1]);
                if ($page === 'home') {
                    return $base . ($m[2] ?? '');
                }
                if (!$this->exists($page) || (!$operator && !$this->isPublic($page))) {
                    return null;
                }
                return $base . '/' . $page . ($m[2] ?? '');
            }
            return null;
        });
        $html = $md->render($this->source($slug));
        $toc = $md->toc;
        return ['html' => $html, 'toc' => $toc, 'footer' => $md->render($this->footer())];
    }
}
