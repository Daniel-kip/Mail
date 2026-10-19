<?php

declare(strict_types=1);

namespace App\Http\Controllers\PublicSite;

use App\Http\Controllers\Controller;
use App\Http\HttpException;
use App\Http\Request;
use App\Http\Response;

/**
 * Free deliverability tools. Network checks run in the API (rate-limited
 * per visitor, SSRF-guarded); the record generator runs here and never
 * leaves the server.
 */
final class ToolsController extends Controller
{
    /** slug => [icon, title, category, summary, keywords] */
    /** slug => [icon, name, category, summary, search keywords, colour tone] */
    public const TOOLS = [
        'domain' => ['tool-health', 'Domain health check', 'Deliverability', 'Score MX, SPF, DKIM, DMARC, MTA-STS, TLS-RPT, BIMI, reverse DNS and blocklists in one run.', 'spf dkim dmarc mx score audit', 'emerald'],
        'dns' => ['tool-dns', 'DNS lookup', 'DNS', 'Query A, AAAA, MX, TXT, CNAME, NS, SOA, CAA, SRV and PTR records through a validating public resolver.', 'mx txt a ns lookup dig', 'sky'],
        'generator' => ['tool-generator', 'DNS record generator', 'DNS', 'Build correct SPF, DMARC, MTA-STS and TLS-RPT records for your providers, ready to paste.', 'spf dmarc generator mta-sts tls-rpt create', 'violet'],
        'headers' => ['tool-headers', 'Email header analyzer', 'Diagnose', 'Trace every hop, delay and TLS use, and read the SPF, DKIM and DMARC verdicts.', 'headers spam trace received', 'amber'],
        'blocklist' => ['tool-blocklist', 'Blocklist checker', 'Reputation', 'Check an IP or every mail server of a domain against SpamCop, Barracuda, PSBL, UCEPROTECT and Mailspike.', 'blacklist dnsbl reputation spamcop barracuda', 'rose'],
        'ports' => ['tool-ports', 'Mail port checker', 'Connectivity', 'Test SMTP 25/465/587, IMAP 993/143, POP3 995/110 and HTTPS 443 from the internet.', 'ports smtp imap pop3 open firewall', 'indigo'],
    ];

    private const DNS_TYPES = ['A', 'AAAA', 'MX', 'TXT', 'CNAME', 'NS', 'SOA', 'CAA', 'SRV', 'PTR'];

    public function index(Request $request): Response
    {
        // Older links used /tools?domain=…
        if (trim((string) $request->query('domain', '')) !== '') {
            return $this->redirect('/tools/domain?' . http_build_query(['domain' => $request->query('domain')]));
        }
        return $this->render('public/tools/hub', [
            'title' => 'Email & DNS tools',
            'description' => 'Free email deliverability and DNS tools: domain health score, DNS lookup, SPF and DMARC generator, header analyzer, blocklist and port checker.',
            'tools' => self::TOOLS,
        ], 'public');
    }

