@echo off
setlocal
chcp 65001 >nul

set "ROOT=%~dp0.."
set "DIST=%ROOT%\dist"
set "STATUS_FILE=%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.status"
set "BUILD_LOG=%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.build.log"
set "RELEASE_ORCHESTRATED=0"
if /I "%~1"=="--orchestrated" set "RELEASE_ORCHESTRATED=1"

if /I "%~1"=="--check" goto :check
if /I "%~2"=="--check" goto :check

goto :build

:check
where cargo >nul 2>&1
if errorlevel 1 (
  echo Windows x86_64 打包环境缺失：请先安装 Rust 工具链并确保 cargo 在 PATH 中。>&2
  exit /b 2
)
exit /b 0

:build
call "%~f0" --check
if errorlevel 1 (
  if "%RELEASE_ORCHESTRATED%"=="1" exit /b 2
  call :pause_on_missing_env
  exit /b 2
)

if not exist "%DIST%" mkdir "%DIST%"

pushd "%ROOT%"
if errorlevel 1 exit /b 1

echo 正在构建 hdd-autopilot-x86_64-pc-windows-msvc.exe...
cargo build --release --package hdd-autopilot > "%BUILD_LOG%" 2>&1
set "BUILD_EXIT=%ERRORLEVEL%"
type "%BUILD_LOG%"
if not "%BUILD_EXIT%"=="0" exit /b %BUILD_EXIT%
copy /Y ".\target\release\hdd-autopilot.exe" "%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.exe" >nul
if errorlevel 1 goto :fail
findstr /I /C:"native backend disabled" "%BUILD_LOG%" >nul
if errorlevel 1 (
  > "%STATUS_FILE%" echo built
) else (
  > "%STATUS_FILE%" echo built_degraded
)

popd

echo 构建完成：%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.exe
exit /b 0

:fail
set "BUILD_EXIT=%ERRORLEVEL%"
popd
exit /b %BUILD_EXIT%

:pause_on_missing_env
echo.
pause
exit /b 2
