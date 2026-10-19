<?php

declare(strict_types=1);

namespace App\View;

/**
 * Server-rendered SVG charts (no JavaScript, works under the strict CSP).
 *
 * Stacked daily bars by delivery outcome. Series order and colours are
 * fixed so a status always keeps its colour; a legend with text labels and
 * a data table accompany every chart, so meaning never relies on colour.
 */
final class Charts
{
    /** Series in stacking order: key => [label, css class]. */
    public const DELIVERY_SERIES = [
        'delivered' => ['Delivered', 's-delivered'],
        'accepted' => ['Handed to mail server', 's-accepted'],
        'pending' => ['Queued or delayed', 's-pending'],
        'failed' => ['Bounced or failed', 's-failed'],
        'other' => ['Suppressed or test', 's-other'],
    ];

    /** Map API statuses onto the chart's five series. */
    public static function bucket(string $status): string
    {
        return match ($status) {
            'delivered' => 'delivered',
            'accepted_by_mta' => 'accepted',
            'queued', 'processing', 'deferred', 'scheduled' => 'pending',
            'bounced', 'failed' => 'failed',
            default => 'other',
        };
    }

    /**
     * @param list<array{date: string, by_status: array<string, int>}> $days
     * @return array{svg: string, rows: list<array{date: string, values: array<string, int>, total: int}>}
     */
    public static function deliveryBars(array $days, int $width = 720, int $height = 220): array
    {
        $rows = [];
        foreach ($days as $day) {
            $values = array_fill_keys(array_keys(self::DELIVERY_SERIES), 0);
            foreach ((array) ($day['by_status'] ?? []) as $status => $n) {
                $values[self::bucket((string) $status)] += (int) $n;
            }
            $rows[] = ['date' => (string) $day['date'], 'values' => $values, 'total' => array_sum($values)];
        }
        $max = max(1, ...array_map(static fn ($r) => $r['total'], $rows ?: [['total' => 0]]));
        $scaleMax = self::niceCeiling($max);

        $padL = 36;
        $padR = 8;
        $padT = 10;
        $padB = 26;
        $plotW = $width - $padL - $padR;
        $plotH = $height - $padT - $padB;
        $n = max(1, count($rows));
        $slot = $plotW / $n;
        $barW = max(3.0, min(28.0, $slot * 0.62));

        $svg = '<svg class="chart" viewBox="0 0 ' . $width . ' ' . $height . '" role="img" aria-label="Messages per day by delivery outcome; the table below lists the values" preserveAspectRatio="none">';
        // Grid lines and axis labels (recessive).
        for ($i = 0; $i <= 4; $i++) {
            $v = (int) round($scaleMax * $i / 4);
            $y = $padT + $plotH - ($plotH * $i / 4);
            $svg .= '<line class="gridline" x1="' . $padL . '" x2="' . ($width - $padR) . '" y1="' . round($y, 1) . '" y2="' . round($y, 1) . '"/>';
            $svg .= '<text class="axis" x="' . ($padL - 6) . '" y="' . round($y + 3.5, 1) . '" text-anchor="end">' . self::short($v) . '</text>';
        }
        foreach ($rows as $i => $row) {
            $x = $padL + $slot * $i + ($slot - $barW) / 2;
            $yCursor = $padT + $plotH;
            $title = (new \DateTimeImmutable($row['date']))->format('D j M') . ': ' . $row['total'] . ' messages';
            $parts = [];
            foreach (self::DELIVERY_SERIES as $key => [$label, $class]) {
                $v = $row['values'][$key];
                if ($v <= 0) {
                    continue;
                }
                $h = $plotH * $v / $scaleMax;
                $yCursor -= $h;
                // 1px surface gap between stacked segments.
                $svg .= '<rect class="' . $class . '" x="' . round($x, 1) . '" y="' . round($yCursor + 0.5, 1) . '" width="' . round($barW, 1) . '" height="' . round(max(0.5, $h - 1), 1) . '" rx="1.5"/>';
                $parts[] = $label . ' ' . $v;
            }
            // Full-height hit target with a native tooltip.
            $svg .= '<rect class="hit" x="' . round($padL + $slot * $i, 1) . '" y="' . $padT . '" width="' . round($slot, 1) . '" height="' . $plotH . '"><title>'
                . htmlspecialchars($title . ($parts !== [] ? ' (' . implode(', ', $parts) . ')' : ''), ENT_QUOTES) . '</title></rect>';
            $labelEvery = (int) ceil($n / 7);
            if ($i % $labelEvery === 0 || $i === $n - 1) {
                $svg .= '<text class="axis" x="' . round($padL + $slot * $i + $slot / 2, 1) . '" y="' . ($height - 8) . '" text-anchor="middle">'
                    . (new \DateTimeImmutable($row['date']))->format('j M') . '</text>';
            }
        }
        $svg .= '<line class="baseline" x1="' . $padL . '" x2="' . ($width - $padR) . '" y1="' . ($padT + $plotH) . '" y2="' . ($padT + $plotH) . '"/>';
        $svg .= '</svg>';
        return ['svg' => $svg, 'rows' => $rows];
    }

    private static function niceCeiling(int $v): int
    {
        if ($v <= 4) {
            return 4;
        }
        $magnitude = 10 ** (int) floor(log10($v));
        foreach ([1, 2, 2.5, 5, 10] as $step) {
            $candidate = (int) ($step * $magnitude);
            if ($candidate >= $v && $candidate % 4 === 0) {
                return $candidate;
            }
        }
        return (int) (ceil($v / $magnitude) * $magnitude);
    }

    private static function short(int $v): string
    {
        return match (true) {
            $v >= 1_000_000 => round($v / 1_000_000, 1) . 'M',
            $v >= 1_000 => round($v / 1_000, 1) . 'k',
            default => (string) $v,
        };
    }

    /** Horizontal meter for quotas: label, used, limit. */
    public static function meter(string $label, int $used, int $limit, string $unit = ''): string
    {
        $pct = $limit > 0 ? min(100, (int) round($used * 100 / $limit)) : 0;
        $tone = $pct >= 90 ? 'danger' : ($pct >= 75 ? 'warning' : 'ok');
        $text = number_format($used) . ' of ' . number_format($limit) . ($unit !== '' ? ' ' . $unit : '');
        return '<div class="meter meter-' . $tone . '"><div class="meter-head"><span>' . htmlspecialchars($label, ENT_QUOTES) . '</span><span class="meter-value">'
            . htmlspecialchars($text, ENT_QUOTES) . '</span></div><div class="meter-track" role="progressbar" aria-label="' . htmlspecialchars($label, ENT_QUOTES)
            . '" aria-valuemin="0" aria-valuemax="' . $limit . '" aria-valuenow="' . $used . '"><span style="width:' . $pct . '%"></span></div></div>';
    }
}
