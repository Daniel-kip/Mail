<?php

declare(strict_types=1);

namespace App\Http;

/**
 * Method + path router with named parameters, groups and per-route
 * middleware.
 *
 *   $r->group(['prefix' => '/console', 'middleware' => ['console']], function (Router $r) {
 *       $r->get('/domains/{id:uuid}', [DomainController::class, 'show'], 'domains.show');
 *   });
 */
final class Router
{
    private const PATTERNS = [
        'uuid' => '[0-9a-fA-F-]{36}',
        'int' => '\d+',
        'slug' => '[A-Za-z0-9_-]+',
        'token' => '[A-Za-z0-9_-]{16,200}',
        'any' => '.+',
        'segment' => '[^/]+',
    ];

    /** @var list<array{method: string, regex: string, handler: array{0: class-string, 1: string}, middleware: list<string>, name: ?string, pattern: string}> */
    private array $routes = [];
    /** @var array<string, string> name => pattern */
    private array $named = [];
    private string $prefix = '';
    /** @var list<string> */
    private array $middleware = [];

    public function get(string $path, array $handler, ?string $name = null, array $middleware = []): void
    {
        $this->add('GET', $path, $handler, $name, $middleware);
    }

    public function post(string $path, array $handler, ?string $name = null, array $middleware = []): void
    {
        $this->add('POST', $path, $handler, $name, $middleware);
    }

    public function put(string $path, array $handler, ?string $name = null, array $middleware = []): void
    {
        $this->add('PUT', $path, $handler, $name, $middleware);
    }

    public function patch(string $path, array $handler, ?string $name = null, array $middleware = []): void
    {
        $this->add('PATCH', $path, $handler, $name, $middleware);
    }

    public function delete(string $path, array $handler, ?string $name = null, array $middleware = []): void
    {
        $this->add('DELETE', $path, $handler, $name, $middleware);
    }

    /** @param array{prefix?: string, middleware?: list<string>} $options */
    public function group(array $options, callable $routes): void
    {
        $previous = [$this->prefix, $this->middleware];
        $this->prefix .= rtrim($options['prefix'] ?? '', '/');
        $this->middleware = array_merge($this->middleware, $options['middleware'] ?? []);
        $routes($this);
        [$this->prefix, $this->middleware] = $previous;
    }

    private function add(string $method, string $path, array $handler, ?string $name, array $middleware): void
    {
        $pattern = $this->prefix . ($path === '/' && $this->prefix !== '' ? '' : $path);
        $pattern = $pattern === '' ? '/' : $pattern;
        $regex = preg_replace_callback('~\{([a-z_]+)(?::([a-z]+))?\}~', static function (array $m): string {
            $type = $m[2] ?? 'segment';
            $inner = self::PATTERNS[$type] ?? throw new \InvalidArgumentException("Unknown route pattern {$type}");
            return '(?P<' . $m[1] . '>' . $inner . ')';
        }, $pattern);
        $this->routes[] = [
            'method' => $method,
            'regex' => '~^' . $regex . '$~',
            'handler' => $handler,
            'middleware' => array_merge($this->middleware, $middleware),
            'name' => $name,
            'pattern' => $pattern,
        ];
        if ($name !== null) {
            $this->named[$name] = $pattern;
        }
    }

    /**
     * @return array{status: 'found', handler: array, middleware: list<string>, params: array<string, string>}|array{status: 'not_found'|'method_not_allowed', allowed?: list<string>}
     */
    public function match(string $method, string $path): array
    {
        $allowed = [];
        $lookup = $method === 'HEAD' ? 'GET' : $method;
        foreach ($this->routes as $route) {
            if (!preg_match($route['regex'], $path, $m)) {
                continue;
            }
            if ($route['method'] !== $lookup) {
                $allowed[] = $route['method'];
                continue;
            }
            $params = array_map('rawurldecode', array_filter($m, 'is_string', ARRAY_FILTER_USE_KEY));
            return ['status' => 'found', 'handler' => $route['handler'], 'middleware' => $route['middleware'], 'params' => $params];
        }
        return $allowed === [] ? ['status' => 'not_found'] : ['status' => 'method_not_allowed', 'allowed' => array_values(array_unique($allowed))];
    }

    /** Build a path from a route name (`route('domains.show', ['id' => $id])`). */
    public function url(string $name, array $params = [], array $query = []): string
    {
        $pattern = $this->named[$name] ?? throw new \InvalidArgumentException("No route named {$name}");
        $path = preg_replace_callback('~\{([a-z_]+)(?::[a-z]+)?\}~', static function (array $m) use ($params, $name): string {
            if (!array_key_exists($m[1], $params)) {
                throw new \InvalidArgumentException("Route {$name} needs parameter {$m[1]}");
            }
            return rawurlencode((string) $params[$m[1]]);
        }, $pattern);
        return $path . ($query !== [] ? '?' . http_build_query($query) : '');
    }

    /** @return list<array{method: string, pattern: string, name: ?string}> */
    public function routes(): array
    {
        return array_map(static fn (array $r) => ['method' => $r['method'], 'pattern' => $r['pattern'], 'name' => $r['name']], $this->routes);
    }
}
