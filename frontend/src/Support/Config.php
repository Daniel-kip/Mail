<?php

declare(strict_types=1);

namespace App\Support;

/**
 * Read-only configuration with dot-notation access (`api.url`).
 */
final class Config
{
    /** @param array<string, mixed> $items */
    public function __construct(private readonly array $items)
    {
    }

    public static function fromFile(string $file): self
    {
        $items = require $file;
        if (!is_array($items)) {
            throw new \RuntimeException("Configuration file {$file} must return an array.");
        }
        return new self($items);
    }

    public function get(string $key, mixed $default = null): mixed
    {
        $value = $this->items;
        foreach (explode('.', $key) as $segment) {
            if (!is_array($value) || !array_key_exists($segment, $value)) {
                return $default;
            }
            $value = $value[$segment];
        }
        return $value;
    }

    public function string(string $key, string $default = ''): string
    {
        $value = $this->get($key, $default);
        return is_scalar($value) ? (string) $value : $default;
    }

    public function bool(string $key, bool $default = false): bool
    {
        return (bool) $this->get($key, $default);
    }

    public function int(string $key, int $default = 0): int
    {
        return (int) $this->get($key, $default);
    }

    /** @return array<string, mixed> */
    public function all(): array
    {
        return $this->items;
    }

    /** A copy with some values replaced (used by tests). */
    public function with(array $overrides): self
    {
        return new self(array_replace_recursive($this->items, $overrides));
    }
}
