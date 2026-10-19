<?php

declare(strict_types=1);

namespace App\Session;

/**
 * Session facade over a storage backend. Flash values written during a
 * request are readable during the *next* request only (post/redirect/get).
 */
abstract class Session
{
    /** @var array<string, mixed> */
    protected array $data = [];
    /** @var array<string, mixed> flash values visible in this request */
    private array $flashNow = [];
    private bool $started = false;

    abstract protected function load(): array;

    abstract protected function persist(array $data): void;

    abstract public function regenerate(): void;

    abstract public function destroy(): void;

    public function start(): void
    {
        if ($this->started) {
            return;
        }
        $this->data = $this->load();
        $this->flashNow = $this->data['_flash'] ?? [];
        unset($this->data['_flash']);
        $this->started = true;
    }

    public function save(): void
    {
        if ($this->started) {
            $this->persist($this->data);
        }
    }

    public function get(string $key, mixed $default = null): mixed
    {
        return $this->data[$key] ?? $default;
    }

    public function put(string $key, mixed $value): void
    {
        $this->data[$key] = $value;
    }

    public function has(string $key): bool
    {
        return isset($this->data[$key]);
    }

    public function forget(string ...$keys): void
    {
        foreach ($keys as $key) {
            unset($this->data[$key]);
        }
    }

    public function pull(string $key, mixed $default = null): mixed
    {
        $value = $this->data[$key] ?? $default;
        unset($this->data[$key]);
        return $value;
    }

    /** Make a value available on the next request. */
    public function flash(string $key, mixed $value): void
    {
        $this->data['_flash'][$key] = $value;
    }

    /** Read a flash value set by the previous request. */
    public function flashed(string $key, mixed $default = null): mixed
    {
        return $this->flashNow[$key] ?? $default;
    }

    /** Keep this request's flash values for one more request. */
    public function reflash(): void
    {
        foreach ($this->flashNow as $key => $value) {
            // Values flashed during this request win over older ones.
            $this->data['_flash'][$key] ??= $value;
        }
    }

    /** Clear everything (sign-out) but keep a usable session. */
    public function clear(): void
    {
        $this->data = [];
        $this->flashNow = [];
    }

    public function csrfToken(): string
    {
        if (empty($this->data['_csrf']) || !is_string($this->data['_csrf'])) {
            $this->data['_csrf'] = bin2hex(random_bytes(32));
        }
        return $this->data['_csrf'];
    }

    /** @return array<string, mixed> */
    public function all(): array
    {
        return $this->data;
    }
}
