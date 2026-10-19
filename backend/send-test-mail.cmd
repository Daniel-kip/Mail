@echo off
rem Runs scripts\send-test-mail.ps1. The policy bypass applies to this script only.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\send-test-mail.ps1" %*
exit /b %ERRORLEVEL%
