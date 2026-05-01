#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
HOST_OS="$(uname -s)"
X86_64_PC_WINDOWS_MSVC_STATUS="failed"
X86_64_PC_WINDOWS_MSVC_REASON=""
X86_64_APPLE_DARWIN_STATUS="failed"
X86_64_APPLE_DARWIN_REASON=""
AARCH64_APPLE_DARWIN_STATUS="failed"
AARCH64_APPLE_DARWIN_REASON=""
X86_64_UNKNOWN_LINUX_GNU_STATUS="failed"
X86_64_UNKNOWN_LINUX_GNU_REASON=""
AARCH64_UNKNOWN_LINUX_GNU_STATUS="failed"
AARCH64_UNKNOWN_LINUX_GNU_REASON=""

run_x86_64_pc_windows_msvc() {
  echo "Checking Windows x86_64 build environment..."
  case "$HOST_OS" in
    MINGW*|MSYS*|CYGWIN*)
      if ! command -v powershell.exe >/dev/null 2>&1; then
        X86_64_PC_WINDOWS_MSVC_STATUS="failed"
        X86_64_PC_WINDOWS_MSVC_REASON="(missing powershell.exe)"
        echo "Windows x86_64 check failed, continuing with other platforms."
        echo
        return 0
      fi
      ;;
    *)
      X86_64_PC_WINDOWS_MSVC_STATUS="failed"
      X86_64_PC_WINDOWS_MSVC_REASON="(current host cannot call Windows batch build)"
      echo "Windows x86_64 check failed, continuing with other platforms."
      echo
      return 0
      ;;
  esac

  local script="$ROOT/scripts/build-x86_64-pc-windows-msvc.bat"
  local windows_script="$script"
  if command -v cygpath >/dev/null 2>&1; then
    windows_script="$(cygpath -w "$script")"
  fi
  if ! powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "& '$windows_script' --check; exit \$LASTEXITCODE"; then
    X86_64_PC_WINDOWS_MSVC_STATUS="failed"
    X86_64_PC_WINDOWS_MSVC_REASON="(missing build environment)"
    echo "Windows x86_64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Windows x86_64..."
  if ! powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "& '$windows_script' --orchestrated; exit \$LASTEXITCODE"; then
    X86_64_PC_WINDOWS_MSVC_STATUS="failed"
    X86_64_PC_WINDOWS_MSVC_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-x86_64-pc-windows-msvc.status" ]; then
      X86_64_PC_WINDOWS_MSVC_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-x86_64-pc-windows-msvc.status")"
    else
      X86_64_PC_WINDOWS_MSVC_STATUS="built"
    fi
    if [ "$X86_64_PC_WINDOWS_MSVC_STATUS" = "built_degraded" ]; then
      X86_64_PC_WINDOWS_MSVC_REASON="(native CUDA backend degraded)"
    else
      X86_64_PC_WINDOWS_MSVC_REASON="(complete)"
    fi
  fi
  echo
}

run_x86_64_apple_darwin() {
  echo "Checking macOS x86_64 build environment..."
  if ! bash "$ROOT/scripts/build-x86_64-apple-darwin.sh" --orchestrated --check; then
    X86_64_APPLE_DARWIN_STATUS="failed"
    X86_64_APPLE_DARWIN_REASON="(missing build environment)"
    echo "macOS x86_64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building macOS x86_64..."
  if ! bash "$ROOT/scripts/build-x86_64-apple-darwin.sh" --orchestrated; then
    X86_64_APPLE_DARWIN_STATUS="failed"
    X86_64_APPLE_DARWIN_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-x86_64-apple-darwin.status" ]; then
      X86_64_APPLE_DARWIN_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-x86_64-apple-darwin.status")"
    else
      X86_64_APPLE_DARWIN_STATUS="built"
    fi
    if [ "$X86_64_APPLE_DARWIN_STATUS" = "built_degraded" ]; then
      X86_64_APPLE_DARWIN_REASON="(OpenCL/Metal backend degraded)"
    else
      X86_64_APPLE_DARWIN_REASON="(complete)"
    fi
  fi
  echo
}

run_aarch64_apple_darwin() {
  echo "Checking macOS aarch64 build environment..."
  if ! bash "$ROOT/scripts/build-aarch64-apple-darwin.sh" --orchestrated --check; then
    AARCH64_APPLE_DARWIN_STATUS="failed"
    AARCH64_APPLE_DARWIN_REASON="(missing build environment)"
    echo "macOS aarch64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building macOS aarch64..."
  if ! bash "$ROOT/scripts/build-aarch64-apple-darwin.sh" --orchestrated; then
    AARCH64_APPLE_DARWIN_STATUS="failed"
    AARCH64_APPLE_DARWIN_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-aarch64-apple-darwin.status" ]; then
      AARCH64_APPLE_DARWIN_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-aarch64-apple-darwin.status")"
    else
      AARCH64_APPLE_DARWIN_STATUS="built"
    fi
    if [ "$AARCH64_APPLE_DARWIN_STATUS" = "built_degraded" ]; then
      AARCH64_APPLE_DARWIN_REASON="(OpenCL/Metal backend degraded)"
    else
      AARCH64_APPLE_DARWIN_REASON="(complete)"
    fi
  fi
  echo
}

