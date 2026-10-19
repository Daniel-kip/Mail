<?php

declare(strict_types=1);

namespace App\Http;

use App\Session\Session;

/**
 * Sealed links for the signed-in areas (console, operator console, webmail).
 *
 * Every address that leads to a private page is replaced by an opaque one,
 * `/app?ref=<token>`, where the token is the real path and query encrypted
 * and authenticated (AES-256-GCM) with a key that lives only in the visitor's
 * session. An address cannot be read, edited or reused in another session:
 * changing one character makes it invalid, and signing out retires them all.
 *
 * - Outgoing: links, form actions, `src` and `data-*` URLs in HTML responses,
 *   and `Location` headers (including `?next=` on sign-in redirects).
 * - Incoming: `/app?ref=…` is opened and routed as the sealed path. Typing a
 *   private address directly is allowed only for fixed pages without record
 *   identifiers (menu entries such as `/console/domains`), which redirect to
 *   their sealed form; anything that names a record is refused.
 *
 * Public pages, email-link pages and standard endpoints (`/.well-known/…`,
 * autoconfig, OAuth callbacks) keep their readable addresses.
 */
final class Links
{
    public const ENTRY = '/app';
    private const SESSION_KEY = '_link_key';
    private const CIPHER = 'aes-256-gcm';
    private const PRIVATE_MIDDLEWARE = ['console', 'admin', 'mailbox'];

    /** @var array<string, array{private: bool, static: bool}> */
    private array $cache = [];

    public function __construct(
        private readonly Router $router,
        /** @var \Closure(): Session */
        private readonly \Closure $session,
        private readonly bool $enabled,
    ) {
    }

    private function session(): Session
    {
        return ($this->session)();
    }

    public function enabled(): bool
    {
        return $this->enabled;
    }

    /** Make sure this session has a link key (called when the session starts). */
    public function ensureKey(): void
    {
        if ($this->enabled && !is_string($this->session()->get(self::SESSION_KEY))) {
            $this->session()->put(self::SESSION_KEY, base64_encode(random_bytes(32)));
        }
    }

    /** Is the path served only to signed-in people? */
    public function isPrivate(string $path): bool
    {
        return $this->classify($path)['private'];
    }

    /** `/console/domains/…?x=1#top` → `/app?ref=…#top` */
    public function seal(string $target): string
    {
        [$target, $fragment] = array_pad(explode('#', $target, 2), 2, null);
        $key = $this->key();
        // Synthetic IV: the same address always seals to the same token within
        // a session (stable links, working "visited" styles) and reveals nothing
        // beyond equality.
        $iv = substr(hash_hmac('sha256', $target, $key, true), 0, 12);
        $tag = '';
        $cipher = openssl_encrypt($target, self::CIPHER, $key, OPENSSL_RAW_DATA, $iv, $tag, 'mailvora-link-v1', 16);
        if ($cipher === false) {
            throw new \RuntimeException('Could not seal a link');
        }
        $ref = rtrim(strtr(base64_encode($iv . $tag . $cipher), '+/', '-_'), '=');
        return self::ENTRY . '?ref=' . $ref . ($fragment !== null ? '#' . $fragment : '');
    }

    /** The sealed target, or null when the token is malformed, edited or from another session. */
    public function open(string $ref): ?string
    {
        if ($ref === '' || strlen($ref) > 4096 || !preg_match('/^[A-Za-z0-9_-]+$/', $ref)) {
            return null;
        }
        $raw = base64_decode(strtr($ref, '-_', '+/') . str_repeat('=', (4 - strlen($ref) % 4) % 4), true);
        $key = $this->session()->get(self::SESSION_KEY);
        if ($raw === false || strlen($raw) < 29 || !is_string($key)) {
            return null;
        }
        $plain = openssl_decrypt(substr($raw, 28), self::CIPHER, base64_decode($key), OPENSSL_RAW_DATA, substr($raw, 0, 12), substr($raw, 12, 16), 'mailvora-link-v1');
        if (!is_string($plain) || !str_starts_with($plain, '/') || str_starts_with($plain, '//') || preg_match('/[\x00-\x1f\\\\]/', $plain)) {
            return null;
        }
        return $plain;
    }

