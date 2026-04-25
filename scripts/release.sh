#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

bash "$ROOT/scripts/build-macos-amd64.sh"
bash "$ROOT/scripts/build-macos-arm64.sh"

echo "Release build finished."
