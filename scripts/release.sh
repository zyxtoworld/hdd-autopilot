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
LINUX_AMD64_STATUS="failed"
LINUX_AMD64_REASON=""
LINUX_ARM64_STATUS="failed"
LINUX_ARM64_REASON=""

run_windows() {
  echo "Checking Windows x64 build environment..."
  case "$HOST_OS" in
    MINGW*|MSYS*|CYGWIN*)
      if ! command -v powershell.exe >/dev/null 2>&1; then
        WIN_STATUS="failed"
        WIN_REASON="(missing powershell.exe)"
        echo "Windows x64 check failed, continuing with other platforms."
        echo
        return 0
      fi
      ;;
    *)
      WIN_STATUS="failed"
      WIN_REASON="(current host cannot call Windows batch build)"
      echo "Windows x64 check failed, continuing with other platforms."
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
    WIN_REASON="(missing build environment)"
    echo "Windows x64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Windows x64..."
  if ! powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "& '$windows_script' --orchestrated; exit \$LASTEXITCODE"; then
    WIN_STATUS="failed"
    WIN_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-win-x64.status" ]; then
      WIN_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-win-x64.status")"
    else
      WIN_STATUS="built"
    fi
    if [ "$WIN_STATUS" = "built_degraded" ]; then
      WIN_REASON="(native CUDA backend degraded)"
    else
      WIN_REASON="(complete)"
    fi
  fi
  echo
}

run_macos_amd64() {
  echo "Checking macOS amd64 build environment..."
  if ! bash "$ROOT/scripts/build-macos-amd64.sh" --orchestrated --check; then
    MAC_AMD64_STATUS="failed"
    MAC_AMD64_REASON="(missing build environment)"
    echo "macOS amd64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building macOS amd64..."
  if ! bash "$ROOT/scripts/build-macos-amd64.sh" --orchestrated; then
    MAC_AMD64_STATUS="failed"
    MAC_AMD64_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-macos-amd64.status" ]; then
      MAC_AMD64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-macos-amd64.status")"
    else
      MAC_AMD64_STATUS="built"
    fi
    if [ "$MAC_AMD64_STATUS" = "built_degraded" ]; then
      MAC_AMD64_REASON="(OpenCL/Metal backend degraded)"
    else
      MAC_AMD64_REASON="(complete)"
    fi
  fi
  echo
}

run_macos_arm64() {
  echo "Checking macOS arm64 build environment..."
  if ! bash "$ROOT/scripts/build-macos-arm64.sh" --orchestrated --check; then
    MAC_ARM64_STATUS="failed"
    MAC_ARM64_REASON="(missing build environment)"
    echo "macOS arm64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building macOS arm64..."
  if ! bash "$ROOT/scripts/build-macos-arm64.sh" --orchestrated; then
    MAC_ARM64_STATUS="failed"
    MAC_ARM64_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-macos-arm64.status" ]; then
      MAC_ARM64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-macos-arm64.status")"
    else
      MAC_ARM64_STATUS="built"
    fi
    if [ "$MAC_ARM64_STATUS" = "built_degraded" ]; then
      MAC_ARM64_REASON="(OpenCL/Metal backend degraded)"
    else
      MAC_ARM64_REASON="(complete)"
    fi
  fi
  echo
}

run_linux_amd64() {
  echo "Checking Linux amd64 build environment..."
  if ! bash "$ROOT/scripts/build-linux-amd64.sh" --orchestrated --check; then
    LINUX_AMD64_STATUS="failed"
    LINUX_AMD64_REASON="(missing build environment)"
    echo "Linux amd64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Linux amd64..."
  if ! bash "$ROOT/scripts/build-linux-amd64.sh" --orchestrated; then
    LINUX_AMD64_STATUS="failed"
    LINUX_AMD64_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-linux-amd64.status" ]; then
      LINUX_AMD64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-linux-amd64.status")"
    else
      LINUX_AMD64_STATUS="built"
    fi
    if [ "$LINUX_AMD64_STATUS" = "built_degraded" ]; then
      LINUX_AMD64_REASON="(native backend degraded)"
    else
      LINUX_AMD64_REASON="(complete)"
    fi
  fi
  echo
}

run_linux_arm64() {
  echo "Checking Linux arm64 build environment..."
  if ! bash "$ROOT/scripts/build-linux-arm64.sh" --orchestrated --check; then
    LINUX_ARM64_STATUS="failed"
    LINUX_ARM64_REASON="(missing build environment)"
    echo "Linux arm64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Linux arm64..."
  if ! bash "$ROOT/scripts/build-linux-arm64.sh" --orchestrated; then
    LINUX_ARM64_STATUS="failed"
    LINUX_ARM64_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-linux-arm64.status" ]; then
      LINUX_ARM64_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-linux-arm64.status")"
    else
      LINUX_ARM64_STATUS="built"
    fi
    if [ "$LINUX_ARM64_STATUS" = "built_degraded" ]; then
      LINUX_ARM64_REASON="(native backend degraded)"
    else
      LINUX_ARM64_REASON="(complete)"
    fi
  fi
  echo
}

run_windows
run_macos_amd64
run_macos_arm64
run_linux_amd64
run_linux_arm64

echo
echo "Release package summary:"
echo "  Windows x64  : $WIN_STATUS $WIN_REASON"
echo "  macOS amd64  : $MAC_AMD64_STATUS $MAC_AMD64_REASON"
echo "  macOS arm64  : $MAC_ARM64_STATUS $MAC_ARM64_REASON"
echo "  Linux amd64  : $LINUX_AMD64_STATUS $LINUX_AMD64_REASON"
echo "  Linux arm64  : $LINUX_ARM64_STATUS $LINUX_ARM64_REASON"
echo

if [ "$WIN_STATUS" = "failed" ] || [ "$MAC_AMD64_STATUS" = "failed" ] || [ "$MAC_ARM64_STATUS" = "failed" ] || [ "$LINUX_AMD64_STATUS" = "failed" ] || [ "$LINUX_ARM64_STATUS" = "failed" ]; then
  exit 1
fi

if [ "$WIN_STATUS" = "built_degraded" ] || [ "$MAC_AMD64_STATUS" = "built_degraded" ] || [ "$MAC_ARM64_STATUS" = "built_degraded" ] || [ "$LINUX_AMD64_STATUS" = "built_degraded" ] || [ "$LINUX_ARM64_STATUS" = "built_degraded" ]; then
  echo "Release packaging completed with degraded package(s)."
else
  echo "Release packaging completed."
fi
