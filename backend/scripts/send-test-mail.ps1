# Deliver a test email into a local mailbox (simulates mail arriving from the internet).
#   send-test-mail.cmd --to teacher@riverside-academy.test [--from "Name <a@b.org>"] [--subject "..."] [--body "..."] [--attach file]
. (Join-Path $PSScriptRoot "local-env.ps1")
$python = Find-Python
if (-not $python) { throw "Python 3.10+ is required (https://www.python.org/downloads/)." }
& $python -I (Join-Path $Backend "infra\scripts\send_inbound.py") @args
exit $LASTEXITCODE
