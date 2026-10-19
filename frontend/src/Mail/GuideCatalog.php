<?php

declare(strict_types=1);

namespace App\Mail;

/**
 * Mail-app setup guides (resources/guides/mail-clients.php) combined with
 * the mailbox's validated connection settings from the platform API.
 */
final class GuideCatalog
{
    /** @var array<string, mixed> */
    private array $data;

    public function __construct(string $file)
    {
        $data = require $file;
        if (!is_array($data)) {
            throw new \RuntimeException('Invalid guide catalogue');
        }
        $this->data = $data;
    }

    public function reviewed(): string
    {
        return (string) $this->data['reviewed'];
    }

    /** @return array<string, array{label: string, icon: string, hint: string}> */
    public function devices(): array
    {
        return $this->data['devices'];
    }

    public function hasDevice(string $device): bool
    {
        return isset($this->data['devices'][$device]);
    }

    /** @return array<string, array{label: string, icon: string, hint: string}> apps offered for a device */
    public function appsFor(string $device): array
    {
        $apps = [];
        foreach ($this->data['device_apps'][$device] ?? [] as $key) {
            $apps[$key] = $this->data['apps'][$key];
        }
        return $apps;
    }

    public function app(string $key): ?array
    {
        return $this->data['apps'][$key] ?? null;
    }

    /** Guide id for a device/app pair, or null when the pair is not offered. */
    public function guideFor(string $device, string $app): ?string
    {
        if (!in_array($app, $this->data['device_apps'][$device] ?? [], true)) {
            return null;
        }
        if ($app === 'webmail') {
            return 'any-webmail';
        }
        $specific = $device . '-' . $app;
        return isset($this->data['guides'][$specific]) ? $specific : 'any-other';
    }

    public function guide(string $id): ?array
    {
        $guide = $this->data['guides'][$id] ?? null;
        return is_array($guide) ? $guide + ['id' => $id] : null;
    }

    /** @return array<string, array{title: string, cause: string, steps: list<string>, support: string}> */
    public function helpTopics(): array
    {
        return $this->data['help'];
    }

    public function help(string $topic): ?array
    {
        return $this->data['help'][$topic] ?? null;
    }

    /**
     * Placeholder values from the API's setup payload.
     *
     * @return array<string, string>
     */
    public static function placeholders(array $setup): array
    {
        $s = $setup['settings'] ?? [];
        $alt = $s['smtp_alternative'] ?? null;
        return [
            '{address}' => (string) ($setup['mailbox']['address'] ?? ''),
            '{org}' => (string) ($setup['mailbox']['organization'] ?? 'your institution'),
            '{imap_host}' => (string) ($s['imap']['host'] ?? ''),
            '{imap_port}' => (string) ($s['imap']['port'] ?? ''),
            '{smtp_host}' => (string) ($s['smtp']['host'] ?? ''),
            '{smtp_port}' => (string) ($s['smtp']['port'] ?? ''),
            '{smtp_security}' => (string) ($s['smtp']['security'] ?? ''),
            '{smtp_alt}' => is_array($alt) ? 'port ' . $alt['port'] . ' with ' . $alt['security'] : 'none is offered',
            '{webmail_url}' => (string) ($setup['webmail_url'] ?? ''),
        ];
    }

    public static function fill(string $text, array $placeholders): string
    {
        return strtr($text, $placeholders);
    }

    /**
     * Copyable settings rows: key => [label, value, help].
     *
     * @return array<string, array{0: string, 1: string, 2: string}>
     */
    public static function fields(array $setup): array
    {
        $s = $setup['settings'] ?? [];
        $alt = $s['smtp_alternative'] ?? null;
        $fields = [
            'username' => ['User name', (string) ($s['username'] ?? ''), 'Always your full email address'],
            'imap_host' => ['Incoming server (IMAP)', (string) ($s['imap']['host'] ?? ''), 'Where your mail is stored'],
            'imap_port' => ['Incoming port', (string) ($s['imap']['port'] ?? ''), 'Use exactly this number'],
            'imap_security' => ['Incoming security', (string) ($s['imap']['security'] ?? 'SSL/TLS'), 'Encryption for reading mail'],
            'smtp_host' => ['Outgoing server (SMTP)', (string) ($s['smtp']['host'] ?? ''), 'Sends your messages'],
            'smtp_port' => ['Outgoing port', (string) ($s['smtp']['port'] ?? ''), 'Use exactly this number'],
            'smtp_security' => ['Outgoing security', (string) ($s['smtp']['security'] ?? ''), 'Encryption for sending mail'],
            'auth' => ['Sign-in method', 'Normal password', 'Outgoing mail always requires sign-in'],
            'webmail' => ['Webmail address', (string) ($setup['webmail_url'] ?? ''), 'Read mail in any browser'],
        ];
        if (is_array($alt)) {
            $fields['smtp_alt'] = ['Alternative outgoing port', $alt['port'] . ' (' . $alt['security'] . ')', 'Only if the main outgoing port does not work on your network'];
        }
        return $fields;
    }
}
