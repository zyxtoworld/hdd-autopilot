@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul

set "ROOT=%~dp0.."
set "ROOT_BASH=%ROOT:\=/%"
set "DIST=%ROOT%\dist"
set "X86_64_PC_WINDOWS_MSVC_STATUS=failed"
set "X86_64_PC_WINDOWS_MSVC_REASON="
set "X86_64_APPLE_DARWIN_STATUS=failed"
set "X86_64_APPLE_DARWIN_REASON="
set "AARCH64_APPLE_DARWIN_STATUS=failed"
set "AARCH64_APPLE_DARWIN_REASON="
set "X86_64_UNKNOWN_LINUX_GNU_STATUS=failed"
set "X86_64_UNKNOWN_LINUX_GNU_REASON="
set "AARCH64_UNKNOWN_LINUX_GNU_STATUS=failed"
set "AARCH64_UNKNOWN_LINUX_GNU_REASON="

call :run_x86_64_pc_windows_msvc
call :run_x86_64_apple_darwin
call :run_aarch64_apple_darwin
call :run_x86_64_unknown_linux_gnu
call :run_aarch64_unknown_linux_gnu

echo.
echo Release package summary:
echo   Windows x86_64  : !X86_64_PC_WINDOWS_MSVC_STATUS! !X86_64_PC_WINDOWS_MSVC_REASON!
echo   macOS x86_64  : !X86_64_APPLE_DARWIN_STATUS! !X86_64_APPLE_DARWIN_REASON!
echo   macOS aarch64  : !AARCH64_APPLE_DARWIN_STATUS! !AARCH64_APPLE_DARWIN_REASON!
echo   Linux x86_64  : !X86_64_UNKNOWN_LINUX_GNU_STATUS! !X86_64_UNKNOWN_LINUX_GNU_REASON!
echo   Linux aarch64  : !AARCH64_UNKNOWN_LINUX_GNU_STATUS! !AARCH64_UNKNOWN_LINUX_GNU_REASON!
echo.

if /I "!X86_64_PC_WINDOWS_MSVC_STATUS!"=="failed" exit /b 1
if /I "!X86_64_APPLE_DARWIN_STATUS!"=="failed" exit /b 1
if /I "!AARCH64_APPLE_DARWIN_STATUS!"=="failed" exit /b 1
if /I "!X86_64_UNKNOWN_LINUX_GNU_STATUS!"=="failed" exit /b 1
if /I "!AARCH64_UNKNOWN_LINUX_GNU_STATUS!"=="failed" exit /b 1

if /I "!X86_64_PC_WINDOWS_MSVC_STATUS!"=="built_degraded" goto :degraded
if /I "!X86_64_APPLE_DARWIN_STATUS!"=="built_degraded" goto :degraded
if /I "!AARCH64_APPLE_DARWIN_STATUS!"=="built_degraded" goto :degraded
if /I "!X86_64_UNKNOWN_LINUX_GNU_STATUS!"=="built_degraded" goto :degraded
if /I "!AARCH64_UNKNOWN_LINUX_GNU_STATUS!"=="built_degraded" goto :degraded

echo Release packaging completed.
exit /b 0

:degraded
echo Release packaging completed with degraded package(s).
exit /b 0

:run_x86_64_pc_windows_msvc
echo Checking Windows x86_64 build environment...
call "%ROOT%\scripts\build-x86_64-pc-windows-msvc.bat" --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "X86_64_PC_WINDOWS_MSVC_STATUS=failed"
  set "X86_64_PC_WINDOWS_MSVC_REASON=(missing build environment)"
  echo Windows x86_64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building Windows x86_64...
