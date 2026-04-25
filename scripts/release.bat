@echo off
setlocal

set "ROOT=%~dp0.."
set "RUN_BATCH_SMOKE="

:parse_args
if "%~1"=="" goto after_args
if /i "%~1"=="--with-batch-smoke" (
    set "RUN_BATCH_SMOKE=1"
    shift
    goto parse_args
)
if /i "%~1"=="--help" goto usage
if /i "%~1"=="-h" goto usage
echo Unknown argument: %~1
goto usage

:usage
echo Usage: release.bat [--with-batch-smoke]
exit /b 1

:after_args
call "%ROOT%\scripts\build-win-x64.bat"
if errorlevel 1 exit /b 1

if defined RUN_BATCH_SMOKE (
    call "%ROOT%\scripts\smoke-batch-menu.bat"
    if errorlevel 1 exit /b 1
)

call "%ROOT%\native\hdd-miner-gpu\scripts\build-native.bat"
if errorlevel 1 exit /b 1

call "%ROOT%\native\invite-miner-gpu\scripts\build-native.bat"
if errorlevel 1 exit /b 1

call "%ROOT%\native\balance-miner-gpu\scripts\build-native.bat"
if errorlevel 1 exit /b 1

echo Release build finished.
