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
expect_panic_marker="0"
expect_panic_hex_contains=""
expect_endpoint_error_prefix=""
bin_name="baker-traffic"
timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-45}"
poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-1}"

case "$pattern" in
  traffic) ;;
  choreofs-traffic)
    bin_name="baker-choreofs-traffic"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-120}"
    poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-5}"
    ;;
  choreofs-traffic-loop)
    bin_name="baker-choreofs-traffic-loop"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-120}"
    poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-5}"
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
  panic-marker)
    bin_name="baker-panic-marker"
    expected_result="48494641"
    expect_panic_marker="1"
    ;;
  endpoint-fault)
    bin_name="baker-endpoint-fault"
    expected_result="48494641"
    expect_panic_marker="1"
    expect_panic_hex_contains="456e64706f696e744572726f72"
    ;;
  endpoint-poison)
    bin_name="baker-endpoint-poison"
    expected_result="48494641"
    expect_panic_marker="1"
    expect_panic_hex_contains="456e64706f696e744572726f72"
    expect_endpoint_error_prefix="57451"
    ;;
  preview-probe)
    bin_name="baker-preview-probe"
    expected_result="48495050"
    ;;
  deadline-fault)
    bin_name="baker-deadline-fault"
    expected_result="48494641"
    expect_panic_marker="1"
    expect_panic_hex_contains="446561646c696e654578636565646564"
    ;;
  timer-route)
    bin_name="baker-timer-route"
    expected_result="48495452"
    features="${HIBANA_PICO_FEATURES-}"
    ;;
  *)
    echo "unknown Baker Link pattern: $pattern" >&2
    echo "expected: traffic, choreofs-traffic, choreofs-traffic-loop, fail-safe, recovery, many-reentry, panic-marker, endpoint-fault, endpoint-poison, preview-probe, deadline-fault, timer-route" >&2
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

read_bytes_hex() {
  local addr="$1"
  local len="$2"
  probe-rs read --chip RP2040 --non-interactive b8 "$addr" "$len" \
    | awk '
      {
        for (i = 1; i <= NF; i++) {
          token = tolower($i)
          if (token ~ /^[0-9a-f][0-9a-f]$/) {
            printf "%s", token
          }
        }
      }
    '
}

read_mmio_word() {
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
panic_file_hash_addr="$(symbol_addr HIBANA_DEMO_PANIC_FILE_HASH)"
panic_line_addr="$(symbol_addr HIBANA_DEMO_PANIC_LINE)"
panic_column_addr="$(symbol_addr HIBANA_DEMO_PANIC_COLUMN)"
panic_message_hash_addr="$(symbol_addr HIBANA_DEMO_PANIC_MESSAGE_HASH)"
panic_message_len_addr="$(symbol_addr HIBANA_DEMO_PANIC_MESSAGE_LEN)"
panic_message_total_len_addr="$(symbol_addr HIBANA_DEMO_PANIC_MESSAGE_TOTAL_LEN)"
panic_message_addr="$(symbol_addr HIBANA_DEMO_PANIC_MESSAGE)"
choreofs_engine_status_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_STATUS)"
choreofs_engine_error_code_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_ERROR_CODE)"
choreofs_path_open_count_addr="$(symbol_addr HIBANA_CHOREOFS_PATH_OPEN_COUNT)"
choreofs_fd_write_count_addr="$(symbol_addr HIBANA_CHOREOFS_FD_WRITE_COUNT)"
choreofs_poll_count_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_COUNT)"
choreofs_last_poll_ticks_lo_addr="$(symbol_addr HIBANA_CHOREOFS_LAST_POLL_TICKS_LO)"
choreofs_last_poll_ticks_hi_addr="$(symbol_addr HIBANA_CHOREOFS_LAST_POLL_TICKS_HI)"
choreofs_last_object_addr="$(symbol_addr HIBANA_CHOREOFS_LAST_OBJECT)"
choreofs_led_mask_addr="$(symbol_addr HIBANA_CHOREOFS_LED_MASK)"
choreofs_seen_led_mask_addr="$(symbol_addr HIBANA_CHOREOFS_SEEN_LED_MASK)"
choreofs_driver_trace_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_TRACE)"
choreofs_sio_trace_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE_COUNT)"
choreofs_sio_trace_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE)"

