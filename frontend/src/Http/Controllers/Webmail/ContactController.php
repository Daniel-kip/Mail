<?php

declare(strict_types=1);

namespace App\Http\Controllers\Webmail;

use App\Http\Request;
use App\Http\Response;
use App\Support\Validator;

/**
 * Personal contacts and the organization directory.
 */
final class ContactController extends WebmailController
{
    public function index(Request $request): Response
    {
        $q = trim((string) $request->query('q', ''));
        $res = $this->mailbox('GET', '/v1/webmail/contacts', ['query' => array_filter(['q' => $q ?: null])]);
        return $this->render('webmail/contacts', [
            'title' => 'Contacts', 'folders' => $this->folders(), 'contacts' => $res->ok() ? $res->items() : [], 'q' => $q,
            'editing' => (string) $request->query('edit', ''), 'mailLayout' => 'page',
        ], 'webmail');
    }

    public function store(Request $request): Response
    {
        $errors = Validator::make($request->all(), ['email' => 'required|email', 'name' => 'nullable|max:200', 'phone' => 'nullable|max:40', 'organization' => 'nullable|max:200', 'notes' => 'nullable|max:2000']);
        if ($errors !== []) {
            return $this->invalid($request, '/mail/contacts', $errors);
        }
        $res = $this->mailbox('POST', '/v1/webmail/contacts', ['json' => $request->only('name', 'email', 'phone', 'organization', 'notes')]);
        if (!$res->ok()) {
            return $this->failed($request, '/mail/contacts', $res);
        }
        $this->flash('success', 'Contact saved.');
        return $this->redirect('/mail/contacts');
    }

    public function update(Request $request): Response
    {
        $res = $this->mailbox('PATCH', '/v1/webmail/contacts/' . $request->param('id'), ['json' => $request->only('name', 'email', 'phone', 'organization', 'notes')]);
        if (!$res->ok()) {
            return $this->failed($request, '/mail/contacts?edit=' . $request->param('id'), $res);
        }
        $this->flash('success', 'Contact updated.');
        return $this->redirect('/mail/contacts');
    }

    public function destroy(Request $request): Response
    {
        $res = $this->mailbox('DELETE', '/v1/webmail/contacts/' . $request->param('id'));
        $this->flash($res->ok() ? 'success' : 'error', $res->ok() ? 'Contact deleted.' : self::describe($res));
        return $this->redirect('/mail/contacts');
    }

    public function directory(Request $request): Response
    {
        $q = trim((string) $request->query('q', ''));
        $res = $this->mailbox('GET', '/v1/webmail/directory', ['query' => array_filter(['q' => $q ?: null])]);
        return $this->render('webmail/directory', [
            'title' => 'Directory', 'folders' => $this->folders(), 'q' => $q,
            'people' => $res->ok() ? (array) $res->get('people', []) : [], 'groups' => $res->ok() ? (array) $res->get('groups', []) : [], 'mailLayout' => 'page',
        ], 'webmail');
    }
}
