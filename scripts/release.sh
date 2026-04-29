#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
HOST_OS="$(uname -s)"
WIN_STATUS="failed"
WIN_REASON=""
MAC_AMD64_STATUS="failed"
MAC_AMD64_REASON=""
MAC_ARM64_STATUS="failed"
MAC_ARM64_REASON=""

run_windows() {
  echo "正在检查 Windows x64 构建环境..."
  case "$HOST_OS" in
    MINGW*|MSYS*|CYGWIN*)
      if ! command -v powershell.exe >/dev/null 2>&1; then
        WIN_STATUS="failed"
        WIN_REASON="(缺少 powershell.exe)"
        echo "Windows x64 检查失败，继续处理其他平台。"
        echo
        return 0
      fi
      ;;
    *)
      WIN_STATUS="failed"
      WIN_REASON="(当前宿主不支持直接调用 Windows 批处理)"
      echo "Windows x64 检查失败，继续处理其他平台。"
      echo
      return 0
      ;;
  esac

  local script="$ROOT/scripts/build-win-x64.bat"
  local windows_script="$script"
  if command -v cygpath >/dev/null 2>&1; then
    windows_script="$(cygpath -w "$script")"
  fi
  if ! powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "& '$windows_script' --check; exit \$LASTEXITCODE"; then
    WIN_STATUS="failed"
    WIN_REASON="(基础打包环境缺失)"
    echo "Windows x64 检查失败，继续处理其他平台。"
    echo
    return 0
  fi

  echo
  echo "正在构建 Windows x64..."
  if ! powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "& '$windows_script' --orchestrated; exit \$LASTEXITCODE"; then
    WIN_STATUS="failed"
    WIN_REASON="(基础包构建失败)"
  else
    if [ -f "$DIST/hdd-win-x64.status" ]; then
      WIN_STATUS="$(tr -d '\r\n' < "$DIST/hdd-win-x64.status")"
    else
      WIN_STATUS="built"
    fi
    if [ "$WIN_STATUS" = "built_degraded" ]; then
      WIN_REASON="(原生 CUDA 后端已自动降级)"
    else
      WIN_REASON="(完整包)"
    fi
  fi
  echo
}

run_macos_amd64() {
  echo "正在检查 macOS amd64 构建环境..."
  if ! bash "$ROOT/scripts/build-macos-amd64.sh" --orchestrated --check; then
    MAC_AMD64_STATUS="failed"
    MAC_AMD64_REASON="(基础打包环境缺失)"
    echo "macOS amd64 检查失败，继续处理其他平台。"
    echo
    return 0
  fi

  echo
  echo "正在构建 macOS amd64..."
  if ! bash "$ROOT/scripts/build-macos-amd64.sh" --orchestrated; then
    MAC_AMD64_STATUS="failed"
    MAC_AMD64_REASON="(基础包构建失败)"
  else
    if [ -f "$DIST/hdd-macos-amd64.status" ]; then
      MAC_AMD64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-macos-amd64.status")"
    else
      MAC_AMD64_STATUS="built"
    fi
    if [ "$MAC_AMD64_STATUS" = "built_degraded" ]; then
      MAC_AMD64_REASON="(OpenCL/Metal 已自动降级)"
    else
      MAC_AMD64_REASON="(完整包)"
    fi
  fi
  echo
}

run_macos_arm64() {
  echo "正在检查 macOS arm64 构建环境..."
  if ! bash "$ROOT/scripts/build-macos-arm64.sh" --orchestrated --check; then
    MAC_ARM64_STATUS="failed"
    MAC_ARM64_REASON="(基础打包环境缺失)"
    echo "macOS arm64 检查失败，继续处理其他平台。"
    echo
    return 0
  fi

  echo
  echo "正在构建 macOS arm64..."
  if ! bash "$ROOT/scripts/build-macos-arm64.sh" --orchestrated; then
    MAC_ARM64_STATUS="failed"
    MAC_ARM64_REASON="(基础包构建失败)"
  else
    if [ -f "$DIST/hdd-macos-arm64.status" ]; then
      MAC_ARM64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-macos-arm64.status")"
    else
      MAC_ARM64_STATUS="built"
    fi
    if [ "$MAC_ARM64_STATUS" = "built_degraded" ]; then
      MAC_ARM64_REASON="(OpenCL/Metal 已自动降级)"
    else
      MAC_ARM64_REASON="(完整包)"
    fi
  fi
  echo
}

run_windows
run_macos_amd64
run_macos_arm64

echo
echo "Release 打包汇总："
echo "  Windows x64  : $WIN_STATUS $WIN_REASON"
echo "  macOS amd64  : $MAC_AMD64_STATUS $MAC_AMD64_REASON"
echo "  macOS arm64  : $MAC_ARM64_STATUS $MAC_ARM64_REASON"
echo

if [ "$WIN_STATUS" = "failed" ] || [ "$MAC_AMD64_STATUS" = "failed" ] || [ "$MAC_ARM64_STATUS" = "failed" ]; then
  exit 1
fi
if [ "$WIN_STATUS" = "built_degraded" ] || [ "$MAC_AMD64_STATUS" = "built_degraded" ] || [ "$MAC_ARM64_STATUS" = "built_degraded" ]; then
  echo "Release 打包完成，部分平台为降级包。"
else
  echo "Release 打包完成。"
fi
