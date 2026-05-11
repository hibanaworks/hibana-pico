#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TARGET="thumbv6m-none-eabi"
TOOLCHAIN="${HIBANA_PICO_TOOLCHAIN:-stable}"
TARGET_DIR="${HIBANA_PICO_TARGET_DIR:-$ROOT/target/pico_demo_budget}"
ENFORCE_PRACTICAL="${HIBANA_PICO_ENFORCE_PRACTICAL:-0}"

PRACTICAL_FLASH_BUDGET=$((768 * 1024))
PRACTICAL_STATIC_SRAM_BUDGET=$((48 * 1024))
PRACTICAL_KERNEL_STACK_BUDGET=$((24 * 1024))
PRACTICAL_PEAK_SRAM_BUDGET=$((96 * 1024))
BAKER_FLASH_BUDGET=$((1280 * 1024))
BAKER_STATIC_SRAM_BUDGET=$((208 * 1024))
BAKER_PEAK_SRAM_BUDGET=$((260 * 1024))

BUDGET_ENTRIES=(
  "rp2040-sio-smoke|hibana-pico-rp2040-sio-smoke|profile-rp2040-pico-min|practical"
  "baker-traffic|hibana-pico-baker-led-demo|profile-rp2040-pico-min embed-wasip1-artifacts|baker"
  "baker-choreofs|hibana-pico-baker-led-demo|profile-rp2040-pico-min embed-wasip1-artifacts baker-choreofs-demo|baker"
  "baker-choreofs-bad-path|hibana-pico-baker-led-demo|profile-rp2040-pico-min embed-wasip1-artifacts baker-choreofs-bad-path-demo|baker"
  "baker-choreofs-bad-payload|hibana-pico-baker-led-demo|profile-rp2040-pico-min embed-wasip1-artifacts baker-choreofs-bad-payload-demo|baker"
  "baker-choreofs-wrong-object|hibana-pico-baker-led-demo|profile-rp2040-pico-min embed-wasip1-artifacts baker-choreofs-wrong-object-demo|baker"
  "baker-fail-safe|hibana-pico-baker-led-demo|profile-rp2040-pico-control-min baker-abort-safe-demo|baker"
  "baker-recoverable-fail-safe|hibana-pico-baker-led-demo|profile-rp2040-pico-control-min baker-recoverable-abort-demo|baker"
)

RUSTUP=(rustup run "$TOOLCHAIN")
TOOLCHAIN_RUSTC="$(rustup which --toolchain "$TOOLCHAIN" rustc)"
TOOLCHAIN_BIN_DIR="$(dirname "$TOOLCHAIN_RUSTC")"
TOOLCHAIN_CARGO="$TOOLCHAIN_BIN_DIR/cargo"

rustup target add "$TARGET" --toolchain "$TOOLCHAIN" >/dev/null
rustup component add llvm-tools-preview --toolchain "$TOOLCHAIN" >/dev/null

SYSROOT="$("${RUSTUP[@]}" rustc --print sysroot)"
HOST="$("${RUSTUP[@]}" rustc -vV | sed -n 's|host: ||p')"
RUST_BIN_DIR="$SYSROOT/lib/rustlib/$HOST/bin"

if [[ -x "$RUST_BIN_DIR/llvm-size" ]]; then
  LLVM_SIZE="$RUST_BIN_DIR/llvm-size"
elif command -v llvm-size >/dev/null 2>&1; then
  LLVM_SIZE="$(command -v llvm-size)"
else
  echo "pico demo budget view requires llvm-size" >&2
  exit 1
fi

if [[ -x "$RUST_BIN_DIR/llvm-nm" ]]; then
  LLVM_NM="$RUST_BIN_DIR/llvm-nm"
elif command -v llvm-nm >/dev/null 2>&1; then
  LLVM_NM="$(command -v llvm-nm)"
else
  echo "pico demo budget view requires llvm-nm" >&2
  exit 1
fi

symbol_addr() {
  local bin="$1"
  local symbol="$2"
  local value
  value="$("$LLVM_NM" -n "$bin" | awk -v sym="$symbol" '$NF == sym { print $1; exit }')"
  if [[ -z "$value" ]]; then
    echo "missing linker symbol '$symbol' in $bin" >&2
    exit 1
  fi
  printf '%s\n' "$((16#$value))"
}

budget_status() {
  local value="$1"
  local budget="$2"
  if (( value <= budget )); then
    printf 'within'
  else
    printf 'outside'
  fi
}

report_practical_budget() {
  local label="$1"
  local value="$2"
  local budget="$3"
  if (( ENFORCE_PRACTICAL != 0 )) && (( value > budget )); then
    echo "pico demo practical contract exceeded for $label: $value > $budget" >&2
    exit 1
  fi
}

printf 'pico demo budget view\n'
printf 'practical budgets: flash<=%d static_sram<=%d kernel_stack_per_core<=%d peak_sram_upper<=%d\n' \
  "$PRACTICAL_FLASH_BUDGET" \
  "$PRACTICAL_STATIC_SRAM_BUDGET" \
  "$PRACTICAL_KERNEL_STACK_BUDGET" \
  "$PRACTICAL_PEAK_SRAM_BUDGET"
