#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/examples/pico-nod/apple/PicoNodApp"

missing=0

note_missing() {
  printf 'missing: %s\n' "$1" >&2
  missing=1
}

if [ ! -d "$APP" ]; then
  note_missing "examples/pico-nod/apple/PicoNodApp"
fi

if [ -d /Applications/Xcode.app/Contents/Developer ]; then
  export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi

if ! command -v xcodebuild >/dev/null 2>&1; then
  note_missing "xcodebuild"
else
  xcodebuild -version >/dev/null
  xcodebuild -license check >/dev/null
  if ! xcodebuild -checkFirstLaunchStatus >/dev/null 2>&1; then
    note_missing "xcodebuild first launch setup"
  fi
fi

if ! command -v swift >/dev/null 2>&1; then
  note_missing "swift"
elif command -v xcodebuild >/dev/null 2>&1; then
  "$ROOT/scripts/check_pico_nod_app.sh" >/dev/null
fi

for name in \
  PICO_NOD_APPLE_TEAM_ID \
  PICO_NOD_BUNDLE_ID \
  PICO_NOD_APNS_KEY_ID \
  PICO_NOD_APNS_TEAM_ID \
  PICO_NOD_APNS_TOPIC \
  PICO_NOD_APNS_PRIVATE_KEY_PATH \
  PICO_NOD_STORE_ISSUER_ID \
  PICO_NOD_STORE_KEY_ID \
  PICO_NOD_STORE_PRIVATE_KEY_PATH \
  PICO_NOD_TLS_TERMINATION \
  PICO_NOD_EXTERNAL_ACTION_ENDPOINT \
  PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH
do
  if [ -z "${!name:-}" ]; then
    note_missing "$name"
  fi
done

for name in \
  PICO_NOD_APNS_PRIVATE_KEY_PATH \
  PICO_NOD_STORE_PRIVATE_KEY_PATH \
  PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH
do
  if [ -n "${!name:-}" ] && [ ! -r "${!name}" ]; then
    note_missing "$name readable file"
  fi
done

for file in \
  "$APP/Package.swift" \
  "$APP/PicoNodApp.xcodeproj/project.pbxproj" \
  "$APP/PicoNodApp.xcodeproj/xcshareddata/xcschemes/PicoNodApp.xcscheme" \
  "$APP/Sources/PicoNodApp/PicoNodApp.swift" \
  "$APP/Sources/PicoNodApp/Assets.xcassets/AppIcon.appiconset/Contents.json" \
  "$APP/Sources/PicoNodApp/Assets.xcassets/AppIcon.appiconset/pico-nod-1024.png" \
  "$APP/Sources/PicoNodApp/LaunchScreen.storyboard" \
  "$APP/Sources/PicoNodApp/PicoNod.entitlements" \
  "$APP/Sources/PicoNodApp/PrivacyInfo.xcprivacy" \
  "$ROOT/examples/pico-nod/src/apns.rs" \
  "$ROOT/examples/pico-nod/src/billing.rs" \
  "$ROOT/examples/pico-nod/src/commit.rs" \
  "$ROOT/examples/pico-nod/src/acceptor.rs" \
  "$ROOT/examples/pico-nod/deploy/env.example" \
  "$ROOT/examples/pico-nod/deploy/launchd/com.hibana.pico-nod.plist" \
  "$ROOT/examples/pico-nod/release/app-store-review.md" \
  "$ROOT/examples/pico-nod/release/privacy-labels.md" \
  "$ROOT/examples/pico-nod/release/operations-runbook.md" \
  "$ROOT/scripts/archive_pico_nod_app.sh"
do
  if [ ! -f "$file" ]; then
    note_missing "${file#$ROOT/}"
  fi
done

if [ -n "${PICO_NOD_TLS_TERMINATION:-}" ] && [ "$PICO_NOD_TLS_TERMINATION" != external-loopback ]; then
  note_missing "PICO_NOD_TLS_TERMINATION=external-loopback"
fi

if [ "$missing" -ne 0 ]; then
  cat >&2 <<'EOF'
pico-nod release readiness: not ready

This is intentional unless the Apple and production boundary credentials above
are configured. Normal proof gates may pass before App Store release or
production server operation is possible.
EOF
  exit 1
fi

"$ROOT/scripts/archive_pico_nod_app.sh"
printf 'pico-nod release readiness: ready\n'
