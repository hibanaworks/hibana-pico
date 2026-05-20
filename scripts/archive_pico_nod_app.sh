#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/examples/pico-nod/apple/PicoNodApp"
PROJECT="$APP/PicoNodApp.xcodeproj"
ARCHIVE_PATH="${PICO_NOD_ARCHIVE_PATH:-$ROOT/target/pico-nod/PicoNodApp.xcarchive}"
EXPORT_PATH="${PICO_NOD_EXPORT_PATH:-$ROOT/target/pico-nod/export}"
EXPORT_OPTIONS="${PICO_NOD_EXPORT_OPTIONS:-$ROOT/target/pico-nod/ExportOptions.plist}"

require_env() {
  if [ -z "${!1:-}" ]; then
    printf 'missing: %s\n' "$1" >&2
    exit 1
  fi
}

require_env PICO_NOD_APPLE_TEAM_ID
require_env PICO_NOD_BUNDLE_ID

if [ -d /Applications/Xcode.app/Contents/Developer ]; then
  export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi

xcodebuild -checkFirstLaunchStatus >/dev/null
mkdir -p "$(dirname "$ARCHIVE_PATH")" "$EXPORT_PATH"

cat >"$EXPORT_OPTIONS" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>destination</key>
  <string>export</string>
  <key>method</key>
  <string>app-store-connect</string>
  <key>signingStyle</key>
  <string>automatic</string>
  <key>teamID</key>
  <string>${PICO_NOD_APPLE_TEAM_ID}</string>
  <key>stripSwiftSymbols</key>
  <true/>
  <key>uploadSymbols</key>
  <true/>
</dict>
</plist>
EOF

xcodebuild \
  -project "$PROJECT" \
  -scheme PicoNodApp \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE_PATH" \
  DEVELOPMENT_TEAM="$PICO_NOD_APPLE_TEAM_ID" \
  PRODUCT_BUNDLE_IDENTIFIER="$PICO_NOD_BUNDLE_ID" \
  archive

xcodebuild \
  -exportArchive \
  -archivePath "$ARCHIVE_PATH" \
  -exportPath "$EXPORT_PATH" \
  -exportOptionsPlist "$EXPORT_OPTIONS"
