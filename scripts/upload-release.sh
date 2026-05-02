#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
TAG=""
REPO=""
TITLE=""
NOTES=""
DRAFT=0
PRERELEASE=0
ALLOW_DEGRADED=0
CLOBBER=1

usage() {
  cat <<'USAGE'
Usage:
  bash scripts/upload-release.sh <tag> [options]

Options:
  --repo <owner/name>     Upload to a specific GitHub repository.
  --dist <path>           Package directory. Defaults to repo-root/dist.
  --title <text>          Release title. Defaults to the tag.
  --notes <text>          Release notes. Defaults to generated notes.
  --draft                 Create the release as a draft when it does not exist.
  --prerelease            Create the release as a prerelease when it does not exist.
  --allow-degraded        Allow assets whose .status is built_degraded.
  --no-clobber            Do not overwrite existing release assets.
  -h, --help              Show this help.

The script scans dist/hdd-autopilot-*.status and uploads matching package files
only when the status is built, unless --allow-degraded is set.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --repo)
      REPO="${2:-}"
      shift 2
      ;;
    --dist)
      DIST="${2:-}"
      shift 2
      ;;
    --title)
      TITLE="${2:-}"
      shift 2
      ;;
    --notes)
      NOTES="${2:-}"
      shift 2
      ;;
    --draft)
      DRAFT=1
      shift
      ;;
    --prerelease)
      PRERELEASE=1
      shift
      ;;
    --allow-degraded)
      ALLOW_DEGRADED=1
      shift
      ;;
    --no-clobber)
      CLOBBER=0
      shift
      ;;
    --*)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$TAG" ]; then
        echo "Unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      TAG="$1"
      shift
      ;;
  esac
done

if [ -z "$TAG" ]; then
  usage >&2
  exit 2
fi
if [ -z "$TITLE" ]; then
  TITLE="$TAG"
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "GitHub CLI 'gh' is required. Install it and run 'gh auth login' first." >&2
  exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
  echo "GitHub CLI is not authenticated. Run 'gh auth login' first." >&2
  exit 1
fi
if [ ! -d "$DIST" ]; then
  echo "Package directory does not exist: $DIST" >&2
  exit 1
fi

shopt -s nullglob
status_files=("$DIST"/hdd-autopilot-*.status)
if [ "${#status_files[@]}" -eq 0 ]; then
  echo "No package status files found in '$DIST'. Run a build script first." >&2
  exit 1
fi

assets=()
blocked=()
for status_file in "${status_files[@]}"; do
  status="$(tr -d '\r\n' < "$status_file")"
  base_name="$(basename "${status_file%.status}")"
  case "$base_name" in
    hdd-autopilot-x86_64-pc-windows-msvc|hdd-autopilot-x86_64-pc-windows-msvc-*|\
    hdd-autopilot-x86_64-apple-darwin|hdd-autopilot-x86_64-apple-darwin-*|\
    hdd-autopilot-aarch64-apple-darwin|hdd-autopilot-aarch64-apple-darwin-*|\
    hdd-autopilot-x86_64-unknown-linux-gnu|hdd-autopilot-x86_64-unknown-linux-gnu-*|\
    hdd-autopilot-aarch64-unknown-linux-gnu|hdd-autopilot-aarch64-unknown-linux-gnu-*)
      ;;
    *)
      blocked+=("$(basename "$status_file")=unsupported-name")
      continue
      ;;
  esac

  base_path="${status_file%.status}"
  asset_path=""

  if [ -f "$base_path" ]; then
    asset_path="$base_path"
  elif [ -f "$base_path.exe" ]; then
    asset_path="$base_path.exe"
  fi

  if [ "$status" = "built" ] || { [ "$ALLOW_DEGRADED" -eq 1 ] && [ "$status" = "built_degraded" ]; }; then
    if [ -z "$asset_path" ]; then
      echo "Status '$(basename "$status_file")' is '$status', but the matching package file is missing." >&2
      exit 1
    fi
    assets+=("$asset_path")
  else
    blocked+=("$(basename "$status_file")=$status")
  fi
done

if [ "${#blocked[@]}" -gt 0 ]; then
  echo "Refusing to upload because non-built package status exists: ${blocked[*]}" >&2
  if [ "$ALLOW_DEGRADED" -eq 0 ]; then
    echo "Use --allow-degraded only if you intentionally want degraded packages." >&2
  fi
  exit 1
fi
if [ "${#assets[@]}" -eq 0 ]; then
  echo "No uploadable package assets found in '$DIST'." >&2
  exit 1
fi

repo_args=()
if [ -n "$REPO" ]; then
  repo_args+=(--repo "$REPO")
fi

if ! gh release view "$TAG" "${repo_args[@]}" >/dev/null 2>&1; then
  create_args=(release create "$TAG" "${repo_args[@]}" --title "$TITLE")
  if [ -n "$NOTES" ]; then
    create_args+=(--notes "$NOTES")
  else
    create_args+=(--generate-notes)
  fi
  if [ "$DRAFT" -eq 1 ]; then
    create_args+=(--draft)
  fi
  if [ "$PRERELEASE" -eq 1 ]; then
    create_args+=(--prerelease)
  fi
  gh "${create_args[@]}"
fi

upload_args=(release upload "$TAG" "${repo_args[@]}" "${assets[@]}")
if [ "$CLOBBER" -eq 1 ]; then
  upload_args+=(--clobber)
fi
gh "${upload_args[@]}"

echo "Uploaded ${#assets[@]} asset(s) to GitHub Release $TAG:"
for asset in "${assets[@]}"; do
  echo "  $(basename "$asset")"
done
