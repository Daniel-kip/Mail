<?php

declare(strict_types=1);

namespace App\Api;

use App\Session\Session;
use App\Support\Logger;

/**
 * cURL client for the Rust API ("backend for frontend" pattern).
 *
 * The browser only holds this application's own session cookie. The API's
 * session cookies and CSRF tokens (one pair for the console, one for
 * webmail) are kept server-side in the PHP session and replayed here, so a
 * script in the page can never read or exfiltrate them.
 */
final class HttpApiClient implements ApiClient
{
    private const COOKIE_KIND = [
        'mv_session' => 'console',
        '__Host-mv_session' => 'console',
        'mv_mail' => 'webmail',
        '__Host-mv_mail' => 'webmail',
    ];

    public function __construct(
        private readonly string $baseUrl,
        private readonly Session $session,
        private readonly Logger $logger,
        private readonly int $connectTimeout = 5,
        private readonly int $timeout = 40,
        private readonly bool $verifyTls = true,
        private readonly ?string $clientIp = null,
    ) {
    }

    public function request(string $method, string $path, array $options = []): ApiResponse
    {
        if (!function_exists('curl_init')) {
            throw new ApiException('The PHP cURL extension is required to reach the platform API.', 'local');
        }
        if (!str_starts_with($path, '/') || str_contains($path, '..') || preg_match('/[\r\n\0]/', $path)) {
            throw new \InvalidArgumentException('Invalid API path');
        }
        $kind = $options['kind'] ?? 'console';
        $requestId = 'web_' . bin2hex(random_bytes(10));
        $url = $this->baseUrl . $path;
        if (!empty($options['query'])) {
            $query = array_filter($options['query'], static fn ($v) => $v !== null && $v !== '');
            if ($query !== []) {
                $url .= '?' . http_build_query($query);
            }
        }

        $headers = ['Accept: application/json', 'X-Request-Id: ' . $requestId];
        if ($this->clientIp !== null && filter_var($this->clientIp, FILTER_VALIDATE_IP) !== false) {
            $headers[] = 'X-Real-IP: ' . $this->clientIp;
        }
        if ($kind !== 'public') {
            $stored = $this->session->get('_api', [])[$kind] ?? null;
            if (is_array($stored) && isset($stored['cookie_name'], $stored['cookie'])) {
                $headers[] = 'Cookie: ' . $stored['cookie_name'] . '=' . $stored['cookie'];
                if (!in_array($method, ['GET', 'HEAD'], true) && !empty($stored['csrf'])) {
                    $headers[] = 'X-CSRF-Token: ' . $stored['csrf'];
                }
            }
        }
        foreach ($options['headers'] ?? [] as $name => $value) {
            if (preg_match('/^[A-Za-z0-9-]{1,64}$/', (string) $name) && !preg_match('/[\r\n]/', (string) $value)) {
                $headers[] = $name . ': ' . $value;
            }
        }

        $body = null;
        if (array_key_exists('json', $options)) {
            $body = json_encode($options['json'], JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE);
            $headers[] = 'Content-Type: application/json';
        } elseif (isset($options['multipart'])) {
            [$body, $contentType] = self::multipart($options['multipart']);
            $headers[] = 'Content-Type: ' . $contentType;
        }

        $responseHeaders = [];
        $curl = curl_init($url);
        curl_setopt_array($curl, [
            CURLOPT_RETURNTRANSFER => true,
            CURLOPT_FOLLOWLOCATION => false,
            CURLOPT_CONNECTTIMEOUT => $this->connectTimeout,
            CURLOPT_TIMEOUT => (int) ($options['timeout'] ?? $this->timeout),
            CURLOPT_CUSTOMREQUEST => $method,
            CURLOPT_HTTPHEADER => $headers,
            CURLOPT_SSL_VERIFYPEER => $this->verifyTls,
            CURLOPT_SSL_VERIFYHOST => $this->verifyTls ? 2 : 0,
            CURLOPT_PROTOCOLS => CURLPROTO_HTTP | CURLPROTO_HTTPS,
            CURLOPT_HEADERFUNCTION => static function ($handle, string $line) use (&$responseHeaders): int {
                if (str_contains($line, ':')) {
                    [$name, $value] = explode(':', $line, 2);
                    $responseHeaders[strtolower(trim($name))][] = trim($value);
                }
                return strlen($line);
            },
        ]);
        if ($body !== null) {
            curl_setopt($curl, CURLOPT_POSTFIELDS, $body);
        }
        $started = microtime(true);
        $result = curl_exec($curl);
        $errno = curl_errno($curl);
        $status = (int) curl_getinfo($curl, CURLINFO_RESPONSE_CODE);
        unset($curl); // handles are freed automatically (curl_close is a no-op since PHP 8.0)

        if ($result === false) {
            $this->logger->error('api_unreachable', ['request_id' => $requestId, 'path' => $path, 'curl_errno' => $errno]);
            throw new ApiException('The email platform is temporarily unavailable. Please try again in a moment.', $requestId);
        }
        $elapsed = (int) ((microtime(true) - $started) * 1000);
        if ($elapsed > 3000) {
            $this->logger->warning('api_slow', ['request_id' => $requestId, 'path' => $path, 'ms' => $elapsed]);
        }

        $response = new ApiResponse($status, (string) $result, $responseHeaders);
        if ($kind !== 'public') {
            $this->captureSession($kind, $response);
        }
        return $response;
    }

