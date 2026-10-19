<?php

declare(strict_types=1);

namespace App\View;

/**
 * QR code encoder (ISO/IEC 18004): byte mode, error correction level M,
 * versions 1–40, rendered as inline SVG. Used for authenticator enrollment
 * (the TOTP secret never leaves this server) and to open the phone setup
 * guide on the phone. QR codes here never contain passwords.
 */
final class QrCode
{
    private const ECC_PER_BLOCK_M = [-1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28];
    private const BLOCKS_M = [-1, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49];
    private const ECL_M_BITS = 0;

    private static function rawDataModules(int $ver): int
    {
        $result = (16 * $ver + 128) * $ver + 64;
        if ($ver >= 2) {
            $numAlign = intdiv($ver, 7) + 2;
            $result -= (25 * $numAlign - 10) * $numAlign - 55;
            if ($ver >= 7) {
                $result -= 36;
            }
        }
        return $result;
    }

    private static function dataCodewords(int $ver): int
    {
        return intdiv(self::rawDataModules($ver), 8) - self::ECC_PER_BLOCK_M[$ver] * self::BLOCKS_M[$ver];
    }

    private static function gfMul(int $x, int $y): int
    {
        $z = 0;
        for ($i = 7; $i >= 0; $i--) {
            $z = ($z << 1) ^ (($z >> 7) * 0x11d);
            $z ^= (($y >> $i) & 1) * $x;
        }
        return $z & 0xff;
    }

    /** @return list<int> */
    private static function rsDivisor(int $degree): array
    {
        $result = array_fill(0, $degree, 0);
        $result[$degree - 1] = 1;
        $root = 1;
        for ($i = 0; $i < $degree; $i++) {
            for ($j = 0; $j < $degree; $j++) {
                $result[$j] = self::gfMul($result[$j], $root);
                if ($j + 1 < $degree) {
                    $result[$j] ^= $result[$j + 1];
                }
            }
            $root = self::gfMul($root, 0x02);
        }
        return $result;
    }

    /** @return list<int> */
    private static function rsRemainder(array $data, array $divisor): array
    {
        $result = array_fill(0, count($divisor), 0);
        foreach ($data as $b) {
            $factor = $b ^ array_shift($result);
            $result[] = 0;
            foreach ($divisor as $i => $coef) {
                $result[$i] ^= self::gfMul($coef, $factor);
            }
        }
        return $result;
    }

    /** @return list<int> */
    private static function alignmentPositions(int $ver, int $size): array
    {
        if ($ver === 1) {
            return [];
        }
        $numAlign = intdiv($ver, 7) + 2;
        $step = $ver === 32 ? 26 : (int) ceil(($ver * 4 + 4) / ($numAlign * 2 - 2)) * 2;
        $result = [6];
        for ($pos = $size - 7; count($result) < $numAlign; $pos -= $step) {
            array_splice($result, 1, 0, [$pos]);
        }
        return $result;
    }

