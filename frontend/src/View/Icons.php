<?php

declare(strict_types=1);

namespace App\View;

/**
 * Inline SVG icon set (24×24 grid, 1.75px strokes, currentColor). Drawn for
 * this project; inline SVG keeps the strict CSP and avoids icon fonts.
 * Decorative by default (aria-hidden); pass a label for meaningful icons.
 */
final class Icons
{
    private const PATHS = [
        'tool-health' => '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.4 7.5 9.5 4.3-1.1 7.5-4.9 7.5-9.5V6z"/><path d="M7.5 12.5h2.3l1.4-2.8 2 5 1.3-2.2h2"/>',
        'tool-dns' => '<circle cx="10.5" cy="10.5" r="7"/><path d="M3.5 10.5h14M10.5 3.5a10.5 10.5 0 0 1 0 14M10.5 3.5a10.5 10.5 0 0 0 0 14"/><path d="m15.5 15.5 5 5"/>',
        'tool-generator' => '<path d="M13.5 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5z"/><path d="M13.5 3v5.5H19"/><path d="m10 12-2 2 2 2M14 12l2 2-2 2"/>',
        'tool-headers' => '<path d="M20 11V7.5A2.5 2.5 0 0 0 17.5 5h-11A2.5 2.5 0 0 0 4 7.5v9A2.5 2.5 0 0 0 6.5 19H11"/><path d="m4.5 7.5 7.5 5.5 7.5-5.5"/><circle cx="17" cy="16.5" r="3"/><path d="m21.5 21-2.4-2.4"/>',
        'tool-blocklist' => '<path d="M4 6h9M4 11h7M4 16h5"/><circle cx="17" cy="15" r="4.5"/><path d="m13.8 18.2 6.4-6.4"/>',
        'tool-ports' => '<rect x="3" y="4" width="18" height="12" rx="2.5"/><path d="M7 8.5v3M10.33 8.5v3M13.67 8.5v3M17 8.5v3"/><path d="M12 16v4M8 20h8"/>',
        'mail' => '<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="m4 7 8 6 8-6"/>',
        'inbox' => '<path d="M3 13h5l1.5 3h5L16 13h5"/><path d="M5.5 5h13L21 13v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5z"/>',
        'send' => '<path d="M21 3 10 14"/><path d="m21 3-7 18-4-7-7-4z"/>',
        'archive' => '<rect x="3" y="4" width="18" height="5" rx="1.5"/><path d="M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9"/><path d="M10 13h4"/>',
        'trash' => '<path d="M4 7h16"/><path d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12"/><path d="M10 11v6M14 11v6"/>',
        'star' => '<path d="m12 3 2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"/>',
        'flag' => '<path d="M5 21V4"/><path d="M5 4h11l-2 4 2 4H5"/>',
        'search' => '<circle cx="11" cy="11" r="6.5"/><path d="m20 20-4.3-4.3"/>',
        'settings' => '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v2.2M12 19.3v2.2M4.6 4.6l1.6 1.6M17.8 17.8l1.6 1.6M2.5 12h2.2M19.3 12h2.2M4.6 19.4l1.6-1.6M17.8 6.2l1.6-1.6"/>',
        'sliders' => '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
        'users' => '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><path d="M16 4.5a3.5 3.5 0 0 1 0 7M18.5 14.5A6.5 6.5 0 0 1 21.5 20"/>',
        'user' => '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
        'user-plus' => '<circle cx="10" cy="8" r="4"/><path d="M2.5 21a7.5 7.5 0 0 1 13.4-4.6"/><path d="M19 14v6M16 17h6"/>',
        'key' => '<circle cx="8" cy="15" r="4.5"/><path d="m11.2 11.8 8.8-8.8M17 6l2.5 2.5M14.5 8.5 16.5 10.5"/>',
        'globe' => '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
        'shield' => '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.4 7.5 9.5 4.3-1.1 7.5-4.9 7.5-9.5V6z"/>',
        'shield-check' => '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.4 7.5 9.5 4.3-1.1 7.5-4.9 7.5-9.5V6z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
        'chart' => '<path d="M4 20V4"/><path d="M4 20h16"/><path d="M8 16v-5M12 16V8M16 16v-3"/>',
        'activity' => '<path d="M3 12h4l3-8 4 16 3-8h4"/>',
        'file' => '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
        'file-text' => '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h6"/>',
        'code' => '<path d="m8 8-4 4 4 4M16 8l4 4-4 4M14 4l-4 16"/>',
        'webhook' => '<circle cx="6.5" cy="17" r="2.5"/><circle cx="17.5" cy="17" r="2.5"/><circle cx="12" cy="6.5" r="2.5"/><path d="m10.6 8.6-3 6M13.4 8.6l3 6M9 17h6"/>',
        'bell' => '<path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z"/><path d="M10 20a2 2 0 0 0 4 0"/>',
        'credit-card' => '<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M3 10h18M7 15h3"/>',
        'receipt' => '<path d="M6 3h12v18l-3-2-3 2-3-2-3 2z"/><path d="M9 8h6M9 12h6M9 16h3"/>',
        'phone' => '<rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M10.5 18.5h3"/>',
        'laptop' => '<rect x="4" y="5" width="16" height="11" rx="1.5"/><path d="M2 19h20"/>',
        'android' => '<path d="M6 10h12v7a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2z"/><path d="M6 9a6 6 0 0 1 12 0"/><path d="m8 4 1.5 2.2M16 4l-1.5 2.2M3.5 11v5M20.5 11v5"/><circle cx="9.5" cy="7.5" r=".6" fill="currentColor"/><circle cx="14.5" cy="7.5" r=".6" fill="currentColor"/>',
        'apple' => '<path d="M15.5 7.5c-1.3 0-2.2.8-3.5.8s-2.1-.8-3.4-.8C6.3 7.5 4.5 9.6 4.5 12.7c0 3.6 2.5 8.3 4.6 8.3 1 0 1.6-.7 2.9-.7s1.8.7 2.9.7c2 0 3.4-3 4.1-4.8a4 4 0 0 1-.5-7.1 4.2 4.2 0 0 0-3-1.6z"/><path d="M12.3 5.6c.3-1.5 1.5-2.6 2.9-2.8-.1 1.6-1.3 2.9-2.9 2.8z"/>',
        'check' => '<path d="m4.5 12.5 4.5 4.5L19.5 6.5"/>',
        'check-circle' => '<circle cx="12" cy="12" r="9"/><path d="m8 12.3 2.7 2.7L16 9.7"/>',
        'x' => '<path d="M6 6l12 12M18 6 6 18"/>',
        'x-circle' => '<circle cx="12" cy="12" r="9"/><path d="m9 9 6 6M15 9l-6 6"/>',
        'alert-circle' => '<circle cx="12" cy="12" r="9"/><path d="M12 7.5v5.5"/><circle cx="12" cy="16.3" r=".7" fill="currentColor"/>',
        'alert-triangle' => '<path d="M10.3 4 2.6 17.5A2 2 0 0 0 4.3 20.5h15.4a2 2 0 0 0 1.7-3L13.7 4a2 2 0 0 0-3.4 0z"/><path d="M12 9.5v4"/><circle cx="12" cy="16.8" r=".7" fill="currentColor"/>',
        'info' => '<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5"/><circle cx="12" cy="7.7" r=".7" fill="currentColor"/>',
        'help' => '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.3a2.6 2.6 0 0 1 5 .9c0 1.7-2.5 2.1-2.5 3.8"/><circle cx="12" cy="17" r=".7" fill="currentColor"/>',
        'chevron-right' => '<path d="m9 5 7 7-7 7"/>',
        'chevron-left' => '<path d="m15 5-7 7 7 7"/>',
        'chevron-down' => '<path d="m5 9 7 7 7-7"/>',
        'chevron-up' => '<path d="m5 15 7-7 7 7"/>',
        'arrow-right' => '<path d="M4 12h15M13 6l6 6-6 6"/>',
        'arrow-left' => '<path d="M20 12H5M11 6l-6 6 6 6"/>',
        'plus' => '<path d="M12 5v14M5 12h14"/>',
        'minus' => '<path d="M5 12h14"/>',
        'copy' => '<rect x="8.5" y="8.5" width="12" height="12" rx="2"/><path d="M15.5 8.5V5.5a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h3"/>',
        'external' => '<path d="M14 4h6v6M20 4l-9 9"/><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>',
        'log-out' => '<path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3"/><path d="M10 16 6 12l4-4M6 12h10"/>',
        'refresh' => '<path d="M20 11a8 8 0 0 0-14.3-4.9L4 8"/><path d="M4 4v4h4"/><path d="M4 13a8 8 0 0 0 14.3 4.9L20 16"/><path d="M20 20v-4h-4"/>',
        'download' => '<path d="M12 4v11M7 10l5 5 5-5"/><path d="M4 19h16"/>',
        'upload' => '<path d="M12 16V5M7 10l5-5 5 5"/><path d="M4 19h16"/>',
        'eye' => '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="3"/>',
        'eye-off' => '<path d="M4 4l16 16"/><path d="M10.6 6A9.6 9.6 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.9 3.7M6.5 7.6A16.7 16.7 0 0 0 2.5 12s3.5 6.5 9.5 6.5a9 9 0 0 0 4-.9"/><path d="M9.9 10a3 3 0 0 0 4.1 4.1"/>',
        'lock' => '<rect x="5" y="10.5" width="14" height="10" rx="2"/><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3"/>',
        'unlock' => '<rect x="5" y="10.5" width="14" height="10" rx="2"/><path d="M8 10.5V7.5a4 4 0 0 1 7.6-1.7"/>',
        'clock' => '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        'calendar' => '<rect x="3.5" y="5" width="17" height="15.5" rx="2"/><path d="M3.5 10h17M8 3v4M16 3v4"/>',
        'filter' => '<path d="M4 5h16l-6 7.5V19l-4 1.5v-8z"/>',
        'menu' => '<path d="M4 7h16M4 12h16M4 17h16"/>',
        'sun' => '<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4"/>',
        'moon' => '<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z"/>',
        'paperclip' => '<path d="m20 11.5-7.8 7.8a5 5 0 0 1-7-7l8.5-8.5a3.3 3.3 0 0 1 4.7 4.7l-8.4 8.4a1.7 1.7 0 0 1-2.4-2.4l7.7-7.7"/>',
        'reply' => '<path d="M10 8 5 12l5 4"/><path d="M5 12h9a5 5 0 0 1 5 5v1"/>',
        'reply-all' => '<path d="M9 8l-5 4 5 4"/><path d="M13 8l-5 4 5 4"/><path d="M8 12h7a5 5 0 0 1 5 5v1"/>',
        'forward' => '<path d="m14 8 5 4-5 4"/><path d="M19 12h-9a5 5 0 0 0-5 5v1"/>',
        'edit' => '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="m13.5 6.5 4 4"/>',
        'more' => '<circle cx="5.5" cy="12" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="18.5" cy="12" r="1.3" fill="currentColor"/>',
        'server' => '<rect x="3.5" y="4" width="17" height="7" rx="2"/><rect x="3.5" y="13" width="17" height="7" rx="2"/><path d="M7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6"/>',
        'layers' => '<path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/>',
        'book' => '<path d="M5 4.5A1.5 1.5 0 0 1 6.5 3H19v15H6.5A1.5 1.5 0 0 0 5 19.5z"/><path d="M5 19.5A1.5 1.5 0 0 0 6.5 21H19v-3"/>',
        'home' => '<path d="m3.5 11 8.5-7 8.5 7"/><path d="M5.5 9.5V20h13V9.5"/><path d="M10 20v-5h4v5"/>',
        'building' => '<rect x="4" y="3" width="11" height="18" rx="1"/><path d="M15 9h4a1 1 0 0 1 1 1v11h-5"/><path d="M8 7h3M8 11h3M8 15h3"/>',
        'at-sign' => '<circle cx="12" cy="12" r="4"/><path d="M16 8v5a2.5 2.5 0 0 0 5 0v-1a9 9 0 1 0-3.5 7.1"/>',
        'qr' => '<rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1"/><rect x="14" y="3.5" width="6.5" height="6.5" rx="1"/><rect x="3.5" y="14" width="6.5" height="6.5" rx="1"/><path d="M14 14h2.5v2.5H14zM18 18h2.5v2.5H18zM14 19h1.5M19 14v1.5"/>',
        'link' => '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
        'zap' => '<path d="M13 2.5 4.5 13.5h6l-1 8 8.5-11h-6z"/>',
        'sparkles' => '<path d="M12 3v4M12 17v4M3 12h4M17 12h4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8"/>',
        'printer' => '<path d="M7 9V3.5h10V9"/><rect x="3.5" y="9" width="17" height="8" rx="2"/><path d="M7 14h10v6.5H7z"/>',
        'pause' => '<path d="M9 5v14M15 5v14"/>',
        'play' => '<path d="m7 4.5 12 7.5-12 7.5z"/>',
        'ban' => '<circle cx="12" cy="12" r="9"/><path d="m5.6 5.6 12.8 12.8"/>',
        'wifi' => '<path d="M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.6 16a5 5 0 0 1 6.8 0"/><circle cx="12" cy="19" r=".8" fill="currentColor"/>',
        'smartphone-check' => '<rect x="5.5" y="2.5" width="11" height="19" rx="2.5"/><path d="m8.5 12 2 2 3.5-3.5"/>',
        'contact' => '<rect x="3.5" y="4" width="17" height="16" rx="2"/><circle cx="12" cy="10.5" r="2.7"/><path d="M7.5 17a4.6 4.6 0 0 1 9 0"/>',
        'list' => '<path d="M9 6h11M9 12h11M9 18h11"/><circle cx="4.5" cy="6" r=".8" fill="currentColor"/><circle cx="4.5" cy="12" r=".8" fill="currentColor"/><circle cx="4.5" cy="18" r=".8" fill="currentColor"/>',
        'grid' => '<rect x="4" y="4" width="7" height="7" rx="1.2"/><rect x="13" y="4" width="7" height="7" rx="1.2"/><rect x="4" y="13" width="7" height="7" rx="1.2"/><rect x="13" y="13" width="7" height="7" rx="1.2"/>',
        'terminal' => '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M12 15h5"/>',
        'dollar' => '<path d="M12 3v18"/><path d="M16.5 7.5c0-1.7-2-3-4.5-3s-4.5 1.3-4.5 3 1.7 2.6 4.5 3.3 4.5 1.6 4.5 3.5-2 3.2-4.5 3.2-4.5-1.3-4.5-3"/>',
    ];

    public static function svg(string $name, string $class = '', ?string $label = null): string
    {
        $paths = self::PATHS[$name] ?? self::PATHS['help'];
        $a11y = $label === null
            ? ' aria-hidden="true" focusable="false"'
            : ' role="img" aria-label="' . htmlspecialchars($label, ENT_QUOTES) . '"';
        return '<svg class="icon' . ($class !== '' ? ' ' . htmlspecialchars($class, ENT_QUOTES) : '') . '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"' . $a11y . '>' . $paths . '</svg>';
    }

    public static function has(string $name): bool
    {
        return isset(self::PATHS[$name]);
    }
}
