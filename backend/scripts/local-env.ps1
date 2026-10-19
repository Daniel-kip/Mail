# Shared helpers for the local scripts (dot-sourced; not run directly).

$ErrorActionPreference = "Stop"
$Backend = Split-Path -Parent $PSScriptRoot
$Root = Split-Path -Parent $Backend
$Compose = Join-Path $Backend "infra\docker-compose.dev.yml"

function Write-Step($text) { Write-Host "==> $text" -ForegroundColor Cyan }
function Write-Ok($text) { Write-Host "    $text" -ForegroundColor Green }
function Write-Warn($text) { Write-Host "    $text" -ForegroundColor Yellow }

function Assert-Command($name, $hint) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "$name was not found. $hint"
    }
}

function Start-DockerIfNeeded {
    Assert-Command docker "Install Docker Desktop: https://www.docker.com/products/docker-desktop/"
    docker info --format '{{.ServerVersion}}' 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) { return }
    Write-Step "Starting Docker Desktop"
    $exe = @("$env:LOCALAPPDATA\Programs\DockerDesktop\Docker Desktop.exe", "C:\Program Files\Docker\Docker\Docker Desktop.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $exe) { throw "Docker is not running and Docker Desktop was not found. Start Docker manually." }
    Start-Process $exe
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Seconds 3
        docker info --format '{{.ServerVersion}}' 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { Write-Ok "Docker is ready"; return }
    }
    throw "Docker did not become ready within 3 minutes."
}

function Start-Dependencies([switch]$MailTier) {
    Start-DockerIfNeeded
    Write-Step "Starting Postgres, Mailpit and Dovecot (docker compose)"
    $composeArgs = @("compose", "-f", $Compose)
    if ($MailTier) { $composeArgs += @("--profile", "mailtier") }
    $composeArgs += @("up", "-d", "--build")
    & docker @composeArgs
    if ($LASTEXITCODE -ne 0) { throw "docker compose failed" }
    for ($i = 0; $i -lt 40; $i++) {
        docker compose -f $Compose exec -T postgres pg_isready -U mailvora -d mailvora 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { Write-Ok "Postgres is ready"; return }
        Start-Sleep -Seconds 2
    }
    throw "Postgres did not become ready."
}

function New-DataKey {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    [Convert]::ToBase64String($bytes)
}

function Initialize-EnvFile {
    $envFile = Join-Path $Backend ".env"
    if (Test-Path $envFile) { return }
    Write-Step "Creating backend\.env with a fresh data encryption key"
    $content = Get-Content (Join-Path $Backend ".env.example") -Raw
    $content = $content -replace "DATA_ENCRYPTION_KEY=.*", "DATA_ENCRYPTION_KEY=$(New-DataKey)"
    [IO.File]::WriteAllText($envFile, $content)
    Write-Ok "backend\.env created (development values only)"
}

# Select a toolchain that can actually link on this machine.
function Use-RustToolchain {
    Assert-Command cargo "Install Rust from https://rustup.rs/"
    $msvcLinker = $false
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vc = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if ($vc) { $msvcLinker = $true }
    }
    if ($msvcLinker) {
        Write-Ok "Using the default MSVC toolchain"
        return
    }
    $gnu = Get-ChildItem "$env:USERPROFILE\.rustup\toolchains" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match "windows-gnu|gnu-manual" -and (Test-Path (Join-Path $_.FullName "bin\rustc.exe")) } |
        Select-Object -First 1
    $msys = @("C:\msys64\ucrt64\bin", "C:\msys64\mingw64\bin") | Where-Object { Test-Path (Join-Path $_ "gcc.exe") } | Select-Object -First 1
    if ($gnu -and $msys) {
        $env:RUSTC = Join-Path $gnu.FullName "bin\rustc.exe"
        $env:PATH = "$msys;$($gnu.FullName)\bin;$env:PATH"
        Write-Ok "Using the GNU toolchain ($($gnu.Name)) with MSYS2 GCC"
        return
    }
    throw @"
No working Rust linker was found.
Install ONE of:
  * Visual Studio Build Tools with "Desktop development with C++", or
  * rustup toolchain install stable-x86_64-pc-windows-gnu  plus MSYS2 GCC (pacman -S mingw-w64-ucrt-x86_64-gcc)
"@
}

function Stop-Server {
    Get-Process mailvora -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

function Wait-Server([int]$Seconds = 90) {
    for ($i = 0; $i -lt $Seconds; $i++) {
        try {
            $r = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 "http://127.0.0.1:8088/readyz"
            if ($r.StatusCode -eq 200) { return $true }
        } catch { }
        Start-Sleep -Seconds 1
    }
    return $false
}

function Find-Python {
    foreach ($c in @("python", "py")) {
        $cmd = Get-Command $c -ErrorAction SilentlyContinue
        if ($cmd -and $cmd.Source -notmatch "WindowsApps") { return $cmd.Source }
    }
    return $null
}
