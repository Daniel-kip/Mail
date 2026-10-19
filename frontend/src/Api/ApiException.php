<?php

declare(strict_types=1);

namespace App\Api;

/**
 * The platform API could not be reached (network error or timeout). Shown
 * to users as a temporary-unavailability page with a request id.
 */
final class ApiException extends \RuntimeException
{
    public function __construct(string $message, public readonly string $requestId)
    {
        parent::__construct($message);
    }
}
