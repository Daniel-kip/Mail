# Run the full local verification suite.
#   run-tests.cmd             Rust unit + integration tests, PHP frontend tests, then end-to-end checks
#                             against the running stack (start it first with ..\start-local.cmd)
#   run-tests.cmd -MailTier   also start Postfix + Rspamd and run the mail-server checks
#   run-tests.cmd -RustOnly   only the cargo tests
param([switch]$MailTier, [switch]$RustOnly)

. (Join-Path $PSScriptRoot "local-env.ps1")
$results = [ordered]@{}

function Run-Check($name, [scriptblock]$block) {
    Write-Step $name
    & $block
    $results[$name] = if ($LASTEXITCODE -eq 0) { "PASS" } else { "FAIL" }
}

Start-Dependencies -MailTier:$MailTier
Initialize-EnvFile
Use-RustToolchain

Push-Location $Backend
try {
    $env:TEST_DATABASE_URL = "postgres://mailvora:mailvora_dev@127.0.0.1:5434/mailvora"
    Run-Check "Rust unit + integration tests" { cargo test -- --test-threads=4 }
} finally { Pop-Location }

$php = Get-Command php -ErrorAction SilentlyContinue
if ($php) {
    $phpArgs = @()
    $extDir = Join-Path (Split-Path $php.Source) 'ext'
    foreach ($ext in @('mbstring', 'openssl')) {
        & $php.Source -r "exit(extension_loaded('$ext') ? 0 : 1);"
        if ($LASTEXITCODE -ne 0 -and (Test-Path (Join-Path $extDir "php_$ext.dll"))) { $phpArgs += @('-d', "extension_dir=$extDir", '-d', "extension=$ext") }
    }
    Push-Location (Join-Path $Root 'frontend')
    try { Run-Check "PHP frontend unit + feature tests" { & $php.Source @phpArgs tests/run.php } } finally { Pop-Location }
} else {
    Write-Warn "PHP not found: skipping the frontend tests"
}

if (-not $RustOnly) {
    $python = Find-Python
    if (-not $python) { throw "Python 3.10+ is required for the end-to-end checks (https://www.python.org/downloads/)." }

    # Use a running API if there is one; otherwise start one in the background.
    $startedHere = $false
    if (-not (Wait-Server -Seconds 1)) {
        Write-Step "Starting the API in the background for end-to-end checks"
        Push-Location $Backend
        try { cargo build | Out-Host } finally { Pop-Location }
        Start-Process -FilePath (Join-Path $Backend "target\debug\mailvora.exe") -ArgumentList "serve" -WorkingDirectory $Backend `
            -WindowStyle Hidden -RedirectStandardOutput (Join-Path $Backend "server.log") -RedirectStandardError (Join-Path $Backend "server.err.log")
        if (-not (Wait-Server)) { throw "API did not start; see backend\server.log" }
        $startedHere = $true
    }

    $scripts = Join-Path $Backend "infra\scripts"
    Run-Check "End-to-end smoke (API, outbox, relay, DKIM)" { & $python -I (Join-Path $scripts "smoke.py") }
    Run-Check "Hosted mailboxes + webmail (real Dovecot)" { & $python -I (Join-Path $scripts "mailbox_e2e.py") }

    $browser = @("C:\Program Files\Google\Chrome\Application\chrome.exe", "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
                 "C:\Program Files\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
    $web = $null
    foreach ($port in 8080..8090) {
        try {
            $r = Invoke-WebRequest -UseBasicParsing -TimeoutSec 3 "http://127.0.0.1:$port/healthz"
            if ($r.Content -match '"web":"ok"') { $web = "http://127.0.0.1:$port"; break }
        } catch {}
    }
    if ($browser -and $web) {
        Run-Check "Web app in a real browser (headless Chrome/Edge)" { & $python -I (Join-Path $Root "frontend\tests\ui_smoke.py") --base $web --chrome $browser --out (Join-Path $env:TEMP "mailvora-ui") }
    } elseif (-not $web) {
        Write-Warn "The PHP web app is not running (start ..\start-local.cmd): skipping the browser check"
    } else {
        Write-Warn "Chrome/Edge not found: skipping the browser UI check"
    }

    if ($MailTier) {
        Run-Check "Mail tier (Postfix + Dovecot + Rspamd)" { & $python -I (Join-Path $scripts "mail_tier_e2e.py") }
    }

    if ($startedHere) { Stop-Server }
}

Write-Host ""
Write-Host "Results" -ForegroundColor White
$failed = 0
foreach ($k in $results.Keys) {
    $color = if ($results[$k] -eq "PASS") { "Green" } else { $failed++; "Red" }
    Write-Host ("  {0}  {1}" -f $results[$k], $k) -ForegroundColor $color
}
if ($failed) { exit 1 } else { Write-Host "`nAll checks passed." -ForegroundColor Green }