printf 'baker-led budgets: flash<=%d static_sram<=%d peak_sram_upper<=%d\n' \
  "$BAKER_FLASH_BUDGET" \
  "$BAKER_STATIC_SRAM_BUDGET" \
  "$BAKER_PEAK_SRAM_BUDGET"

for entry in "${BUDGET_ENTRIES[@]}"; do
  IFS='|' read -r label bin_name features budget_kind <<<"$entry"
  entry_target_dir="$TARGET_DIR/$label"
  PATH="$TOOLCHAIN_BIN_DIR:$PATH" \
  RUSTC="$TOOLCHAIN_RUSTC" \
    "$TOOLCHAIN_CARGO" build \
      --release \
      --target "$TARGET" \
      --target-dir "$entry_target_dir" \
      --bin "$bin_name" \
      --features "$features"

  bin="$entry_target_dir/$TARGET/release/$bin_name"
  if [[ ! -f "$bin" ]]; then
    echo "missing pico demo binary: $bin" >&2
    exit 1
  fi

  read -r text data bss _dec _hex _name < <(
    "$LLVM_SIZE" --format=berkeley "$bin" | awk 'NR==2 { print $1, $2, $3, $4, $5, $6 }'
  )

  flash_bytes=$((text + data))
  static_sram_bytes=$((data + bss))
  flash_budget="$PRACTICAL_FLASH_BUDGET"
  static_sram_budget="$PRACTICAL_STATIC_SRAM_BUDGET"
  peak_sram_budget="$PRACTICAL_PEAK_SRAM_BUDGET"
  if [[ "$budget_kind" == "baker" ]]; then
    flash_budget="$BAKER_FLASH_BUDGET"
    static_sram_budget="$BAKER_STATIC_SRAM_BUDGET"
    peak_sram_budget="$BAKER_PEAK_SRAM_BUDGET"
    symbol_addr "$bin" HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES >/dev/null
    symbol_addr "$bin" HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES >/dev/null
  fi

  stack_top_addr="$(symbol_addr "$bin" __stack_top)"
  stack_limit_addr="$(symbol_addr "$bin" __stack_limit)"
  core1_stack_top_addr="$(symbol_addr "$bin" __core1_stack_top)"

  core0_stack_reserve_bytes=$((stack_top_addr - core1_stack_top_addr))
  core1_stack_reserve_bytes=$((core1_stack_top_addr - stack_limit_addr))
  total_stack_reserve_bytes=$((stack_top_addr - stack_limit_addr))
  peak_sram_upper_bound_bytes=$((static_sram_bytes + total_stack_reserve_bytes))

  report_practical_budget "flash bytes ($label)" "$flash_bytes" "$flash_budget"
  report_practical_budget "static sram bytes ($label)" "$static_sram_bytes" "$static_sram_budget"
  report_practical_budget "core0 kernel stack reserve bytes ($label)" "$core0_stack_reserve_bytes" "$PRACTICAL_KERNEL_STACK_BUDGET"
  report_practical_budget "core1 kernel stack reserve bytes ($label)" "$core1_stack_reserve_bytes" "$PRACTICAL_KERNEL_STACK_BUDGET"
  report_practical_budget "peak sram upper-bound bytes ($label)" "$peak_sram_upper_bound_bytes" "$peak_sram_budget"

  printf '== %s (%s) ==\n' "$label" "$bin_name"
  printf 'features: %s\n' "$features"
  printf 'flash bytes: %d (%s practical budget %d)\n' \
    "$flash_bytes" "$(budget_status "$flash_bytes" "$flash_budget")" "$flash_budget"
  printf 'static sram bytes: %d (%s practical budget %d)\n' \
    "$static_sram_bytes" "$(budget_status "$static_sram_bytes" "$static_sram_budget")" "$static_sram_budget"
  printf 'kernel stack reserve bytes (core0): %d (%s practical budget %d)\n' \
    "$core0_stack_reserve_bytes" "$(budget_status "$core0_stack_reserve_bytes" "$PRACTICAL_KERNEL_STACK_BUDGET")" "$PRACTICAL_KERNEL_STACK_BUDGET"
  printf 'kernel stack reserve bytes (core1): %d (%s practical budget %d)\n' \
    "$core1_stack_reserve_bytes" "$(budget_status "$core1_stack_reserve_bytes" "$PRACTICAL_KERNEL_STACK_BUDGET")" "$PRACTICAL_KERNEL_STACK_BUDGET"
  printf 'dual-core stack reserve bytes: %d\n' "$total_stack_reserve_bytes"
  printf 'peak sram upper-bound bytes: %d (%s practical budget %d)\n' \
    "$peak_sram_upper_bound_bytes" "$(budget_status "$peak_sram_upper_bound_bytes" "$peak_sram_budget")" "$peak_sram_budget"
done
