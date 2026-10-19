<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Controllers\Controller;
use App\Http\Request;

/**
 * Shared helpers for signed-in webmail pages.
 */
abstract class WebmailController extends Controller
{
    /** @return list<array<string, mixed>> */
    protected function folders(): array
    {
        $res = $this->mailbox('GET', '/v1/webmail/folders');
        return $res->ok() ? $res->items() : [];
    }

    protected function mail(Request $request): array
    {
        return (array) $request->attribute('mail', []);
    }

    protected static function folderPath(string $folder): string
    {
        return '/v1/webmail/folders/' . rawurlencode($folder);
    }

    protected static function folderUrl(string $folder): string
    {
        return '/mail/folder/' . rawurlencode($folder);
    }

    /** Folder name for a role (inbox, sent, drafts, trash, junk, archive). */
    protected static function roleFolder(array $folders, string $role, string $fallback): string
    {
        foreach ($folders as $f) {
            if (($f['role'] ?? null) === $role) {
                return (string) $f['name'];
            }
        }
        return $fallback;
    }
}