call "%ROOT%\scripts\build-x86_64-pc-windows-msvc.bat" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "X86_64_PC_WINDOWS_MSVC_STATUS=failed"
  set "X86_64_PC_WINDOWS_MSVC_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.status" (
    set /p X86_64_PC_WINDOWS_MSVC_STATUS=<"%DIST%\hdd-autopilot-x86_64-pc-windows-msvc.status"
  ) else (
    set "X86_64_PC_WINDOWS_MSVC_STATUS=built"
  )
  if /I "!X86_64_PC_WINDOWS_MSVC_STATUS!"=="built_degraded" (
    set "X86_64_PC_WINDOWS_MSVC_REASON=(native CUDA backend degraded)"
  ) else (
    set "X86_64_PC_WINDOWS_MSVC_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_x86_64_apple_darwin
echo Checking macOS x86_64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "X86_64_APPLE_DARWIN_STATUS=failed"
  set "X86_64_APPLE_DARWIN_REASON=(missing bash)"
  echo macOS x86_64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-x86_64-apple-darwin.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "X86_64_APPLE_DARWIN_STATUS=failed"
  set "X86_64_APPLE_DARWIN_REASON=(missing build environment)"
  echo macOS x86_64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building macOS x86_64...
bash "%ROOT_BASH%/scripts/build-x86_64-apple-darwin.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "X86_64_APPLE_DARWIN_STATUS=failed"
  set "X86_64_APPLE_DARWIN_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-autopilot-x86_64-apple-darwin.status" (
    set /p X86_64_APPLE_DARWIN_STATUS=<"%DIST%\hdd-autopilot-x86_64-apple-darwin.status"
  ) else (
    set "X86_64_APPLE_DARWIN_STATUS=built"
  )
  if /I "!X86_64_APPLE_DARWIN_STATUS!"=="built_degraded" (
    set "X86_64_APPLE_DARWIN_REASON=(OpenCL/Metal backend degraded)"
  ) else (
    set "X86_64_APPLE_DARWIN_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_aarch64_apple_darwin
echo Checking macOS aarch64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "AARCH64_APPLE_DARWIN_STATUS=failed"
  set "AARCH64_APPLE_DARWIN_REASON=(missing bash)"
  echo macOS aarch64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-aarch64-apple-darwin.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "AARCH64_APPLE_DARWIN_STATUS=failed"
  set "AARCH64_APPLE_DARWIN_REASON=(missing build environment)"
  echo macOS aarch64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building macOS aarch64...
bash "%ROOT_BASH%/scripts/build-aarch64-apple-darwin.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "AARCH64_APPLE_DARWIN_STATUS=failed"
  set "AARCH64_APPLE_DARWIN_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-autopilot-aarch64-apple-darwin.status" (
    set /p AARCH64_APPLE_DARWIN_STATUS=<"%DIST%\hdd-autopilot-aarch64-apple-darwin.status"
  ) else (
    set "AARCH64_APPLE_DARWIN_STATUS=built"
  )
  if /I "!AARCH64_APPLE_DARWIN_STATUS!"=="built_degraded" (
    set "AARCH64_APPLE_DARWIN_REASON=(OpenCL/Metal backend degraded)"
  ) else (
    set "AARCH64_APPLE_DARWIN_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_x86_64_unknown_linux_gnu
echo Checking Linux x86_64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "X86_64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "X86_64_UNKNOWN_LINUX_GNU_REASON=(missing bash)"
  echo Linux x86_64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-x86_64-unknown-linux-gnu.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "X86_64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "X86_64_UNKNOWN_LINUX_GNU_REASON=(missing build environment)"
  echo Linux x86_64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building Linux x86_64...
bash "%ROOT_BASH%/scripts/build-x86_64-unknown-linux-gnu.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "X86_64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "X86_64_UNKNOWN_LINUX_GNU_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-autopilot-x86_64-unknown-linux-gnu.status" (
    set /p X86_64_UNKNOWN_LINUX_GNU_STATUS=<"%DIST%\hdd-autopilot-x86_64-unknown-linux-gnu.status"
  ) else (
    set "X86_64_UNKNOWN_LINUX_GNU_STATUS=built"
  )
  if /I "!X86_64_UNKNOWN_LINUX_GNU_STATUS!"=="built_degraded" (
    set "X86_64_UNKNOWN_LINUX_GNU_REASON=(native backend degraded)"
  ) else (
    set "X86_64_UNKNOWN_LINUX_GNU_REASON=(complete)"
  )
)
echo.
exit /b 0

:run_aarch64_unknown_linux_gnu
echo Checking Linux aarch64 build environment...
where bash >nul 2>&1
if errorlevel 1 (
  set "AARCH64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "AARCH64_UNKNOWN_LINUX_GNU_REASON=(missing bash)"
  echo Linux aarch64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-aarch64-unknown-linux-gnu.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "AARCH64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "AARCH64_UNKNOWN_LINUX_GNU_REASON=(missing build environment)"
  echo Linux aarch64 check failed, continuing with other platforms.
  echo.
  exit /b 0
)

echo.
echo Building Linux aarch64...
bash "%ROOT_BASH%/scripts/build-aarch64-unknown-linux-gnu.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "AARCH64_UNKNOWN_LINUX_GNU_STATUS=failed"
  set "AARCH64_UNKNOWN_LINUX_GNU_REASON=(build failed)"
) else (
  if exist "%DIST%\hdd-autopilot-aarch64-unknown-linux-gnu.status" (
    set /p AARCH64_UNKNOWN_LINUX_GNU_STATUS=<"%DIST%\hdd-autopilot-aarch64-unknown-linux-gnu.status"
  ) else (
    set "AARCH64_UNKNOWN_LINUX_GNU_STATUS=built"
  )
  if /I "!AARCH64_UNKNOWN_LINUX_GNU_STATUS!"=="built_degraded" (
    set "AARCH64_UNKNOWN_LINUX_GNU_REASON=(native backend degraded)"
  ) else (
    set "AARCH64_UNKNOWN_LINUX_GNU_REASON=(complete)"
  )
)
echo.
exit /b 0
