@echo off
rem Runs scripts\seed-demo.ps1. The policy bypass applies to this script only.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\seed-demo.ps1" %*
set EXITCODE=%ERRORLEVEL%
rem Keep the window open when started by double-click so the output can be read.
echo %cmdcmdline% | find /i "%~nx0" >nul && pause
exit /b %EXITCODE%