    private function captureSession(string $kind, ApiResponse $response): void
    {
        $api = $this->session->get('_api', []);
        foreach ($response->headers['set-cookie'] ?? [] as $cookie) {
            $pair = trim(explode(';', $cookie, 2)[0]);
            if (!str_contains($pair, '=')) {
                continue;
            }
            [$name, $value] = array_map('trim', explode('=', $pair, 2));
            if ((self::COOKIE_KIND[$name] ?? null) !== $kind) {
                continue;
            }
            if ($value === '' || preg_match('/max-age\s*=\s*0/i', $cookie)) {
                unset($api[$kind]);
            } elseif (preg_match('/^[A-Za-z0-9_-]{20,200}$/', $value)) {
                $api[$kind] = ['cookie_name' => $name, 'cookie' => $value, 'csrf' => $api[$kind]['csrf'] ?? null];
            }
        }
        $csrf = $response->ok() ? ($response->json()['csrf_token'] ?? null) : null;
        if (is_string($csrf) && isset($api[$kind])) {
            $api[$kind]['csrf'] = $csrf;
        }
        if ($response->status === 401 && isset($api[$kind])) {
            unset($api[$kind]);
        }
        $this->session->put('_api', $api);
    }

    public function forget(string $kind): void
    {
        $api = $this->session->get('_api', []);
        unset($api[$kind]);
        $this->session->put('_api', $api);
    }

    public function hasSession(string $kind): bool
    {
        return isset($this->session->get('_api', [])[$kind]['cookie']);
    }

    /**
     * Encode multipart/form-data by hand: the API expects repeated part
     * names (several `attachment` parts), which cURL's array form cannot
     * express.
     *
     * @return array{0: string, 1: string} body and content type
     */
    public static function multipart(array $spec): array
    {
        $boundary = 'mv-' . bin2hex(random_bytes(16));
        $clean = static fn (string $s): string => str_replace(['"', "\r", "\n", '\\'], ['_', '', '', '_'], $s);
        $body = '';
        foreach ($spec['fields'] ?? [] as $name => $value) {
            foreach ((array) $value as $item) {
                $body .= "--{$boundary}\r\nContent-Disposition: form-data; name=\"" . $clean((string) $name) . "\"\r\n\r\n" . $item . "\r\n";
            }
        }
        foreach ($spec['files'] ?? [] as $file) {
            [$field, $path, $filename, $type] = $file;
            $data = is_file($path) ? file_get_contents($path) : false;
            if ($data === false) {
                throw new \RuntimeException('An uploaded file could not be read.');
            }
            $type = preg_match('~^[A-Za-z0-9.+-]+/[A-Za-z0-9.+-]+$~', (string) $type) ? $type : 'application/octet-stream';
            $body .= "--{$boundary}\r\nContent-Disposition: form-data; name=\"" . $clean($field) . '"; filename="' . $clean(basename((string) $filename) ?: 'attachment') . "\"\r\n"
                . "Content-Type: {$type}\r\n\r\n" . $data . "\r\n";
        }
        $body .= "--{$boundary}--\r\n";
        return [$body, 'multipart/form-data; boundary=' . $boundary];
    }
}
