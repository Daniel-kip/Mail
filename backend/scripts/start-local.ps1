# Start the Mailvora API on localhost: dependencies, configuration, build, server.
#   start-local.cmd            build (incremental) and run in this window; Ctrl+C stops the API
#   start-local.cmd -NoBuild   run the last build
#   start-local.cmd -Background  start the API in the background (log: backend\server.log)
param([switch]$NoBuild, [switch]$Background)

. (Join-Path $PSScriptRoot "local-env.ps1")

Start-Dependencies
Initialize-EnvFile
Use-RustToolchain
Stop-Server

Push-Location $Backend
try {
    if (-not $NoBuild) {
        Write-Step "Building the API (first build takes several minutes)"
        cargo build
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    }
    $exe = Join-Path $Backend "target\debug\mailvora.exe"
    if (-not (Test-Path $exe)) { throw "No build found at $exe. Run without -NoBuild." }

    Write-Host ""
    Write-Host "  Mailvora API is starting:" -ForegroundColor White
    Write-Host "    API          http://127.0.0.1:8088" -ForegroundColor White
    Write-Host "    Web (PHP)    start everything with ..\start-local.cmd -> http://127.0.0.1:8080" -ForegroundColor White
    Write-Host "    Caught mail  http://localhost:8025  (Mailpit: verification emails, sent messages)" -ForegroundColor White
    Write-Host "    Demo data    backend\seed-demo.cmd   (creates ready-to-use accounts)" -ForegroundColor White
    Write-Host ""

    if ($Background) {
        $log = Join-Path $Backend "server.log"
        Start-Process -FilePath $exe -ArgumentList "serve" -WorkingDirectory $Backend -WindowStyle Hidden `
            -RedirectStandardOutput $log -RedirectStandardError (Join-Path $Backend "server.err.log")
        if (Wait-Server) { Write-Ok "API is ready (running in the background; stop-local.cmd stops it)" }
        else { throw "API did not become ready; see $log" }
    } else {
        Write-Host "  Press Ctrl+C to stop the API (Docker services keep running; stop-local.cmd stops them)." -ForegroundColor DarkGray
        & $exe serve
    }
} finally {
    Pop-Location
}
