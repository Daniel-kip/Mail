<?php

declare(strict_types=1);

namespace App\View;

/**
 * Plain-PHP template renderer.
 *
 * Templates live in resources/views and receive their data as local
 * variables plus `$view` (this renderer). Every value must be escaped with
 * `e()` at output; templates never echo raw input. A template is rendered
 * first, then wrapped by its layout, which receives the content as
 * `$content` and the same data.
 */
final class View
{
    /** @var array<string, mixed> data shared with every template */
    private array $shared = [];

    public function __construct(private readonly string $root)
    {
    }

    public function share(string $key, mixed $value): void
    {
        $this->shared[$key] = $value;
    }

    public function shared(string $key, mixed $default = null): mixed
    {
        return $this->shared[$key] ?? $default;
    }

    /** @param array<string, mixed> $data */
    public function render(string $template, array $data = [], ?string $layout = null): string
    {
        $data = array_merge($this->shared, $data);
        $content = $this->evaluate($template, $data);
        if ($layout === null) {
            return $content;
        }
        return $this->evaluate('layouts/' . $layout, $data + ['content' => $content]);
    }

    /** Render a partial inside a template: `<?= $view->insert('partials/x', [...]) ?>`. */
    public function insert(string $template, array $data = []): string
    {
        return $this->evaluate($template, array_merge($this->shared, $data));
    }

    public function exists(string $template): bool
    {
        return is_file($this->path($template));
    }

    private function path(string $template): string
    {
        if (!preg_match('~^[a-z0-9/_-]+$~', $template)) {
            throw new \InvalidArgumentException("Invalid template name {$template}");
        }
        return $this->root . '/' . $template . '.php';
    }

    /** @param array<string, mixed> $__data */
    private function evaluate(string $__template, array $__data): string
    {
        $__file = $this->path($__template);
        if (!is_file($__file)) {
            throw new \RuntimeException("View {$__template} not found");
        }
        $view = $this;
        extract($__data, EXTR_SKIP);
        ob_start();
        try {
            require $__file;
        } catch (\Throwable $e) {
            ob_end_clean();
            throw $e;
        }
        return (string) ob_get_clean();
    }
}
