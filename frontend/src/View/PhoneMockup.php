<?php

declare(strict_types=1);

namespace App\View;

/**
 * Illustrative phone diagrams for the setup guide.
 *
 * Deliberately schematic rather than vendor screenshots: they show *where
 * to look* (the highlighted control) without pretending to be the exact
 * screen of a particular app version, which would go stale. Styled by CSS
 * classes so they follow light and dark themes.
 */
final class PhoneMockup
{
    private const W = 240;
    private const H = 440;

    /** @param array{title?: string, kind?: string, rows?: list<string>, buttons?: list<string>, highlight?: int|string} $mock */
    public static function render(array $mock, array $placeholders = [], string $caption = ''): string
    {
        $fill = static fn (string $s): string => strtr($s, $placeholders);
        $title = self::fit($fill((string) ($mock['title'] ?? '')), 24);
        $kind = (string) ($mock['kind'] ?? 'list');
        $rows = array_map($fill, $mock['rows'] ?? []);
        $buttons = array_map($fill, $mock['buttons'] ?? []);
        $highlight = $mock['highlight'] ?? null;

        $esc = static fn (string $s): string => htmlspecialchars($s, ENT_QUOTES | ENT_XML1);
        $label = 'Illustration: ' . ($caption !== '' ? $caption : $title) . '. Your screen may look slightly different.';
        $out = '<svg class="phone" viewBox="0 0 ' . self::W . ' ' . self::H . '" role="img" aria-label="' . $esc($label) . '">';
        $out .= '<rect class="phone-body" x="4" y="4" width="232" height="432" rx="34"/>';
        $out .= '<rect class="phone-screen" x="14" y="14" width="212" height="412" rx="26"/>';
        $out .= '<rect class="phone-notch" x="92" y="20" width="56" height="14" rx="7"/>';
        $out .= '<text class="phone-status" x="30" y="32">9:41</text>';
        $out .= '<rect class="phone-bar" x="14" y="44" width="212" height="40"/>';
        $out .= '<text class="phone-title" x="30" y="69">' . $esc($title) . '</text>';

        $y = 98;
        if ($kind === 'inbox') {
            $out .= '<rect class="phone-field" x="26" y="' . $y . '" width="188" height="30" rx="15"/>';
            $out .= '<text class="phone-muted" x="40" y="' . ($y + 19) . '">Search in mail</text>';
            $ring = $highlight === 'avatar' ? ' phone-highlight' : '';
            $out .= '<circle class="phone-avatar' . $ring . '" cx="198" cy="' . ($y + 15) . '" r="11"/>';
            if ($highlight === 'avatar') {
                $out .= '<circle class="phone-pulse" cx="198" cy="' . ($y + 15) . '" r="18"/>';
            }
            $y += 46;
            for ($i = 0; $i < 6; $i++) {
                $out .= '<circle class="phone-dot" cx="38" cy="' . ($y + 14) . '" r="11"/>';
                $out .= '<rect class="phone-line" x="58" y="' . ($y + 5) . '" width="' . (110 - ($i % 3) * 18) . '" height="7" rx="3.5"/>';
                $out .= '<rect class="phone-line phone-line-soft" x="58" y="' . ($y + 17) . '" width="' . (140 - ($i % 2) * 30) . '" height="6" rx="3"/>';
                $y += 40;
            }
        } elseif ($kind === 'form') {
            foreach ($rows as $row) {
                $out .= '<rect class="phone-field" x="26" y="' . $y . '" width="188" height="38" rx="8"/>';
                $out .= '<text class="phone-text" x="38" y="' . ($y + 24) . '">' . $esc(self::fit($row, 30)) . '</text>';
                $y += 48;
            }
            $by = max($y + 10, 340);
            $bx = 214;
            foreach (array_reverse($buttons) as $button) {
                $w = max(44, (int) (mb_strlen($button) * 7.2) + 26);
                $bx -= $w;
                $hl = (string) $highlight === $button;
                $out .= '<rect class="phone-button' . ($hl ? ' phone-highlight' : '') . '" x="' . $bx . '" y="' . $by . '" width="' . $w . '" height="34" rx="17"/>';
                $out .= '<text class="phone-button-text" x="' . ($bx + $w / 2) . '" y="' . ($by + 22) . '" text-anchor="middle">' . $esc($button) . '</text>';
                if ($hl) {
                    $out .= '<rect class="phone-pulse" x="' . ($bx - 5) . '" y="' . ($by - 5) . '" width="' . ($w + 10) . '" height="44" rx="22"/>';
                }
                $bx -= 10;
            }
        } else {
            foreach ($rows as $i => $row) {
                $hl = is_int($highlight) && $highlight === $i;
                $out .= '<rect class="phone-row' . ($hl ? ' phone-highlight' : '') . '" x="22" y="' . $y . '" width="196" height="40" rx="10"/>';
                $out .= '<text class="phone-text' . ($hl ? ' phone-text-strong' : '') . '" x="36" y="' . ($y + 25) . '">' . $esc(self::fit($row, 27)) . '</text>';
                if ($hl) {
                    $out .= '<rect class="phone-pulse" x="17" y="' . ($y - 5) . '" width="206" height="50" rx="14"/>';
                    $out .= '<path class="phone-chevron" d="M200 ' . ($y + 15) . 'l6 5-6 5"/>';
                }
                $y += 48;
            }
        }
        $out .= '<rect class="phone-home" x="88" y="412" width="64" height="5" rx="2.5"/>';
        $out .= '</svg>';
        return $out;
    }

    private static function fit(string $s, int $max): string
    {
        return mb_strlen($s) > $max ? mb_substr($s, 0, $max - 1) . '…' : $s;
    }
}
