<?php

declare(strict_types=1);

namespace App\View;

/**
 * A small, safe Markdown renderer for the documentation wiki.
 *
 * Supports ATX headings (with anchor ids), paragraphs, fenced code blocks,
 * GitHub-style tables, ordered and unordered lists (one nested level, with
 * wrapped continuation lines), and inline code, bold, italic and links.
 * Every character of text is HTML-escaped: Markdown cannot inject markup.
 * Links pass through a callback that maps a target to an href, or to null to
 * render the label as plain text (for pages the reader may not open).
 */
final class Markdown
{
    /** @var list<array{id: string, text: string}> second-level headings, for "On this page" */
    public array $toc = [];

    /** @param \Closure(string): ?string $link */
    public function __construct(private readonly \Closure $link)
    {
    }

    public function render(string $markdown): string
    {
        $this->toc = [];
        $lines = preg_split('/\r\n|\n|\r/', $markdown) ?: [];
        $html = '';
        $count = count($lines);
        for ($i = 0; $i < $count;) {
            $line = $lines[$i];
            if (trim($line) === '') {
                $i++;
                continue;
            }
            if (preg_match('/^```\s*([\w-]*)\s*$/', $line, $m)) {
                $code = [];
                for ($i++; $i < $count && !preg_match('/^```\s*$/', $lines[$i]); $i++) {
                    $code[] = $lines[$i];
                }
                $i++;
                $lang = $m[1] !== '' ? ' data-lang="' . self::e($m[1]) . '"' : '';
                $html .= '<pre class="doc-code"' . $lang . '><code>' . self::e(implode("\n", $code)) . '</code></pre>';
                continue;
            }
            if (preg_match('/^(#{1,4})\s+(.+?)\s*#*\s*$/', $line, $m)) {
                $level = max(2, strlen($m[1]));
                $text = $this->inline($m[2]);
                $id = self::slug(strip_tags($text));
                if ($level === 2) {
                    $this->toc[] = ['id' => $id, 'text' => html_entity_decode(strip_tags($text), ENT_QUOTES)];
                }
                $html .= "<h{$level} id=\"{$id}\"><a class=\"doc-anchor\" href=\"#{$id}\" aria-hidden=\"true\">#</a>{$text}</h{$level}>";
                $i++;
                continue;
            }
            if (preg_match('/^\s*(-{3,}|\*{3,})\s*$/', $line)) {
                $html .= '<hr>';
                $i++;
                continue;
            }
            if (str_starts_with(ltrim($line), '|') && isset($lines[$i + 1]) && preg_match('/^\s*\|?\s*:?-{2,}/', $lines[$i + 1])) {
                $html .= $this->table($lines, $i);
                continue;
            }
            if (preg_match('/^([-*]|\d+\.)\s+/', $line)) {
                $html .= $this->list($lines, $i);
                continue;
            }
            $para = [];
            while ($i < $count && trim($lines[$i]) !== '' && !preg_match('/^(```|#{1,4}\s|[-*]\s|\d+\.\s|\s*\|)/', $lines[$i])) {
                $para[] = trim($lines[$i]);
                $i++;
            }
            if ($para === []) {
                $para[] = trim($lines[$i]);
                $i++;
            }
            $html .= '<p>' . $this->inline(implode(' ', $para)) . '</p>';
        }
        return $html;
    }

    /** The first paragraph as plain text (a page's lead sentence). */
    public static function lead(string $markdown): string
    {
        foreach (preg_split('/\n\s*\n/', $markdown) ?: [] as $block) {
            $block = trim($block);
            if ($block === '' || preg_match('/^(#|\||```|[-*]\s)/', $block)) {
                continue;
            }
            $text = preg_replace(['/\[([^\]]+)\]\([^)]*\)/', '/[*`_]/'], ['$1', ''], $block) ?? $block;
            return trim((string) preg_replace('/\s+/', ' ', $text));
        }
        return '';
    }

    public static function slug(string $text): string
    {
        $slug = strtolower(trim((string) preg_replace('/[^A-Za-z0-9]+/', '-', html_entity_decode($text, ENT_QUOTES)), '-'));
        return $slug !== '' ? $slug : 'section';
    }

    /** @param list<string> $lines */
    private function table(array $lines, int &$i): string
    {
        $row = static fn (string $l): array => array_map('trim', preg_split('/(?<!\\\\)\|/', trim(trim($l), '|')) ?: []);
        $head = $row($lines[$i]);
        $i += 2;
        // A header row of empty cells (`| | |`) marks a key/value "facts" table.
        $facts = implode('', $head) === '';
        $html = '<div class="table-wrap doc-table' . ($facts ? ' doc-facts' : '') . '"><table class="table">';
        if (!$facts) {
            $html .= '<thead><tr>';
            foreach ($head as $cell) {
                $html .= '<th>' . $this->inline(str_replace('\|', '|', $cell)) . '</th>';
            }
            $html .= '</tr></thead>';
        }
        $html .= '<tbody>';
        for ($n = count($lines); $i < $n && str_starts_with(ltrim($lines[$i]), '|'); $i++) {
            $html .= '<tr>';
            foreach ($row($lines[$i]) as $cell) {
                $html .= '<td>' . $this->inline(str_replace('\|', '|', $cell)) . '</td>';
            }
            $html .= '</tr>';
        }
        return $html . '</tbody></table></div>';
    }