run_x86_64_unknown_linux_gnu() {
  echo "Checking Linux x86_64 build environment..."
  if ! bash "$ROOT/scripts/build-x86_64-unknown-linux-gnu.sh" --orchestrated --check; then
    X86_64_UNKNOWN_LINUX_GNU_STATUS="failed"
    X86_64_UNKNOWN_LINUX_GNU_REASON="(missing build environment)"
    echo "Linux x86_64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Linux x86_64..."
  if ! bash "$ROOT/scripts/build-x86_64-unknown-linux-gnu.sh" --orchestrated; then
    X86_64_UNKNOWN_LINUX_GNU_STATUS="failed"
    X86_64_UNKNOWN_LINUX_GNU_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-x86_64-unknown-linux-gnu.status" ]; then
      X86_64_UNKNOWN_LINUX_GNU_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-x86_64-unknown-linux-gnu.status")"
    else
      X86_64_UNKNOWN_LINUX_GNU_STATUS="built"
    fi
    if [ "$X86_64_UNKNOWN_LINUX_GNU_STATUS" = "built_degraded" ]; then
      X86_64_UNKNOWN_LINUX_GNU_REASON="(native backend degraded)"
    else
      X86_64_UNKNOWN_LINUX_GNU_REASON="(complete)"
    fi
  fi
  echo
}

run_aarch64_unknown_linux_gnu() {
  echo "Checking Linux aarch64 build environment..."
  if ! bash "$ROOT/scripts/build-aarch64-unknown-linux-gnu.sh" --orchestrated --check; then
    AARCH64_UNKNOWN_LINUX_GNU_STATUS="failed"
    AARCH64_UNKNOWN_LINUX_GNU_REASON="(missing build environment)"
    echo "Linux aarch64 check failed, continuing with other platforms."
    echo
    return 0
  fi

  echo
  echo "Building Linux aarch64..."
  if ! bash "$ROOT/scripts/build-aarch64-unknown-linux-gnu.sh" --orchestrated; then
    AARCH64_UNKNOWN_LINUX_GNU_STATUS="failed"
    AARCH64_UNKNOWN_LINUX_GNU_REASON="(build failed)"
  else
    if [ -f "$DIST/hdd-autopilot-aarch64-unknown-linux-gnu.status" ]; then
      AARCH64_UNKNOWN_LINUX_GNU_STATUS="$(tr -d '\r\n' < "$DIST/hdd-autopilot-aarch64-unknown-linux-gnu.status")"
    else
      AARCH64_UNKNOWN_LINUX_GNU_STATUS="built"
    fi
    if [ "$AARCH64_UNKNOWN_LINUX_GNU_STATUS" = "built_degraded" ]; then
      AARCH64_UNKNOWN_LINUX_GNU_REASON="(native backend degraded)"
    else
      AARCH64_UNKNOWN_LINUX_GNU_REASON="(complete)"
    fi
  fi
  echo
}

run_x86_64_pc_windows_msvc
run_x86_64_apple_darwin
run_aarch64_apple_darwin
run_x86_64_unknown_linux_gnu
run_aarch64_unknown_linux_gnu

echo
echo "Release package summary:"
echo "  Windows x86_64  : $X86_64_PC_WINDOWS_MSVC_STATUS $X86_64_PC_WINDOWS_MSVC_REASON"
echo "  macOS x86_64  : $X86_64_APPLE_DARWIN_STATUS $X86_64_APPLE_DARWIN_REASON"
echo "  macOS aarch64  : $AARCH64_APPLE_DARWIN_STATUS $AARCH64_APPLE_DARWIN_REASON"
echo "  Linux x86_64  : $X86_64_UNKNOWN_LINUX_GNU_STATUS $X86_64_UNKNOWN_LINUX_GNU_REASON"
echo "  Linux aarch64  : $AARCH64_UNKNOWN_LINUX_GNU_STATUS $AARCH64_UNKNOWN_LINUX_GNU_REASON"
echo

if [ "$X86_64_PC_WINDOWS_MSVC_STATUS" = "failed" ] || [ "$X86_64_APPLE_DARWIN_STATUS" = "failed" ] || [ "$AARCH64_APPLE_DARWIN_STATUS" = "failed" ] || [ "$X86_64_UNKNOWN_LINUX_GNU_STATUS" = "failed" ] || [ "$AARCH64_UNKNOWN_LINUX_GNU_STATUS" = "failed" ]; then
  exit 1
fi

if [ "$X86_64_PC_WINDOWS_MSVC_STATUS" = "built_degraded" ] || [ "$X86_64_APPLE_DARWIN_STATUS" = "built_degraded" ] || [ "$AARCH64_APPLE_DARWIN_STATUS" = "built_degraded" ] || [ "$X86_64_UNKNOWN_LINUX_GNU_STATUS" = "built_degraded" ] || [ "$AARCH64_UNKNOWN_LINUX_GNU_STATUS" = "built_degraded" ]; then
  echo "Release packaging completed with degraded package(s)."
else
  echo "Release packaging completed."
fi
