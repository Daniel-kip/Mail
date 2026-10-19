<?php

declare(strict_types=1);

namespace App\Session;

/**
 * PHP native sessions with hardened settings: strict mode, HttpOnly,
 * SameSite=Lax, `__Host-` prefix over HTTPS, server-side files outside the
 * web root, idle expiry and periodic ID rotation.
 */
final class NativeSession extends Session
{
    public function __construct(
        private readonly string $name,
        private readonly bool $secure,
        private readonly string $savePath,
        private readonly int $idleMinutes,
    ) {
    }

    protected function load(): array
    {
        if (session_status() !== PHP_SESSION_ACTIVE) {
            if (!is_dir($this->savePath)) {
                @mkdir($this->savePath, 0700, true);
            }
            if (is_dir($this->savePath) && is_writable($this->savePath)) {
                session_save_path($this->savePath);
            }
            ini_set('session.use_strict_mode', '1');
            ini_set('session.use_only_cookies', '1');
            ini_set('session.use_trans_sid', '0');
            if (PHP_VERSION_ID < 80400) {
                // Deprecated (and already strong by default) from PHP 8.4.
                ini_set('session.sid_length', '48');
                ini_set('session.sid_bits_per_character', '6');
            }
            ini_set('session.gc_maxlifetime', (string) ($this->idleMinutes * 60));
            ini_set('session.cache_limiter', '');
            session_name($this->secure ? '__Host-' . $this->name : $this->name);
            session_set_cookie_params([
                'lifetime' => 0,
                'path' => '/',
                'secure' => $this->secure,
                'httponly' => true,
                'samesite' => 'Lax',
            ]);
            if (!session_start()) {
                throw new \RuntimeException('Could not start the session. Check that the session directory is writable.');
            }
        }
        $data = $_SESSION;
        $now = time();
        // Idle expiry: an abandoned browser session cannot be resumed later.
        if (isset($data['_last_seen']) && $now - (int) $data['_last_seen'] > $this->idleMinutes * 60) {
            $data = [];
            session_regenerate_id(true);
        }
        // Rotate the session id every 15 minutes to limit fixation windows.
        if (!isset($data['_rotated']) || $now - (int) $data['_rotated'] > 900) {
            session_regenerate_id(true);
            $data['_rotated'] = $now;
        }
        $data['_last_seen'] = $now;
        return $data;
    }

    protected function persist(array $data): void
    {
        if (session_status() === PHP_SESSION_ACTIVE) {
            $_SESSION = $data;
            session_write_close();
        }
    }

    public function regenerate(): void
    {
        if (session_status() === PHP_SESSION_ACTIVE) {
            session_regenerate_id(true);
            $this->data['_rotated'] = time();
        }
    }

    public function destroy(): void
    {
        $this->clear();
        if (session_status() === PHP_SESSION_ACTIVE) {
            $_SESSION = [];
            session_regenerate_id(true);
        }
    }

    /**
     * Release the session lock during slow backend reads so parallel
     * requests from the same browser are not serialized.
     */
    public function releaseLock(): void
    {
        if (session_status() === PHP_SESSION_ACTIVE) {
            $_SESSION = $this->data;
            session_write_close();
        }
    }
}
