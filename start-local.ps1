param([switch]$NoBuild)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Backend = Join-Path $Root 'backend'
$Frontend = Join-Path $Root 'frontend'
$BackendLauncher = Join-Path $Backend 'scripts\start-local.ps1'
$PhpOut = Join-Path $Frontend 'server.log'
$PhpErr = Join-Path $Frontend 'server.err.log'

function Test-HttpReady([string]$Url) {
    try {
        $response = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $Url
        return $response.StatusCode -eq 200
    } catch {
        return $false
    }
}

function Test-FrontendReady([int]$Port) {
    try {
        $response = Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 "http://127.0.0.1:$Port/healthz"
        return $response.StatusCode -eq 200 -and $response.Content -match '"web":"ok"'
    } catch {
        return $false
    }
}

function Test-PortInUse([int]$Port) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $task = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
        return $task.AsyncWaitHandle.WaitOne(500) -and $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

$php = Get-Command php -ErrorAction SilentlyContinue
if (-not $php) { throw 'PHP 8.2+ was not found on PATH. Install PHP (for example: winget install PHP.PHP.8.5).' }
& $php.Source -r 'exit(PHP_VERSION_ID >= 80200 ? 0 : 1);'
if ($LASTEXITCODE -ne 0) { throw 'PHP 8.2 or newer is required.' }

# Load the extensions the frontend needs for this session only, without
# editing php.ini (WinGet installs ship them commented out).
$phpRuntimeArgs = @()
$phpExtDir = Join-Path (Split-Path $php.Source) 'ext'
foreach ($ext in @('curl', 'mbstring', 'openssl')) {
    & $php.Source @phpRuntimeArgs -r "exit(extension_loaded('$ext') ? 0 : 1);"
    if ($LASTEXITCODE -eq 0) { continue }
    if (-not (Test-Path (Join-Path $phpExtDir "php_$ext.dll"))) {
        throw "PHP extension '$ext' is not enabled and php_$ext.dll was not found in $phpExtDir."
    }
    if ($phpRuntimeArgs -notcontains "extension_dir=$phpExtDir") { $phpRuntimeArgs += @('-d', "extension_dir=$phpExtDir") }
    $phpRuntimeArgs += @('-d', "extension=$ext")
}
& $php.Source @phpRuntimeArgs -r "exit(extension_loaded('curl') && extension_loaded('mbstring') ? 0 : 1);"
if ($LASTEXITCODE -ne 0) { throw 'PHP extensions curl and mbstring could not be loaded. Check the PHP installation.' }
foreach ($dir in @('storage\sessions', 'storage\logs')) { New-Item -ItemType Directory -Force (Join-Path $Frontend $dir) | Out-Null }

if (-not (Test-Path (Join-Path $Frontend '.env'))) {
    Copy-Item (Join-Path $Frontend '.env.example') (Join-Path $Frontend '.env')
}

# Choose the web port first: emailed links (activation, reset, unsubscribe)
# and phone-setup QR codes must point at the address the browser will use.
$FrontendPort = 8080
$webReady = Test-FrontendReady $FrontendPort
if (-not $webReady -and (Test-PortInUse $FrontendPort)) {
    $available = $null
    foreach ($candidate in 8081..8090) {
        if (Test-FrontendReady $candidate) { $FrontendPort = $candidate; $webReady = $true; break }
        if (-not (Test-PortInUse $candidate)) { $available = $candidate; break }
    }
    if (-not $webReady -and $available) {
        $FrontendPort = $available
        Write-Warning "Port 8080 is used by another application. Starting Mailvora on port $FrontendPort instead."
    } elseif (-not $webReady) {
        throw 'Port 8080 is occupied by another application, and no free frontend port was found between 8081 and 8090.'
    }
}
$WebUrl = "http://127.0.0.1:$FrontendPort"
# Process environment wins over both .env files, for the API and for PHP.
$env:PUBLIC_BASE_URL = $WebUrl
$env:APP_URL = $WebUrl

$apiReady = Test-HttpReady 'http://127.0.0.1:8088/readyz'
if (-not $apiReady) {
    if (Test-PortInUse 8088) { throw 'Port 8088 is occupied, but the Mailvora API is not ready. Stop the conflicting process and retry.' }
    Write-Host 'Starting backend and development dependencies...' -ForegroundColor Cyan
    $backendArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $BackendLauncher, '-Background')
    if ($NoBuild) { $backendArgs += '-NoBuild' }
    & powershell.exe @backendArgs
    if ($LASTEXITCODE -ne 0) { throw 'Backend startup failed. Review backend\server.log and backend\server.err.log.' }
    if (-not (Test-HttpReady 'http://127.0.0.1:8088/readyz')) { throw 'Backend launcher returned, but API readiness check failed at http://127.0.0.1:8088/readyz.' }
} else {
    Write-Host 'Backend API is already ready.' -ForegroundColor Green
    if ($FrontendPort -ne 8080) { Write-Warning "If emailed links point at port 8080, run stop-local.cmd and start again so the API uses $WebUrl." }
}

if (-not $webReady) {
    Write-Host 'Starting PHP frontend...' -ForegroundColor Cyan
    Start-Process -FilePath $php.Source -ArgumentList ($phpRuntimeArgs + @('-S', "127.0.0.1:$FrontendPort", '-t', 'public', 'public/router.php')) `
        -WorkingDirectory $Frontend -WindowStyle Hidden -RedirectStandardOutput $PhpOut -RedirectStandardError $PhpErr | Out-Null
    $webReady = $false
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Seconds 1
        if (Test-FrontendReady $FrontendPort) { $webReady = $true; break }
    }
    if (-not $webReady) {
        $detail = if (Test-Path $PhpErr) { (Get-Content $PhpErr -Tail 20) -join [Environment]::NewLine } else { 'No PHP error log was created.' }
        throw "PHP frontend failed to become ready. $detail"
    }
} else {
    Write-Host 'PHP frontend is already responding.' -ForegroundColor Green
}

Write-Host ''
Write-Host 'Mailvora is running:' -ForegroundColor Green
Write-Host "  Website and console: http://127.0.0.1:$FrontendPort"
Write-Host "  Webmail:             http://127.0.0.1:$FrontendPort/mail"
Write-Host '  API:                 http://127.0.0.1:8088'
Write-Host '  Caught mail:         http://127.0.0.1:8025  (Mailpit)'
Write-Host '  Demo data:           backend\seed-demo.cmd  (accounts are listed in backend\demo-credentials.txt)'
Write-Host "  Logs:                $Frontend\server.log and $Backend\server.log"
