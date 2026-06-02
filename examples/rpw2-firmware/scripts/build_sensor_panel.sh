#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

guest_manifest="examples/rpw2-firmware/wasip1/guest/Cargo.toml"
guest_target_dir="target/wasip1-apps"
guest_rustflags="-C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536 -C link-arg=-zstack-size=4096"

RUSTFLAGS="$guest_rustflags" CARGO_TARGET_DIR="$guest_target_dir" \
  cargo build \
    --manifest-path "$guest_manifest" \
    --release \
    --target wasm32-wasip1 \
    --bin rpw2-sensor-panel-guest

cargo build \
  -p rpw2-firmware \
  --release \
  --target thumbv8m.main-none-eabi \
  --bin rpw2-sensor-panel \
  --no-default-features \
  --features wasm-engine-core,wasip1-sys-fd-read,wasip1-sys-fd-write,wasip1-sys-path-open,wasip1-sys-poll-oneoff,wasip1-sys-proc-exit,embed-wasip1-artifacts,embed-cyw43-artifacts \
  --config 'target.thumbv8m.main-none-eabi.rustflags=["-C","link-arg=-Texamples/rpw2-firmware/linker/rp2350.ld","-C","link-arg=--gc-sections"]'