    public function show(Request $request): Response
    {
        $tool = $request->param('tool');
        if (!isset(self::TOOLS[$tool])) {
            throw HttpException::notFound();
        }
        $data = [
            'title' => self::TOOLS[$tool][1], 'description' => self::TOOLS[$tool][3], 'tool' => $tool, 'tools' => self::TOOLS,
            'result' => null, 'error' => null, 'input' => [], 'types' => self::DNS_TYPES,
        ];
        switch ($tool) {
            case 'domain':
                $data['input']['domain'] = trim((string) $request->query('domain', ''));
                if ($data['input']['domain'] !== '') {
                    $this->fetch($data, 'GET', '/v1/tools/domain', ['query' => ['domain' => $data['input']['domain']], 'timeout' => 45]);
                }
                break;
            case 'dns':
                $data['input'] = ['name' => trim((string) $request->query('name', '')), 'type' => strtoupper((string) $request->query('type', 'A'))];
                if ($data['input']['name'] !== '') {
                    $this->fetch($data, 'GET', '/v1/tools/dns', ['query' => $data['input']]);
                }
                break;
            case 'blocklist':
            case 'ports':
                $data['input']['target'] = trim((string) $request->query('target', ''));
                if ($data['input']['target'] !== '') {
                    $this->fetch($data, 'GET', '/v1/tools/' . $tool, ['query' => ['target' => $data['input']['target']], 'timeout' => 45]);
                }
                break;
            case 'headers':
                $data['result'] = $this->session()->flashed('header_analysis');
                break;
            case 'generator':
                $data['input'] = $request->allQuery();
                $data['spfInclude'] = (string) $this->app->config->get('mail.spf_include', '');
                $data['result'] = isset($data['input']['domain']) ? self::generate($data['input'], $data['spfInclude']) : null;
                if (is_array($data['result']) && isset($data['result']['error'])) {
                    $data['error'] = $data['result']['error'];
                    $data['result'] = null;
                }
                break;
        }
        return $this->render('public/tools/show', $data, 'public');
    }

    public function headers(Request $request): Response
    {
        $raw = (string) $request->input('headers', '');
        if (trim($raw) === '') {
            $this->flash('error', 'Paste the full headers of a message first.');
            return $this->redirect('/tools/headers');
        }
        $res = $this->public('POST', '/v1/tools/headers', ['json' => ['headers' => $raw]]);
        if (!$res->ok()) {
            $this->flash('error', self::describe($res));
            return $this->redirect('/tools/headers');
        }
        $this->session()->flash('header_analysis', $res->json());
        return $this->redirect('/tools/headers#result');
    }

    private function fetch(array &$data, string $method, string $path, array $options): void
    {
        $res = $this->public($method, $path, $options);
        if ($res->ok()) {
            $data['result'] = $res->json();
        } else {
            $data['error'] = self::describe($res);
        }
    }

