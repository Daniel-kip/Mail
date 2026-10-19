<?php

declare(strict_types=1);

namespace Tests\Support;

use App\Api\ApiClient;
use App\Api\ApiResponse;

/**
 * In-memory stand-in for the platform API.
 *
 *   $api->on('GET', '/v1/plans', 200, ['data' => [...]]);
 *   $api->on('POST', '#^/v1/mailboxes/[^/]+/unlock$#', 200, [...]); // regex
 *
 * Unmatched requests return 404 so tests notice missing stubs. Every call is
 * recorded for assertions.
 */
final class FakeApiClient implements ApiClient
{
    /** @var list<array{method: string, path: string, response: ApiResponse|\Closure}> */
    private array $routes = [];
    /** @var list<array{method: string, path: string, options: array}> */
    public array $calls = [];
    /** @var array<string, bool> */
    public array $sessions = [];

    public function on(string $method, string $path, int $status = 200, array $json = [], array $headers = []): self
    {
        $this->routes[] = ['method' => $method, 'path' => $path, 'response' => ApiResponse::fake($status, $json, $headers)];
        return $this;
    }

    /** Respond dynamically: fn (string $path, array $options): ApiResponse */
    public function onCallback(string $method, string $path, \Closure $handler): self
    {
        $this->routes[] = ['method' => $method, 'path' => $path, 'response' => $handler];
        return $this;
    }

    public function signIn(string $kind): self
    {
        $this->sessions[$kind] = true;
        return $this;
    }

    public function request(string $method, string $path, array $options = []): ApiResponse
    {
        $this->calls[] = ['method' => $method, 'path' => $path, 'options' => $options];
        foreach (array_reverse($this->routes) as $route) {
            $matches = str_starts_with($route['path'], '#') ? (bool) preg_match($route['path'], $path) : $route['path'] === $path;
            if ($route['method'] === $method && $matches) {
                $response = $route['response'];
                return $response instanceof \Closure ? $response($path, $options) : $response;
            }
        }
        return ApiResponse::fake(404, ['error' => ['code' => 'not_found', 'message' => "No stub for {$method} {$path}"]]);
    }

    public function forget(string $kind): void
    {
        unset($this->sessions[$kind]);
    }

    public function hasSession(string $kind): bool
    {
        return !empty($this->sessions[$kind]);
    }

    /** @return list<array{method: string, path: string, options: array}> */
    public function callsTo(string $method, string $path): array
    {
        return array_values(array_filter($this->calls, static fn ($c) => $c['method'] === $method && $c['path'] === $path));
    }
}
