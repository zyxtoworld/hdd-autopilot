#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

echo "Building hdd-macos-amd64..."
GOOS=darwin GOARCH=amd64 go build -o "$DIST/hdd-macos-amd64" ./cmd/hdd

echo "Done: $DIST/hdd-macos-amd64"
