# Stop the local API and Docker services.
#   stop-local.cmd          stop everything, keep data
#   stop-local.cmd -Reset   stop everything and DELETE local data (database, mailboxes)
param([switch]$Reset)

. (Join-Path $PSScriptRoot "local-env.ps1")

Write-Step "Stopping the API"
Stop-Server
Write-Step "Stopping the PHP web server"
Get-CimInstance Win32_Process -Filter "Name = 'php.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -match 'router\.php' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Write-Step "Stopping Docker services"
if ($Reset) {
    docker compose -f $Compose --profile mailtier down -v
    Write-Warn "Local database and mailboxes deleted. The next start creates fresh ones."
} else {
    docker compose -f $Compose --profile mailtier stop
}
Write-Ok "Stopped"
