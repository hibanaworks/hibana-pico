#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

target="${HIBANA_PICO_TARGET:-thumbv6m-none-eabi}"
package_name="baker-firmware"
baker_features="wasm-engine-core embed-wasip1-artifacts"
bins="baker-traffic baker-choreofs-traffic baker-choreofs-traffic-loop baker-fail-safe baker-recovery baker-many-reentry baker-panic-marker baker-endpoint-fault baker-endpoint-poison baker-preview-probe baker-deadline-fault baker-timer-route baker-session-mismatch baker-capacity-fault baker-epf-policy-timer"

sysroot="$(rustc --print sysroot)"
host="$(rustc -vV | sed -n 's/^host: //p')"
llvm_size="$sysroot/lib/rustlib/$host/bin/llvm-size"
if [[ ! -x "$llvm_size" ]]; then
  echo "missing llvm-size at $llvm_size" >&2
  exit 1
fi

budget_for_bin() {
  case "$1" in
    baker-choreofs-traffic | baker-choreofs-traffic-loop | baker-session-mismatch)
      echo "1000000 260000 4096 240000 1000000"
      ;;
    baker-panic-marker)
      echo "16000 4096 1024 2304 16000"
      ;;
    baker-many-reentry | baker-timer-route | baker-epf-policy-timer)
      echo "835000 180000 4096 160000 835000"
      ;;
    baker-endpoint-fault | baker-endpoint-poison | baker-deadline-fault | baker-capacity-fault)
      echo "820000 180000 4096 160000 820000"
      ;;
    baker-traffic | baker-fail-safe | baker-recovery | baker-preview-probe)
      echo "825000 180000 4096 160000 825000"
      ;;
    *)
      echo "missing section budget for $1" >&2
      exit 1
      ;;
  esac
}

section_size() {
  local elf="$1"
  local section="$2"
  "$llvm_size" -A "$elf" | awk -v section="$section" '
    $1 == section { size = $2; found = 1 }
    END {
      if (found) {
        print size + 0
      } else {
        print 0
      }
    }
  '
}

check_budget() {
  local bin="$1"
  local name="$2"
  local value="$3"
  local max="$4"
  if (( value > max )); then
    echo "section budget failed: $bin $name=$value exceeds $max" >&2
    exit 1
  fi
}

for bin in $bins; do
  if [[ "$bin" == "baker-timer-route" || "$bin" == "baker-capacity-fault" || "$bin" == "baker-epf-policy-timer" ]]; then
    cargo build --quiet --target "$target" --release -p "$package_name" --bin "$bin"
  else
    cargo build --quiet --target "$target" --release -p "$package_name" --bin "$bin" --features "$baker_features"
  fi

  elf="target/$target/release/$bin"
  read -r text_max rodata_max data_max bss_max flash_max <<<"$(budget_for_bin "$bin")"
  text_size="$(section_size "$elf" ".text")"
  rodata_size="$(section_size "$elf" ".rodata")"
  data_size="$(section_size "$elf" ".data")"
  bss_size="$(section_size "$elf" ".bss")"
  flash_size=$((text_size + rodata_size + data_size))

  check_budget "$bin" ".text" "$text_size" "$text_max"
  check_budget "$bin" ".rodata" "$rodata_size" "$rodata_max"
  check_budget "$bin" ".data" "$data_size" "$data_max"
  check_budget "$bin" ".bss" "$bss_size" "$bss_max"
  check_budget "$bin" "flash(.text+.rodata+.data)" "$flash_size" "$flash_max"

  printf 'section-budget bin=%s text=%s/%s rodata=%s/%s data=%s/%s bss=%s/%s flash=%s/%s\n' \
    "$bin" \
    "$text_size" "$text_max" \
    "$rodata_size" "$rodata_max" \
    "$data_size" "$data_max" \
    "$bss_size" "$bss_max" \
    "$flash_size" "$flash_max"
done

echo "baker section budgets ok"
