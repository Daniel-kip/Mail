<?php

declare(strict_types=1);

namespace App\Kernel;

use App\Api\ApiClient;
use App\Api\ApiException;
use App\Api\HttpApiClient;
use App\Http\HttpException;
use App\Http\Links;
use App\Http\Middleware\Middleware;
use App\Http\Request;
use App\Http\Response;
use App\Http\Router;
use App\Mail\AppleProfileSigner;
use App\Mail\GuideCatalog;
use App\Session\NativeSession;
use App\Session\Session;
use App\Support\Config;
use App\Support\Logger;
use App\View\View;

/**
 * The HTTP kernel: wires services, runs the middleware pipeline, dispatches
 * to controllers and turns failures into friendly, reference-numbered error
 * pages (details go to the log, never to the browser in production).
 */
final class Application
{
    private static ?self $current = null;
    private ?Request $request = null;

    public function __construct(
        public readonly string $root,
        public readonly Config $config,
        public readonly Container $container,
        public readonly Router $router,
    ) {
    }

    public static function create(string $root, ?Config $config = null): self
    {
        $config ??= Config::fromFile($root . '/config/app.php');
        $container = new Container();
        $router = new Router();
        $app = new self($root, $config, $container, $router);

        $container->instance(Config::class, $config);
        $container->instance(self::class, $app);
        $container->set(Logger::class, static fn () => new Logger($config->string('log_file') ?: null));
        $container->set(Session::class, static fn () => new NativeSession(
            $config->string('session.name'),
            $config->bool('session.secure'),
            $config->string('session.path'),
            $config->int('session.idle_minutes'),
        ));
        $container->set(ApiClient::class, static fn (Container $c) => new HttpApiClient(
            $config->string('api.url'),
            $c->get(Session::class),
            $c->get(Logger::class),
            $config->int('api.connect_timeout'),
            $config->int('api.timeout'),
            $config->bool('api.verify_tls'),
            $app->request?->ip(),
        ));
        $container->set(View::class, static fn () => new View($root . '/resources/views'));
        $container->set(Links::class, static fn (Container $c) => new Links($router, static fn () => $c->get(Session::class), $config->bool('sealed_links')));
        $container->set(GuideCatalog::class, static fn () => new GuideCatalog($root . '/resources/guides/mail-clients.php'));
        $container->set(AppleProfileSigner::class, static fn (Container $c) => new AppleProfileSigner(
            $config->get('apple_profile.cert'),
            $config->get('apple_profile.key'),
            $config->get('apple_profile.chain'),
            $config->get('apple_profile.key_passphrase'),
            $c->get(Logger::class),
        ));

        (require $root . '/config/routes.php')($router);
        self::$current = $app;
        return $app;
    }

    public static function current(): self
    {
        return self::$current ?? throw new \LogicException('The application has not been created');
    }

    public function request(): ?Request
    {
        return $this->request;
    }

    public function get(string $id): mixed
    {
        return $this->container->get($id);
    }

    public function session(): Session
    {
        return $this->container->get(Session::class);
    }

    public function view(): View
    {
        return $this->container->get(View::class);
    }

    public function api(): ApiClient
    {
        return $this->container->get(ApiClient::class);
    }

    public function links(): Links
    {
        return $this->container->get(Links::class);
    }

    public function logger(): Logger
    {
        return $this->container->get(Logger::class);
    }

    /** Front-controller entry point. */
    public function run(): void
    {
        $request = Request::fromGlobals($this->config->bool('trust_proxy'));
        $response = $this->handle($request);
        $this->session()->save();
        $response->send();
    }

    public function handle(Request $request): Response
    {
        $this->request = $request;
        self::$current = $this;
        try {
            // The session starts first: sealed links are opened with a key
            // that lives in it. Routing then sees the real (unsealed) path.
            return $this->pipeline(['headers', 'session'], function (Request $request): Response {
                $links = $this->links();
                $resolved = $links->resolve(
                    $request,
                    fn (string $type, string $message) => $this->session()->flash($type, $message),
                    fn (): string => !$this->api()->hasSession('console') && $this->api()->hasSession('webmail') ? '/mail' : '/console',
                );
                if ($resolved instanceof Response) {
                    return $links->rewrite($resolved);
                }
                $request = $resolved;
                $this->request = $request;
                $this->view()->share('request', $request);
                return $links->rewrite($this->dispatch($request));
            })($request);
        } catch (\Throwable $e) {
            return $this->renderException($request, $e);
        }
    }

    private function dispatch(Request $request): Response
    {
        $match = $this->router->match($request->method(), $request->path());
        $middleware = ['csrf'];
        if ($match['status'] === 'found') {
            $request = $request->withParams($match['params']);
            $middleware = array_merge($middleware, $match['middleware']);
            $handler = function (Request $r) use ($match): Response {
                $this->request = $r;
                [$class, $method] = $match['handler'];
                $controller = new $class($this);
                return $controller->{$method}($r);
            };
        } else {
            $status = $match['status'] === 'method_not_allowed' ? 405 : 404;
            $handler = fn (Request $r): Response => $this->errorPage($r, $status);
        }
        return $this->pipeline($middleware, $handler)($request);
    }

    /** @param list<string> $names */
    private function pipeline(array $names, \Closure $core): \Closure
    {
        $next = $core;
        foreach (array_reverse($names) as $name) {
            [$alias, $arg] = array_pad(explode(':', $name, 2), 2, null);
            $middleware = Middleware::resolve($alias, $arg, $this);
            $inner = $next;
            $next = function (Request $r) use ($middleware, $inner): Response {
                try {
                    return $middleware->handle($r, $inner);
                } catch (\Throwable $e) {
                    return $this->renderException($r, $e);
                }
            };
        }
        return $next;
    }

    public function renderException(Request $request, \Throwable $e): Response
    {
        if ($e instanceof HttpException) {
            return $this->errorPage($request, $e->status, $e->getMessage(), $e->requestId);
        }
        if ($e instanceof ApiException) {
            return $this->errorPage($request, 503, $e->getMessage(), $e->requestId);
        }
        $reference = 'err_' . bin2hex(random_bytes(6));
        $this->logger()->error('unhandled_exception', [
            'reference' => $reference,
            'type' => $e::class,
            'message' => $e->getMessage(),
            'file' => $e->getFile() . ':' . $e->getLine(),
            'path' => $request->path(),
        ]);
        $message = $this->config->bool('debug') ? $e::class . ': ' . $e->getMessage() . ' (' . basename($e->getFile()) . ':' . $e->getLine() . ')' : '';
        return $this->errorPage($request, 500, $message, $reference);
    }

    public function errorPage(Request $request, int $status, string $message = '', ?string $reference = null): Response
    {
        $titles = [
            403 => 'Access denied',
            404 => 'Page not found',
            405 => 'Method not allowed',
            419 => 'Page expired',
            429 => 'Too many requests',
            500 => 'Something went wrong',
            503 => 'Temporarily unavailable',
        ];
        $title = $titles[$status] ?? 'Error';
        if ($request->wantsJson()) {
            return Response::json(['error' => ['message' => $message ?: $title, 'reference' => $reference]], $status);
        }
        try {
            $html = $this->view()->render('errors/error', [
                'status' => $status,
                'title' => $title,
                'message' => $message,
                'reference' => $reference,
            ], 'minimal');
        } catch (\Throwable) {
            $html = '<!doctype html><title>' . $status . '</title><h1>' . htmlspecialchars($title) . '</h1>';
        }
        return Response::html($html, $status);
    }
}
