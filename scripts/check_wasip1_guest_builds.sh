#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed | rg -q '^wasm32-wasip1$'; then
  echo "wasm32-wasip1 target is not installed; run: rustup target add wasm32-wasip1" >&2
  exit 1
fi

target_dir="$ROOT/target/wasip1-apps"
artifact_dir="$target_dir/wasm32-wasip1/release"
wasip1_rustflags="${RUSTFLAGS:-} -C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536 -C link-arg=-zstack-size=4096"
expected_wasms=(
  wasip1-led-choreofs-traffic-cycle.wasm
  wasip1-led-choreofs-traffic-once.wasm
  wasip1-session-mismatch-fd-write.wasm
  rp2w-epf-policy-timer-guest.wasm
  rp2w-sensor-panel-guest.wasm
  uno-q-llm-face-shell-loop.wasm
  uno-q-llm-face-shell.wasm
)

rm -rf "$target_dir"
mkdir -p "$target_dir"

RUSTFLAGS="$wasip1_rustflags" \
CARGO_TARGET_DIR="$target_dir" \
  cargo build \
    --manifest-path examples/baker-firmware/wasip1/guest/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

RUSTFLAGS="$wasip1_rustflags" \
CARGO_TARGET_DIR="$target_dir" \
  cargo build \
    --manifest-path examples/rp2w-firmware/wasip1/guest/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

RUSTFLAGS="$wasip1_rustflags" \
CARGO_TARGET_DIR="$target_dir" \
  cargo build \
    --manifest-path examples/uno-q-heterogeneous/wasip1/guest/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

expected_list="$(mktemp "${TMPDIR:-/tmp}/hibana-pico-expected-wasm.XXXXXX")"
actual_list="$(mktemp "${TMPDIR:-/tmp}/hibana-pico-actual-wasm.XXXXXX")"
trap 'rm -f "$expected_list" "$actual_list"' EXIT

printf '%s\n' "${expected_wasms[@]}" | sort > "$expected_list"
find "$artifact_dir" -maxdepth 1 -type f -name '*.wasm' -exec basename {} \; | sort > "$actual_list"
if ! diff -u "$expected_list" "$actual_list"; then
  echo "WASI P1 guest artifact set differs from expected current outputs" >&2
  exit 1
fi

while IFS= read -r artifact; do
  wasm="$artifact_dir/$artifact"
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
done < "$expected_list"

echo "example wasip1 guest artifacts ok"
