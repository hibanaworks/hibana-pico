#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

cd "${REPO_ROOT}"

cargo build \
  -p uno-q-heterogeneous \
  --target thumbv8m.main-none-eabi \
  --release \
  --bin uno-q-m33-native-kernel \
  --config 'target.thumbv8m.main-none-eabi.rustflags=["-C", "link-arg=-Texamples/uno-q-heterogeneous/linker/stm32u585-native.ld", "-C", "link-arg=--gc-sections"]'

echo "${REPO_ROOT}/target/thumbv8m.main-none-eabi/release/uno-q-m33-native-kernel"
