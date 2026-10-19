@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0start-local.ps1" %*
set "EXITCODE=%ERRORLEVEL%"
if not "%EXITCODE%"=="0" (
  echo.
  echo Mailvora did not start. Read the error above and the backend/frontend server logs.
  pause
)
exit /b %EXITCODE%