    /**
     * Build SPF, DMARC, MTA-STS and TLS-RPT records from the form.
     *
     * @return array<string, mixed>
     */
    public static function generate(array $in, string $platformInclude = ''): array
    {
        $domain = strtolower(trim((string) ($in['domain'] ?? '')));
        $domain = preg_replace('~^https?://~', '', $domain) ?? $domain;
        if (!preg_match('/^(?=.{4,253}$)([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/', $domain)) {
            return ['error' => 'Enter a domain such as yourcompany.co.ke'];
        }
        $providers = [
            'mailvora' => $platformInclude !== '' ? 'include:' . preg_replace('/^include:/', '', $platformInclude) : null,
            'google' => 'include:_spf.google.com',
            'microsoft' => 'include:spf.protection.outlook.com',
            'zoho' => 'include:zoho.com',
            'sendgrid' => 'include:sendgrid.net',
            'mailchimp' => 'include:servers.mcsv.net',
        ];
        $mechanisms = [];
        if (!empty($in['mx'])) {
            $mechanisms[] = 'mx';
        }
        foreach ((array) ($in['providers'] ?? []) as $p) {
            if (isset($providers[$p]) && $providers[$p] !== null) {
                $mechanisms[] = $providers[$p];
            }
        }
        foreach (preg_split('/[\s,]+/', (string) ($in['ips'] ?? '')) ?: [] as $ip) {
            if ($ip === '') {
                continue;
            }
            [$addr] = explode('/', $ip);
            if (filter_var($addr, FILTER_VALIDATE_IP, FILTER_FLAG_IPV4)) {
                $mechanisms[] = 'ip4:' . $ip;
            } elseif (filter_var($addr, FILTER_VALIDATE_IP, FILTER_FLAG_IPV6)) {
                $mechanisms[] = 'ip6:' . $ip;
            }
        }
        $all = in_array($in['all'] ?? '', ['-all', '~all'], true) ? $in['all'] : '~all';
        $mechanisms = array_values(array_unique($mechanisms));
        $spf = trim('v=spf1 ' . implode(' ', $mechanisms) . ' ' . $all);
        $lookups = count(array_filter($mechanisms, static fn ($m) => str_starts_with($m, 'include:') || $m === 'mx' || $m === 'a'));

        $policy = in_array($in['policy'] ?? '', ['none', 'quarantine', 'reject'], true) ? $in['policy'] : 'none';
        $dmarc = 'v=DMARC1; p=' . $policy;
        if ($policy !== 'none' && in_array($in['sp'] ?? '', ['none', 'quarantine', 'reject'], true)) {
            $dmarc .= '; sp=' . $in['sp'];
        }
        $pct = (int) ($in['pct'] ?? 100);
        if ($policy !== 'none' && $pct > 0 && $pct < 100) {
            $dmarc .= '; pct=' . $pct;
        }
        $rua = trim((string) ($in['rua'] ?? ''));
        if ($rua !== '' && filter_var($rua, FILTER_VALIDATE_EMAIL)) {
            $dmarc .= '; rua=mailto:' . $rua;
        }
        $dmarc .= '; adkim=' . (($in['adkim'] ?? 'r') === 's' ? 's' : 'r') . '; aspf=' . (($in['aspf'] ?? 'r') === 's' ? 's' : 'r');

        $records = [
            ['type' => 'TXT', 'host' => $domain, 'value' => $spf, 'purpose' => 'SPF: servers allowed to send for ' . $domain],
            ['type' => 'TXT', 'host' => '_dmarc.' . $domain, 'value' => $dmarc, 'purpose' => 'DMARC: what receivers do with mail that fails SPF/DKIM'],
        ];
        $notes = [];
        if ($lookups > 10) {
            $notes[] = ['fail', "This SPF record needs about {$lookups} DNS lookups; the limit is 10. Remove a provider or use ip4: ranges."];
        } elseif ($lookups >= 8) {
            $notes[] = ['warn', "About {$lookups} of 10 SPF DNS lookups used (each include may add more)."];
        }
        if ($mechanisms === []) {
            $notes[] = ['warn', 'No senders selected: this record says the domain sends no email at all.'];
        }
        if ($policy === 'none') {
            $notes[] = ['info', 'p=none only monitors. Move to quarantine, then reject, once reports show all your senders pass.'];
        }

        $mx = array_values(array_filter(array_map('trim', preg_split('/[\s,]+/', (string) ($in['mx_hosts'] ?? '')) ?: [])));
        $mode = in_array($in['sts_mode'] ?? '', ['testing', 'enforce'], true) ? $in['sts_mode'] : null;
        $policyFile = null;
        if ($mode !== null && $mx !== []) {
            $id = gmdate('YmdHis');
            $records[] = ['type' => 'TXT', 'host' => '_mta-sts.' . $domain, 'value' => 'v=STSv1; id=' . $id, 'purpose' => 'MTA-STS: announces your policy (change id when the policy changes)'];
            $policyFile = "version: STSv1\nmode: {$mode}\n" . implode('', array_map(static fn ($h) => "mx: {$h}\n", $mx)) . "max_age: 604800\n";
            $notes[] = ['info', "Publish the policy file at https://mta-sts.{$domain}/.well-known/mta-sts.txt with a valid certificate."];
        }
        $tls = trim((string) ($in['tlsrpt'] ?? ''));
        if ($tls !== '' && filter_var($tls, FILTER_VALIDATE_EMAIL)) {
            $records[] = ['type' => 'TXT', 'host' => '_smtp._tls.' . $domain, 'value' => 'v=TLSRPTv1; rua=mailto:' . $tls, 'purpose' => 'TLS-RPT: daily reports about TLS delivery failures'];
        }
        return ['domain' => $domain, 'records' => $records, 'policy_file' => $policyFile, 'notes' => $notes];
    }
}
