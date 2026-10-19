<?php

declare(strict_types=1);

/*
 * Template helpers. Every helper that outputs markup escapes its inputs;
 * `e()` must be used for everything else a template prints.
 */

use App\Kernel\Application;
use App\View\Format;
use App\View\Icons;

function app(): Application
{
    return Application::current();
}

function e(mixed $value): string
{
    if ($value === null || is_bool($value)) {
        return '';
    }
    return htmlspecialchars(is_scalar($value) ? (string) $value : json_encode($value), ENT_QUOTES | ENT_SUBSTITUTE | ENT_HTML5, 'UTF-8');
}

/** Local URL with an optional query string. */
function url(string $path, array $query = []): string
{
    $query = array_filter($query, static fn ($v) => $v !== null && $v !== '' && $v !== false);
    return $path . ($query !== [] ? '?' . http_build_query($query) : '');
}

function route(string $name, array $params = [], array $query = []): string
{
    return app()->router->url($name, $params, $query);
}

/** Static asset URL with a content-based cache buster. */
function asset(string $path): string
{
    $path = ltrim($path, '/');
    $file = app()->root . '/public/assets/' . $path;
    $version = is_file($file) ? substr(md5((string) filemtime($file) . filesize($file)), 0, 10) : '0';
    return '/assets/' . $path . '?v=' . $version;
}

function csrf_field(): string
{
    return '<input type="hidden" name="_token" value="' . e(app()->session()->csrfToken()) . '">';
}

function method_field(string $method): string
{
    return '<input type="hidden" name="_method" value="' . e(strtoupper($method)) . '">';
}

/** Previously submitted value after a failed form, or the default. */
function old(string $field, mixed $default = ''): mixed
{
    $old = app()->view()->shared('old', []);
    return array_key_exists($field, $old) ? $old[$field] : $default;
}

function field_error(string $field): ?string
{
    $errors = app()->view()->shared('errors', []);
    return isset($errors[$field]) ? (string) $errors[$field] : null;
}

/** Attributes for an input with an error: aria-invalid + aria-describedby. */
function error_attrs(string $field): string
{
    return field_error($field) !== null ? ' aria-invalid="true" aria-describedby="' . e($field) . '-error"' : '';
}

function error_text(string $field): string
{
    $message = field_error($field);
    return $message === null ? '' : '<p class="field-error" id="' . e($field) . '-error">' . icon('alert-circle', 'icon-sm') . e($message) . '</p>';
}

function icon(string $name, string $class = '', ?string $label = null): string
{
    return Icons::svg($name, $class, $label);
}

/** Session payload of the signed-in console user (or empty). */
function auth(): array
{
    return (array) app()->view()->shared('auth', []);
}

function can(string $permission): bool
{
    return in_array($permission, auth()['organization']['permissions'] ?? [], true);
}

function is_institution(): bool
{
    return (auth()['organization']['account_type'] ?? 'institution') === 'institution';
}

/** `is-active` when the current path is (or is under) $prefix. */
function nav_active(string $prefix, bool $exact = false): string
{
    $path = app()->request()?->path() ?? '/';
    $match = $exact ? $path === $prefix : ($path === $prefix || str_starts_with($path, rtrim($prefix, '/') . '/'));
    return $match ? ' is-active" aria-current="page' : '';
}

function bytes(int|float|null $n): string
{
    return Format::bytes($n);
}

function money(int|float|null $cents, string $currency): string
{
    return Format::money($cents, $currency);
}

function date_short(?string $iso): string
{
    return Format::date($iso);
}

function datetime(?string $iso): string
{
    return Format::datetime($iso);
}

function ago(?string $iso): string
{
    return Format::ago($iso);
}

/** <time> element with a relative label and the exact time on hover. */
function time_tag(?string $iso, bool $relative = true): string
{
    if ($iso === null || $iso === '') {
        return '<span class="muted">—</span>';
    }
    return '<time datetime="' . e($iso) . '" title="' . e(Format::datetime($iso)) . '">' . e($relative ? Format::ago($iso) : Format::datetime($iso)) . '</time>';
}

function number(int|float|null $n): string
{
    return Format::number($n);
}

function plural(int $n, string $singular, ?string $plural = null): string
{
    return number($n) . ' ' . ($n === 1 ? $singular : ($plural ?? $singular . 's'));
}

/** Status pill: colour plus icon plus text, never colour alone. */
function badge(?string $status, ?string $label = null): string
{
    return Format::badge((string) $status, $label);
}

function initials(?string $name): string
{
    $name = trim((string) $name);
    if ($name === '') {
        return '?';
    }
    $parts = preg_split('/[\s@._-]+/', $name) ?: [$name];
    $letters = mb_substr($parts[0], 0, 1) . (isset($parts[1]) ? mb_substr($parts[1], 0, 1) : '');
    return mb_strtoupper($letters);
}

/** Selected/checked helpers for form controls. */
function selected(mixed $a, mixed $b): string
{
    return (string) $a === (string) $b ? ' selected' : '';
}

function checked(bool $condition): string
{
    return $condition ? ' checked' : '';
}
