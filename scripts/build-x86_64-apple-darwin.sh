#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARTIFACT_DIR="$ROOT/dist"
TARGET="x86_64-apple-darwin"
OUTPUT="$ROOT/target/$TARGET/release/hdd-autopilot"
ARTIFACT="$ARTIFACT_DIR/hdd-autopilot-x86_64-apple-darwin"
STATUS_FILE="$ARTIFACT.status"
CHECK_ONLY="${1:-}"
ORCHESTRATED=0
if [ "$CHECK_ONLY" = "--orchestrated" ]; then
  ORCHESTRATED=1
  CHECK_ONLY="${2:-}"
fi

host_is_macos() {
  [ "$(uname -s)" = "Darwin" ]
}

has_c_compiler() {
  command -v cc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1
}

has_zig_toolchain() {
  command -v cargo-zigbuild >/dev/null 2>&1 && command -v zig >/dev/null 2>&1
}

resolve_apple_sdk() {
  if [ -n "${SDKROOT:-}" ] && [ -d "${SDKROOT}" ]; then
    printf '%s\n' "$SDKROOT"
    return 0
  fi
  if [ -n "${APPLE_SDK_ROOT:-}" ] && [ -d "${APPLE_SDK_ROOT}" ]; then
    printf '%s\n' "$APPLE_SDK_ROOT"
    return 0
  fi
  if host_is_macos && command -v xcrun >/dev/null 2>&1; then
    local sdk
    sdk="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
    if [ -n "$sdk" ] && [ -d "$sdk" ]; then
      printf '%s\n' "$sdk"
      return 0
    fi
  fi
  return 1
}

print_missing_env_message() {
  if host_is_macos; then
    echo "macOS x86_64 打包环境缺失：请安装 Xcode Command Line Tools，或安装 cargo-zigbuild + zig，并确保 xcrun/SDKROOT 可提供 MacOSX.sdk。" >&2
  else
    echo "macOS x86_64 打包环境缺失：请安装 cargo-zigbuild、zig，并设置 SDKROOT 或 APPLE_SDK_ROOT 指向可用的 MacOSX.sdk；否则请改到 macOS 主机上构建。" >&2
  fi
}

pause_on_missing_env() {
  if [ -t 0 ]; then
    printf '\n按任意键退出...'
    local _key
    IFS= read -r -n 1 _key || true
    printf '\n'
  fi
}

check_env() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "macOS x86_64 打包环境缺失：请先安装 Rust 工具链并确保 cargo 在 PATH 中。" >&2
    return 2
  fi

  if host_is_macos && has_c_compiler && command -v xcrun >/dev/null 2>&1; then
    return 0
  fi

  if has_zig_toolchain; then
    return 0
  fi

  print_missing_env_message
  return 2
}

write_wrapper() {
  local output="$1"
  local artifact="$2"
  local payload_line
  payload_line=$(cat <<'EOF'
#!/bin/sh
set -eu
SELF="$0"
PAYLOAD_LINE=$(awk '/^__HDD_AUTOPILOT_PAYLOAD_BELOW__$/ { print NR + 1; exit }' "$SELF")
if [ -z "$PAYLOAD_LINE" ]; then
  echo "包装损坏：缺少 payload 标记。" >&2
  exit 1
fi
TMPDIR_VALUE="${TMPDIR:-/tmp}"
TMPFILE=$(mktemp "$TMPDIR_VALUE/hdd-autopilot-x86_64-apple-darwin.XXXXXX")
cleanup() {
  rm -f "$TMPFILE"
}
trap cleanup EXIT INT TERM HUP
TAIL_BIN=$(command -v tail)
"$TAIL_BIN" -n +"$PAYLOAD_LINE" "$SELF" > "$TMPFILE"
chmod +x "$TMPFILE"
"$TMPFILE" "$@"
STATUS=$?
exit "$STATUS"
__HDD_AUTOPILOT_PAYLOAD_BELOW__
EOF
)
  printf '%s\n' "$payload_line" > "$artifact"
  cat "$output" >> "$artifact"
  chmod +x "$artifact"
}

if [ "$CHECK_ONLY" = "--check" ]; then
  check_env
  exit $?
fi

if ! check_env; then
  status=$?
  if [ "$status" -eq 2 ] && [ "$ORCHESTRATED" -ne 1 ]; then
    pause_on_missing_env
  fi
  exit "$status"
fi
mkdir -p "$ARTIFACT_DIR"

echo "正在构建 hdd-autopilot-x86_64-apple-darwin..."
rustup target add "$TARGET" >/dev/null 2>&1 || true
BUILD_LOG="$ARTIFACT.log"
rm -f "$BUILD_LOG" "$STATUS_FILE"
if has_zig_toolchain; then
  if SDK_PATH="$(resolve_apple_sdk 2>/dev/null)"; then
    SDKROOT="$SDK_PATH" cargo zigbuild --release --package hdd-autopilot --target "$TARGET" 2>&1 | tee "$BUILD_LOG"
  else
    cargo zigbuild --release --package hdd-autopilot --target "$TARGET" 2>&1 | tee "$BUILD_LOG"
  fi
else
  cargo build --release --package hdd-autopilot --target "$TARGET" 2>&1 | tee "$BUILD_LOG"
fi
if [ ! -f "$OUTPUT" ]; then
  echo "macOS x86_64 构建失败：当前环境缺少可用的 Apple 目标工具链；至少需要可用的 zig+Apple SDK，或可工作的 $TARGET C/Framework 交叉编译环境。" >&2
  exit 1
fi
if grep -Eqi "(OpenCL|Metal) native backend disabled" "$BUILD_LOG"; then
  printf 'built_degraded\n' > "$STATUS_FILE"
else
  printf 'built\n' > "$STATUS_FILE"
fi
write_wrapper "$OUTPUT" "$ARTIFACT"

echo "构建完成：$ARTIFACT"
