<?php

declare(strict_types=1);

namespace App\Api;

/**
 * A response from the platform API. Non-2xx responses are values, not
 * exceptions: controllers decide how to present each error code.
 */
final class ApiResponse
{
    private ?array $decoded = null;

    /** @param array<string, list<string>> $headers lower-case names */
    public function __construct(
        public readonly int $status,
        public readonly string $body,
        public readonly array $headers = [],
    ) {
    }

    public static function fake(int $status, array $json = [], array $headers = []): self
    {
        $headers += ['content-type' => ['application/json']];
        return new self($status, (string) json_encode($json), $headers);
    }

    public function ok(): bool
    {
        return $this->status >= 200 && $this->status < 300;
    }

    /** Decoded JSON body (empty array for non-JSON bodies). */
    public function json(): array
    {
        if ($this->decoded === null) {
            $decoded = json_decode($this->body, true);
            $this->decoded = is_array($decoded) ? $decoded : [];
        }
        return $this->decoded;
    }

    public function get(string $key, mixed $default = null): mixed
    {
        $value = $this->json();
        foreach (explode('.', $key) as $segment) {
            if (!is_array($value) || !array_key_exists($segment, $value)) {
                return $default;
            }
            $value = $value[$segment];
        }
        return $value;
    }

    /** @return list<array<string, mixed>> the `data` array of list endpoints */
    public function items(): array
    {
        $data = $this->json()['data'] ?? [];
        return is_array($data) ? array_values($data) : [];
    }

    public function errorCode(): string
    {
        return (string) ($this->json()['error']['code'] ?? ($this->ok() ? '' : 'http_' . $this->status));
    }

    public function errorMessage(string $fallback = 'Something went wrong. Please try again.'): string
    {
        $message = $this->json()['error']['message'] ?? null;
        return is_string($message) && $message !== '' ? $message : $fallback;
    }

    public function errorDetails(): array
    {
        $details = $this->json()['error']['details'] ?? [];
        return is_array($details) ? $details : [];
    }

    public function requestId(): ?string
    {
        $id = $this->json()['error']['request_id'] ?? ($this->headers['x-request-id'][0] ?? null);
        return is_string($id) ? $id : null;
    }

    public function header(string $name): ?string
    {
        return $this->headers[strtolower($name)][0] ?? null;
    }
}
