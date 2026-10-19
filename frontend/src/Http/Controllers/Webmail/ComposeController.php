<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Request;
use App\Http\Response;

/**
 * New message, reply, reply-all, forward (with original attachments),
 * drafts and recipient autocomplete.
 */
final class ComposeController extends WebmailController
{
    private const MAX_UPLOAD = 20 * 1024 * 1024;

    public function show(Request $request): Response
    {
        $mail = $this->mail($request);
        $me = strtolower((string) ($mail['mailbox']['address'] ?? ''));
        $prefs = $this->mailbox('GET', '/v1/webmail/preferences');
        $signature = $prefs->ok() ? (string) ($prefs->get('signature') ?? '') : '';
        $draft = ['to' => (string) $request->query('to', ''), 'cc' => '', 'bcc' => '', 'subject' => (string) $request->query('subject', ''), 'text' => '', 'in_reply_to' => '', 'references' => '', 'mode' => 'new', 'source' => null];
        $mode = (string) $request->query('mode', '');
        $folder = (string) $request->query('folder', '');
        $uid = (int) $request->query('uid', 0);
        if (in_array($mode, ['reply', 'reply-all', 'forward', 'draft'], true) && $folder !== '' && $uid > 0) {
            $res = $this->mailbox('GET', self::folderPath($folder) . '/messages/' . $uid);
            if ($res->ok()) {
                $draft = self::fromOriginal($res->json(), $mode, $me, $folder) + $draft;
            }
        }
        if ($signature !== '' && $mode !== 'draft') {
            $draft['text'] = "\n\n-- \n" . $signature . ($draft['text'] !== '' ? "\n" . $draft['text'] : '');
        }
        return $this->render('webmail/compose', ['title' => match ($draft['mode']) { 'reply', 'reply-all' => 'Reply', 'forward' => 'Forward', default => 'New message' }, 'folders' => $this->folders(), 'draft' => $draft, 'mailLayout' => 'page'], 'webmail');
    }

    public function submit(Request $request): Response
    {
        $isDraft = $request->string('intent') === 'draft';
        $split = static fn (string $v): array => array_values(array_filter(array_map('trim', preg_split('/[,;]+/', $v) ?: [])));
        $payload = [
            'to' => $split($request->string('to')),
            'cc' => $split($request->string('cc')),
            'bcc' => $split($request->string('bcc')),
            'subject' => $request->string('subject'),
            'text' => (string) $request->input('text', ''),
        ];
        foreach (['in_reply_to', 'references'] as $h) {
            if ($request->string($h) !== '') {
                $payload[$h] = $request->string($h);
            }
        }
        $errors = [];
        if (!$isDraft && $payload['to'] === [] && $payload['cc'] === [] && $payload['bcc'] === []) {
            $errors['to'] = 'Add at least one recipient.';
        }
        foreach (array_merge($payload['to'], $payload['cc'], $payload['bcc']) as $addr) {
            $email = preg_match('/<([^>]+)>\s*$/', $addr, $m) ? $m[1] : $addr;
            if (!filter_var($email, FILTER_VALIDATE_EMAIL)) {
                $errors['to'] = '"' . $addr . '" is not a valid email address.';
                break;
            }
        }
        $files = [];
        $temp = [];
        $total = 0;
        foreach (self::uploads($request->files()['attachments'] ?? null) as $f) {
            if ($f['error'] === UPLOAD_ERR_NO_FILE) {
                continue;
            }
            if ($f['error'] !== UPLOAD_ERR_OK) {
                $errors['attachments'] = 'An attachment could not be uploaded (it may be too large).';
                break;
            }
            $total += (int) $f['size'];
            $files[] = ['attachment', $f['tmp_name'], $f['name'], $f['type'] ?: 'application/octet-stream'];
        }
        $fwdFolder = $request->string('source_folder');
        $fwdUid = (int) $request->string('source_uid');
        foreach ((array) $request->input('forward_attachments', []) as $index) {
            if ($fwdFolder === '' || $fwdUid <= 0) {
                break;
            }
            $res = $this->mailbox('GET', self::folderPath($fwdFolder) . '/messages/' . $fwdUid . '/attachments/' . (int) $index);
            if (!$res->ok()) {
                continue;
            }
            $tmp = tempnam(sys_get_temp_dir(), 'mvfwd');
            file_put_contents($tmp, $res->body);
            $temp[] = $tmp;
            $total += strlen($res->body);
            $name = preg_match('/filename="([^"]+)"/', $res->header('content-disposition'), $m) ? $m[1] : 'attachment';
            $files[] = ['attachment', $tmp, $name, 'application/octet-stream'];
        }
        if ($total > self::MAX_UPLOAD) {
            $errors['attachments'] = 'Attachments total more than 20 MB. Share large files with a link instead.';
        }
        if ($errors !== []) {
            array_map('unlink', $temp);
            return $this->invalid($request, '/mail/compose', $errors, 'The message was not sent.');
        }
        try {
            $res = $this->mailbox('POST', $isDraft ? '/v1/webmail/drafts' : '/v1/webmail/send', [
                'multipart' => ['fields' => ['payload' => json_encode($payload, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES)], 'files' => $files],
                'timeout' => 90,
            ]);
        } finally {
            array_map('unlink', $temp);
        }
        if (!$res->ok()) {
            return $this->failed($request, '/mail/compose', $res, 'The message was not sent.');
        }
        $this->flash('success', $isDraft ? 'Draft saved to your Drafts folder.' : 'Message sent.');
        return $this->redirectToLocal($request->string('back'), '/mail');
    }

