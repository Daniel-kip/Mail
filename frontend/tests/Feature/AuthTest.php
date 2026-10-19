<?php

declare(strict_types=1);

namespace Tests\Feature;

use Tests\Support\Fixtures;
use Tests\Support\TestCase;

final class AuthTest extends TestCase
{
    public function setUp(): void
    {
        $this->boot();
    }

    public function testLoginPageAndSuccessfulSignInRedirectsToNext(): void
    {
        $this->assertContains('Sign in', $this->get('/login')->body());
        $this->api->onCallback('POST', '/v1/auth/login', function () {
            $this->api->signIn('console');
            return \App\Api\ApiResponse::fake(200, Fixtures::consoleSession());
        });
        $res = $this->post('/login', ['email' => 'grace@riverside.example', 'password' => 'correct horse battery', 'next' => '/console/domains']);
        $this->assertRedirect('/console/domains', $res);
    }

    public function testLoginRejectsOpenRedirects(): void
    {
        $this->api->on('POST', '/v1/auth/login', 200, Fixtures::consoleSession());
        $this->assertRedirect('/console', $this->post('/login', ['email' => 'grace@riverside.example', 'password' => 'x', 'next' => '//evil.example/']));
    }

    public function testWrongPasswordKeepsEmailButNeverThePassword(): void
    {
        $this->api->on('POST', '/v1/auth/login', 401, ['error' => ['code' => 'invalid_credentials', 'message' => 'Invalid email or password']]);
        $res = $this->post('/login', ['email' => 'grace@riverside.example', 'password' => 'secret-value-123']);
        $this->assertRedirect('/login', $res);
        $page = $this->get('/login')->body();
        $this->assertContains('Those details do not match an account.', $page);
        $this->assertContains('grace@riverside.example', $page);
        $this->assertNotContains('secret-value-123', $page);
    }

    public function testMfaChallengeFlow(): void
    {
        $this->api->onCallback('POST', '/v1/auth/login', function () {
            $this->api->signIn('console');
            return \App\Api\ApiResponse::fake(200, ['mfa_required' => true]);
        });
        $this->assertRedirect('/login/mfa', $this->post('/login', ['email' => 'grace@riverside.example', 'password' => 'pw']));
        $this->assertContains('authenticator', $this->get('/login/mfa')->body());
        $this->assertRedirect('/login/mfa', $this->post('/login/mfa', ['code' => '12']));
        $this->api->on('POST', '/v1/auth/mfa/verify', 200, Fixtures::consoleSession());
        $this->assertRedirect('/console', $this->post('/login/mfa', ['code' => '123 456']));
        $this->assertSame('123456', $this->api->callsTo('POST', '/v1/auth/mfa/verify')[0]['options']['json']['code']);
    }

    public function testRegisterSendsAccountTypeAndValidates(): void
    {
        $page = $this->get('/register?type=individual')->body();
        $this->assertContains('My app', $page);
        $this->assertRedirect('/register?type=institution', $this->post('/register', ['account_type' => 'institution', 'full_name' => 'A', 'username' => 'abc', 'phone' => '0712345678', 'email' => 'a@b.co', 'password' => 'longenough-pass', 'terms' => '1']));
        $this->assertSame('Institution name is required.', $this->flashed('errors')['organization_name'] ?? null);

        $this->api->on('POST', '/v1/auth/register', 201, Fixtures::consoleSession('owner', 'individual'));
        $res = $this->post('/register', ['account_type' => 'individual', 'full_name' => 'Dev One', 'username' => '@DevOne', 'phone' => '0712345678', 'email' => 'dev@example.com', 'password' => 'longenough-pass', 'terms' => '1', 'plan' => 'developer_plus']);
        $this->assertRedirect('/console/billing/plans?highlight=developer_plus', $res);
        $sent = $this->api->callsTo('POST', '/v1/auth/register')[0]['options']['json'];
        $this->assertSame('individual', $sent['account_type']);
        $this->assertSame('devone', $sent['username']);
        $this->assertSame('0712345678', $sent['phone']);
        $this->assertTrue(!isset($sent['organization_name']), 'individuals default their org name server-side');
    }

    public function testForgotPasswordNeverRevealsWhetherTheAccountExists(): void
    {
        $this->api->on('POST', '/v1/auth/password/forgot', 202, ['ok' => true]);
        $this->assertRedirect('/forgot-password', $this->post('/forgot-password', ['email' => 'nobody@example.com']));
        $this->assertContains('If an account exists', $this->get('/forgot-password')->body());
    }

