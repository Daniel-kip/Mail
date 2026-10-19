<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Request;
use App\Http\Response;

/**
 * Folders, message list, reading pane and bulk actions.
 *
 * Message HTML is sanitized by the API and served from a dedicated route
 * into a sandboxed iframe (no scripts, opaque origin) with its own strict
 * CSP, so even a sanitizer miss cannot reach the webmail session.
 */
final class MailController extends WebmailController
{
    private const ACTIONS = ['read', 'unread', 'flag', 'unflag', 'archive', 'spam', 'not_spam', 'delete', 'move'];

    public function home(Request $request): Response
    {
        return $this->redirect(self::folderUrl(self::roleFolder($this->folders(), 'inbox', 'INBOX')));
    }

    public function folder(Request $request): Response
    {
        $folder = $request->param('folder');
        $folders = $this->folders();
        $list = $this->list($request, $folder);
        return $this->render('webmail/folder', [
            'title' => self::displayName($folders, $folder),
            'folders' => $folders, 'currentFolder' => $folder, 'list' => $list,
            'q' => (string) $request->query('q', ''), 'mailLayout' => 'list',
        ], 'webmail');
    }

    public function read(Request $request): Response
    {
        $folder = $request->param('folder');
        $uid = (int) $request->param('uid');
        $res = $this->mailbox('GET', self::folderPath($folder) . '/messages/' . $uid);
        $message = $this->must($res)->json();
        $folders = $this->folders();
        $list = $this->list($request, $folder);
        $uids = array_map(static fn ($m) => (int) $m['uid'], $list['data']);
        $pos = array_search($uid, $uids, true);
        return $this->render('webmail/read', [
            'title' => $message['subject'] ?: '(no subject)',
            'folders' => $folders, 'currentFolder' => $folder, 'list' => $list, 'm' => $message,
            'q' => (string) $request->query('q', ''),
            'prevUid' => $pos !== false && $pos > 0 ? $uids[$pos - 1] : null,
            'nextUid' => $pos !== false && $pos < count($uids) - 1 ? $uids[$pos + 1] : null,
            'remote' => (string) $request->query('remote', '') === '1',
            'roles' => ['archive' => self::roleFolder($folders, 'archive', ''), 'junk' => self::roleFolder($folders, 'junk', ''), 'trash' => self::roleFolder($folders, 'trash', '')],
            'mailLayout' => 'list', 'reading' => true,
        ], 'webmail');
    }

    public function body(Request $request): Response
    {
        $remote = (string) $request->query('remote', '') === '1';
        $res = $this->mailbox('GET', self::folderPath($request->param('folder')) . '/messages/' . (int) $request->param('uid'), ['query' => $remote ? ['remote' => 'true'] : []]);
        if (!$res->ok()) {
            return Response::html('<p style="font-family:sans-serif">This message could not be displayed.</p>', $res->status === 404 ? 404 : 502);
        }
        $doc = (string) $res->get('html_document', '');
        $base = '<base target="_blank">';
        $doc = stripos($doc, '<head>') !== false ? preg_replace('/<head>/i', '<head>' . $base, $doc, 1) : $base . $doc;
        $img = $remote ? "img-src data: https:; " : 'img-src data:; ';
        return Response::html((string) $doc)
            ->withHeader('Content-Security-Policy', "default-src 'none'; " . $img . "style-src 'unsafe-inline'; font-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'self'")
            ->withHeader('X-Frame-Options', 'SAMEORIGIN')
            ->withHeader('Cross-Origin-Resource-Policy', 'same-origin')
            ->withHeader('Referrer-Policy', 'no-referrer')
            ->withHeader('Cache-Control', 'private, no-store');
    }

    public function attachment(Request $request): Response
    {
        $res = $this->mailbox('GET', self::folderPath($request->param('folder')) . '/messages/' . (int) $request->param('uid') . '/attachments/' . (int) $request->param('index'));
        $this->must($res);
        $disposition = $res->header('content-disposition') ?: 'attachment; filename="attachment"';
        return (new Response($res->body, 200, [
            'Content-Type' => 'application/octet-stream',
            'Content-Disposition' => $disposition,
            'X-Content-Type-Options' => 'nosniff',
            'Cache-Control' => 'private, no-store',
        ]));
    }

    public function action(Request $request): Response
    {
        $folder = $request->string('folder', 'INBOX');
        $action = $request->string('action');
        $uids = array_values(array_unique(array_filter(array_map('intval', (array) $request->input('uids', [])), static fn ($u) => $u > 0)));
        $back = $this->redirectBack($request, $folder);
        if ($uids === [] || !in_array($action, self::ACTIONS, true)) {
            $this->flash('info', 'Select at least one message first.');
            return $back;
        }
        $folders = $this->folders();
        $path = self::folderPath($folder);
        $move = static fn (string $dest) => ['/move', ['uids' => $uids, 'destination' => $dest]];
        [$suffix, $body] = match ($action) {
            'read' => ['/flags', ['uids' => $uids, 'seen' => true]],
            'unread' => ['/flags', ['uids' => $uids, 'seen' => false]],
            'flag' => ['/flags', ['uids' => $uids, 'flagged' => true]],
            'unflag' => ['/flags', ['uids' => $uids, 'flagged' => false]],
            'archive' => $move(self::roleFolder($folders, 'archive', 'Archive')),
            'spam' => $move(self::roleFolder($folders, 'junk', 'Junk')),
            'not_spam' => $move(self::roleFolder($folders, 'inbox', 'INBOX')),
            'move' => $move($request->string('destination')),
            'delete' => ['/delete', ['uids' => $uids]],
        };
        $res = $this->mailbox('POST', $path . $suffix, ['json' => $body]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $back;
        }
        $n = count($uids);
        $labels = ['archive' => 'archived', 'spam' => 'moved to Junk', 'not_spam' => 'moved to Inbox', 'move' => 'moved', 'delete' => $folder === self::roleFolder($folders, 'trash', 'Trash') ? 'deleted permanently' : 'moved to Trash'];
        if (isset($labels[$action])) {
            $this->flash('success', $n . ' ' . ($n === 1 ? 'message' : 'messages') . ' ' . $labels[$action] . '.');
            return $this->redirect(self::folderUrl($folder) . ($request->string('q') !== '' ? '?q=' . rawurlencode($request->string('q')) : ''));
        }
        return $back;
    }

    private function redirectBack(Request $request, string $folder): Response
    {
        return $this->redirectToLocal($request->string('back'), self::folderUrl($folder));
    }

    private function list(Request $request, string $folder): array
    {
        $prefs = $this->mailbox('GET', '/v1/webmail/preferences');
        $perPage = $prefs->ok() ? (int) $prefs->get('messages_per_page', 50) : 50;
        $res = $this->mailbox('GET', self::folderPath($folder) . '/messages', ['query' => array_filter([
            'page' => max(1, (int) $request->query('page', 1)),
            'per_page' => $perPage,
            'q' => trim((string) $request->query('q', '')) ?: null,
        ])]);
        if ($res->status === 404) {
            throw \App\Http\HttpException::notFound();
        }
        return $res->ok() ? ['data' => $res->items(), 'page' => (int) $res->get('page', 1), 'per_page' => (int) $res->get('per_page', $perPage), 'total' => (int) $res->get('total', 0), 'error' => null]
            : ['data' => [], 'page' => 1, 'per_page' => $perPage, 'total' => 0, 'error' => self::describe($res)];
    }

    private static function displayName(array $folders, string $folder): string
    {
        foreach ($folders as $f) {
            if ($f['name'] === $folder) {
                return (string) $f['display_name'];
            }
        }
        return $folder;
    }
}