result=""
stage=""
deadline=$((SECONDS + timeout_seconds))
while :; do
  result="$(read_word "$result_addr")"
  stage="$(read_word "$stage_addr")"
  core1_stage="$(read_word "$core1_stage_addr")"
  if [[ "$expect_panic_marker" == "1" && "$result" == "$expected_result" ]]; then
    break
  fi
  if [[ "$result" == "$expected_result" && ( "$core1_stage" == "$expected_core1_stage" || ( "$allow_core1_ready" == "1" && "$core1_stage" == "4849000a" ) ) ]]; then
    break
  fi
  if [[ "$result" == "48494641" ]]; then
    break
  fi
  if (( SECONDS >= deadline )); then
    break
  fi
  sleep "$poll_seconds"
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
panic_file_hash="$(read_word "$panic_file_hash_addr")"
panic_line="$(read_word "$panic_line_addr")"
panic_column="$(read_word "$panic_column_addr")"
panic_message_hash="$(read_word "$panic_message_hash_addr")"
panic_message_len="$(read_word "$panic_message_len_addr")"
panic_message_total_len="$(read_word "$panic_message_total_len_addr")"
printf 'panic_file_hash_addr=%s hash=0x%s\n' "$panic_file_hash_addr" "$panic_file_hash"
printf 'panic_line_addr=%s line=0x%s\n' "$panic_line_addr" "$panic_line"
printf 'panic_column_addr=%s column=0x%s\n' "$panic_column_addr" "$panic_column"
printf 'panic_message_hash_addr=%s hash=0x%s\n' "$panic_message_hash_addr" "$panic_message_hash"
printf 'panic_message_len_addr=%s len=0x%s\n' "$panic_message_len_addr" "$panic_message_len"
printf 'panic_message_total_len_addr=%s total=0x%s\n' "$panic_message_total_len_addr" "$panic_message_total_len"
panic_message_len_dec="$((16#$panic_message_len))"
panic_message_hex=""
if (( panic_message_len_dec > 0 )); then
  panic_message_hex="$(read_bytes_hex "$panic_message_addr" "$panic_message_len_dec")"
  printf 'panic_message_addr=%s hex=%s\n' "$panic_message_addr" "$panic_message_hex"
fi
choreofs_engine_status="$(read_word "$choreofs_engine_status_addr")"
choreofs_engine_error_code="$(read_word "$choreofs_engine_error_code_addr")"
choreofs_path_open_count="$(read_word "$choreofs_path_open_count_addr")"
choreofs_fd_write_count="$(read_word "$choreofs_fd_write_count_addr")"
choreofs_poll_count="$(read_word "$choreofs_poll_count_addr")"
choreofs_last_poll_ticks_lo="$(read_word "$choreofs_last_poll_ticks_lo_addr")"
choreofs_last_poll_ticks_hi="$(read_word "$choreofs_last_poll_ticks_hi_addr")"
choreofs_last_object="$(read_word "$choreofs_last_object_addr")"
choreofs_led_mask="$(read_word "$choreofs_led_mask_addr")"
choreofs_seen_led_mask="$(read_word "$choreofs_seen_led_mask_addr")"
choreofs_driver_trace="$(read_word "$choreofs_driver_trace_addr")"
choreofs_sio_trace_count="$(read_word "$choreofs_sio_trace_count_addr")"
watchdog_tick="$(read_mmio_word 0x4005802c)"
clk_ref_ctrl="$(read_mmio_word 0x40008030)"
clk_ref_selected="$(read_mmio_word 0x40008038)"
xosc_status="$(read_mmio_word 0x40024004)"
printf 'choreofs_engine_status_addr=%s status=0x%s\n' "$choreofs_engine_status_addr" "$choreofs_engine_status"
printf 'choreofs_engine_error_code_addr=%s code=0x%s\n' "$choreofs_engine_error_code_addr" "$choreofs_engine_error_code"
printf 'choreofs_path_open_count_addr=%s count=0x%s\n' "$choreofs_path_open_count_addr" "$choreofs_path_open_count"
printf 'choreofs_fd_write_count_addr=%s count=0x%s\n' "$choreofs_fd_write_count_addr" "$choreofs_fd_write_count"
printf 'choreofs_poll_count_addr=%s count=0x%s\n' "$choreofs_poll_count_addr" "$choreofs_poll_count"
printf 'choreofs_last_poll_ticks_lo_addr=%s lo=0x%s\n' "$choreofs_last_poll_ticks_lo_addr" "$choreofs_last_poll_ticks_lo"
printf 'choreofs_last_poll_ticks_hi_addr=%s hi=0x%s\n' "$choreofs_last_poll_ticks_hi_addr" "$choreofs_last_poll_ticks_hi"
printf 'choreofs_last_object_addr=%s object=0x%s\n' "$choreofs_last_object_addr" "$choreofs_last_object"
printf 'choreofs_led_mask_addr=%s mask=0x%s\n' "$choreofs_led_mask_addr" "$choreofs_led_mask"
printf 'choreofs_seen_led_mask_addr=%s mask=0x%s\n' "$choreofs_seen_led_mask_addr" "$choreofs_seen_led_mask"
printf 'choreofs_driver_trace_addr=%s trace=0x%s\n' "$choreofs_driver_trace_addr" "$choreofs_driver_trace"
printf 'choreofs_sio_trace_count_addr=%s count=0x%s\n' "$choreofs_sio_trace_count_addr" "$choreofs_sio_trace_count"
printf 'baker_clock_watchdog_tick=0x%s\n' "$watchdog_tick"
printf 'baker_clock_clk_ref_ctrl=0x%s\n' "$clk_ref_ctrl"
printf 'baker_clock_clk_ref_selected=0x%s\n' "$clk_ref_selected"
printf 'baker_clock_xosc_status=0x%s\n' "$xosc_status"
trace_count_dec="$((16#$choreofs_sio_trace_count))"
if (( trace_count_dec > 8 )); then
  trace_count_dec=8
