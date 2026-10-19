<?php

declare(strict_types=1);

namespace App\Support;

/**
 * Small declarative validator for form input. The Rust API is the source of
 * truth for every business rule; this layer gives fast, field-level feedback
 * for the obvious mistakes before a round trip.
 *
 *   $errors = Validator::make($input, [
 *       'email' => 'required|email|max:254',
 *       'password' => 'required|min:12',
 *   ]);
 */
final class Validator
{
    /**
     * @param array<string, mixed> $input
     * @param array<string, string> $rules
     * @param array<string, string> $labels
     * @return array<string, string> field => first error message
     */
    public static function make(array $input, array $rules, array $labels = []): array
    {
        $errors = [];
        foreach ($rules as $field => $spec) {
            $value = $input[$field] ?? null;
            $label = $labels[$field] ?? ucfirst(str_replace('_', ' ', $field));
            $string = is_scalar($value) ? trim((string) $value) : '';
            $present = $string !== '' || (is_array($value) && $value !== []);
            foreach (explode('|', $spec) as $rule) {
                [$name, $arg] = array_pad(explode(':', $rule, 2), 2, null);
                $message = match ($name) {
                    'required' => $present ? null : "{$label} is required.",
                    'email' => !$present || filter_var($string, FILTER_VALIDATE_EMAIL) ? null : "Enter a valid email address.",
                    'min' => !$present || mb_strlen($string) >= (int) $arg ? null : "{$label} must be at least {$arg} characters.",
                    'max' => !$present || mb_strlen($string) <= (int) $arg ? null : "{$label} must be at most {$arg} characters.",
                    'in' => !$present || in_array($string, explode(',', (string) $arg), true) ? null : "Choose a valid {$label}.",
                    'url' => !$present || (filter_var($string, FILTER_VALIDATE_URL) && preg_match('~^https?://~i', $string)) ? null : "{$label} must be an http(s) URL.",
                    'https' => !$present || str_starts_with(strtolower($string), 'https://') ? null : "{$label} must start with https://.",
                    'domain' => !$present || preg_match('/^(?=.{3,253}$)([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/i', $string) ? null : "Enter a domain name such as example.org.",
                    'integer' => !$present || preg_match('/^-?\d+$/', $string) ? null : "{$label} must be a whole number.",
                    'same' => $string === trim((string) ($input[$arg] ?? '')) ? null : "{$label} does not match.",
                    'localpart' => !$present || (preg_match('/^[a-z0-9]([a-z0-9._+-]{0,62}[a-z0-9])?$/i', $string) && !str_contains($string, '..')) ? null : "Use letters, numbers, dots, hyphens or underscores (for example amina.odhiambo).",
                    'date' => !$present || \DateTimeImmutable::createFromFormat('!Y-m-d', $string) !== false ? null : "{$label} must be a date (YYYY-MM-DD).",
                    'nullable', '' => null,
                    default => throw new \InvalidArgumentException("Unknown validation rule {$name}"),
                };
                if ($message !== null) {
                    $errors[$field] = $message;
                    break;
                }
            }
        }
        return $errors;
    }
}
