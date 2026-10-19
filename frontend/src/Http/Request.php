<?php

declare(strict_types=1);

namespace App\Http;

/**
 * Immutable view of the incoming HTTP request.
 */
final class Request
{
    /** @var array<string, string> route parameters set by the router */
    private array $params = [];
    /** @var array<string, mixed> per-request attributes (current user, mailbox...) */
    private array $attributes = [];

    /**
     * @param array<string, mixed> $query
     * @param array<string, mixed> $post
     * @param array<string, mixed> $files
     * @param array<string, string> $headers lower-case names
     * @param array<string, string> $cookies
     */
    public function __construct(
        private readonly string $method,
        private readonly string $path,
        private readonly array $query = [],
        private readonly array $post = [],
        private readonly array $files = [],
        private readonly array $headers = [],
        private readonly array $cookies = [],
        private readonly string $ip = '127.0.0.1',
        private readonly bool $secure = false,
        private readonly string $rawBody = '',
    ) {
    }

    public static function fromGlobals(bool $trustProxy): self
    {
        $headers = [];
        foreach ($_SERVER as $key => $value) {
            if (str_starts_with($key, 'HTTP_')) {
                $headers[strtolower(str_replace('_', '-', substr($key, 5)))] = (string) $value;
            }
        }
        if (isset($_SERVER['CONTENT_TYPE'])) {
            $headers['content-type'] = (string) $_SERVER['CONTENT_TYPE'];
        }
        $path = parse_url((string) ($_SERVER['REQUEST_URI'] ?? '/'), PHP_URL_PATH) ?: '/';
        $path = '/' . ltrim(rawurldecode($path), '/');

        $ip = (string) ($_SERVER['REMOTE_ADDR'] ?? '127.0.0.1');
        $secure = (!empty($_SERVER['HTTPS']) && strtolower((string) $_SERVER['HTTPS']) !== 'off')
            || (int) ($_SERVER['SERVER_PORT'] ?? 0) === 443;
        if ($trustProxy) {
            // Only meaningful behind the operator's own reverse proxy, which
            // must overwrite (not append to) these headers.
            $real = $headers['x-real-ip'] ?? '';
            if (filter_var($real, FILTER_VALIDATE_IP) !== false) {
                $ip = $real;
            }
            if (strtolower($headers['x-forwarded-proto'] ?? '') === 'https') {
                $secure = true;
            }
        }

        $method = strtoupper((string) ($_SERVER['REQUEST_METHOD'] ?? 'GET'));
        $raw = in_array($method, ['POST', 'PUT', 'PATCH', 'DELETE'], true) && !str_starts_with(strtolower($headers['content-type'] ?? ''), 'multipart/')
            ? (string) file_get_contents('php://input', false, null, 0, 2_000_000)
            : '';

        return new self($method, $path, $_GET, $_POST, $_FILES, $headers, $_COOKIE, $ip, $secure, $raw);
    }

    /** The effective method, honouring `_method` overrides on HTML forms. */
    public function method(): string
    {
        if ($this->method === 'POST') {
            $override = strtoupper((string) ($this->post['_method'] ?? ''));
            if (in_array($override, ['PUT', 'PATCH', 'DELETE'], true)) {
                return $override;
            }
        }
        return $this->method;
    }

    public function isSafe(): bool
    {
        return in_array($this->method, ['GET', 'HEAD', 'OPTIONS'], true);
    }

    public function path(): string
    {
        return $this->path;
    }

    public function query(string $key, mixed $default = null): mixed
    {
        return $this->query[$key] ?? $default;
    }

    /** @return array<string, mixed> */
    public function allQuery(): array
    {
        return $this->query;
    }

    public function input(string $key, mixed $default = null): mixed
    {
        return $this->post[$key] ?? $default;
    }

    /** Trimmed string input (empty string when absent or not a string). */
    public function string(string $key, string $default = ''): string
    {
        $value = $this->post[$key] ?? $this->query[$key] ?? $default;
        return is_string($value) ? trim($value) : $default;
    }

    public function bool(string $key): bool
    {
        $value = $this->post[$key] ?? null;
        return in_array($value, ['1', 'on', 'true', 'yes'], true);
    }

    /** @return array<string, mixed> */
    public function all(): array
    {
        return $this->post;
    }

    /** @return array<string, mixed> */
    public function only(string ...$keys): array
    {
        return array_intersect_key($this->post, array_flip($keys));
    }

    /** @return array<string, mixed> */
    public function files(): array
    {
        return $this->files;
    }

    public function header(string $name, string $default = ''): string
    {
        return $this->headers[strtolower($name)] ?? $default;
    }

    public function cookie(string $name): ?string
    {
        return $this->cookies[$name] ?? null;
    }

    public function ip(): string
    {
        return $this->ip;
    }

    public function isSecure(): bool
    {
        return $this->secure;
    }

    public function rawBody(): string
    {
        return $this->rawBody;
    }

    public function wantsJson(): bool
    {
        return str_contains($this->header('accept'), 'application/json')
            || strtolower($this->header('x-requested-with')) === 'fetch';
    }

    public function param(string $name, string $default = ''): string
    {
        return $this->params[$name] ?? $default;
    }

    /** @param array<string, string> $params */
    public function withParams(array $params): self
    {
        $clone = clone $this;
        $clone->params = $params;
        return $clone;
    }

    /**
     * The same request aimed at another path and query (used when a sealed
     * `/app?ref=…` link is opened).
     *
     * @param array<string, mixed> $query
     */
    public function withTarget(string $path, array $query): self
    {
        $clone = new self($this->method, $path, $query, $this->post, $this->files, $this->headers, $this->cookies, $this->ip, $this->secure, $this->rawBody);
        $clone->params = $this->params;
        $clone->attributes = $this->attributes;
        return $clone;
    }

    public function attribute(string $name, mixed $default = null): mixed
    {
        return $this->attributes[$name] ?? $default;
    }

    public function withAttribute(string $name, mixed $value): self
    {
        $clone = clone $this;
        $clone->attributes[$name] = $value;
        return $clone;
    }

    /** Full URL path with query string, for "return here after sign-in". */
    public function fullPath(): string
    {
        return $this->path . ($this->query !== [] ? '?' . http_build_query($this->query) : '');
    }
}
