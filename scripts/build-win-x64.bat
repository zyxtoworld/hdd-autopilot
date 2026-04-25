@echo off
setlocal

set "ROOT=%~dp0.."
set "DIST=%ROOT%\dist"
if not exist "%DIST%" mkdir "%DIST%"

pushd "%ROOT%"
if errorlevel 1 exit /b 1

echo Building hdd-win-x64.exe...
go build -o "%DIST%\hdd-win-x64.exe" .\cmd\hdd
set "BUILD_EXIT=%errorlevel%"

popd
if errorlevel 1 exit /b %BUILD_EXIT%

echo Done: %DIST%\hdd-win-x64.exe
exit /b %BUILD_EXIT%
