#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

cd "${REPO_ROOT}"

rustflags_sep="$(printf '\037')"
CARGO_ENCODED_RUSTFLAGS="-C${rustflags_sep}link-arg=-Texamples/uno-q-heterogeneous/linker/stm32u585-native.ld${rustflags_sep}-C${rustflags_sep}link-arg=--gc-sections" \
cargo build \
  -p uno-q-heterogeneous \
  --target thumbv8m.main-none-eabi \
  --release \
  --bin uno-q-m33-native-kernel

echo "${REPO_ROOT}/target/thumbv8m.main-none-eabi/release/uno-q-m33-native-kernel"
