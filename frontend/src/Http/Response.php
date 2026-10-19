<?php

declare(strict_types=1);

namespace App\Http;

/**
 * An HTTP response value. Nothing is emitted until `send()`, which keeps
 * controllers testable.
 */
final class Response
{
    /** @param array<string, string|list<string>> $headers */
    public function __construct(
        private string $body = '',
        private int $status = 200,
        private array $headers = [],
    ) {
    }

    public static function html(string $body, int $status = 200): self
    {
        return new self($body, $status, ['Content-Type' => 'text/html; charset=utf-8', 'Cache-Control' => 'no-store']);
    }

    public static function json(mixed $data, int $status = 200): self
    {
        return new self(
            (string) json_encode($data, JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE),
            $status,
            ['Content-Type' => 'application/json; charset=utf-8', 'Cache-Control' => 'no-store'],
        );
    }

    public static function redirect(string $location, int $status = 303): self
    {
        // Only same-site relative locations are allowed from user input;
        // absolute URLs are produced by the application itself.
        return new self('', $status, ['Location' => $location, 'Cache-Control' => 'no-store']);
    }

    public static function download(string $content, string $filename, string $contentType): self
    {
        $safe = preg_replace('/[^A-Za-z0-9._-]+/', '-', $filename) ?: 'download';
        return new self($content, 200, [
            'Content-Type' => $contentType,
            'Content-Disposition' => 'attachment; filename="' . $safe . '"',
            'Cache-Control' => 'no-store',
            'X-Content-Type-Options' => 'nosniff',
        ]);
    }

    public static function text(string $body, int $status = 200, string $type = 'text/plain; charset=utf-8'): self
    {
        return new self($body, $status, ['Content-Type' => $type]);
    }

    public function status(): int
    {
        return $this->status;
    }

    public function body(): string
    {
        return $this->body;
    }

    public function header(string $name): ?string
    {
        foreach ($this->headers as $key => $value) {
            if (strcasecmp($key, $name) === 0) {
                return is_array($value) ? implode(', ', $value) : $value;
            }
        }
        return null;
    }

    /** @return array<string, string|list<string>> */
    public function headers(): array
    {
        return $this->headers;
    }

    public function withHeader(string $name, string $value): self
    {
        $clone = clone $this;
        foreach (array_keys($clone->headers) as $key) {
            if (strcasecmp($key, $name) === 0) {
                unset($clone->headers[$key]);
            }
        }
        $clone->headers[$name] = $value;
        return $clone;
    }

    public function withBody(string $body): self
    {
        $clone = clone $this;
        $clone->body = $body;
        return $clone;
    }

    public function withoutHeader(string $name): self
    {
        $clone = clone $this;
        foreach (array_keys($clone->headers) as $key) {
            if (strcasecmp($key, $name) === 0) {
                unset($clone->headers[$key]);
            }
        }
        return $clone;
    }

    public function hasHeader(string $name): bool
    {
        return $this->header($name) !== null;
    }

    public function send(): void
    {
        if (!headers_sent()) {
            http_response_code($this->status);
            foreach ($this->headers as $name => $value) {
                foreach ((array) $value as $v) {
                    header($name . ': ' . str_replace(["\r", "\n"], '', (string) $v), false);
                }
            }
        }
        echo $this->body;
    }
}
