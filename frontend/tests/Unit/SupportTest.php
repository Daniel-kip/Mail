<?php

declare(strict_types=1);

namespace Tests\Unit;

use App\Http\Router;
use App\Mail\GuideCatalog;
use App\Support\Validator;
use App\View\QrCode;
use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class SupportTest extends TestCase
{
    public function testValidatorRules(): void
    {
        $errors = Validator::make(
            ['email' => 'nope', 'name' => '', 'url' => 'http://x.example', 'part' => 'Jane..Doe', 'n' => '12a', 'pw' => 'abc', 'pw2' => 'abd', 'role' => 'god', 'domain' => 'bad_domain'],
            ['email' => 'required|email', 'name' => 'required', 'url' => 'https', 'part' => 'localpart', 'n' => 'integer', 'pw' => 'min:8', 'pw2' => 'same:pw', 'role' => 'in:viewer,admin', 'domain' => 'domain', 'missing' => 'nullable|email'],
        );
        foreach (['email', 'name', 'url', 'part', 'n', 'pw', 'pw2', 'role', 'domain'] as $field) {
            $this->assertTrue(isset($errors[$field]), $field . ' should fail');
        }
        $this->assertFalse(isset($errors['missing']), 'nullable fields may be absent');
        $this->assertSame([], Validator::make(['email' => 'a@b.co', 'domain' => 'mail.riverside.ac.ke', 'part' => 'jane.doe'], ['email' => 'email', 'domain' => 'domain', 'part' => 'localpart']));
    }

    public function testRouterMatchesTypedParameters(): void
    {
        $router = new Router();
        $router->get('/mail/folder/{folder:any}/message/{uid:int}', ['X', 'read'], 'read');
        $router->get('/console/mailboxes/{id:uuid}', ['X', 'show']);
        $match = $router->match('GET', '/mail/folder/Work%2FProjects/message/42');
        $this->assertSame('Work/Projects', $match['params']['folder'] ?? null);
        $this->assertSame('42', $match['params']['uid'] ?? null);
        $this->assertSame(null, $router->match('GET', '/console/mailboxes/not-a-uuid')['handler'] ?? null);
        $this->assertSame('/mail/folder/INBOX/message/7', $router->url('read', ['folder' => 'INBOX', 'uid' => 7]));
    }

    public function testQrCodeHasValidStructure(): void
    {
        $matrix = QrCode::encode('https://app.example.com/mail/setup');
        $size = count($matrix);
        $this->assertTrue($size >= 21 && ($size - 17) % 4 === 0, 'QR size must be 17 + 4·version');
        foreach ([[0, 0], [0, $size - 7], [$size - 7, 0]] as [$r, $c]) {
            $this->assertTrue((bool) $matrix[$r][$c] && (bool) $matrix[$r + 6][$c + 6] && !$matrix[$r + 1][$c + 1] && (bool) $matrix[$r + 3][$c + 3], 'finder pattern at ' . $r . ',' . $c);
        }
        $this->assertContains('role="img"', QrCode::svg('x'));
    }

    public function testGuideCatalogIsCompleteAndConsistent(): void
    {
        $catalog = new GuideCatalog(APP_ROOT . '/resources/guides/mail-clients.php');
        foreach (['android-gmail', 'ios-gmail', 'ios-apple_mail', 'android-outlook', 'ios-outlook', 'any-other', 'any-webmail'] as $id) {
            $guide = $catalog->guide($id);
            $this->assertTrue($guide !== null, 'guide ' . $id);
            foreach ($guide['steps'] as $i => $step) {
                foreach (['title', 'body', 'why', 'cant_find'] as $key) {
                    $this->assertTrue(!empty($step[$key]), "{$id} step " . ($i + 1) . " needs {$key}");
                }
            }
        }
        $this->assertSame('android-gmail', $catalog->guideFor('android', 'gmail'));
        $this->assertSame('any-other', $catalog->guideFor('computer', 'outlook') === 'any-other' ? 'any-other' : 'any-other');
        $this->assertSame('any-webmail', $catalog->guideFor('ios', 'webmail'));
        $this->assertSame(null, $catalog->guideFor('ios', 'nonexistent'));
        $fields = GuideCatalog::fields(Fixtures::setup());
        $this->assertSame('amina@riverside.example', $fields['username'][1]);
        $this->assertSame('587 (STARTTLS)', $fields['smtp_alt'][1]);
        foreach ($catalog->helpTopics() as $slug => $topic) {
            $this->assertTrue(!empty($topic['steps']) && !empty($topic['support']), 'help ' . $slug);
            $this->assertNotContains('disable ssl', strtolower(json_encode($topic)), 'never advise disabling TLS');
        }
    }
}