fi
trace_idx=0
while (( trace_idx < trace_count_dec )); do
  trace_addr="$(printf '0x%x' "$((choreofs_sio_trace_addr + trace_idx * 4))")"
  trace_word="$(read_word "$trace_addr")"
  printf 'choreofs_sio_trace[%d]_addr=%s value=0x%s\n' "$trace_idx" "$trace_addr" "$trace_word"
  trace_idx=$((trace_idx + 1))
done

if [[ "$result" != "$expected_result" ]]; then
  echo "Baker hardware pattern $pattern failed: result mismatch" >&2
  exit 1
fi

if [[ "$expect_panic_marker" == "1" ]]; then
  if [[ "$hardfault_pc" != "00000000" || "$hardfault_lr" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: hardfault marker was set" >&2
    exit 1
  fi
  if [[ "$stage" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic failure stage was not set" >&2
    exit 1
  fi
  if [[ "$panic_file_hash" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic file hash was not recorded" >&2
    exit 1
  fi
  if [[ "$panic_line" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic line was not recorded" >&2
    exit 1
  fi
  if [[ "$panic_column" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic column was not recorded" >&2
    exit 1
  fi
  if [[ "$panic_message_hash" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic message hash was not recorded" >&2
    exit 1
  fi
  if [[ "$panic_message_len" == "00000000" || "$panic_message_total_len" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: panic message bytes were not recorded" >&2
    exit 1
  fi
  if [[ -n "$expect_panic_hex_contains" && "$panic_message_hex" != *"$expect_panic_hex_contains"* ]]; then
    echo "Baker hardware pattern $pattern failed: panic message did not include expected evidence" >&2
    echo "expected hex substring: $expect_panic_hex_contains" >&2
    exit 1
  fi
  echo "Baker hardware pattern $pattern ok"
  exit 0
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

watchdog_tick_dec="$((16#$watchdog_tick))"
clk_ref_selected_dec="$((16#$clk_ref_selected))"
xosc_status_dec="$((16#$xosc_status))"
if (( (watchdog_tick_dec & 0x200) == 0 || (watchdog_tick_dec & 0x1ff) != 12 )); then
  echo "Baker hardware pattern $pattern failed: watchdog tick is not 1MHz from 12MHz XOSC" >&2
  exit 1
fi
if (( (clk_ref_selected_dec & 0x4) == 0 )); then
  echo "Baker hardware pattern $pattern failed: clk_ref did not select XOSC" >&2
  exit 1
fi
if (( (xosc_status_dec & 0x80000000) == 0 )); then
  echo "Baker hardware pattern $pattern failed: XOSC is not stable" >&2
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
  if [[ "$choreofs_engine_error_code" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: ChoreoFS error marker was set" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000003" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_fd_write_count" != "00000027" ]]; then
    echo "Baker hardware pattern $pattern failed: fd_write count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_poll_count" != "00000027" ]]; then
    echo "Baker hardware pattern $pattern failed: poll_oneoff count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_last_object" != "00000003" ]]; then
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
  if [[ "$choreofs_engine_error_code" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: ChoreoFS error marker was set" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000003" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  choreofs_fd_write_count_dec="$((16#$choreofs_fd_write_count))"
  choreofs_poll_count_dec="$((16#$choreofs_poll_count))"
  if (( choreofs_fd_write_count_dec < 13 )); then
    echo "Baker hardware pattern $pattern failed: fd_write count did not reach one visual cycle" >&2
    exit 1
  fi
  if (( choreofs_poll_count_dec < 13 )); then
    echo "Baker hardware pattern $pattern failed: poll_oneoff count did not reach one visual cycle" >&2
    exit 1
  fi
  if [[ "$choreofs_seen_led_mask" != "00000007" ]]; then
    echo "Baker hardware pattern $pattern failed: LED cycle evidence mismatch" >&2
    exit 1
  fi
fi

if [[ -n "$expect_endpoint_error_prefix" && "$choreofs_engine_error_code" != "$expect_endpoint_error_prefix"* ]]; then
  echo "Baker hardware pattern $pattern failed: endpoint preview error evidence mismatch" >&2
  echo "expected prefix: 0x$expect_endpoint_error_prefix" >&2
  exit 1
fi

echo "Baker hardware pattern $pattern ok"
