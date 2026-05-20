#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_DIR="$ROOT/examples/pico-nod/apple/PicoNodApp"

if ! command -v swift >/dev/null 2>&1; then
  echo "pico-nod app gate failed: swift toolchain is required" >&2
  exit 1
fi

if [[ -d /Applications/Xcode.app/Contents/Developer ]]; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

cd "$APP_DIR"
swift test
swift build -c release

if command -v xcodebuild >/dev/null 2>&1; then
  log="$(mktemp)"
  trap 'rm -f "$log"' EXIT
  xcodebuild \
    -project PicoNodApp.xcodeproj \
    -scheme PicoNodApp \
    -configuration Release \
    -destination 'generic/platform=iOS' \
    CODE_SIGNING_ALLOWED=NO \
    build >"$log" 2>&1
  actionable_log="$(mktemp)"
  trap 'rm -f "$log" "$actionable_log"' EXIT
  rg -v "warning: Metadata extraction skipped\\. No AppIntents\\.framework dependency found\\." "$log" >"$actionable_log" || true
  if rg -n "warning:|error:" "$actionable_log"; then
    echo "pico-nod app gate failed: xcodebuild emitted warnings" >&2
    exit 1
  fi
else
  echo "pico-nod app gate failed: xcodebuild is required" >&2
  exit 1
fi

echo "pico-nod app gate ok"