    public function suggest(Request $request): Response
    {
        $q = trim((string) $request->query('q', ''));
        if (mb_strlen($q) < 2) {
            return Response::json(['data' => []]);
        }
        $res = $this->mailbox('GET', '/v1/webmail/recipients/suggest', ['query' => ['q' => $q]]);
        return Response::json(['data' => $res->ok() ? $res->items() : []]);
    }

    private static function fromOriginal(array $m, string $mode, string $me, string $folder): array
    {
        $fmt = static fn (array $list): array => array_map(static fn ($a) => ($a['name'] ?? '') !== '' ? $a['name'] . ' <' . $a['email'] . '>' : (string) $a['email'], $list);
        $subject = (string) ($m['subject'] ?? '');
        $from = $fmt($m['from'] ?? []);
        $quoteHead = 'On ' . (isset($m['date']) ? datetime($m['date']) : 'an earlier date') . ', ' . ($from[0] ?? 'the sender') . ' wrote:';
        $body = (string) ($m['text'] ?? '');
        $quoted = implode("\n", array_map(static fn ($l) => '> ' . $l, explode("\n", rtrim($body))));
        if ($mode === 'draft') {
            return ['to' => implode(', ', $fmt($m['to'] ?? [])), 'cc' => implode(', ', $fmt($m['cc'] ?? [])), 'subject' => $subject, 'text' => $body, 'mode' => 'new'];
        }
        if ($mode === 'forward') {
            $header = "\n\n---------- Forwarded message ----------\nFrom: " . implode(', ', $from) . "\nDate: " . ($m['date'] ?? '') . "\nSubject: " . $subject . "\nTo: " . implode(', ', $fmt($m['to'] ?? [])) . "\n\n";
            return ['subject' => preg_match('/^fwd?:/i', $subject) ? $subject : 'Fwd: ' . $subject, 'text' => $header . $body, 'mode' => 'forward',
                'source' => ['folder' => $folder, 'uid' => $m['uid'], 'attachments' => $m['attachments'] ?? []]];
        }
        $replyTo = $fmt(($m['reply_to'] ?? []) ?: ($m['from'] ?? []));
        $to = $replyTo;
        $cc = [];
        if ($mode === 'reply-all') {
            $others = array_filter(array_merge($fmt($m['to'] ?? []), $fmt($m['cc'] ?? [])), static fn ($a) => !str_contains(strtolower($a), $me));
            $cc = array_values(array_diff($others, $to));
        }
        $refs = trim(((string) ($m['references'] ?? '')) . ' ' . ($m['message_id'] ? '<' . trim((string) $m['message_id'], '<>') . '>' : ''));
        return [
            'to' => implode(', ', $to), 'cc' => implode(', ', $cc),
            'subject' => preg_match('/^re:/i', $subject) ? $subject : 'Re: ' . $subject,
            'text' => "\n\n" . $quoteHead . "\n" . $quoted,
            'in_reply_to' => $m['message_id'] ? '<' . trim((string) $m['message_id'], '<>') . '>' : '',
            'references' => $refs,
            'mode' => $mode,
        ];
    }

    /** Normalise PHP's multi-file upload array. */
    private static function uploads(mixed $spec): array
    {
        if (!is_array($spec) || !isset($spec['name'])) {
            return [];
        }
        if (!is_array($spec['name'])) {
            return [$spec];
        }
        $out = [];
        foreach (array_keys($spec['name']) as $i) {
            $out[] = ['name' => $spec['name'][$i], 'type' => $spec['type'][$i], 'tmp_name' => $spec['tmp_name'][$i], 'error' => $spec['error'][$i], 'size' => $spec['size'][$i]];
        }
        return $out;
    }
}
