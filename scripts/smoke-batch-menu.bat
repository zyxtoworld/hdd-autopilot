@echo off
setlocal
chcp 65001 >nul

set "ROOT=%~dp0.."

powershell -NoLogo -ExecutionPolicy Bypass -File "%ROOT%\scripts\smoke-batch-menu.ps1" %*
if errorlevel 1 exit /b 1

echo 批量菜单烟测完成。
