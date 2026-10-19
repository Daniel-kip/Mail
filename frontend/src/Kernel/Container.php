<?php

declare(strict_types=1);

namespace App\Kernel;

/**
 * Minimal service container: lazily built shared services that tests can
 * replace (for example the API client with an in-memory fake).
 */
final class Container
{
    /** @var array<string, \Closure> */
    private array $factories = [];
    /** @var array<string, mixed> */
    private array $instances = [];

    public function set(string $id, \Closure $factory): void
    {
        $this->factories[$id] = $factory;
        unset($this->instances[$id]);
    }

    public function instance(string $id, mixed $value): void
    {
        $this->instances[$id] = $value;
    }

    public function get(string $id): mixed
    {
        if (array_key_exists($id, $this->instances)) {
            return $this->instances[$id];
        }
        $factory = $this->factories[$id] ?? throw new \RuntimeException("Service {$id} is not registered");
        return $this->instances[$id] = $factory($this);
    }

    public function has(string $id): bool
    {
        return isset($this->factories[$id]) || array_key_exists($id, $this->instances);
    }
}
