@echo off
setlocal

set "ROOT=%~dp0.."

powershell -NoLogo -ExecutionPolicy Bypass -File "%ROOT%\scripts\smoke-batch-menu.ps1" %*
if errorlevel 1 exit /b 1

echo Batch smoke finished.