    public function testEmailVerificationRequiresAButtonPress(): void
    {
        $token = str_repeat('a', 32);
        $page = $this->get('/verify-email?token=' . $token)->body();
        $this->assertContains('Confirm email', $page);
        $this->assertSame([], $this->api->callsTo('POST', '/v1/auth/email/verify'), 'GET must not consume the token');
        $this->api->on('POST', '/v1/auth/email/verify', 200, ['ok' => true]);
        $this->assertContains('Email confirmed', $this->post('/verify-email', ['token' => $token])->body());
    }

    public function testInvitationPage(): void
    {
        $token = str_repeat('b', 40);
        $this->api->on('GET', '/v1/invitations/' . $token, 200, ['organization_name' => 'Riverside Academy', 'email' => 'new@riverside.example', 'role' => 'support', 'expires_at' => '2030-01-01T00:00:00Z']);
        $page = $this->get('/invitations/' . $token)->body();
        $this->assertContains('Join Riverside Academy', $page);
        $this->assertContains('Sign in to accept', $page);
    }

    public function testCsrfIsEnforced(): void
    {
        $res = $this->call('POST', '/login', ['email' => 'a@b.co', 'password' => 'x', '_token' => 'wrong']);
        $this->assertStatus(419, $res);
    }

    public function testPhoneVerificationFlowAfterRegistration(): void
    {
        $session = Fixtures::consoleSession('owner', 'individual');
        $session['verification'] = ['channel' => 'phone', 'sent_to' => '+254712•••678'];
        $this->api->on('POST', '/v1/auth/register', 201, $session);
        $res = $this->post('/register', ['account_type' => 'individual', 'full_name' => 'Dev', 'username' => 'dev1', 'phone' => '0712345678', 'email' => 'dev@example.com', 'password' => 'longenough-pass', 'terms' => '1']);
        $this->assertRedirect('/console/verify-phone?next=%2Fconsole', $res);

        $this->signInConsole(['user' => ['email_verified' => false, 'phone' => '+254712345678', 'phone_verified' => false]]);
        $this->assertContains('Enter the six-digit code', $this->get('/console/verify-phone')->body());
        $this->api->on('POST', '/v1/auth/phone/verify', 400, ['error' => ['code' => 'invalid_code', 'message' => 'That code is not correct.']]);
        $this->post('/console/verify-phone', ['code' => '111111']);
        $this->assertSame('That code is not correct.', $this->flashed('errors')['code'] ?? null);
        $this->api->on('POST', '/v1/auth/phone/verify', 200, ['verified' => true]);
        $this->assertRedirect('/console', $this->post('/console/verify-phone', ['code' => '123 456']));
        $this->api->on('POST', '/v1/auth/phone/send', 200, ['sent_to' => '+254722•••111']);
        $this->post('/console/verify-phone/send', ['phone' => '0722000111']);
        $this->assertSame('0722000111', $this->api->callsTo('POST', '/v1/auth/phone/send')[0]['options']['json']['phone']);
    }

    public function testGoogleSignInUsesStatePkceAndNonce(): void
    {
        $this->api->on('GET', '/v1/auth/providers', 200, ['google' => ['client_id' => 'cid.apps.googleusercontent.com'], 'phone_verification' => true]);
        $this->assertContains('Continue with Google', $this->get('/login')->body());
        $res = $this->get('/auth/google?type=institution');
        $this->assertStatus(302, $res);
        $location = (string) $res->headers()['Location'];
        parse_str((string) parse_url($location, PHP_URL_QUERY), $q);
        $this->assertTrue(str_starts_with($location, 'https://accounts.google.com/o/oauth2/v2/auth?'));
        $this->assertSame('S256', $q['code_challenge_method']);
        $this->assertTrue(str_ends_with($q['redirect_uri'], '/auth/google/callback'));

        $this->assertRedirect('/login', $this->get('/auth/google/callback?code=abc&state=forged'));
        $this->get('/auth/google');
        $location = (string) $this->get('/auth/google')->headers()['Location'];
        parse_str((string) parse_url($location, PHP_URL_QUERY), $q);
        $this->api->on('POST', '/v1/auth/google', 200, Fixtures::consoleSession() + ['created' => true]);
        $this->assertRedirect('/console', $this->get('/auth/google/callback?code=abc&state=' . $q['state']));
        $sent = $this->api->callsTo('POST', '/v1/auth/google')[0]['options']['json'];
        $this->assertSame($q['nonce'], $sent['nonce']);
        $this->assertSame($q['code_challenge'], rtrim(strtr(base64_encode(hash('sha256', $sent['code_verifier'], true)), '+/', '-_'), '='));
    }
}
