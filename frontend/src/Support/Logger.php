<?php

declare(strict_types=1);

namespace App\Support;

/**
 * Structured JSON-lines logger. Never pass passwords, tokens, cookies,
 * API keys or message bodies in the context: the caller is responsible, and
 * `sanitize()` removes the most common sensitive keys as a safety net.
 */
final class Logger
{
    private const SENSITIVE = ['password', 'token', 'secret', 'cookie', 'authorization', 'key', 'csrf', 'code'];

    public function __construct(private readonly ?string $file, private readonly bool $stderr = false)
    {
    }

    public function info(string $message, array $context = []): void
    {
        $this->write('info', $message, $context);
    }

    public function warning(string $message, array $context = []): void
    {
        $this->write('warning', $message, $context);
    }

    public function error(string $message, array $context = []): void
    {
        $this->write('error', $message, $context);
    }

    private function write(string $level, string $message, array $context): void
    {
        $line = json_encode([
            'ts' => gmdate('Y-m-d\TH:i:s\Z'),
            'level' => $level,
            'msg' => $message,
            'ctx' => self::sanitize($context),
        ], JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE) . "\n";

        if ($this->file !== null) {
            $dir = dirname($this->file);
            if (is_dir($dir) || @mkdir($dir, 0750, true)) {
                @file_put_contents($this->file, $line, FILE_APPEND | LOCK_EX);
                return;
            }
        }
        if ($this->stderr || $this->file === null) {
            error_log(rtrim($line));
        }
    }

    public static function sanitize(array $context): array
    {
        $clean = [];
        foreach ($context as $key => $value) {
            $lower = strtolower((string) $key);
            foreach (self::SENSITIVE as $needle) {
                if (str_contains($lower, $needle)) {
                    $clean[$key] = '[redacted]';
                    continue 2;
                }
            }
            $clean[$key] = is_array($value) ? self::sanitize($value) : $value;
        }
        return $clean;
    }
}
