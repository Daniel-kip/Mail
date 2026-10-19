<?php

declare(strict_types=1);

namespace App\Api;

/**
 * Contract for talking to the platform API. Implemented over HTTP in
 * production and by an in-memory fake in tests.
 *
 * Options:
 *  - kind:      'console' | 'webmail' | 'public' (which backend session to use)
 *  - json:      request body encoded as JSON
 *  - query:     query-string parameters
 *  - multipart: ['fields' => [name => value|list], 'files' => [[field, path, filename, type], ...]]
 *  - headers:   extra request headers (name => value)
 *  - timeout:   seconds
 */
interface ApiClient
{
    public function request(string $method, string $path, array $options = []): ApiResponse;

    /** Forget the stored backend session of one kind (sign-out, 401). */
    public function forget(string $kind): void;

    public function hasSession(string $kind): bool;
}
