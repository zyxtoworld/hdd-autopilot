@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul

set "ROOT=%~dp0.."
set "ROOT_BASH=%ROOT:\=/%"
set "DIST=%ROOT%\dist"
set "WIN_STATUS=failed"
set "WIN_REASON="
set "MAC_AMD64_STATUS=failed"
set "MAC_AMD64_REASON="
set "MAC_ARM64_STATUS=failed"
set "MAC_ARM64_REASON="
set "LINUX_AMD64_STATUS=failed"
set "LINUX_AMD64_REASON="

call :run_windows
call :run_macos_amd64
call :run_macos_arm64
call :run_linux_amd64

echo.
echo Release package summary:
echo   Windows x64  : !WIN_STATUS! !WIN_REASON!
echo   macOS amd64  : !MAC_AMD64_STATUS! !MAC_AMD64_REASON!
echo   macOS arm64  : !MAC_ARM64_STATUS! !MAC_ARM64_REASON!
echo   Linux amd64  : !LINUX_AMD64_STATUS! !LINUX_AMD64_REASON!
echo.

if /I "!WIN_STATUS!"=="failed" exit /b 1
if /I "!MAC_AMD64_STATUS!"=="failed" exit /b 1
if /I "!MAC_ARM64_STATUS!"=="failed" exit /b 1
if /I "!LINUX_AMD64_STATUS!"=="failed" exit /b 1

if /I "!WIN_STATUS!"=="built_degraded" goto :degraded
if /I "!MAC_AMD64_STATUS!"=="built_degraded" goto :degraded
if /I "!MAC_ARM64_STATUS!"=="built_degraded" goto :degraded
if /I "!LINUX_AMD64_STATUS!"=="built_degraded" goto :degraded

echo Release packaging completed.
exit /b 0

:degraded
echo Release packaging completed with degraded package(s).
exit /b 0

:run_windows
echo Checking Windows x64 build environment...
call "%ROOT%\scripts\build-win-x64.bat" --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "WIN_STATUS=failed"
  set "WIN_REASON=(missing build environment)"
  echo Windows x64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building Windows x64...
call "%ROOT%\scripts\build-win-x64.bat" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "WIN_STATUS=failed"
  set "WIN_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-win-x64.status" (
    set /p WIN_STATUS=<"%DIST%\hdd-win-x64.status"
  ) else (
    set "WIN_STATUS=built"
  )
  if /I "!WIN_STATUS!"=="built_degraded" (
    set "WIN_REASON=(native CUDA backend degraded)"
  ) else (
    set "WIN_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_macos_amd64
echo Checking macOS amd64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(missing bash)"
  echo macOS amd64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-macos-amd64.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(missing build environment)"
  echo macOS amd64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building macOS amd64...
bash "%ROOT_BASH%/scripts/build-macos-amd64.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-macos-amd64.status" (
    set /p MAC_AMD64_STATUS=<"%DIST%\hdd-macos-amd64.status"
  ) else (
    set "MAC_AMD64_STATUS=built"
  )
  if /I "!MAC_AMD64_STATUS!"=="built_degraded" (
    set "MAC_AMD64_REASON=(OpenCL/Metal backend degraded)"
  ) else (
    set "MAC_AMD64_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_macos_arm64
echo Checking macOS arm64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(missing bash)"
  echo macOS arm64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-macos-arm64.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(missing build environment)"
  echo macOS arm64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building macOS arm64...
bash "%ROOT_BASH%/scripts/build-macos-arm64.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-macos-arm64.status" (
    set /p MAC_ARM64_STATUS=<"%DIST%\hdd-macos-arm64.status"
  ) else (
    set "MAC_ARM64_STATUS=built"
  )
  if /I "!MAC_ARM64_STATUS!"=="built_degraded" (
    set "MAC_ARM64_REASON=(OpenCL/Metal backend degraded)"
  ) else (
    set "MAC_ARM64_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_linux_amd64
echo Checking Linux amd64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "LINUX_AMD64_STATUS=failed"
  set "LINUX_AMD64_REASON=(missing bash)"
  echo Linux amd64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-linux-amd64.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "LINUX_AMD64_STATUS=failed"
  set "LINUX_AMD64_REASON=(missing build environment)"
  echo Linux amd64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building Linux amd64...
bash "%ROOT_BASH%/scripts/build-linux-amd64.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "LINUX_AMD64_STATUS=failed"
  set "LINUX_AMD64_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-linux-amd64.status" (
    set /p LINUX_AMD64_STATUS=<"%DIST%\hdd-linux-amd64.status"
  ) else (
    set "LINUX_AMD64_STATUS=built"
  )
  if /I "!LINUX_AMD64_STATUS!"=="built_degraded" (
    set "LINUX_AMD64_REASON=(native backend degraded)"
  ) else (
    set "LINUX_AMD64_REASON=(complete)"
  )
)
echo.
exit /b 0