    /** @param list<string> $lines */
    private function list(array $lines, int &$i): string
    {
        $ordered = (bool) preg_match('/^\d+\./', $lines[$i]);
        $items = [];
        for ($n = count($lines); $i < $n; $i++) {
            $line = $lines[$i];
            if (preg_match('/^([-*]|\d+\.)\s+(.*)$/', $line, $m)) {
                $items[] = ['text' => $m[2], 'children' => [], 'childOrdered' => false];
                continue;
            }
            if ($items !== [] && preg_match('/^\s{2,}([-*]|\d+\.)\s+(.*)$/', $line, $m)) {
                $last = count($items) - 1;
                $items[$last]['childOrdered'] = ctype_digit(rtrim($m[1], '.'));
                $items[$last]['children'][] = $m[2];
                continue;
            }
            if ($items !== [] && trim($line) !== '' && preg_match('/^\s+\S/', $line)) {
                $last = count($items) - 1;
                if ($items[$last]['children'] !== []) {
                    $items[$last]['children'][count($items[$last]['children']) - 1] .= ' ' . trim($line);
                } else {
                    $items[$last]['text'] .= ' ' . trim($line);
                }
                continue;
            }
            break;
        }
        $tag = $ordered ? 'ol' : 'ul';
        $html = "<{$tag}>";
        foreach ($items as $item) {
            // Task-list items: "[ ] to do" and "[x] done".
            if (preg_match('/^\[([ xX])\]\s+(.*)$/s', $item['text'], $task)) {
                $done = $task[1] !== ' ';
                $html .= '<li class="doc-task"><span class="doc-check' . ($done ? ' is-done' : '') . '" role="img" aria-label="' . ($done ? 'Done' : 'To do') . '"></span>' . $this->inline($task[2]);
            } else {
                $html .= '<li>' . $this->inline($item['text']);
            }
            if ($item['children'] !== []) {
                $child = $item['childOrdered'] ? 'ol' : 'ul';
                $html .= "<{$child}>" . implode('', array_map(fn (string $c) => '<li>' . $this->inline($c) . '</li>', $item['children'])) . "</{$child}>";
            }
            $html .= '</li>';
        }
        return $html . "</{$tag}>";
    }

    private function inline(string $text): string
    {
        // Code spans first, so nothing inside them is interpreted.
        $parts = preg_split('/(`+)(.+?)\1/', $text, -1, PREG_SPLIT_DELIM_CAPTURE) ?: [$text];
        $out = '';
        for ($k = 0, $n = count($parts); $k < $n; $k++) {
            if ($k % 3 === 0) {
                $out .= $this->emphasis($parts[$k]);
            } elseif ($k % 3 === 2) {
                $out .= '<code>' . self::e($parts[$k]) . '</code>';
            }
        }
        return $out;
    }

    private function emphasis(string $text): string
    {
        $links = [];
        $text = (string) preg_replace_callback('/\[([^\]]+)\]\(([^)\s]+)\)|<(https?:\/\/[^>\s]+)>/', function (array $m) use (&$links): string {
            $links[] = isset($m[3]) ? [$m[3], $m[3]] : [$m[1], $m[2]];
            return "\x01" . (count($links) - 1) . "\x02";
        }, $text);
        $html = self::e($text);
        $html = (string) preg_replace('/\*\*(.+?)\*\*/', '<strong>$1</strong>', $html);
        $html = (string) preg_replace('/(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])/', '<em>$1</em>', $html);
        $html = (string) preg_replace('/(?<![\w])_(?!\s)(.+?)(?<!\s)_(?![\w])/', '<em>$1</em>', $html);
        return (string) preg_replace_callback('/\x01(\d+)\x02/', function (array $m) use ($links): string {
            [$label, $target] = $links[(int) $m[1]];
            $labelHtml = $this->emphasis($label);
            $href = ($this->link)($target);
            if ($href === null) {
                return '<span class="doc-ref">' . $labelHtml . '</span>';
            }
            $external = preg_match('#^https?://#', $href) === 1;
            return '<a href="' . self::e($href) . '"' . ($external ? ' rel="noopener noreferrer" target="_blank"' : '') . '>' . $labelHtml . '</a>';
        }, $html);
    }

    private static function e(string $text): string
    {
        return htmlspecialchars($text, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
    }
}
