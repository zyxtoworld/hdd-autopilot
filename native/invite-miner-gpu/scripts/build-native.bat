@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "PROJECT_NAME=invite-miner-gpu"
set "SCRIPT_DIR=%~dp0"
set "PROJECT_DIR=%SCRIPT_DIR%.."
set "ROOT_DIR=%PROJECT_DIR%\..\.."
set "BUILD_DIR=%PROJECT_DIR%\build"
set "DIST_EXE=%ROOT_DIR%\dist\invite-miner-gpu-win-x64.exe"
set "CUDA_ARCH=%CMAKE_CUDA_ARCHITECTURES%"
if "!CUDA_ARCH!"=="" set "CUDA_ARCH=89"

set "VS_INSTALLER_DIR=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer"
if exist "!VS_INSTALLER_DIR!" set "PATH=!VS_INSTALLER_DIR!;!PATH!"
set "VSWHERE=!VS_INSTALLER_DIR!\vswhere.exe"
if not exist "!VSWHERE!" (
    echo [!PROJECT_NAME!] 找不到 vswhere.exe，请先安装 Visual Studio 2022 Build Tools。
    exit /b 1
)

set "VS_INSTALL="
for /f "delims=" %%I in ('"!VSWHERE!" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2^>nul') do set "VS_INSTALL=%%I"
if "!VS_INSTALL!"=="" (
    echo [!PROJECT_NAME!] 找不到可用的 Visual Studio C++ 构建环境。
    exit /b 1
)

set "VSDEVCMD=!VS_INSTALL!\Common7\Tools\VsDevCmd.bat"
if not exist "!VSDEVCMD!" (
    echo [!PROJECT_NAME!] 找不到 VsDevCmd.bat：!VSDEVCMD!
    exit /b 1
)

echo [!PROJECT_NAME!] 先准备 VS 和 CUDA 的构建环境...
call "!VSDEVCMD!" -arch=x64 -host_arch=x64
if errorlevel 1 exit /b 1

set "CMAKE_CMD="
for /f "delims=" %%I in ('where cmake 2^>nul') do if not defined CMAKE_CMD set "CMAKE_CMD=%%I"
if not defined CMAKE_CMD set "CMAKE_CMD=!VS_INSTALL!\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
if not exist "!CMAKE_CMD!" (
    echo [!PROJECT_NAME!] 找不到 cmake，请安装 CMake 或使用带 CMake 的 Visual Studio 组件。
    exit /b 1
)

set "NINJA_CMD="
for /f "delims=" %%I in ('where ninja 2^>nul') do if not defined NINJA_CMD set "NINJA_CMD=%%I"
if not defined NINJA_CMD set "NINJA_CMD=!VS_INSTALL!\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja\ninja.exe"
if not exist "!NINJA_CMD!" (
    echo [!PROJECT_NAME!] 找不到 ninja，请安装 Ninja 或使用带 Ninja 的 Visual Studio 组件。
    exit /b 1
)
for %%I in ("!NINJA_CMD!") do set "PATH=%%~dpI;!PATH!"

echo [!PROJECT_NAME!] 开始配置 CMake...
"!CMAKE_CMD!" --fresh -S "!PROJECT_DIR!" -B "!BUILD_DIR!" -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_CUDA_ARCHITECTURES=!CUDA_ARCH!
if errorlevel 1 exit /b 1

echo [!PROJECT_NAME!] 开始编译程序...
"!CMAKE_CMD!" --build "!BUILD_DIR!" --config Release
if errorlevel 1 exit /b 1

echo [!PROJECT_NAME!] 已经编译完成，产物在 !DIST_EXE!
