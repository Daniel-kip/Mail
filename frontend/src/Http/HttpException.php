<?php

declare(strict_types=1);

namespace App\Http;

/**
 * Raised by controllers to stop with a specific HTTP status and a message
 * that is safe to show.
 */
class HttpException extends \RuntimeException
{
    public function __construct(public readonly int $status, string $message = '', public readonly ?string $requestId = null)
    {
        parent::__construct($message);
    }

    public static function notFound(string $message = 'The page you are looking for does not exist or you do not have access to it.'): self
    {
        return new self(404, $message);
    }

    public static function forbidden(string $message = 'You do not have permission to do that.'): self
    {
        return new self(403, $message);
    }
}