    /**
     * Translate an incoming request. Returns the request to route (possibly
     * re-targeted to the sealed path) or a response that ends it here.
     *
     * @param \Closure(string, string): void $flash  flash a notice for the next page
     * @param \Closure(): string $home  where a signed-in visitor starts (/console or /mail)
     */
    public function resolve(Request $request, \Closure $flash, \Closure $home): Request|Response
    {
        if (!$this->enabled) {
            return $request;
        }
        $path = $request->path();
        if ($path === self::ENTRY) {
            $target = $this->open((string) $request->query('ref', ''));
            if ($target === null) {
                $flash('warning', 'That link has expired or was changed. Open the page again from the menu.');
                return Response::redirect($home());
            }
            [$targetPath, $targetQuery] = array_pad(explode('?', $target, 2), 2, '');
            parse_str($targetQuery, $sealed);
            $extra = $request->allQuery();
            unset($extra['ref']);
            // Sealed parameters cannot be overridden; extra ones (GET form
            // fields such as filters and search terms) are added.
            return $request->withTarget($targetPath, $sealed + $extra)->withAttribute('sealed_link', true);
        }
        if (!$this->isPrivate($path)) {
            return $request;
        }
        $isRead = in_array($request->method(), ['GET', 'HEAD'], true);
        if ($isRead && $this->classify($path)['static']) {
            return Response::redirect($this->seal($request->fullPath()));
        }
        if ($isRead) {
            $flash('warning', 'Pages that show a specific record can only be opened from inside the app.');
            return Response::redirect($this->seal($this->homeFor($path)));
        }
        throw HttpException::notFound();
    }

    /** Seal private addresses in an outgoing response. */
    public function rewrite(Response $response): Response
    {
        if (!$this->enabled) {
            return $response;
        }
        $location = $response->header('Location');
        if ($location !== null) {
            $response = $response->withHeader('Location', $this->rewriteLocation($location));
        }
        if (str_starts_with(strtolower((string) $response->header('Content-Type')), 'text/html') && $response->body() !== '') {
            $response = $response->withBody($this->rewriteHtml($response->body()));
        }
        return $response;
    }

    public function rewriteLocation(string $location): string
    {
        if ($this->shouldSeal($location)) {
            return $this->seal($location);
        }
        // `/login?next=/console/…`: seal the return address too.
        if (str_starts_with($location, '/') && str_contains($location, 'next=')) {
            [$path, $query] = explode('?', $location, 2) + [1 => ''];
            parse_str($query, $params);
            if (is_string($params['next'] ?? null) && $this->shouldSeal($params['next'])) {
                $params['next'] = $this->seal($params['next']);
                return $path . '?' . http_build_query($params);
            }
        }
        return $location;
    }

    public function rewriteHtml(string $html): string
    {
        // Forms first: a GET form loses the query string of its action when
        // submitted, so the token travels in a hidden field instead.
        $html = (string) preg_replace_callback('~<form\b[^>]*>~i', function (array $m): string {
            $tag = $m[0];
            if (!preg_match('~\saction="([^"]*)"~i', $tag, $a)) {
                return $tag;
            }
            $target = html_entity_decode($a[1], ENT_QUOTES | ENT_HTML5);
            if (!$this->shouldSeal($target)) {
                return $tag;
            }
            $sealed = $this->seal($target);
            if (preg_match('~\smethod="?post"?~i', $tag)) {
                return str_replace($a[0], ' action="' . htmlspecialchars($sealed, ENT_QUOTES) . '"', $tag);
            }
            $ref = substr($sealed, strlen(self::ENTRY . '?ref='));
            return str_replace($a[0], ' action="' . self::ENTRY . '"', $tag)
                . '<input type="hidden" name="ref" value="' . htmlspecialchars($ref, ENT_QUOTES) . '">';
        }, $html);

        return (string) preg_replace_callback('~(\s(?:href|src|action|formaction|data-[a-z0-9-]+))="(/[^"]*)"~i', function (array $m): string {
            $target = html_entity_decode($m[2], ENT_QUOTES | ENT_HTML5);
            return $this->shouldSeal($target) ? $m[1] . '="' . htmlspecialchars($this->seal($target), ENT_QUOTES) . '"' : $m[0];
        }, $html);
    }

    private function shouldSeal(string $target): bool
    {
        if (!str_starts_with($target, '/') || str_starts_with($target, '//') || str_starts_with($target, '/assets/')) {
            return false;
        }
        $path = (string) strtok(strtok($target, '#') ?: '/', '?');
        return $path !== self::ENTRY && $this->isPrivate($path);
    }

    /** @return array{private: bool, static: bool} */
    private function classify(string $path): array
    {
        if (isset($this->cache[$path])) {
            return $this->cache[$path];
        }
        $private = false;
        $static = false;
        foreach (['GET', 'POST'] as $method) {
            $match = $this->router->match($method, $path);
            if ($match['status'] !== 'found') {
                continue;
            }
            foreach ($match['middleware'] as $name) {
                if (in_array(explode(':', $name, 2)[0], self::PRIVATE_MIDDLEWARE, true)) {
                    $private = true;
                    $static = $static || ($method === 'GET' && $match['params'] === []);
                }
            }
        }
        return $this->cache[$path] = ['private' => $private, 'static' => $static];
    }

    private function homeFor(string $pathOrUrl): string
    {
        $path = (string) (parse_url($pathOrUrl, PHP_URL_PATH) ?: '');
        return match (true) {
            str_starts_with($path, '/admin') => '/admin',
            str_starts_with($path, '/mail') => '/mail',
            default => '/console',
        };
    }

    private function key(): string
    {
        $this->ensureKey();
        return base64_decode((string) $this->session()->get(self::SESSION_KEY));
    }
}
