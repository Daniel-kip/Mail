<?php

declare(strict_types=1);

use App\Support\Env;

/*
 * Application configuration. Every value comes from the environment (or
 * `.env` in development) and is validated here, so a misconfigured
 * deployment fails at boot with a clear message instead of at first use.
 */

$environment = Env::get('APP_ENV', 'development');
if (!in_array($environment, ['development', 'production', 'testing'], true)) {
    throw new RuntimeException('APP_ENV must be development, production or testing.');
}
$production = $environment === 'production';

$origin = static function (string $key, string $default) use ($production): string {
    $value = rtrim((string) Env::get($key, $default), '/');
    $parts = parse_url($value);
    if ($parts === false || !in_array($parts['scheme'] ?? '', ['http', 'https'], true) || empty($parts['host'])
        || isset($parts['user']) || isset($parts['pass']) || isset($parts['query']) || isset($parts['fragment'])) {
        throw new RuntimeException("{$key} must be an http(s) origin without credentials, query or fragment.");
    }
    return $value;
};

$appUrl = $origin('APP_URL', 'http://127.0.0.1:8080');
$apiUrl = $origin('API_URL', Env::get('MAILVORA_API_URL', 'http://127.0.0.1:8088'));
$publicApiUrl = $origin('PUBLIC_API_URL', Env::get('MAILVORA_PUBLIC_API_URL', $apiUrl));

if ($production) {
    $problems = [];
    if (!str_starts_with($appUrl, 'https://')) {
        $problems[] = 'APP_URL must use https://';
    }
    if (!str_starts_with($publicApiUrl, 'https://')) {
        $problems[] = 'PUBLIC_API_URL must use https://';
    }
    if (Env::bool('APP_DEBUG', false)) {
        $problems[] = 'APP_DEBUG must be false';
    }
    if (!Env::bool('SESSION_SECURE', true)) {
        $problems[] = 'SESSION_SECURE must be true';
    }
    if ($problems !== []) {
        throw new RuntimeException("Refusing to start in production:\n - " . implode("\n - ", $problems));
    }
}

$root = dirname(__DIR__);
$storage = rtrim((string) Env::get('STORAGE_PATH', $root . '/storage'), '/\\');

return [
    'name' => Env::get('PRODUCT_NAME', 'Mailvora'),
    'tagline' => Env::get('PRODUCT_TAGLINE', 'Email infrastructure for developers and institutions'),
    'env' => $environment,
    'debug' => Env::bool('APP_DEBUG', !$production),
    'url' => $appUrl,
    'support_email' => Env::get('SUPPORT_EMAIL'),
    'company' => [
        'name' => Env::get('COMPANY_NAME', Env::get('PRODUCT_NAME', 'Mailvora')),
        'address' => Env::get('COMPANY_ADDRESS'),
        'country' => Env::get('COMPANY_COUNTRY', 'Kenya'),
    ],
    'api' => [
        // Private address the PHP server uses to reach the Rust API.
        'url' => $apiUrl,
        // Address shown to developers in documentation and examples.
        'public_url' => $publicApiUrl,
        'connect_timeout' => Env::int('API_CONNECT_TIMEOUT', 5, 1, 60),
        'timeout' => Env::int('API_TIMEOUT', 40, 5, 300),
        'verify_tls' => Env::bool('API_VERIFY_TLS', true),
    ],
    'session' => [
        'name' => Env::get('SESSION_NAME', 'mv_session'),
        'secure' => Env::bool('SESSION_SECURE', $production),
        'idle_minutes' => Env::int('SESSION_IDLE_MINUTES', 120, 5, 1440),
        'path' => Env::get('SESSION_PATH', $storage . '/sessions'),
    ],
    'storage' => $storage,
    'log_file' => Env::get('LOG_FILE', $storage . '/logs/app.log'),
    'trust_proxy' => Env::bool('TRUST_PROXY', $production),
    'apple_profile' => [
        // Optional: sign .mobileconfig files so iOS shows them as verified.
        'cert' => Env::get('APPLE_PROFILE_SIGNING_CERT'),
        'key' => Env::get('APPLE_PROFILE_SIGNING_KEY'),
        'chain' => Env::get('APPLE_PROFILE_SIGNING_CHAIN'),
        'key_passphrase' => Env::get('APPLE_PROFILE_SIGNING_KEY_PASSPHRASE'),
    ],
    'signup_enabled' => Env::bool('SIGNUP_ENABLED', true),
    // Opaque, session-bound addresses for every signed-in page (see App\Http\Links).
    // Always on in production; development may switch it off to debug routes.
    'sealed_links' => $production || Env::bool('SEALED_LINKS', true),
    'mail' => [
        // The platform's SPF include (same value as the API's SPF_INCLUDE), offered in the record generator.
        'spf_include' => Env::get('SPF_INCLUDE', ''),
    ],
];
