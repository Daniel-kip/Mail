<?php

declare(strict_types=1);

namespace App\Support;

/**
 * Minimal `.env` loader. Real environment variables always win, so the same
 * code runs unchanged under Docker, systemd or `php -S`.
 */
final class Env
{
    public static function load(string $file): void
    {
        if (!is_file($file) || !is_readable($file)) {
            return;
        }
        foreach (file($file, FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES) ?: [] as $line) {
            $line = trim($line);
            if ($line === '' || str_starts_with($line, '#') || !str_contains($line, '=')) {
                continue;
            }
            [$key, $value] = array_map('trim', explode('=', $line, 2));
            if (!preg_match('/^[A-Z][A-Z0-9_]*$/', $key) || getenv($key) !== false) {
                continue;
            }
            // Strip an inline comment that follows whitespace (VALUE # note).
            if (!str_starts_with($value, '"') && !str_starts_with($value, "'")) {
                $value = trim((string) preg_replace('/\s+#.*$/', '', $value));
            }
            if (strlen($value) >= 2 && ($value[0] === '"' || $value[0] === "'") && str_ends_with($value, $value[0])) {
                $value = substr($value, 1, -1);
            }
            putenv($key . '=' . $value);
            $_ENV[$key] = $value;
        }
    }

    public static function get(string $key, ?string $default = null): ?string
    {
        $value = getenv($key);
        if ($value === false || $value === '') {
            return $default;
        }
        return $value;
    }

    public static function bool(string $key, bool $default): bool
    {
        $value = self::get($key);
        if ($value === null) {
            return $default;
        }
        $parsed = filter_var($value, FILTER_VALIDATE_BOOL, FILTER_NULL_ON_FAILURE);
        if ($parsed === null) {
            throw new \RuntimeException("{$key} must be a boolean (true/false), got '{$value}'.");
        }
        return $parsed;
    }

    public static function int(string $key, int $default, int $min = PHP_INT_MIN, int $max = PHP_INT_MAX): int
    {
        $value = self::get($key);
        if ($value === null) {
            return $default;
        }
        if (!preg_match('/^-?\d+$/', $value) || (int) $value < $min || (int) $value > $max) {
            throw new \RuntimeException("{$key} must be an integer between {$min} and {$max}.");
        }
        return (int) $value;
    }
}
