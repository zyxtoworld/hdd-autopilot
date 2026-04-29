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

call :run_windows
call :run_macos_amd64
call :run_macos_arm64

echo.
echo Release 打包汇总：
echo   Windows x64  : !WIN_STATUS! !WIN_REASON!
echo   macOS amd64  : !MAC_AMD64_STATUS! !MAC_AMD64_REASON!
echo   macOS arm64  : !MAC_ARM64_STATUS! !MAC_ARM64_REASON!

echo.
if /I "!WIN_STATUS!"=="failed" exit /b 1
if /I "!MAC_AMD64_STATUS!"=="failed" exit /b 1
if /I "!MAC_ARM64_STATUS!"=="failed" exit /b 1
if /I "!WIN_STATUS!"=="built_degraded" goto :degraded
if /I "!MAC_AMD64_STATUS!"=="built_degraded" goto :degraded
if /I "!MAC_ARM64_STATUS!"=="built_degraded" goto :degraded
echo Release 打包完成。
exit /b 0

:degraded
echo Release 打包完成，部分平台为降级包。
exit /b 0

:run_windows
echo 正在检查 Windows x64 构建环境...
call "%ROOT%\scripts\build-win-x64.bat" --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "WIN_STATUS=failed"
  set "WIN_REASON=(基础打包环境缺失)"
  echo Windows x64 检查失败，继续处理其他平台。
  echo.
  exit /b 0
)

echo.
echo 正在构建 Windows x64...
call "%ROOT%\scripts\build-win-x64.bat" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "WIN_STATUS=failed"
  set "WIN_REASON=(基础包构建失败)"
) else (
  if exist "%DIST%\hdd-win-x64.status" (
    set /p WIN_STATUS=<"%DIST%\hdd-win-x64.status"
  ) else (
    set "WIN_STATUS=built"
  )
  if /I "!WIN_STATUS!"=="built_degraded" (
    set "WIN_REASON=(原生 CUDA 后端已自动降级)"
  ) else (
    set "WIN_REASON=(完整包)"
  )
)
echo.
exit /b 0

:run_macos_amd64
echo 正在检查 macOS amd64 构建环境...
where bash >nul 2>&1
if errorlevel 1 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(缺少 bash 运行环境)"
  echo macOS amd64 检查失败，继续处理其他平台。
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-macos-amd64.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(基础打包环境缺失)"
  echo macOS amd64 检查失败，继续处理其他平台。
  echo.
  exit /b 0
)

echo.
echo 正在构建 macOS amd64...
bash "%ROOT_BASH%/scripts/build-macos-amd64.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "MAC_AMD64_STATUS=failed"
  set "MAC_AMD64_REASON=(基础包构建失败)"
) else (
  if exist "%DIST%\hdd-macos-amd64.status" (
    set /p MAC_AMD64_STATUS=<"%DIST%\hdd-macos-amd64.status"
  ) else (
    set "MAC_AMD64_STATUS=built"
  )
  if /I "!MAC_AMD64_STATUS!"=="built_degraded" (
    set "MAC_AMD64_REASON=(OpenCL/Metal 已自动降级)"
  ) else (
    set "MAC_AMD64_REASON=(完整包)"
  )
)
echo.
exit /b 0

:run_macos_arm64
echo 正在检查 macOS arm64 构建环境...
where bash >nul 2>&1
if errorlevel 1 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(缺少 bash 运行环境)"
  echo macOS arm64 检查失败，继续处理其他平台。
  echo.
  exit /b 0
)

bash "%ROOT_BASH%/scripts/build-macos-arm64.sh" --orchestrated --check
set "CHECK_EXIT=%ERRORLEVEL%"
if !CHECK_EXIT! neq 0 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(基础打包环境缺失)"
  echo macOS arm64 检查失败，继续处理其他平台。
  echo.
  exit /b 0
)

echo.
echo 正在构建 macOS arm64...
bash "%ROOT_BASH%/scripts/build-macos-arm64.sh" --orchestrated
set "BUILD_EXIT=%ERRORLEVEL%"
if !BUILD_EXIT! neq 0 (
  set "MAC_ARM64_STATUS=failed"
  set "MAC_ARM64_REASON=(基础包构建失败)"
) else (
  if exist "%DIST%\hdd-macos-arm64.status" (
    set /p MAC_ARM64_STATUS=<"%DIST%\hdd-macos-arm64.status"
  ) else (
    set "MAC_ARM64_STATUS=built"
  )
  if /I "!MAC_ARM64_STATUS!"=="built_degraded" (
    set "MAC_ARM64_REASON=(OpenCL/Metal 已自动降级)"
  ) else (
    set "MAC_ARM64_REASON=(完整包)"
  )
)
echo.
exit /b 0
