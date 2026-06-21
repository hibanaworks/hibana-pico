#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

: "${RP2W_SENSOR_PANEL_WIFI_SSID:?set RP2W_SENSOR_PANEL_WIFI_SSID}"
: "${RP2W_SENSOR_PANEL_WIFI_KEY:?set RP2W_SENSOR_PANEL_WIFI_KEY}"
: "${RP2W_SENSOR_PANEL_LOCAL_MAC:?set RP2W_SENSOR_PANEL_LOCAL_MAC}"
: "${RP2W_SENSOR_PANEL_LOCAL_IP:?set RP2W_SENSOR_PANEL_LOCAL_IP}"
: "${RP2W_SENSOR_PANEL_UNO_Q_MAC:?set RP2W_SENSOR_PANEL_UNO_Q_MAC}"
: "${RP2W_SENSOR_PANEL_UNO_Q_IP:?set RP2W_SENSOR_PANEL_UNO_Q_IP}"

guest_manifest="examples/rp2w-firmware/wasip1/guest/Cargo.toml"
guest_target_dir="target/wasip1-apps"
guest_rustflags="-C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536 -C link-arg=-zstack-size=4096"

RUSTFLAGS="$guest_rustflags" CARGO_TARGET_DIR="$guest_target_dir" \
  cargo build \
    --manifest-path "$guest_manifest" \
    --release \
    --target wasm32-wasip1 \
    --bin rp2w-sensor-panel-guest

cargo build \
  -p rp2w-firmware \
  --release \
  --target thumbv8m.main-none-eabi \
  --bin rp2w-sensor-panel \
  --no-default-features \
  --features wasm-engine-core,embed-wasip1-artifacts,embed-cyw43-artifacts
