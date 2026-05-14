#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed | rg -q '^wasm32-wasip1$'; then
  echo "wasm32-wasip1 target is not installed; run: rustup target add wasm32-wasip1" >&2
  exit 1
fi

target_dir="$ROOT/target/wasip1-apps"
wasip1_rustflags="${RUSTFLAGS:-} -C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536 -C link-arg=-zstack-size=4096"

RUSTFLAGS="$wasip1_rustflags" \
CARGO_TARGET_DIR="$target_dir" \
  cargo build \
    --manifest-path apps/wasip1/swarm-node-apps/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

RUSTFLAGS="$wasip1_rustflags" \
CARGO_TARGET_DIR="$target_dir" \
  cargo build \
    --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

artifact_dir="$target_dir/wasm32-wasip1/release"

while IFS= read -r wasm; do
  if [[ ! -s "$wasm" ]]; then
    echo "missing or empty WASI P1 guest artifact: $wasm" >&2
    exit 1
  fi
  if ! rg -a -q 'wasi_snapshot_preview1' "$wasm"; then
    echo "WASI P1 guest artifact lacks preview1 imports: $wasm" >&2
    exit 1
  fi
  if rg -a -q 'wasi:|wasm32-wasip2|wasip2|wasi_snapshot_preview2|preview2|wit-bindgen|wit_component|component-model' "$wasm"; then
    echo "WASI P1 guest artifact contains forbidden P2/WIT/Component surface: $wasm" >&2
    exit 1
  fi
done < <(find "$artifact_dir" -maxdepth 1 -type f -name '*.wasm' | sort)

echo "wasip1 guest artifacts ok"
