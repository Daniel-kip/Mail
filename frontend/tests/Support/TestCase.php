<?php

declare(strict_types=1);

namespace Tests\Support;

use App\Api\ApiClient;
use App\Http\Request;
use App\Http\Response;
use App\Kernel\Application;
use App\Session\ArraySession;
use App\Session\Session;
use App\Support\Config;

/**
 * Base class for unit and feature tests (no third-party dependencies).
 * Test methods are public methods whose names start with `test`.
 */
abstract class TestCase
{
    protected ?Application $app = null;
    protected FakeApiClient $api;
    protected ArraySession $session;
    public int $assertions = 0;

    public function setUp(): void
    {
    }

    public function tearDown(): void
    {
    }

    /** Fresh application wired to the fake API and an in-memory session. */
    protected function boot(array $config = []): Application
    {
        $base = Config::fromFile(APP_ROOT . '/config/app.php')->with(array_replace_recursive([
            'env' => 'testing',
            'debug' => true,
            'log_file' => sys_get_temp_dir() . '/mailvora-test.log',
            'url' => 'http://localhost',
            // Controllers are tested at their real paths; SealedLinksTest covers sealing.
            'sealed_links' => false,
        ], $config));
        $this->app = Application::create(APP_ROOT, $base);
        $this->api = new FakeApiClient();
        $this->session = new ArraySession();
        $this->app->container->instance(ApiClient::class, $this->api);
        $this->app->container->instance(Session::class, $this->session);
        return $this->app;
    }

    /** Simulate a request; the session persists across calls like a browser. */
    protected function call(string $method, string $path, array $post = [], array $headers = [], string $body = ''): Response
    {
        $app = $this->app ?? $this->boot();
        [$pathOnly, $queryString] = array_pad(explode('?', $path, 2), 2, '');
        parse_str($queryString, $query);
        if ($method !== 'GET' && !array_key_exists('_token', $post) && !isset($headers['x-csrf-token'])) {
            $post['_token'] = $this->csrf();
        }
        $state = $this->session->snapshot();
        $this->session = new ArraySession($state);
        $app->container->instance(Session::class, $this->session);
        // The view is shared across requests in tests; rebuild it per call.
        $app->container->set(\App\View\View::class, static fn () => new \App\View\View(APP_ROOT . '/resources/views'));
        $request = new Request($method === 'GET' ? 'GET' : 'POST', $pathOnly, $query, $method === 'GET' ? [] : $post + (in_array($method, ['PUT', 'PATCH', 'DELETE'], true) ? ['_method' => $method] : []), [], array_change_key_case($headers), [], '203.0.113.9', false, $body);
        $response = $app->handle($request);
        $this->session->save();
        return $response;
    }

    protected function get(string $path, array $headers = []): Response
    {
        return $this->call('GET', $path, [], $headers);
    }

    protected function post(string $path, array $data = [], array $headers = []): Response
    {
        return $this->call('POST', $path, $data, $headers);
    }

    protected function csrf(): string
    {
        $state = $this->session->snapshot();
        if (empty($state['_csrf'])) {
            $state['_csrf'] = bin2hex(random_bytes(32));
            $this->session = new ArraySession($state);
            $this->app?->container->instance(Session::class, $this->session);
        }
        return (string) $state['_csrf'];
    }

    /** Pretend the browser is signed in to the console with this session payload. */
    protected function signInConsole(array $overrides = []): array
    {
        $auth = array_replace_recursive(Fixtures::consoleSession(), $overrides);
        if (isset($overrides['organization']['permissions'])) {
            // Lists replace wholesale; a recursive merge would keep extra entries.
            $auth['organization']['permissions'] = $overrides['organization']['permissions'];
        }
        $this->api->signIn('console')->on('GET', '/v1/auth/session', 200, $auth);
        return $auth;
    }

    protected function signInMailbox(array $overrides = []): array
    {
        $mail = array_replace_recursive(Fixtures::mailboxSession(), $overrides);
        $this->api->signIn('webmail')->on('GET', '/v1/webmail/session', 200, $mail)
            ->on('GET', '/v1/webmail/folders', 200, Fixtures::folders());
        return $mail;
    }

    /** Flash data the previous response stored for the next request. */
    protected function flashed(string $key): mixed
    {
        return $this->session->snapshot()['_flash'][$key] ?? null;
    }

    // ── assertions ──────────────────────────────────────────────────────

    protected function assertTrue(mixed $condition, string $message = 'Expected true'): void
    {
        $this->assertions++;
        if ($condition !== true) {
            throw new AssertionFailed($message);
        }
    }

    protected function assertFalse(mixed $condition, string $message = 'Expected false'): void
    {
        $this->assertTrue($condition === false, $message);
    }

    protected function assertSame(mixed $expected, mixed $actual, string $message = ''): void
    {
        $this->assertions++;
        if ($expected !== $actual) {
            throw new AssertionFailed(($message !== '' ? $message . ': ' : '') . 'expected ' . var_export($expected, true) . ', got ' . var_export($actual, true));
        }
    }

    protected function assertContains(string $needle, string $haystack, string $message = ''): void
    {
        $this->assertions++;
        if (!str_contains($haystack, $needle)) {
            throw new AssertionFailed(($message !== '' ? $message . ': ' : '') . 'did not find "' . $needle . '" in ' . substr(strip_tags($haystack), 0, 400));
        }
    }

    protected function assertNotContains(string $needle, string $haystack, string $message = ''): void
    {
        $this->assertions++;
        if (str_contains($haystack, $needle)) {
            throw new AssertionFailed(($message !== '' ? $message . ': ' : '') . 'unexpectedly found "' . $needle . '"');
        }
    }

    protected function assertStatus(int $expected, Response $response): void
    {
        $this->assertions++;
        if ($response->status() !== $expected) {
            $hint = $response->status() >= 300 && $response->status() < 400 ? ' -> ' . $response->header('Location') : ' ' . substr(trim(strip_tags($response->body())), 0, 600);
            throw new AssertionFailed("Expected HTTP {$expected}, got {$response->status()}{$hint}");
        }
    }

    protected function assertRedirect(string $location, Response $response): void
    {
        $this->assertStatus(303, $response);
        $this->assertSame($location, $response->header('Location'), 'Redirect location');
    }

    protected function assertThrows(string $class, \Closure $fn): void
    {
        $this->assertions++;
        try {
            $fn();
        } catch (\Throwable $e) {
            if ($e instanceof $class) {
                return;
            }
            throw new AssertionFailed('Expected ' . $class . ', got ' . $e::class . ': ' . $e->getMessage());
        }
        throw new AssertionFailed('Expected ' . $class . ' to be thrown');
    }
}
