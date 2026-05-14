#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

target="${HIBANA_PICO_TARGET:-thumbv6m-none-eabi}"
package_name="baker-firmware"
pattern="${1:-${HIBANA_BAKER_PATTERN:-traffic}}"
features="${HIBANA_PICO_FEATURES:-wasm-engine-core wasip1-sys-args-env wasip1-sys-fd-write wasip1-sys-path-open wasip1-sys-poll-oneoff wasip1-sys-proc-exit embed-wasip1-artifacts}"
expected_result="48494f4b"
expected_core1_stage="48490004"
allow_core1_ready="1"
bin_name="baker-traffic"

case "$pattern" in
  traffic) ;;
  choreofs-traffic)
    bin_name="baker-choreofs-traffic"
    expected_core1_stage="4849000a"
    allow_core1_ready="0"
    ;;
  choreofs-traffic-loop)
    bin_name="baker-choreofs-traffic-loop"
    expected_core1_stage="4849000a"
    allow_core1_ready="0"
    ;;
  fail-safe)
    bin_name="baker-fail-safe"
    expected_result="48494653"
    ;;
  recovery)
    bin_name="baker-recovery"
    expected_result="48495243"
    ;;
  many-reentry)
    bin_name="baker-many-reentry"
    expected_result="4849524d"
    ;;
  *)
    echo "unknown Baker Link pattern: $pattern" >&2
    echo "expected: traffic, choreofs-traffic, choreofs-traffic-loop, fail-safe, recovery, many-reentry" >&2
    exit 2
    ;;
esac

bash ./scripts/check_wasip1_guest_builds.sh
cargo build \
  --target "$target" \
  --release \
  -p "$package_name" \
  --bin "$bin_name" \
  --features "$features"

elf="target/$target/release/$bin_name"

probe-rs download \
  --chip RP2040 \
  --non-interactive \
  --verify \
  --disable-progressbars \
  "$elf"
probe-rs reset --chip RP2040 --non-interactive

sysroot="$(rustc --print sysroot)"
host="$(rustc -vV | sed -n 's/^host: //p')"
llvm_nm="$sysroot/lib/rustlib/$host/bin/llvm-nm"
if [[ ! -x "$llvm_nm" ]]; then
  echo "missing llvm-nm at $llvm_nm" >&2
  exit 1
fi

symbol_addr() {
  local symbol="$1"
  local value
  value="$("$llvm_nm" -n "$elf" | awk -v sym="$symbol" '$NF == sym { print $1; exit }')"
  if [[ -z "$value" ]]; then
    echo "missing symbol $symbol in $elf" >&2
    exit 1
  fi
  printf '0x%s\n' "$value"
}

read_word() {
  local addr="$1"
  probe-rs read --chip RP2040 --non-interactive b32 "$addr" 1 \
    | awk 'NF { value=$NF } END { print tolower(value) }'
}

result_addr="$(symbol_addr HIBANA_DEMO_RESULT)"
stage_addr="$(symbol_addr HIBANA_DEMO_FAILURE_STAGE)"
hardfault_pc_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_PC)"
hardfault_lr_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_LR)"
core0_stack_addr="$(symbol_addr HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES)"
core1_stack_addr="$(symbol_addr HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES)"
core0_stage_addr="$(symbol_addr HIBANA_DEMO_CORE0_STAGE)"
core1_stage_addr="$(symbol_addr HIBANA_DEMO_CORE1_STAGE)"
choreofs_engine_status_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_STATUS)"
choreofs_engine_error_code_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_ERROR_CODE)"
choreofs_path_open_count_addr="$(symbol_addr HIBANA_CHOREOFS_PATH_OPEN_COUNT)"
choreofs_fd_write_count_addr="$(symbol_addr HIBANA_CHOREOFS_FD_WRITE_COUNT)"
choreofs_poll_count_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_COUNT)"
choreofs_last_object_addr="$(symbol_addr HIBANA_CHOREOFS_LAST_OBJECT)"
choreofs_led_mask_addr="$(symbol_addr HIBANA_CHOREOFS_LED_MASK)"
choreofs_seen_led_mask_addr="$(symbol_addr HIBANA_CHOREOFS_SEEN_LED_MASK)"

