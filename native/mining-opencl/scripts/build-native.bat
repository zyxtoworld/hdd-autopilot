@echo off
setlocal EnableExtensions
chcp 65001 >nul

echo [mining-opencl] OpenCL 原生后端仅支持在 macOS 上随 cargo/build.rs 构建。
echo [mining-opencl] 请在 macOS 上运行 cargo build 或 scripts/build-macos-*.sh。
exit /b 1
