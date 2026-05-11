#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed | rg -q '^wasm32-wasip1$'; then
  echo "wasm32-wasip1 target is not installed; run: rustup target add wasm32-wasip1" >&2
  exit 1
fi

CARGO_TARGET_DIR="$ROOT/target/wasip1-apps" \
  cargo build \
    --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bin wasip1-std-choreofs-read \
    --bin wasip1-std-choreofs-append \
    --bin wasip1-std-bad-path \
    --bin wasip1-std-choreofs-static-write

artifact_dir="$ROOT/target/wasip1-apps/wasm32-wasip1/release"

for artifact in \
  wasip1-std-choreofs-read \
  wasip1-std-choreofs-append \
  wasip1-std-bad-path \
  wasip1-std-choreofs-static-write
do
  wasm="$artifact_dir/$artifact.wasm"
  if [[ ! -s "$wasm" ]]; then
    echo "missing or empty ChoreoFS WASI P1 artifact: $wasm" >&2
    exit 1
  fi
  if ! rg -a -q 'wasi_snapshot_preview1' "$wasm"; then
    echo "ChoreoFS artifact lacks Preview 1 imports: $wasm" >&2
    exit 1
  fi
  if rg -a -q 'wasi:|wasm32-wasip2|wasip2|wasi_snapshot_preview2|preview2|wit-bindgen|wit_component|component-model' "$wasm"; then
    echo "ChoreoFS artifact contains forbidden P2/WIT/Component surface: $wasm" >&2
    exit 1
  fi
  if ! rg -a -q 'path_open' "$wasm"; then
    echo "ChoreoFS artifact lacks path_open: $wasm" >&2
    exit 1
  fi
done

run_ignored_test() {
  local test_name="$1"
  HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
    cargo test \
      --test host_wasip1_artifacts \
      --features profile-host-linux-wasip1-full \
      "$test_name" \
      -- \
      --ignored \
      --exact
}

run_ignored_test rust_built_std_choreofs_app_uses_resource_store_through_host_full_runner
run_ignored_test rust_built_std_choreofs_append_app_writes_and_reads_resource_store
run_ignored_test rust_built_bad_std_path_app_rejects_before_hidden_host_fs
run_ignored_test rust_built_bad_std_static_write_rejects_at_choreofs_control

echo "ChoreoFS supplemental WASI P1 demo ok"