result=""
stage=""
deadline=$((SECONDS + ${HIBANA_BAKER_TIMEOUT_SECONDS:-45}))
while :; do
  result="$(read_word "$result_addr")"
  stage="$(read_word "$stage_addr")"
  core1_stage="$(read_word "$core1_stage_addr")"
  if [[ "$result" == "$expected_result" && "$core1_stage" == "$expected_core1_stage" ]]; then
    break
  fi
  if [[ "$result" == "48494641" ]]; then
    break
  fi
  if (( SECONDS >= deadline )); then
    break
  fi
  sleep "${HIBANA_BAKER_POLL_SECONDS:-1}"
done

printf 'pattern=%s\n' "$pattern"
printf 'bin=%s\n' "$bin_name"
printf 'features=%s\n' "$features"
printf 'result_addr=%s result=0x%s expected=0x%s\n' "$result_addr" "$result" "$expected_result"
printf 'stage_addr=%s stage=0x%s\n' "$stage_addr" "$stage"
hardfault_pc="$(read_word "$hardfault_pc_addr")"
hardfault_lr="$(read_word "$hardfault_lr_addr")"
printf 'hardfault_pc_addr=%s pc=0x%s\n' "$hardfault_pc_addr" "$hardfault_pc"
printf 'hardfault_lr_addr=%s lr=0x%s\n' "$hardfault_lr_addr" "$hardfault_lr"
core0_stack="$(read_word "$core0_stack_addr")"
core1_stack="$(read_word "$core1_stack_addr")"
printf 'core0_stack_high_water_addr=%s used=0x%s\n' "$core0_stack_addr" "$core0_stack"
printf 'core1_stack_high_water_addr=%s used=0x%s\n' "$core1_stack_addr" "$core1_stack"
core0_stage="$(read_word "$core0_stage_addr")"
core1_stage="$(read_word "$core1_stage_addr")"
printf 'core0_stage_addr=%s stage=0x%s\n' "$core0_stage_addr" "$core0_stage"
printf 'core1_stage_addr=%s stage=0x%s\n' "$core1_stage_addr" "$core1_stage"
choreofs_engine_status="$(read_word "$choreofs_engine_status_addr")"
choreofs_engine_error_code="$(read_word "$choreofs_engine_error_code_addr")"
choreofs_path_open_count="$(read_word "$choreofs_path_open_count_addr")"
choreofs_fd_write_count="$(read_word "$choreofs_fd_write_count_addr")"
choreofs_poll_count="$(read_word "$choreofs_poll_count_addr")"
choreofs_last_object="$(read_word "$choreofs_last_object_addr")"
choreofs_led_mask="$(read_word "$choreofs_led_mask_addr")"
choreofs_seen_led_mask="$(read_word "$choreofs_seen_led_mask_addr")"
printf 'choreofs_engine_status_addr=%s status=0x%s\n' "$choreofs_engine_status_addr" "$choreofs_engine_status"
printf 'choreofs_engine_error_code_addr=%s code=0x%s\n' "$choreofs_engine_error_code_addr" "$choreofs_engine_error_code"
printf 'choreofs_path_open_count_addr=%s count=0x%s\n' "$choreofs_path_open_count_addr" "$choreofs_path_open_count"
printf 'choreofs_fd_write_count_addr=%s count=0x%s\n' "$choreofs_fd_write_count_addr" "$choreofs_fd_write_count"
printf 'choreofs_poll_count_addr=%s count=0x%s\n' "$choreofs_poll_count_addr" "$choreofs_poll_count"
printf 'choreofs_last_object_addr=%s object=0x%s\n' "$choreofs_last_object_addr" "$choreofs_last_object"
printf 'choreofs_led_mask_addr=%s mask=0x%s\n' "$choreofs_led_mask_addr" "$choreofs_led_mask"
printf 'choreofs_seen_led_mask_addr=%s mask=0x%s\n' "$choreofs_seen_led_mask_addr" "$choreofs_seen_led_mask"

