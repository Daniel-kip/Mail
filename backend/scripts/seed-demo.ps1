# Create the Riverside Academy demo tenant (requires a running API: start-local.cmd).
. (Join-Path $PSScriptRoot "local-env.ps1")
$python = Find-Python
if (-not $python) { throw "Python 3.10+ is required (https://www.python.org/downloads/)." }
if (-not (Wait-Server -Seconds 3)) { throw "The API is not running. Start it with start-local.cmd first (or start-local.cmd -Background)." }
& $python -I (Join-Path $Backend "infra\scripts\seed_demo.py") --exe (Join-Path $Backend "target\debug\mailvora.exe")
exit $LASTEXITCODE
