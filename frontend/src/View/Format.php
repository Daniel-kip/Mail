<?php

declare(strict_types=1);

namespace App\View;

use App\Support\Env;

/**
 * Presentation formatting: dates in the operator's time zone, sizes, money
 * and status badges. Status is always conveyed by icon + text, with colour
 * as a secondary cue (WCAG 1.4.1).
 */
final class Format
{
    /** status => [tone, icon, label] */
    private const STATUSES = [
        // messages
        'scheduled' => ['info', 'clock', 'Scheduled'],
        'queued' => ['neutral', 'clock', 'Queued'],
        'processing' => ['info', 'refresh', 'Sending'],
        'accepted_by_mta' => ['info', 'send', 'Handed to mail server'],
        'delivered' => ['success', 'check-circle', 'Delivered'],
        'deferred' => ['warning', 'clock', 'Delayed, retrying'],
        'bounced' => ['danger', 'x-circle', 'Bounced'],
        'failed' => ['danger', 'x-circle', 'Failed'],
        'suppressed' => ['warning', 'ban', 'Suppressed'],
        'sandboxed' => ['neutral', 'layers', 'Test mode'],
        'cancelled' => ['neutral', 'x', 'Cancelled'],
        // domains / generic
        'verified' => ['success', 'check-circle', 'Verified'],
        'pending' => ['warning', 'clock', 'Pending'],
        'missing' => ['danger', 'x-circle', 'Missing'],
        'unchecked' => ['neutral', 'help', 'Not checked'],
        'optional' => ['neutral', 'info', 'Optional'],
        // mailboxes, members, orgs, keys
        'active' => ['success', 'check-circle', 'Active'],
        'disabled' => ['neutral', 'pause', 'Disabled'],
        'suspended' => ['danger', 'ban', 'Suspended'],
        'revoked' => ['neutral', 'ban', 'Revoked'],
        'expired' => ['neutral', 'clock', 'Expired'],
        'locked' => ['warning', 'lock', 'Locked'],
        // billing
        'trialing' => ['info', 'sparkles', 'Trial'],
        'past_due' => ['danger', 'alert-triangle', 'Payment overdue'],
        'open' => ['warning', 'receipt', 'Open'],
        'paid' => ['success', 'check-circle', 'Paid'],
        'void' => ['neutral', 'x', 'Void'],
        'overdue' => ['danger', 'alert-triangle', 'Overdue'],
        // webhooks
        'succeeded' => ['success', 'check-circle', 'Succeeded'],
        // readiness / setup
        'ready' => ['success', 'check-circle', 'Ready'],
        'needs_attention' => ['danger', 'alert-triangle', 'Needs attention'],
        'unknown' => ['neutral', 'help', 'Unknown'],
        'full' => ['danger', 'alert-triangle', 'Mailbox full'],
        'nearly_full' => ['warning', 'alert-triangle', 'Almost full'],
        'not_started' => ['neutral', 'minus', 'Not started'],
        'setup_started' => ['info', 'phone', 'Setup started'],
        'partially_verified' => ['warning', 'smartphone-check', 'Partly verified'],
        'not_verified' => ['neutral', 'help', 'Not verified'],
    ];

    private static ?\DateTimeZone $zone = null;

    public static function zone(): \DateTimeZone
    {
        if (self::$zone === null) {
            try {
                self::$zone = new \DateTimeZone(Env::get('APP_TIMEZONE', 'Africa/Nairobi'));
            } catch (\Exception) {
                self::$zone = new \DateTimeZone('UTC');
            }
        }
        return self::$zone;
    }

    private static function parse(?string $iso): ?\DateTimeImmutable
    {
        if ($iso === null || $iso === '') {
            return null;
        }
        try {
            return (new \DateTimeImmutable($iso))->setTimezone(self::zone());
        } catch (\Exception) {
            return null;
        }
    }

    public static function date(?string $iso): string
    {
        return self::parse($iso)?->format('j M Y') ?? '—';
    }

    public static function datetime(?string $iso): string
    {
        $d = self::parse($iso);
        return $d === null ? '—' : $d->format('j M Y, H:i') . ' ' . $d->format('T');
    }

    public static function time(?string $iso): string
    {
        return self::parse($iso)?->format('H:i') ?? '';
    }

    /** "just now", "5 min ago", "in 3 days", falling back to a date. */
    public static function ago(?string $iso, ?\DateTimeImmutable $now = null): string
    {
        $d = self::parse($iso);
        if ($d === null) {
            return '—';
        }
        $now ??= new \DateTimeImmutable('now', self::zone());
        $diff = $now->getTimestamp() - $d->getTimestamp();
        $future = $diff < 0;
        $s = abs($diff);
        $phrase = match (true) {
            $s < 45 => null,
            $s < 3600 => (int) round($s / 60) . ' min',
            $s < 86400 => ($h = (int) round($s / 3600)) . ' ' . ($h === 1 ? 'hour' : 'hours'),
            $s < 86400 * 30 => ($days = (int) round($s / 86400)) . ' ' . ($days === 1 ? 'day' : 'days'),
            default => false,
        };
        if ($phrase === null) {
            return $future ? 'in a moment' : 'just now';
        }
        if ($phrase === false) {
            return $d->format('j M Y');
        }
        return $future ? 'in ' . $phrase : $phrase . ' ago';
    }

    public static function bytes(int|float|null $n): string
    {
        if ($n === null) {
            return '—';
        }
        $units = ['B', 'KB', 'MB', 'GB', 'TB'];
        $i = 0;
        $value = (float) $n;
        while ($value >= 1024 && $i < count($units) - 1) {
            $value /= 1024;
            $i++;
        }
        return ($i === 0 ? (string) (int) $value : number_format($value, $value < 10 ? 1 : 0)) . ' ' . $units[$i];
    }

    public static function money(int|float|null $cents, string $currency): string
    {
        if ($cents === null) {
            return '—';
        }
        $cents = (int) $cents;
        $decimals = $currency === 'KES' && $cents % 100 === 0 ? 0 : 2;
        return $currency . ' ' . number_format($cents / 100, $decimals);
    }

    public static function number(int|float|null $n): string
    {
        return $n === null ? '—' : number_format((float) $n, is_float($n) && floor($n) != $n ? 1 : 0);
    }

    public static function badge(string $status, ?string $label = null): string
    {
        [$tone, $icon, $text] = self::STATUSES[$status] ?? ['neutral', 'info', ucfirst(str_replace('_', ' ', $status))];
        return '<span class="badge badge-' . $tone . '">' . Icons::svg($icon, 'icon-xs') . '<span>'
            . htmlspecialchars($label ?? $text, ENT_QUOTES) . '</span></span>';
    }

    public static function statusLabel(string $status): string
    {
        return self::STATUSES[$status][2] ?? ucfirst(str_replace('_', ' ', $status));
    }
}
