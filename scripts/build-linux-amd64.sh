#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARTIFACT_DIR="$ROOT/dist"
TARGET="x86_64-unknown-linux-gnu"
ZIG_TARGET="$TARGET.2.17"
OUTPUT="$ROOT/target/$TARGET/release/hdd-autopilot"
ARTIFACT="$ARTIFACT_DIR/hdd-autopilot-linux-amd64"
STATUS_FILE="$ARTIFACT.status"
CHECK_ONLY="${1:-}"
ORCHESTRATED=0
if [ "$CHECK_ONLY" = "--orchestrated" ]; then
  ORCHESTRATED=1
  CHECK_ONLY="${2:-}"
fi

host_is_linux_amd64() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  [ "$os" = "Linux" ] && { [ "$arch" = "x86_64" ] || [ "$arch" = "amd64" ]; }
}

has_c_compiler() {
  command -v cc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1
}

has_zig_toolchain() {
  command -v cargo-zigbuild >/dev/null 2>&1 && command -v zig >/dev/null 2>&1
}

print_missing_env_message() {
  echo "Linux amd64 packaging environment missing: install Rust cargo plus cargo-zigbuild and zig, or build on a Linux amd64 host with a C compiler." >&2
}

pause_on_missing_env() {
  if [ -t 0 ]; then
    printf '\nPress any key to exit...'
    local _key
    IFS= read -r -n 1 _key || true
    printf '\n'
  fi
}

check_env() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Linux amd64 packaging environment missing: cargo is not in PATH." >&2
    return 2
  fi

  if has_zig_toolchain; then
    return 0
  fi

  if host_is_linux_amd64 && has_c_compiler; then
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
  echo "Package is corrupted: missing payload marker." >&2
  exit 1
fi
TMPDIR_VALUE="${TMPDIR:-/tmp}"
TMPFILE=$(mktemp "$TMPDIR_VALUE/hdd-autopilot-linux-amd64.XXXXXX")
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

echo "Building hdd-autopilot-linux-amd64..."
rustup target add "$TARGET" >/dev/null 2>&1 || true
BUILD_LOG="$ARTIFACT.log"
rm -f "$BUILD_LOG" "$STATUS_FILE"
if has_zig_toolchain; then
  echo "Using cargo-zigbuild target $ZIG_TARGET for wider Linux compatibility." | tee "$BUILD_LOG"
  cargo zigbuild --release --package hdd-autopilot --target "$ZIG_TARGET" 2>&1 | tee -a "$BUILD_LOG"
elif host_is_linux_amd64 && has_c_compiler; then
  echo "Using native Linux build; compatibility follows this host glibc version." | tee "$BUILD_LOG"
  cargo build --release --package hdd-autopilot --target "$TARGET" 2>&1 | tee -a "$BUILD_LOG"
fi
if [ ! -f "$OUTPUT" ]; then
  echo "Linux amd64 build failed: expected output not found at $OUTPUT." >&2
  exit 1
fi
if grep -Eqi "native backend disabled" "$BUILD_LOG"; then
  printf 'built_degraded\n' > "$STATUS_FILE"
else
  printf 'built\n' > "$STATUS_FILE"
fi
write_wrapper "$OUTPUT" "$ARTIFACT"

echo "Build completed: $ARTIFACT"
