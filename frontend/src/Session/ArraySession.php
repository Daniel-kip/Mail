<?php

declare(strict_types=1);

namespace App\Session;

/**
 * In-memory session for tests and CLI use. Persisted state can be carried
 * across simulated requests with `snapshot()` / a new instance.
 */
final class ArraySession extends Session
{
    public bool $regenerated = false;

    /** @param array<string, mixed> $initial */
    public function __construct(private array $stored = [])
    {
    }

    protected function load(): array
    {
        return $this->stored;
    }

    protected function persist(array $data): void
    {
        $this->stored = $data;
    }

    public function regenerate(): void
    {
        $this->regenerated = true;
    }

    public function destroy(): void
    {
        $this->clear();
        $this->stored = [];
    }

    /** @return array<string, mixed> state as it would be stored */
    public function snapshot(): array
    {
        return $this->stored;
    }
}
