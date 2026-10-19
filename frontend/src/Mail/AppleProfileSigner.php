<?php

declare(strict_types=1);

namespace App\Mail;

use App\Support\Logger;

/**
 * Optional CMS signing of Apple configuration profiles.
 *
 * Unsigned profiles install fine but iOS labels them "Not Verified". When
 * the operator configures a certificate (for example the web server's
 * publicly trusted certificate and key), profiles are signed as CMS
 * SignedData in DER, the format iOS expects, and show as "Verified".
 * The profile content never includes a password either way.
 */
final class AppleProfileSigner
{
    public function __construct(
        private readonly ?string $certFile,
        private readonly ?string $keyFile,
        private readonly ?string $chainFile,
        private readonly ?string $passphrase,
        private readonly Logger $logger,
    ) {
    }

    public function enabled(): bool
    {
        return $this->certFile !== null && $this->keyFile !== null && function_exists('openssl_cms_sign')
            && is_readable($this->certFile) && is_readable($this->keyFile);
    }

    /** @return array{0: string, 1: bool} body and whether it is signed */
    public function sign(string $profile): array
    {
        if (!$this->enabled()) {
            return [$profile, false];
        }
        $in = tempnam(sys_get_temp_dir(), 'mvp');
        $out = tempnam(sys_get_temp_dir(), 'mvs');
        try {
            file_put_contents($in, $profile);
            $key = $this->passphrase !== null ? ['file://' . $this->keyFile, $this->passphrase] : 'file://' . $this->keyFile;
            $ok = openssl_cms_sign(
                $in,
                $out,
                'file://' . $this->certFile,
                $key,
                [],
                OPENSSL_CMS_BINARY | OPENSSL_CMS_NOSMIMECAP,
                OPENSSL_ENCODING_DER,
                $this->chainFile !== null && is_readable($this->chainFile) ? $this->chainFile : null,
            );
            if (!$ok) {
                $this->logger->error('apple_profile_signing_failed', ['openssl' => openssl_error_string() ?: 'unknown']);
                return [$profile, false];
            }
            return [(string) file_get_contents($out), true];
        } finally {
            @unlink($in);
            @unlink($out);
        }
    }
}
