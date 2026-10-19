<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Mailbox settings: password, signature, list density and out-of-office.
 */
final class SettingsController extends WebmailController
{
    public function show(Request $request): Response
    {
        $prefs = $this->mailbox('GET', '/v1/webmail/preferences');
        return $this->render('webmail/settings', ['title' => 'Settings', 'folders' => $this->folders(), 'prefs' => $prefs->ok() ? $prefs->json() : [], 'mailLayout' => 'page'], 'webmail');
    }

    public function password(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['current_password' => 'required', 'new_password' => 'required|min:10|max:256', 'password_confirmation' => 'same:new_password'], ['password_confirmation' => 'Confirmation']);
        if ($errors !== []) {
            return $this->invalid($request, '/mail/settings#password', $errors);
        }
        $res = $this->mailbox('POST', '/v1/webmail/password', ['json' => ['current_password' => (string) $request->input('current_password'), 'new_password' => (string) $request->input('new_password')]]);
        if (!$res->ok()) {
            return $this->invalid($request, '/mail/settings#password', [$res->errorCode() === 'invalid_credentials' ? 'current_password' : 'new_password' => self::describe($res)], 'The password was not changed.');
        }
        $this->flash('success', 'Password changed. Update it in the mail app on your phone too; until then the app will ask for it.');
        return $this->redirect('/mail/settings');
    }

    public function preferences(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['signature' => 'nullable|max:2000', 'messages_per_page' => 'required|in:25,50,100']);
        if ($errors !== []) {
            return $this->invalid($request, '/mail/settings', $errors);
        }
        $res = $this->mailbox('PUT', '/v1/webmail/preferences', ['json' => ['signature' => (string) $request->input('signature', ''), 'messages_per_page' => (int) $request->string('messages_per_page')]]);
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Preferences saved.' : self::describe($res));
        return $this->redirect('/mail/settings');
    }

    public function autoReply(Request $request): Response
    {
        $enabled = $request->bool('auto_reply_enabled');
        $rules = ['auto_reply_starts_on' => 'nullable|date', 'auto_reply_ends_on' => 'nullable|date', 'auto_reply_subject' => 'nullable|max:200', 'auto_reply_body' => ($enabled ? 'required|' : 'nullable|') . 'max:4000'];
        $errors = Validator::make($request->all(), $rules, ['auto_reply_body' => 'Message', 'auto_reply_starts_on' => 'Start date', 'auto_reply_ends_on' => 'End date']);
        if ($errors !== []) {
            return $this->invalid($request, '/mail/settings#auto-reply', $errors);
        }
        $res = $this->mailbox('PUT', '/v1/webmail/preferences', ['json' => [
            'auto_reply_enabled' => $enabled,
            'auto_reply_subject' => $request->string('auto_reply_subject') ?: null,
            'auto_reply_body' => (string) $request->input('auto_reply_body', '') ?: null,
            'auto_reply_starts_on' => $request->string('auto_reply_starts_on') ?: null,
            'auto_reply_ends_on' => $request->string('auto_reply_ends_on') ?: null,
        ]]);
        if (!$res->ok()) {
            return $this->failed($request, '/mail/settings#auto-reply', $res);
        }
        $syncError = $res->get('auto_reply.sync_error');
        $this->flash($syncError ? 'warning' : 'success', $syncError ? 'Saved, but the mail server could not apply it yet: ' . $syncError : ($enabled ? 'Out-of-office reply is on.' : 'Out-of-office reply is off.'));
        return $this->redirect('/mail/settings#auto-reply');
    }
}