if [[ "$result" != "$expected_result" ]]; then
  echo "Baker hardware pattern $pattern failed: result mismatch" >&2
  exit 1
fi
if [[ "$stage" != "00000000" ]]; then
  echo "Baker hardware pattern $pattern failed: failure stage was set" >&2
  exit 1
fi
if [[ "$core0_stage" != "4849000a" ]]; then
  echo "Baker hardware pattern $pattern failed: core0 did not reach runtime-ready marker" >&2
  exit 1
fi
if [[ "$core1_stage" != "$expected_core1_stage" && ! ( "$allow_core1_ready" == "1" && "$core1_stage" == "4849000a" ) ]]; then
  echo "Baker hardware pattern $pattern failed: core1 stage mismatch" >&2
  echo "expected core1 stage: 0x$expected_core1_stage" >&2
  exit 1
fi
if [[ "$hardfault_pc" != "00000000" || "$hardfault_lr" != "00000000" ]]; then
  echo "Baker hardware pattern $pattern failed: hardfault marker was set" >&2
  exit 1
fi

stack_budget_dec="$((8 * 1024))"
core0_stack_dec="$((16#$core0_stack))"
core1_stack_dec="$((16#$core1_stack))"
if (( core0_stack_dec == 0 || core0_stack_dec > stack_budget_dec )); then
  echo "Baker hardware pattern $pattern failed: core0 stack high-water invalid: $core0_stack_dec" >&2
  exit 1
fi
if (( core1_stack_dec == 0 || core1_stack_dec > stack_budget_dec )); then
  echo "Baker hardware pattern $pattern failed: core1 stack high-water invalid: $core1_stack_dec" >&2
  exit 1
fi

if [[ "$pattern" == "choreofs-traffic" ]]; then
  if [[ "$choreofs_engine_status" != "57414f4b" ]]; then
    echo "Baker hardware pattern $pattern failed: WASI engine did not complete through endpoint/carrier" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_fd_write_count" != "00000009" ]]; then
    echo "Baker hardware pattern $pattern failed: fd_write count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_poll_count" != "00000009" ]]; then
    echo "Baker hardware pattern $pattern failed: poll_oneoff count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_last_object" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: final ChoreoFS object mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_led_mask" != "00000004" ]]; then
    echo "Baker hardware pattern $pattern failed: final LED mask mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_seen_led_mask" != "00000007" ]]; then
    echo "Baker hardware pattern $pattern failed: LED cycle evidence mismatch" >&2
    exit 1
  fi
fi

if [[ "$pattern" == "choreofs-traffic-loop" ]]; then
  if [[ "$choreofs_engine_status" == "00000000" || "$choreofs_engine_status" == "57414641" ]]; then
    echo "Baker hardware pattern $pattern failed: WASI engine did not enter visual loop" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  choreofs_fd_write_count_dec="$((16#$choreofs_fd_write_count))"
  choreofs_poll_count_dec="$((16#$choreofs_poll_count))"
  if (( choreofs_fd_write_count_dec < 3 )); then
    echo "Baker hardware pattern $pattern failed: fd_write count did not reach one visual cycle" >&2
    exit 1
  fi
  if (( choreofs_poll_count_dec < 3 )); then
    echo "Baker hardware pattern $pattern failed: poll_oneoff count did not reach one visual cycle" >&2
    exit 1
  fi
  if [[ "$choreofs_last_object" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: final ChoreoFS object mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_seen_led_mask" != "00000007" ]]; then
    echo "Baker hardware pattern $pattern failed: LED cycle evidence mismatch" >&2
    exit 1
  fi
fi

echo "Baker hardware pattern $pattern ok"