    /** @return list<list<bool>> module matrix, true = dark */
    public static function encode(string $text): array
    {
        $bytes = array_values(unpack('C*', $text) ?: []);
        $ver = 1;
        for (; $ver <= 40; $ver++) {
            $ccBits = $ver < 10 ? 8 : 16;
            if (4 + $ccBits + count($bytes) * 8 <= self::dataCodewords($ver) * 8) {
                break;
            }
        }
        if ($ver > 40) {
            throw new \InvalidArgumentException('Data too long for a QR code');
        }
        $size = $ver * 4 + 17;

        $bits = [];
        $put = static function (int $val, int $len) use (&$bits): void {
            for ($i = $len - 1; $i >= 0; $i--) {
                $bits[] = ($val >> $i) & 1;
            }
        };
        $put(0b0100, 4);
        $put(count($bytes), $ver < 10 ? 8 : 16);
        foreach ($bytes as $b) {
            $put($b, 8);
        }
        $capacity = self::dataCodewords($ver) * 8;
        $put(0, min(4, $capacity - count($bits)));
        $put(0, (8 - (count($bits) % 8)) % 8);
        for ($pad = 0xec; count($bits) < $capacity; $pad ^= 0xec ^ 0x11) {
            $put($pad, 8);
        }
        $data = [];
        for ($i = 0, $n = count($bits); $i < $n; $i += 8) {
            $byte = 0;
            for ($j = 0; $j < 8; $j++) {
                $byte = ($byte << 1) | $bits[$i + $j];
            }
            $data[] = $byte;
        }

        $numBlocks = self::BLOCKS_M[$ver];
        $eccLen = self::ECC_PER_BLOCK_M[$ver];
        $rawCodewords = intdiv(self::rawDataModules($ver), 8);
        $numShort = $numBlocks - ($rawCodewords % $numBlocks);
        $shortLen = intdiv($rawCodewords, $numBlocks);
        $divisor = self::rsDivisor($eccLen);
        $blocks = [];
        for ($i = 0, $k = 0; $i < $numBlocks; $i++) {
            $dat = array_slice($data, $k, $shortLen - $eccLen + ($i < $numShort ? 0 : 1));
            $k += count($dat);
            $ecc = self::rsRemainder($dat, $divisor);
            if ($i < $numShort) {
                $dat[] = 0;
            }
            $blocks[] = array_merge($dat, $ecc);
        }
        $codewords = [];
        $blockLen = count($blocks[0]);
        for ($i = 0; $i < $blockLen; $i++) {
            foreach ($blocks as $j => $b) {
                if ($i !== $shortLen - $eccLen || $j >= $numShort) {
                    $codewords[] = $b[$i];
                }
            }
        }

        $modules = array_fill(0, $size, array_fill(0, $size, false));
        $isFn = array_fill(0, $size, array_fill(0, $size, false));
        $setFn = static function (int $x, int $y, bool $dark) use (&$modules, &$isFn): void {
            $modules[$y][$x] = $dark;
            $isFn[$y][$x] = true;
        };
        for ($i = 0; $i < $size; $i++) {
            $setFn(6, $i, $i % 2 === 0);
            $setFn($i, 6, $i % 2 === 0);
        }
        $finder = static function (int $cx, int $cy) use ($setFn, $size): void {
            for ($dy = -4; $dy <= 4; $dy++) {
                for ($dx = -4; $dx <= 4; $dx++) {
                    $x = $cx + $dx;
                    $y = $cy + $dy;
                    if ($x >= 0 && $x < $size && $y >= 0 && $y < $size) {
                        $d = max(abs($dx), abs($dy));
                        $setFn($x, $y, $d !== 2 && $d !== 4);
                    }
                }
            }
        };
        $finder(3, 3);
        $finder($size - 4, 3);
        $finder(3, $size - 4);
        $align = self::alignmentPositions($ver, $size);
        $last = count($align) - 1;
        foreach ($align as $i => $ay) {
            foreach ($align as $j => $ax) {
                if (($i === 0 && $j === 0) || ($i === 0 && $j === $last) || ($i === $last && $j === 0)) {
                    continue;
                }
                for ($dy = -2; $dy <= 2; $dy++) {
                    for ($dx = -2; $dx <= 2; $dx++) {
                        $setFn($ax + $dx, $ay + $dy, max(abs($dx), abs($dy)) !== 1);
                    }
                }
            }
        }
        $drawFormat = static function (int $mask) use ($setFn, $size): void {
            $d = (self::ECL_M_BITS << 3) | $mask;
            $rem = $d;
            for ($i = 0; $i < 10; $i++) {
                $rem = ($rem << 1) ^ (($rem >> 9) * 0x537);
            }
            $f = (($d << 10) | $rem) ^ 0x5412;
            $bit = static fn (int $i): bool => (($f >> $i) & 1) !== 0;
            for ($i = 0; $i <= 5; $i++) {
                $setFn(8, $i, $bit($i));
            }
            $setFn(8, 7, $bit(6));
            $setFn(8, 8, $bit(7));
            $setFn(7, 8, $bit(8));
            for ($i = 9; $i < 15; $i++) {
                $setFn(14 - $i, 8, $bit($i));
            }
            for ($i = 0; $i < 8; $i++) {
                $setFn($size - 1 - $i, 8, $bit($i));
            }
            for ($i = 8; $i < 15; $i++) {
                $setFn(8, $size - 15 + $i, $bit($i));
            }
            $setFn(8, $size - 8, true);
        };
        $drawFormat(0);
        if ($ver >= 7) {
            $rem = $ver;
            for ($i = 0; $i < 12; $i++) {
                $rem = ($rem << 1) ^ (($rem >> 11) * 0x1f25);
            }
            $v = ($ver << 12) | $rem;
            for ($i = 0; $i < 18; $i++) {
                $dark = (($v >> $i) & 1) !== 0;
                $a = $size - 11 + ($i % 3);
                $b = intdiv($i, 3);
                $setFn($a, $b, $dark);
                $setFn($b, $a, $dark);
            }
        }

        $idx = 0;
        $totalBits = count($codewords) * 8;
        for ($right = $size - 1; $right >= 1; $right -= 2) {
            if ($right === 6) {
                $right = 5;
            }
            for ($vert = 0; $vert < $size; $vert++) {
                for ($j = 0; $j < 2; $j++) {
                    $x = $right - $j;
                    $upward = (($right + 1) & 2) === 0;
                    $y = $upward ? $size - 1 - $vert : $vert;
                    if (!$isFn[$y][$x] && $idx < $totalBits) {
                        $modules[$y][$x] = (($codewords[$idx >> 3] >> (7 - ($idx & 7))) & 1) !== 0;
                        $idx++;
                    }
                }
            }
        }

        $mask = static function (int $m, int $x, int $y): bool {
            return match ($m) {
                0 => ($x + $y) % 2 === 0,
                1 => $y % 2 === 0,
                2 => $x % 3 === 0,
                3 => ($x + $y) % 3 === 0,
                4 => (intdiv($x, 3) + intdiv($y, 2)) % 2 === 0,
                5 => (($x * $y) % 2) + (($x * $y) % 3) === 0,
                6 => ((($x * $y) % 2) + (($x * $y) % 3)) % 2 === 0,
                default => ((($x + $y) % 2) + (($x * $y) % 3)) % 2 === 0,
            };
        };
        $applyMask = static function (int $m) use (&$modules, $isFn, $size, $mask): void {
            for ($y = 0; $y < $size; $y++) {
                for ($x = 0; $x < $size; $x++) {
                    if (!$isFn[$y][$x] && $mask($m, $x, $y)) {
                        $modules[$y][$x] = !$modules[$y][$x];
                    }
                }
            }
        };
        $penalty = static function () use (&$modules, $size): int {
            $p = 0;
            for ($y = 0; $y < $size; $y++) {
                $run = 1;
                for ($x = 1; $x < $size; $x++) {
                    if ($modules[$y][$x] === $modules[$y][$x - 1]) {
                        $run++;
                        $p += $run === 5 ? 3 : ($run > 5 ? 1 : 0);
                    } else {
                        $run = 1;
                    }
                }
            }
            for ($x = 0; $x < $size; $x++) {
                $run = 1;
                for ($y = 1; $y < $size; $y++) {
                    if ($modules[$y][$x] === $modules[$y - 1][$x]) {
                        $run++;
                        $p += $run === 5 ? 3 : ($run > 5 ? 1 : 0);
                    } else {
                        $run = 1;
                    }
                }
            }
            $dark = 0;
            for ($y = 0; $y < $size; $y++) {
                for ($x = 0; $x < $size; $x++) {
                    if ($modules[$y][$x]) {
                        $dark++;
                    }
                    if ($x < $size - 1 && $y < $size - 1) {
                        $c = $modules[$y][$x];
                        if ($c === $modules[$y][$x + 1] && $c === $modules[$y + 1][$x] && $c === $modules[$y + 1][$x + 1]) {
                            $p += 3;
                        }
                    }
                }
            }
            return $p + intdiv(abs($dark * 20 - $size * $size * 10), $size * $size) * 10;
        };
        $best = 0;
        $bestScore = PHP_INT_MAX;
        for ($m = 0; $m < 8; $m++) {
            $applyMask($m);
            $drawFormat($m);
            $score = $penalty();
            if ($score < $bestScore) {
                $bestScore = $score;
                $best = $m;
            }
            $applyMask($m);
        }
        $applyMask($best);
        $drawFormat($best);
        return $modules;
    }

    public static function svg(string $text, int $scale = 5, string $label = 'QR code'): string
    {
        $modules = self::encode($text);
        $size = count($modules);
        $quiet = 4;
        $total = $size + $quiet * 2;
        $d = '';
        foreach ($modules as $y => $row) {
            foreach ($row as $x => $dark) {
                if ($dark) {
                    $d .= 'M' . ($x + $quiet) . ',' . ($y + $quiet) . 'h1v1h-1z';
                }
            }
        }
        $px = $total * $scale;
        return '<svg class="qr" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' . $total . ' ' . $total . '" width="' . $px . '" height="' . $px
            . '" role="img" aria-label="' . htmlspecialchars($label, ENT_QUOTES) . '" shape-rendering="crispEdges">'
            . '<rect width="' . $total . '" height="' . $total . '" fill="#ffffff"/><path d="' . $d . '" fill="#000000"/></svg>';
    }
}
