#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

echo "Building hdd-macos-arm64..."
GOOS=darwin GOARCH=arm64 go build -o "$DIST/hdd-macos-arm64" ./cmd/hdd

echo "Done: $DIST/hdd-macos-arm64"
