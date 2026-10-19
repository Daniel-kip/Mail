<?php

declare(strict_types=1);

namespace App\Http\Controllers;

use App\Api\ApiResponse;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;
use App\Kernel\Application;
use App\Session\Session;

/**
 * Base controller: rendering, redirects and API helpers.
 *
 * Mutations follow post/redirect/get. Validation and API errors are flashed
 * with the submitted input (secrets excluded) so the form re-renders with
 * field-level messages and nothing is lost.
 */
abstract class Controller
{
    /** Never echo these back into forms after an error. */
    private const SECRET_FIELDS = ['password', 'password_confirmation', 'current_password', 'new_password', 'code', '_token'];

    public function __construct(protected readonly Application $app)
    {
    }

    protected function session(): Session
    {
        return $this->app->session();
    }

    /** @param array<string, mixed> $data */
    protected function render(string $template, array $data = [], string $layout = 'console', int $status = 200): Response
    {
        return Response::html($this->app->view()->render($template, $data, $layout), $status);
    }

    protected function redirect(string $to): Response
    {
        return Response::redirect($to);
    }

    /** Redirect to a local path supplied by the user (e.g. ?next=), or the fallback. */
    protected function redirectToLocal(?string $target, string $fallback): Response
    {
        $target = (string) $target;
        $safe = $target !== '' && str_starts_with($target, '/') && !str_starts_with($target, '//') && !str_contains($target, '\\')
            && !preg_match('/[\r\n]/', $target);
        return Response::redirect($safe ? $target : $fallback);
    }

    protected function flash(string $type, string $message): void
    {
        $this->session()->flash($type, $message);
    }

    /** @param array<string, string> $errors */
    protected function invalid(Request $request, string $to, array $errors, ?string $summary = null): Response
    {
        $this->session()->flash('errors', $errors);
        $this->session()->flash('old', array_diff_key($request->all(), array_flip(self::SECRET_FIELDS)));
        $this->flash('error', $summary ?? 'Please correct the highlighted fields.');
        return $this->redirect($to);
    }

    /** Flash an API error and return to the form with the user's input. */
    protected function failed(Request $request, string $to, ApiResponse $res, ?string $fallback = null): Response
    {
        $this->session()->flash('old', array_diff_key($request->all(), array_flip(self::SECRET_FIELDS)));
        $this->flash('error', self::describe($res, $fallback));
        return $this->redirect($to);
    }

    /** A user-safe sentence for an API error, with its reference when useful. */
    public static function describe(ApiResponse $res, ?string $fallback = null): string
    {
        $message = match ($res->errorCode()) {
            'csrf_failed' => 'Your session token expired. Reload the page and try again.',
            'rate_limited' => 'Too many attempts in a short time. Wait a minute and try again.',
            'organization_suspended' => 'This organization is suspended. Changes are disabled until it is reinstated; contact support.',
            'insufficient_permissions', 'forbidden' => $res->errorMessage('Your role does not allow this action.'),
            'internal_error', 'service_unavailable' => 'The platform could not complete the request. Please try again shortly.',
            default => $res->errorMessage($fallback ?? 'The request could not be completed.'),
        };
        if ($res->status >= 500 && ($id = $res->requestId())) {
            $message .= ' (Reference: ' . $id . ')';
        }
        return $message;
    }

    /**
     * Return the response when it succeeded; otherwise stop with the matching
     * error page (404 hides other tenants' resources, 403 explains access).
     */
    protected function must(ApiResponse $res): ApiResponse
    {
        if ($res->ok()) {
            return $res;
        }
        throw match (true) {
            $res->status === 404 => HttpException::notFound(),
            $res->status === 403 => HttpException::forbidden(self::describe($res)),
            $res->status === 401 => new HttpException(401, 'Your session has ended. Sign in again.'),
            $res->status === 429 => new HttpException(429, self::describe($res)),
            default => new HttpException($res->status >= 500 ? 503 : $res->status, self::describe($res), $res->requestId()),
        };
    }

    protected function console(string $method, string $path, array $options = []): ApiResponse
    {
        return $this->app->api()->request($method, $path, $options + ['kind' => 'console']);
    }

    protected function mailbox(string $method, string $path, array $options = []): ApiResponse
    {
        return $this->app->api()->request($method, $path, $options + ['kind' => 'webmail']);
    }

    protected function public(string $method, string $path, array $options = []): ApiResponse
    {
        return $this->app->api()->request($method, $path, $options + ['kind' => 'public']);
    }

    protected function auth(Request $request): array
    {
        return (array) $request->attribute('auth', []);
    }

    protected function can(Request $request, string $permission): bool
    {
        return in_array($permission, $this->auth($request)['organization']['permissions'] ?? [], true);
    }

    protected function isInstitution(Request $request): bool
    {
        return ($this->auth($request)['organization']['account_type'] ?? 'institution') === 'institution';
    }

    /** Store a value shown exactly once on the next page (new API keys, links). */
    protected function reveal(string $key, array $value): void
    {
        $this->session()->flash('reveal_' . $key, $value);
    }

    protected function revealed(string $key): ?array
    {
        $value = $this->session()->flashed('reveal_' . $key);
        return is_array($value) ? $value : null;
    }
}
