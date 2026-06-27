#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

target="${HIBANA_PICO_TARGET:-thumbv6m-none-eabi}"
chip="${HIBANA_BAKER_CHIP:-RP2040}"
package_name="baker-firmware"
pattern="${1:-${HIBANA_BAKER_PATTERN:-traffic}}"
features="${HIBANA_PICO_FEATURES:-wasm-engine-core embed-wasip1-artifacts}"
expected_result="48494f4b"
expected_core1_stage="48490004"
allow_core1_ready="1"
runtime_ready_core="core0"
expect_panic_marker="0"
expect_panic_hex_contains=""
expect_endpoint_error_prefix=""
skip_result_check="0"
bin_name="baker-traffic"
timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-45}"
poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-1}"
initial_poll_delay_seconds="${HIBANA_BAKER_INITIAL_POLL_DELAY_SECONDS:-0}"

case "$pattern" in
  traffic) ;;
  choreofs-traffic)
    bin_name="baker-choreofs-traffic"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-120}"
    poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-5}"
    initial_poll_delay_seconds="${HIBANA_BAKER_INITIAL_POLL_DELAY_SECONDS:-3}"
    ;;
  choreofs-traffic-loop)
    bin_name="baker-choreofs-traffic-loop"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    runtime_ready_core="core0"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-120}"
    poll_seconds="${HIBANA_BAKER_POLL_SECONDS:-5}"
    initial_poll_delay_seconds="${HIBANA_BAKER_INITIAL_POLL_DELAY_SECONDS:-3}"
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
    expect_panic_hex_contains="5472616e73706f7274284429"
    ;;
  timer-route)
    bin_name="baker-timer-route"
    expected_core1_stage="4849000a"
    allow_core1_ready="0"
    expected_result="48495452"
    features="${HIBANA_PICO_FEATURES-}"
    ;;
  epf-policy-timer)
    bin_name="baker-epf-policy-timer"
    expected_core1_stage="4849000a"
    allow_core1_ready="1"
    expected_result="48494550"
    target="${HIBANA_PICO_TARGET:-thumbv6m-none-eabi}"
    chip="${HIBANA_BAKER_CHIP:-RP2040}"
    features="${HIBANA_PICO_FEATURES-}"
    ;;
  session-mismatch)
    bin_name="baker-session-mismatch"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    expected_result="00000000"
    skip_result_check="1"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-12}"
    features="${HIBANA_PICO_FEATURES:-wasm-engine-core embed-wasip1-artifacts}"
    ;;
  capacity-fault)
    bin_name="baker-capacity-fault"
    expected_core1_stage="48490004"
    allow_core1_ready="0"
    expected_result="00000000"
    skip_result_check="1"
    timeout_seconds="${HIBANA_BAKER_TIMEOUT_SECONDS:-12}"
    features="${HIBANA_PICO_FEATURES-}"
    ;;
  *)
    echo "unknown Baker Link pattern: $pattern" >&2
    echo "expected: traffic, choreofs-traffic, choreofs-traffic-loop, fail-safe, recovery, many-reentry, panic-marker, endpoint-fault, endpoint-poison, preview-probe, deadline-fault, timer-route, epf-policy-timer, session-mismatch, capacity-fault" >&2
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

probe_args=()
if [[ -n "${PROBE_RS_PROBE:-}" ]]; then
  probe_args=(--probe "$PROBE_RS_PROBE")
fi

probe-rs download \
  "${probe_args[@]}" \
  --chip "$chip" \
  --non-interactive \
  --verify \
  --disable-progressbars \
  "$elf"
probe-rs reset "${probe_args[@]}" --chip "$chip" --non-interactive

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

symbol_addr_or_empty() {
  local symbol="$1"
  local value
  value="$("$llvm_nm" -n "$elf" | awk -v sym="$symbol" '$NF == sym { print $1; exit }')"
  if [[ -z "$value" ]]; then
    printf '\n'
    return 0
  fi
  printf '0x%s\n' "$value"
}

probe_read() {
  local width="$1"
  local addr="$2"
  local len="$3"
  local attempt=0
  local output
  while :; do
    if output="$(probe-rs read "${probe_args[@]}" --chip "$chip" --non-interactive "$width" "$addr" "$len" 2>&1)"; then
      printf '%s\n' "$output"
      return 0
    fi
    attempt=$((attempt + 1))
    if (( attempt >= 5 )); then
      printf '%s\n' "$output" >&2
      return 1
    fi
    sleep 0.25
  done
}

read_word() {
  local addr="$1"
  probe_read b32 "$addr" 1 | awk 'NF { value=$NF } END { print tolower(value) }'
}

read_word_or_zero() {
  local addr="$1"
  if [[ -z "$addr" ]]; then
    printf '00000000\n'
    return 0
  fi
  read_word "$addr"
}

probe_write() {
  local width="$1"
  local addr="$2"
  shift 2
  probe-rs write \
    "${probe_args[@]}" \
    --chip "$chip" \
    --non-interactive \
    "$width" \
    "$addr" \
    "$@" >/dev/null
}

read_bytes_hex() {
  local addr="$1"
  local len="$2"
  probe_read b8 "$addr" "$len" \
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
  probe_read b32 "$addr" 1 | awk 'NF { value=$NF } END { print tolower(value) }'
}

result_addr="$(symbol_addr HIBANA_DEMO_RESULT)"
stage_addr="$(symbol_addr HIBANA_DEMO_FAILURE_STAGE)"
hardfault_pc_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_PC)"
hardfault_lr_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_LR)"
hardfault_r0_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_R0)"
hardfault_r1_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_R1)"
hardfault_r2_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_R2)"
hardfault_r3_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_R3)"
hardfault_r12_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_R12)"
hardfault_sp_addr="$(symbol_addr HIBANA_DEMO_HARDFAULT_SP)"
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
choreofs_engine_gap_count_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_GAP_COUNT)"
choreofs_engine_gap_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_GAP_LAST_US)"
choreofs_engine_gap_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_GAP_TOTAL_US)"
choreofs_engine_gap_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_ENGINE_GAP_MAX_US)"
choreofs_driver_import_count_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_IMPORT_COUNT)"
choreofs_driver_import_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_IMPORT_LAST_US)"
choreofs_driver_import_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_IMPORT_TOTAL_US)"
choreofs_driver_import_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_IMPORT_MAX_US)"
choreofs_poll_delay_count_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_DELAY_COUNT)"
choreofs_poll_delay_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_DELAY_LAST_US)"
choreofs_poll_delay_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_DELAY_TOTAL_US)"
choreofs_poll_delay_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_POLL_DELAY_MAX_US)"
choreofs_request_recv_count_addr="$(symbol_addr HIBANA_CHOREOFS_REQUEST_RECV_COUNT)"
choreofs_request_recv_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_REQUEST_RECV_LAST_US)"
choreofs_request_recv_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_REQUEST_RECV_TOTAL_US)"
choreofs_request_recv_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_REQUEST_RECV_MAX_US)"
choreofs_reply_send_count_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_COUNT)"
choreofs_reply_send_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_LAST_US)"
choreofs_reply_send_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_TOTAL_US)"
choreofs_reply_send_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_MAX_US)"
choreofs_reply_send_fd_write_object_count_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_FD_WRITE_OBJECT_COUNT)"
choreofs_reply_send_fd_write_object_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_FD_WRITE_OBJECT_TOTAL_US)"
choreofs_reply_send_poll_oneoff_count_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_POLL_ONEOFF_COUNT)"
choreofs_reply_send_poll_oneoff_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_REPLY_SEND_POLL_ONEOFF_TOTAL_US)"
choreofs_reply_send_future_poll_count_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_COUNT)"
choreofs_reply_send_future_poll_last_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_LAST_US)"
choreofs_reply_send_future_poll_total_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_TOTAL_US)"
choreofs_reply_send_future_poll_max_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_MAX_US)"
choreofs_reply_encode_count_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_ENCODE_COUNT)"
choreofs_reply_encode_last_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_ENCODE_LAST_US)"
choreofs_reply_encode_total_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_ENCODE_TOTAL_US)"
choreofs_reply_encode_max_us_addr="$(symbol_addr_or_empty HIBANA_CHOREOFS_REPLY_ENCODE_MAX_US)"
choreofs_measurements_frozen_addr="$(symbol_addr HIBANA_CHOREOFS_MEASUREMENTS_FROZEN)"
choreofs_driver_trace_addr="$(symbol_addr HIBANA_CHOREOFS_DRIVER_TRACE)"
choreofs_sio_trace_core0_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE_CORE0_COUNT)"
choreofs_sio_trace_core0_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE_CORE0)"
choreofs_sio_trace_core1_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE_CORE1_COUNT)"
choreofs_sio_trace_core1_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TRACE_CORE1)"
choreofs_sio_core0_to_core1_tx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_TX_COUNT)"
choreofs_sio_core0_to_core1_rx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_RX_COUNT)"
choreofs_sio_core1_to_core0_tx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_TX_COUNT)"
choreofs_sio_core1_to_core0_rx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_RX_COUNT)"
choreofs_sio_role1_pending_seen_core0_tx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_ROLE1_PENDING_SEEN_CORE0_TX)"
choreofs_sio_role1_poll_seen_core0_tx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_ROLE1_POLL_SEEN_CORE0_TX)"
choreofs_sio_role1_ready_seen_core0_tx_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_ROLE1_READY_SEEN_CORE0_TX)"
choreofs_sio_rx_wait_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_COUNT)"
choreofs_sio_rx_wait_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_LAST_US)"
choreofs_sio_rx_wait_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_TOTAL_US)"
choreofs_sio_rx_wait_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_MAX_US)"
choreofs_sio_rx_wait_role0_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE0_COUNT)"
choreofs_sio_rx_wait_role0_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE0_LAST_US)"
choreofs_sio_rx_wait_role0_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE0_TOTAL_US)"
choreofs_sio_rx_wait_role0_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE0_MAX_US)"
choreofs_sio_rx_wait_role1_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE1_COUNT)"
choreofs_sio_rx_wait_role1_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE1_LAST_US)"
choreofs_sio_rx_wait_role1_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE1_TOTAL_US)"
choreofs_sio_rx_wait_role1_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE1_MAX_US)"
choreofs_sio_tx_wait_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_COUNT)"
choreofs_sio_tx_wait_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_LAST_US)"
choreofs_sio_tx_wait_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_TOTAL_US)"
choreofs_sio_tx_wait_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_MAX_US)"
choreofs_sio_tx_wait_role0_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE0_COUNT)"
choreofs_sio_tx_wait_role0_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE0_LAST_US)"
choreofs_sio_tx_wait_role0_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE0_TOTAL_US)"
choreofs_sio_tx_wait_role0_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE0_MAX_US)"
choreofs_sio_tx_wait_role1_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE1_COUNT)"
choreofs_sio_tx_wait_role1_last_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE1_LAST_US)"
choreofs_sio_tx_wait_role1_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE1_TOTAL_US)"
choreofs_sio_tx_wait_role1_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE1_MAX_US)"
choreofs_sio_tx_poll_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_COUNT)"
choreofs_sio_tx_poll_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_TOTAL_US)"
choreofs_sio_tx_poll_max_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_MAX_US)"
choreofs_sio_tx_poll_role0_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_ROLE0_COUNT)"
choreofs_sio_tx_poll_role0_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_ROLE0_TOTAL_US)"
choreofs_sio_tx_poll_role1_count_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_ROLE1_COUNT)"
choreofs_sio_tx_poll_role1_total_us_addr="$(symbol_addr HIBANA_CHOREOFS_SIO_TX_POLL_ROLE1_TOTAL_US)"
appkit_wasi_resume_count_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_RESUME_COUNT)"
appkit_wasi_resume_last_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_RESUME_LAST_US)"
appkit_wasi_resume_total_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_RESUME_TOTAL_US)"
appkit_wasi_resume_max_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_RESUME_MAX_US)"
appkit_wasi_request_send_count_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_REQUEST_SEND_COUNT)"
appkit_wasi_request_send_last_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_REQUEST_SEND_LAST_US)"
appkit_wasi_request_send_total_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_REQUEST_SEND_TOTAL_US)"
appkit_wasi_request_send_max_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_REQUEST_SEND_MAX_US)"
appkit_wasi_completion_recv_count_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETION_RECV_COUNT)"
appkit_wasi_completion_recv_last_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETION_RECV_LAST_US)"
appkit_wasi_completion_recv_total_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETION_RECV_TOTAL_US)"
appkit_wasi_completion_recv_max_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETION_RECV_MAX_US)"
appkit_wasi_complete_count_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETE_COUNT)"
appkit_wasi_complete_last_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETE_LAST_US)"
appkit_wasi_complete_total_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETE_TOTAL_US)"
appkit_wasi_complete_max_us_addr="$(symbol_addr_or_empty HIBANA_APPKIT_WASI_COMPLETE_MAX_US)"
epf_core0_epoch_addr="$(symbol_addr HIBANA_EPF_CORE0_EPOCH)"
epf_core0_kind_addr="$(symbol_addr HIBANA_EPF_CORE0_KIND)"
epf_core0_reason_addr="$(symbol_addr HIBANA_EPF_CORE0_REASON)"
epf_core0_arg0_addr="$(symbol_addr HIBANA_EPF_CORE0_ARG0)"
epf_core0_arg1_addr="$(symbol_addr HIBANA_EPF_CORE0_ARG1)"
epf_core0_arg2_addr="$(symbol_addr HIBANA_EPF_CORE0_ARG2)"
epf_core0_fuel_addr="$(symbol_addr HIBANA_EPF_CORE0_FUEL_USED)"
epf_core1_epoch_addr="$(symbol_addr HIBANA_EPF_CORE1_EPOCH)"
epf_core1_kind_addr="$(symbol_addr HIBANA_EPF_CORE1_KIND)"
epf_core1_reason_addr="$(symbol_addr HIBANA_EPF_CORE1_REASON)"
epf_core1_arg0_addr="$(symbol_addr HIBANA_EPF_CORE1_ARG0)"
epf_core1_arg1_addr="$(symbol_addr HIBANA_EPF_CORE1_ARG1)"
epf_core1_arg2_addr="$(symbol_addr HIBANA_EPF_CORE1_ARG2)"
epf_core1_fuel_addr="$(symbol_addr HIBANA_EPF_CORE1_FUEL_USED)"
epf_load_epoch_addr="$(symbol_addr HIBANA_EPF_LOAD_EPOCH)"
epf_image_digest_addr="$(symbol_addr HIBANA_EPF_IMAGE_DIGEST)"
epf_load_reason_addr="$(symbol_addr HIBANA_EPF_LOAD_REASON)"
epf_active_target_kind_addr="$(symbol_addr HIBANA_EPF_ACTIVE_TARGET_KIND)"
epf_active_policy_id_addr="$(symbol_addr HIBANA_EPF_ACTIVE_POLICY_ID)"
epf_policy_timer_irq_ready_addr="$(symbol_addr HIBANA_EPF_POLICY_TIMER_IRQ_READY)"
epf_policy_timer_fact_kind_addr="$(symbol_addr HIBANA_EPF_POLICY_TIMER_FACT_KIND)"
epf_policy_timer_fact_arg0_addr="$(symbol_addr HIBANA_EPF_POLICY_TIMER_FACT_ARG0)"
epf_policy_timer_fact_fuel_addr="$(symbol_addr HIBANA_EPF_POLICY_TIMER_FACT_FUEL)"
epf_image_ingress_addr="$(symbol_addr HIBANA_EPF_IMAGE_INGRESS)"
epf_bytecode_state_addr="$(symbol_addr HIBANA_EPF_BYTECODE_STATE)"
epf_bytecode_len_addr="$(symbol_addr HIBANA_EPF_BYTECODE_LEN)"
epf_bytecode_hash_addr="$(symbol_addr HIBANA_EPF_BYTECODE_HASH)"
epf_bytecode_image_addr="$(symbol_addr HIBANA_EPF_BYTECODE_IMAGE)"
epf_core0_tap_spool_len_addr="$(symbol_addr HIBANA_EPF_CORE0_TAP_SPOOL_LEN)"
epf_core0_tap_spool_read_addr="$(symbol_addr HIBANA_EPF_CORE0_TAP_SPOOL_READ)"
epf_tap_spool_len_addr="$(symbol_addr HIBANA_EPF_TAP_SPOOL_LEN)"
epf_tap_spool_read_addr="$(symbol_addr HIBANA_EPF_TAP_SPOOL_READ)"

write_epf_policy_timer_mailbox_image() {
  local lines
  mapfile -t lines < <(python3 <<'PY'
def fnv32(values):
    h = 0x811c9dc5
    for value in values:
        h ^= value & 0xff
        h = (h * 0x01000193) & 0xffffffff
    return h

image = bytes([
    0x45, 0x50, 0x46, 0x30, 0x01, 0x00, 0x01, 0x00,
    0x39, 0x00, 0x28, 0x00, 0x20, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x45, 0x48, 0xe5, 0x71, 0x4d, 0xc0,
    0x11, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x11, 0x02, 0x00, 0x00,
    0x00, 0x00, 0x20, 0x00, 0x01, 0x02, 0x01, 0x01,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
])
print(f"{fnv32(image):08x}")
print(" ".join(f"0x{byte:02x}" for byte in image))
PY
  )
  local digest="${lines[0]}"
  local image
  read -r -a image <<< "${lines[1]}"
  local image_len="${#image[@]}"

  probe_write b32 "$epf_bytecode_state_addr" 0x00000000
  probe_write b8 "$epf_bytecode_image_addr" "${image[@]}"
  probe_write b32 "$epf_bytecode_len_addr" "$image_len"
  probe_write b32 "$epf_bytecode_hash_addr" "0x$digest"
  probe_write b32 "$epf_bytecode_state_addr" 0x434f4d54
}

if [[ "$pattern" == "epf-policy-timer" ]]; then
  write_epf_policy_timer_mailbox_image
fi

if (( initial_poll_delay_seconds > 0 )); then
  sleep "$initial_poll_delay_seconds"
fi

result=""
stage=""
deadline=$((SECONDS + timeout_seconds))
while :; do
  result="$(read_word "$result_addr")"
  stage="$(read_word "$stage_addr")"
  core0_stage="$(read_word "$core0_stage_addr")"
  core1_stage="$(read_word "$core1_stage_addr")"
  if [[ "$expect_panic_marker" == "1" && "$result" == "$expected_result" ]]; then
    break
  fi
  if [[ "$runtime_ready_core" == "core0" && "$result" == "$expected_result" && "$core0_stage" == "4849000a" && ( "$core1_stage" == "$expected_core1_stage" || ( "$allow_core1_ready" == "1" && "$core1_stage" == "4849000a" ) ) ]]; then
    break
  fi
  if [[ "$runtime_ready_core" == "core1" && "$result" == "$expected_result" && "$core1_stage" == "4849000a" && ( "$core0_stage" == "48490004" || "$core0_stage" == "4849000a" ) ]]; then
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

fnv32_bytes() {
  python3 - "$@" <<'PY'
import sys
h = 0x811c9dc5
for arg in sys.argv[1:]:
    h ^= int(arg, 0) & 0xff
    h = (h * 0x01000193) & 0xffffffff
print(f"{h:08x}")
PY
}

write_session_mismatch_epf_image() {
  local image
  read -r -a image < <(python3 <<'PY'
def fnv32(values):
    h = 0x811c9dc5
    for value in values:
        h ^= value & 0xff
        h = (h * 0x01000193) & 0xffffffff
    return h

code = [
    0x15, 0x05, 0x02,
    0x16, 0x01,
    0x13, 0x00,
    0x14, 0x01,
    0x10, 0x02, 0x00,
    0x10, 0x03, 0x01,
    0x10, 0x04, 0x03,
    0x20, 0x00, 0x01, 0x02, 0x03, 0x04,
    0xff,
]
code_hash = fnv32(code)
header = []
header.extend(b"EPF0")
header.extend((1).to_bytes(2, "little"))          # abi
header.extend([0, 0])                              # Target::Observe, reserved
header.extend((0).to_bytes(2, "little"))          # policy_id
header.extend(len(code).to_bytes(2, "little"))
header.extend((32).to_bytes(2, "little"))         # fuel_max
header.extend((0).to_bytes(2, "little"))          # mem_len
header.extend((0x48450001).to_bytes(4, "little")) # Evidence schema hash
header.extend(code_hash.to_bytes(4, "little"))
print(" ".join(f"0x{byte:02x}" for byte in header + code))
PY
  )
  local image_len="${#image[@]}"
  local digest
  digest="$(fnv32_bytes "${image[@]}")"

  probe_write b32 "$epf_bytecode_state_addr" 0x00000000
  probe_write b8 "$epf_bytecode_image_addr" "${image[@]}"
  probe_write b32 "$epf_bytecode_len_addr" "$image_len"
  probe_write b32 "$epf_bytecode_hash_addr" "0x$digest"
  probe_write b32 "$epf_bytecode_state_addr" 0x434f4d54

  local wait_deadline=$((SECONDS + 8))
  local load_epoch="00000000"
  local fuel="00000000"
  while :; do
    load_epoch="$(read_word "$epf_load_epoch_addr")"
    fuel="$(read_word "$epf_core1_fuel_addr")"
    if [[ "$load_epoch" != "00000000" && "$fuel" != "00000000" ]]; then
      break
    fi
    if (( SECONDS >= wait_deadline )); then
      break
    fi
    sleep 0.25
  done
}

write_capacity_fault_epf_image() {
  local image
  read -r -a image < <(python3 <<'PY'
def fnv32(values):
    h = 0x811c9dc5
    for value in values:
        h ^= value & 0xff
        h = (h * 0x01000193) & 0xffffffff
    return h

code = [
    0x15, 0x07, 0x02,
    0x16, 0x03,
    0x13, 0x00,
    0x14, 0x01,
    0x11, 0x02, 0x01, 0x00, 0x00, 0x00,
    0x11, 0x03, 0x00, 0x00, 0x00, 0x00,
    0x10, 0x04, 0x03,
    0x20, 0x00, 0x01, 0x02, 0x03, 0x04,
    0xff,
]
code_hash = fnv32(code)
header = []
header.extend(b"EPF0")
header.extend((1).to_bytes(2, "little"))          # abi
header.extend([0, 0])                              # Target::Observe, reserved
header.extend((0).to_bytes(2, "little"))          # policy_id
header.extend(len(code).to_bytes(2, "little"))
header.extend((32).to_bytes(2, "little"))         # fuel_max
header.extend((0).to_bytes(2, "little"))          # mem_len
header.extend((0x48450001).to_bytes(4, "little")) # Evidence schema hash
header.extend(code_hash.to_bytes(4, "little"))
print(" ".join(f"0x{byte:02x}" for byte in header + code))
PY
  )
  local image_len="${#image[@]}"
  local digest
  digest="$(fnv32_bytes "${image[@]}")"

  probe_write b32 "$epf_bytecode_state_addr" 0x00000000
  probe_write b8 "$epf_bytecode_image_addr" "${image[@]}"
  probe_write b32 "$epf_bytecode_len_addr" "$image_len"
  probe_write b32 "$epf_bytecode_hash_addr" "0x$digest"
  probe_write b32 "$epf_bytecode_state_addr" 0x434f4d54

  local wait_deadline=$((SECONDS + 8))
  local load_epoch="00000000"
  local fuel="00000000"
  while :; do
    load_epoch="$(read_word "$epf_load_epoch_addr")"
    fuel="$(read_word "$epf_core1_fuel_addr")"
    if [[ "$load_epoch" != "00000000" && "$fuel" != "00000000" ]]; then
      break
    fi
    if (( SECONDS >= wait_deadline )); then
      break
    fi
    sleep 0.25
  done
}

if [[ "$pattern" == "session-mismatch" ]]; then
  write_session_mismatch_epf_image
elif [[ "$pattern" == "capacity-fault" ]]; then
  write_capacity_fault_epf_image
fi

printf 'pattern=%s\n' "$pattern"
printf 'bin=%s\n' "$bin_name"
printf 'target=%s\n' "$target"
printf 'chip=%s\n' "$chip"
printf 'features=%s\n' "$features"
printf 'result_addr=%s result=0x%s expected=0x%s\n' "$result_addr" "$result" "$expected_result"
printf 'stage_addr=%s stage=0x%s\n' "$stage_addr" "$stage"
hardfault_pc="$(read_word "$hardfault_pc_addr")"
hardfault_lr="$(read_word "$hardfault_lr_addr")"
hardfault_r0="$(read_word "$hardfault_r0_addr")"
hardfault_r1="$(read_word "$hardfault_r1_addr")"
hardfault_r2="$(read_word "$hardfault_r2_addr")"
hardfault_r3="$(read_word "$hardfault_r3_addr")"
hardfault_r12="$(read_word "$hardfault_r12_addr")"
hardfault_sp="$(read_word "$hardfault_sp_addr")"
printf 'hardfault_pc_addr=%s pc=0x%s\n' "$hardfault_pc_addr" "$hardfault_pc"
printf 'hardfault_lr_addr=%s lr=0x%s\n' "$hardfault_lr_addr" "$hardfault_lr"
printf 'hardfault_r0_addr=%s r0=0x%s\n' "$hardfault_r0_addr" "$hardfault_r0"
printf 'hardfault_r1_addr=%s r1=0x%s\n' "$hardfault_r1_addr" "$hardfault_r1"
printf 'hardfault_r2_addr=%s r2=0x%s\n' "$hardfault_r2_addr" "$hardfault_r2"
printf 'hardfault_r3_addr=%s r3=0x%s\n' "$hardfault_r3_addr" "$hardfault_r3"
printf 'hardfault_r12_addr=%s r12=0x%s\n' "$hardfault_r12_addr" "$hardfault_r12"
printf 'hardfault_sp_addr=%s sp=0x%s\n' "$hardfault_sp_addr" "$hardfault_sp"
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
choreofs_engine_gap_count="$(read_word "$choreofs_engine_gap_count_addr")"
choreofs_engine_gap_last_us="$(read_word "$choreofs_engine_gap_last_us_addr")"
choreofs_engine_gap_total_us="$(read_word "$choreofs_engine_gap_total_us_addr")"
choreofs_engine_gap_max_us="$(read_word "$choreofs_engine_gap_max_us_addr")"
choreofs_driver_import_count="$(read_word "$choreofs_driver_import_count_addr")"
choreofs_driver_import_last_us="$(read_word "$choreofs_driver_import_last_us_addr")"
choreofs_driver_import_total_us="$(read_word "$choreofs_driver_import_total_us_addr")"
choreofs_driver_import_max_us="$(read_word "$choreofs_driver_import_max_us_addr")"
choreofs_poll_delay_count="$(read_word "$choreofs_poll_delay_count_addr")"
choreofs_poll_delay_last_us="$(read_word "$choreofs_poll_delay_last_us_addr")"
choreofs_poll_delay_total_us="$(read_word "$choreofs_poll_delay_total_us_addr")"
choreofs_poll_delay_max_us="$(read_word "$choreofs_poll_delay_max_us_addr")"
choreofs_request_recv_count="$(read_word "$choreofs_request_recv_count_addr")"
choreofs_request_recv_last_us="$(read_word "$choreofs_request_recv_last_us_addr")"
choreofs_request_recv_total_us="$(read_word "$choreofs_request_recv_total_us_addr")"
choreofs_request_recv_max_us="$(read_word "$choreofs_request_recv_max_us_addr")"
choreofs_reply_send_count="$(read_word "$choreofs_reply_send_count_addr")"
choreofs_reply_send_last_us="$(read_word "$choreofs_reply_send_last_us_addr")"
choreofs_reply_send_total_us="$(read_word "$choreofs_reply_send_total_us_addr")"
choreofs_reply_send_max_us="$(read_word "$choreofs_reply_send_max_us_addr")"
choreofs_reply_send_fd_write_object_count="$(read_word "$choreofs_reply_send_fd_write_object_count_addr")"
choreofs_reply_send_fd_write_object_total_us="$(read_word "$choreofs_reply_send_fd_write_object_total_us_addr")"
choreofs_reply_send_poll_oneoff_count="$(read_word "$choreofs_reply_send_poll_oneoff_count_addr")"
choreofs_reply_send_poll_oneoff_total_us="$(read_word "$choreofs_reply_send_poll_oneoff_total_us_addr")"
choreofs_reply_send_future_poll_count="$(read_word_or_zero "$choreofs_reply_send_future_poll_count_addr")"
choreofs_reply_send_future_poll_last_us="$(read_word_or_zero "$choreofs_reply_send_future_poll_last_us_addr")"
choreofs_reply_send_future_poll_total_us="$(read_word_or_zero "$choreofs_reply_send_future_poll_total_us_addr")"
choreofs_reply_send_future_poll_max_us="$(read_word_or_zero "$choreofs_reply_send_future_poll_max_us_addr")"
choreofs_reply_encode_count="$(read_word_or_zero "$choreofs_reply_encode_count_addr")"
choreofs_reply_encode_last_us="$(read_word_or_zero "$choreofs_reply_encode_last_us_addr")"
choreofs_reply_encode_total_us="$(read_word_or_zero "$choreofs_reply_encode_total_us_addr")"
choreofs_reply_encode_max_us="$(read_word_or_zero "$choreofs_reply_encode_max_us_addr")"
choreofs_measurements_frozen="$(read_word "$choreofs_measurements_frozen_addr")"
choreofs_driver_trace="$(read_word "$choreofs_driver_trace_addr")"
choreofs_sio_trace_core0_count="$(read_word "$choreofs_sio_trace_core0_count_addr")"
choreofs_sio_trace_core1_count="$(read_word "$choreofs_sio_trace_core1_count_addr")"
choreofs_sio_core0_to_core1_tx="$(read_word "$choreofs_sio_core0_to_core1_tx_addr")"
choreofs_sio_core0_to_core1_rx="$(read_word "$choreofs_sio_core0_to_core1_rx_addr")"
choreofs_sio_core1_to_core0_tx="$(read_word "$choreofs_sio_core1_to_core0_tx_addr")"
choreofs_sio_core1_to_core0_rx="$(read_word "$choreofs_sio_core1_to_core0_rx_addr")"
choreofs_sio_role1_pending_seen_core0_tx="$(read_word "$choreofs_sio_role1_pending_seen_core0_tx_addr")"
choreofs_sio_role1_poll_seen_core0_tx="$(read_word "$choreofs_sio_role1_poll_seen_core0_tx_addr")"
choreofs_sio_role1_ready_seen_core0_tx="$(read_word "$choreofs_sio_role1_ready_seen_core0_tx_addr")"
choreofs_sio_rx_wait_count="$(read_word "$choreofs_sio_rx_wait_count_addr")"
choreofs_sio_rx_wait_last_us="$(read_word "$choreofs_sio_rx_wait_last_us_addr")"
choreofs_sio_rx_wait_total_us="$(read_word "$choreofs_sio_rx_wait_total_us_addr")"
choreofs_sio_rx_wait_max_us="$(read_word "$choreofs_sio_rx_wait_max_us_addr")"
choreofs_sio_rx_wait_role0_count="$(read_word "$choreofs_sio_rx_wait_role0_count_addr")"
choreofs_sio_rx_wait_role0_last_us="$(read_word "$choreofs_sio_rx_wait_role0_last_us_addr")"
choreofs_sio_rx_wait_role0_total_us="$(read_word "$choreofs_sio_rx_wait_role0_total_us_addr")"
choreofs_sio_rx_wait_role0_max_us="$(read_word "$choreofs_sio_rx_wait_role0_max_us_addr")"
choreofs_sio_rx_wait_role1_count="$(read_word "$choreofs_sio_rx_wait_role1_count_addr")"
choreofs_sio_rx_wait_role1_last_us="$(read_word "$choreofs_sio_rx_wait_role1_last_us_addr")"
choreofs_sio_rx_wait_role1_total_us="$(read_word "$choreofs_sio_rx_wait_role1_total_us_addr")"
choreofs_sio_rx_wait_role1_max_us="$(read_word "$choreofs_sio_rx_wait_role1_max_us_addr")"
choreofs_sio_tx_wait_count="$(read_word "$choreofs_sio_tx_wait_count_addr")"
choreofs_sio_tx_wait_last_us="$(read_word "$choreofs_sio_tx_wait_last_us_addr")"
choreofs_sio_tx_wait_total_us="$(read_word "$choreofs_sio_tx_wait_total_us_addr")"
choreofs_sio_tx_wait_max_us="$(read_word "$choreofs_sio_tx_wait_max_us_addr")"
choreofs_sio_tx_wait_role0_count="$(read_word "$choreofs_sio_tx_wait_role0_count_addr")"
choreofs_sio_tx_wait_role0_last_us="$(read_word "$choreofs_sio_tx_wait_role0_last_us_addr")"
choreofs_sio_tx_wait_role0_total_us="$(read_word "$choreofs_sio_tx_wait_role0_total_us_addr")"
choreofs_sio_tx_wait_role0_max_us="$(read_word "$choreofs_sio_tx_wait_role0_max_us_addr")"
choreofs_sio_tx_wait_role1_count="$(read_word "$choreofs_sio_tx_wait_role1_count_addr")"
choreofs_sio_tx_wait_role1_last_us="$(read_word "$choreofs_sio_tx_wait_role1_last_us_addr")"
choreofs_sio_tx_wait_role1_total_us="$(read_word "$choreofs_sio_tx_wait_role1_total_us_addr")"
choreofs_sio_tx_wait_role1_max_us="$(read_word "$choreofs_sio_tx_wait_role1_max_us_addr")"
choreofs_sio_tx_poll_count="$(read_word "$choreofs_sio_tx_poll_count_addr")"
choreofs_sio_tx_poll_total_us="$(read_word "$choreofs_sio_tx_poll_total_us_addr")"
choreofs_sio_tx_poll_max_us="$(read_word "$choreofs_sio_tx_poll_max_us_addr")"
choreofs_sio_tx_poll_role0_count="$(read_word "$choreofs_sio_tx_poll_role0_count_addr")"
choreofs_sio_tx_poll_role0_total_us="$(read_word "$choreofs_sio_tx_poll_role0_total_us_addr")"
choreofs_sio_tx_poll_role1_count="$(read_word "$choreofs_sio_tx_poll_role1_count_addr")"
choreofs_sio_tx_poll_role1_total_us="$(read_word "$choreofs_sio_tx_poll_role1_total_us_addr")"
appkit_wasi_resume_count="$(read_word_or_zero "$appkit_wasi_resume_count_addr")"
appkit_wasi_resume_last_us="$(read_word_or_zero "$appkit_wasi_resume_last_us_addr")"
appkit_wasi_resume_total_us="$(read_word_or_zero "$appkit_wasi_resume_total_us_addr")"
appkit_wasi_resume_max_us="$(read_word_or_zero "$appkit_wasi_resume_max_us_addr")"
appkit_wasi_request_send_count="$(read_word_or_zero "$appkit_wasi_request_send_count_addr")"
appkit_wasi_request_send_last_us="$(read_word_or_zero "$appkit_wasi_request_send_last_us_addr")"
appkit_wasi_request_send_total_us="$(read_word_or_zero "$appkit_wasi_request_send_total_us_addr")"
appkit_wasi_request_send_max_us="$(read_word_or_zero "$appkit_wasi_request_send_max_us_addr")"
appkit_wasi_completion_recv_count="$(read_word_or_zero "$appkit_wasi_completion_recv_count_addr")"
appkit_wasi_completion_recv_last_us="$(read_word_or_zero "$appkit_wasi_completion_recv_last_us_addr")"
appkit_wasi_completion_recv_total_us="$(read_word_or_zero "$appkit_wasi_completion_recv_total_us_addr")"
appkit_wasi_completion_recv_max_us="$(read_word_or_zero "$appkit_wasi_completion_recv_max_us_addr")"
appkit_wasi_complete_count="$(read_word_or_zero "$appkit_wasi_complete_count_addr")"
appkit_wasi_complete_last_us="$(read_word_or_zero "$appkit_wasi_complete_last_us_addr")"
appkit_wasi_complete_total_us="$(read_word_or_zero "$appkit_wasi_complete_total_us_addr")"
appkit_wasi_complete_max_us="$(read_word_or_zero "$appkit_wasi_complete_max_us_addr")"
epf_core0_epoch="$(read_word "$epf_core0_epoch_addr")"
epf_core0_kind="$(read_word "$epf_core0_kind_addr")"
epf_core0_reason="$(read_word "$epf_core0_reason_addr")"
epf_core0_arg0="$(read_word "$epf_core0_arg0_addr")"
epf_core0_arg1="$(read_word "$epf_core0_arg1_addr")"
epf_core0_arg2="$(read_word "$epf_core0_arg2_addr")"
epf_core0_fuel="$(read_word "$epf_core0_fuel_addr")"
epf_core1_epoch="$(read_word "$epf_core1_epoch_addr")"
epf_core1_kind="$(read_word "$epf_core1_kind_addr")"
epf_core1_reason="$(read_word "$epf_core1_reason_addr")"
epf_core1_arg0="$(read_word "$epf_core1_arg0_addr")"
epf_core1_arg1="$(read_word "$epf_core1_arg1_addr")"
epf_core1_arg2="$(read_word "$epf_core1_arg2_addr")"
epf_core1_fuel="$(read_word "$epf_core1_fuel_addr")"
epf_load_epoch="$(read_word "$epf_load_epoch_addr")"
epf_image_digest="$(read_word "$epf_image_digest_addr")"
epf_load_reason="$(read_word "$epf_load_reason_addr")"
epf_active_target_kind="$(read_word "$epf_active_target_kind_addr")"
epf_active_policy_id="$(read_word "$epf_active_policy_id_addr")"
epf_policy_timer_irq_ready="$(read_word "$epf_policy_timer_irq_ready_addr")"
epf_policy_timer_fact_kind="$(read_word "$epf_policy_timer_fact_kind_addr")"
epf_policy_timer_fact_arg0="$(read_word "$epf_policy_timer_fact_arg0_addr")"
epf_policy_timer_fact_fuel="$(read_word "$epf_policy_timer_fact_fuel_addr")"
epf_image_ingress="$(read_word "$epf_image_ingress_addr")"
epf_bytecode_state="$(read_word "$epf_bytecode_state_addr")"
epf_core0_tap_spool_len="$(read_word "$epf_core0_tap_spool_len_addr")"
epf_core0_tap_spool_read="$(read_word "$epf_core0_tap_spool_read_addr")"
epf_tap_spool_len="$(read_word "$epf_tap_spool_len_addr")"
epf_tap_spool_read="$(read_word "$epf_tap_spool_read_addr")"
watchdog_tick="$(read_mmio_word 0x4005802c)"
clk_ref_ctrl="$(read_mmio_word 0x40008030)"
clk_ref_selected="$(read_mmio_word 0x40008038)"
clk_sys_ctrl="$(read_mmio_word 0x4000803c)"
clk_sys_selected="$(read_mmio_word 0x40008044)"
clk_peri_ctrl="$(read_mmio_word 0x40008048)"
clk_peri_selected="$(read_mmio_word 0x40008050)"
xosc_status="$(read_mmio_word 0x40024004)"
pll_sys_cs="$(read_mmio_word 0x40028000)"
pll_sys_fbdiv="$(read_mmio_word 0x40028008)"
pll_sys_prim="$(read_mmio_word 0x4002800c)"

print_choreofs_markers="${HIBANA_BAKER_PRINT_CHOREOFS:-1}"

if [[ "$pattern" != "session-mismatch" && "$print_choreofs_markers" == "1" ]]; then
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
  printf 'choreofs_engine_gap_count_addr=%s count=0x%s\n' "$choreofs_engine_gap_count_addr" "$choreofs_engine_gap_count"
  printf 'choreofs_engine_gap_last_us_addr=%s us=0x%s\n' "$choreofs_engine_gap_last_us_addr" "$choreofs_engine_gap_last_us"
  printf 'choreofs_engine_gap_total_us_addr=%s us=0x%s\n' "$choreofs_engine_gap_total_us_addr" "$choreofs_engine_gap_total_us"
  printf 'choreofs_engine_gap_max_us_addr=%s us=0x%s\n' "$choreofs_engine_gap_max_us_addr" "$choreofs_engine_gap_max_us"
  printf 'choreofs_driver_import_count_addr=%s count=0x%s\n' "$choreofs_driver_import_count_addr" "$choreofs_driver_import_count"
  printf 'choreofs_driver_import_last_us_addr=%s us=0x%s\n' "$choreofs_driver_import_last_us_addr" "$choreofs_driver_import_last_us"
  printf 'choreofs_driver_import_total_us_addr=%s us=0x%s\n' "$choreofs_driver_import_total_us_addr" "$choreofs_driver_import_total_us"
  printf 'choreofs_driver_import_max_us_addr=%s us=0x%s\n' "$choreofs_driver_import_max_us_addr" "$choreofs_driver_import_max_us"
  printf 'choreofs_poll_delay_count_addr=%s count=0x%s\n' "$choreofs_poll_delay_count_addr" "$choreofs_poll_delay_count"
  printf 'choreofs_poll_delay_last_us_addr=%s us=0x%s\n' "$choreofs_poll_delay_last_us_addr" "$choreofs_poll_delay_last_us"
  printf 'choreofs_poll_delay_total_us_addr=%s us=0x%s\n' "$choreofs_poll_delay_total_us_addr" "$choreofs_poll_delay_total_us"
  printf 'choreofs_poll_delay_max_us_addr=%s us=0x%s\n' "$choreofs_poll_delay_max_us_addr" "$choreofs_poll_delay_max_us"
  printf 'choreofs_request_recv_count_addr=%s count=0x%s\n' "$choreofs_request_recv_count_addr" "$choreofs_request_recv_count"
  printf 'choreofs_request_recv_last_us_addr=%s us=0x%s\n' "$choreofs_request_recv_last_us_addr" "$choreofs_request_recv_last_us"
  printf 'choreofs_request_recv_total_us_addr=%s us=0x%s\n' "$choreofs_request_recv_total_us_addr" "$choreofs_request_recv_total_us"
  printf 'choreofs_request_recv_max_us_addr=%s us=0x%s\n' "$choreofs_request_recv_max_us_addr" "$choreofs_request_recv_max_us"
  printf 'choreofs_reply_send_count_addr=%s count=0x%s\n' "$choreofs_reply_send_count_addr" "$choreofs_reply_send_count"
  printf 'choreofs_reply_send_last_us_addr=%s us=0x%s\n' "$choreofs_reply_send_last_us_addr" "$choreofs_reply_send_last_us"
  printf 'choreofs_reply_send_total_us_addr=%s us=0x%s\n' "$choreofs_reply_send_total_us_addr" "$choreofs_reply_send_total_us"
  printf 'choreofs_reply_send_max_us_addr=%s us=0x%s\n' "$choreofs_reply_send_max_us_addr" "$choreofs_reply_send_max_us"
  printf 'choreofs_reply_send_fd_write_object_count_addr=%s count=0x%s\n' "$choreofs_reply_send_fd_write_object_count_addr" "$choreofs_reply_send_fd_write_object_count"
  printf 'choreofs_reply_send_fd_write_object_total_us_addr=%s us=0x%s\n' "$choreofs_reply_send_fd_write_object_total_us_addr" "$choreofs_reply_send_fd_write_object_total_us"
  printf 'choreofs_reply_send_poll_oneoff_count_addr=%s count=0x%s\n' "$choreofs_reply_send_poll_oneoff_count_addr" "$choreofs_reply_send_poll_oneoff_count"
  printf 'choreofs_reply_send_poll_oneoff_total_us_addr=%s us=0x%s\n' "$choreofs_reply_send_poll_oneoff_total_us_addr" "$choreofs_reply_send_poll_oneoff_total_us"
  if [[ -n "$choreofs_reply_send_future_poll_count_addr" ]]; then
    printf 'choreofs_reply_send_future_poll_count_addr=%s count=0x%s\n' "$choreofs_reply_send_future_poll_count_addr" "$choreofs_reply_send_future_poll_count"
    printf 'choreofs_reply_send_future_poll_last_us_addr=%s us=0x%s\n' "$choreofs_reply_send_future_poll_last_us_addr" "$choreofs_reply_send_future_poll_last_us"
    printf 'choreofs_reply_send_future_poll_total_us_addr=%s us=0x%s\n' "$choreofs_reply_send_future_poll_total_us_addr" "$choreofs_reply_send_future_poll_total_us"
    printf 'choreofs_reply_send_future_poll_max_us_addr=%s us=0x%s\n' "$choreofs_reply_send_future_poll_max_us_addr" "$choreofs_reply_send_future_poll_max_us"
  fi
  if [[ -n "$choreofs_reply_encode_count_addr" ]]; then
    printf 'choreofs_reply_encode_count_addr=%s count=0x%s\n' "$choreofs_reply_encode_count_addr" "$choreofs_reply_encode_count"
    printf 'choreofs_reply_encode_last_us_addr=%s us=0x%s\n' "$choreofs_reply_encode_last_us_addr" "$choreofs_reply_encode_last_us"
    printf 'choreofs_reply_encode_total_us_addr=%s us=0x%s\n' "$choreofs_reply_encode_total_us_addr" "$choreofs_reply_encode_total_us"
    printf 'choreofs_reply_encode_max_us_addr=%s us=0x%s\n' "$choreofs_reply_encode_max_us_addr" "$choreofs_reply_encode_max_us"
  fi
  if [[ -n "$choreofs_reply_send_future_poll_total_us_addr" && -n "$choreofs_reply_encode_total_us_addr" ]]; then
    reply_future_poll_dec="$((16#$choreofs_reply_send_future_poll_total_us))"
    reply_encode_dec="$((16#$choreofs_reply_encode_total_us))"
    reply_transport_dec="$((16#$choreofs_sio_tx_poll_role0_total_us))"
    if (( reply_future_poll_dec > reply_encode_dec + reply_transport_dec )); then
      reply_endpoint_residual_dec="$((reply_future_poll_dec - reply_encode_dec - reply_transport_dec))"
    else
      reply_endpoint_residual_dec=0
    fi
    printf 'choreofs_reply_send_endpoint_residual_us=0x%08x\n' "$reply_endpoint_residual_dec"
  fi
  printf 'choreofs_measurements_frozen_addr=%s frozen=0x%s\n' "$choreofs_measurements_frozen_addr" "$choreofs_measurements_frozen"
  printf 'choreofs_driver_trace_addr=%s trace=0x%s\n' "$choreofs_driver_trace_addr" "$choreofs_driver_trace"
  printf 'choreofs_sio_trace_core0_count_addr=%s count=0x%s\n' "$choreofs_sio_trace_core0_count_addr" "$choreofs_sio_trace_core0_count"
  printf 'choreofs_sio_trace_core1_count_addr=%s count=0x%s\n' "$choreofs_sio_trace_core1_count_addr" "$choreofs_sio_trace_core1_count"
  printf 'choreofs_sio_core0_to_core1_tx_addr=%s count=0x%s\n' "$choreofs_sio_core0_to_core1_tx_addr" "$choreofs_sio_core0_to_core1_tx"
  printf 'choreofs_sio_core0_to_core1_rx_addr=%s count=0x%s\n' "$choreofs_sio_core0_to_core1_rx_addr" "$choreofs_sio_core0_to_core1_rx"
  printf 'choreofs_sio_core1_to_core0_tx_addr=%s count=0x%s\n' "$choreofs_sio_core1_to_core0_tx_addr" "$choreofs_sio_core1_to_core0_tx"
  printf 'choreofs_sio_core1_to_core0_rx_addr=%s count=0x%s\n' "$choreofs_sio_core1_to_core0_rx_addr" "$choreofs_sio_core1_to_core0_rx"
  printf 'choreofs_sio_role1_pending_seen_core0_tx_addr=%s count=0x%s\n' "$choreofs_sio_role1_pending_seen_core0_tx_addr" "$choreofs_sio_role1_pending_seen_core0_tx"
  printf 'choreofs_sio_role1_poll_seen_core0_tx_addr=%s count=0x%s\n' "$choreofs_sio_role1_poll_seen_core0_tx_addr" "$choreofs_sio_role1_poll_seen_core0_tx"
  printf 'choreofs_sio_role1_ready_seen_core0_tx_addr=%s count=0x%s\n' "$choreofs_sio_role1_ready_seen_core0_tx_addr" "$choreofs_sio_role1_ready_seen_core0_tx"
  printf 'choreofs_sio_rx_wait_count_addr=%s count=0x%s\n' "$choreofs_sio_rx_wait_count_addr" "$choreofs_sio_rx_wait_count"
  printf 'choreofs_sio_rx_wait_last_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_last_us_addr" "$choreofs_sio_rx_wait_last_us"
  printf 'choreofs_sio_rx_wait_total_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_total_us_addr" "$choreofs_sio_rx_wait_total_us"
  printf 'choreofs_sio_rx_wait_max_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_max_us_addr" "$choreofs_sio_rx_wait_max_us"
  printf 'choreofs_sio_rx_wait_role0_count_addr=%s count=0x%s\n' "$choreofs_sio_rx_wait_role0_count_addr" "$choreofs_sio_rx_wait_role0_count"
  printf 'choreofs_sio_rx_wait_role0_last_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role0_last_us_addr" "$choreofs_sio_rx_wait_role0_last_us"
  printf 'choreofs_sio_rx_wait_role0_total_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role0_total_us_addr" "$choreofs_sio_rx_wait_role0_total_us"
  printf 'choreofs_sio_rx_wait_role0_max_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role0_max_us_addr" "$choreofs_sio_rx_wait_role0_max_us"
  printf 'choreofs_sio_rx_wait_role1_count_addr=%s count=0x%s\n' "$choreofs_sio_rx_wait_role1_count_addr" "$choreofs_sio_rx_wait_role1_count"
  printf 'choreofs_sio_rx_wait_role1_last_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role1_last_us_addr" "$choreofs_sio_rx_wait_role1_last_us"
  printf 'choreofs_sio_rx_wait_role1_total_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role1_total_us_addr" "$choreofs_sio_rx_wait_role1_total_us"
  printf 'choreofs_sio_rx_wait_role1_max_us_addr=%s us=0x%s\n' "$choreofs_sio_rx_wait_role1_max_us_addr" "$choreofs_sio_rx_wait_role1_max_us"
  printf 'choreofs_sio_tx_wait_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_wait_count_addr" "$choreofs_sio_tx_wait_count"
  printf 'choreofs_sio_tx_wait_last_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_last_us_addr" "$choreofs_sio_tx_wait_last_us"
  printf 'choreofs_sio_tx_wait_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_total_us_addr" "$choreofs_sio_tx_wait_total_us"
  printf 'choreofs_sio_tx_wait_max_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_max_us_addr" "$choreofs_sio_tx_wait_max_us"
  printf 'choreofs_sio_tx_wait_role0_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_wait_role0_count_addr" "$choreofs_sio_tx_wait_role0_count"
  printf 'choreofs_sio_tx_wait_role0_last_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role0_last_us_addr" "$choreofs_sio_tx_wait_role0_last_us"
  printf 'choreofs_sio_tx_wait_role0_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role0_total_us_addr" "$choreofs_sio_tx_wait_role0_total_us"
  printf 'choreofs_sio_tx_wait_role0_max_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role0_max_us_addr" "$choreofs_sio_tx_wait_role0_max_us"
  printf 'choreofs_sio_tx_wait_role1_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_wait_role1_count_addr" "$choreofs_sio_tx_wait_role1_count"
  printf 'choreofs_sio_tx_wait_role1_last_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role1_last_us_addr" "$choreofs_sio_tx_wait_role1_last_us"
  printf 'choreofs_sio_tx_wait_role1_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role1_total_us_addr" "$choreofs_sio_tx_wait_role1_total_us"
  printf 'choreofs_sio_tx_wait_role1_max_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_wait_role1_max_us_addr" "$choreofs_sio_tx_wait_role1_max_us"
  printf 'choreofs_sio_tx_poll_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_poll_count_addr" "$choreofs_sio_tx_poll_count"
  printf 'choreofs_sio_tx_poll_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_poll_total_us_addr" "$choreofs_sio_tx_poll_total_us"
  printf 'choreofs_sio_tx_poll_max_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_poll_max_us_addr" "$choreofs_sio_tx_poll_max_us"
  printf 'choreofs_sio_tx_poll_role0_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_poll_role0_count_addr" "$choreofs_sio_tx_poll_role0_count"
  printf 'choreofs_sio_tx_poll_role0_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_poll_role0_total_us_addr" "$choreofs_sio_tx_poll_role0_total_us"
  printf 'choreofs_sio_tx_poll_role1_count_addr=%s count=0x%s\n' "$choreofs_sio_tx_poll_role1_count_addr" "$choreofs_sio_tx_poll_role1_count"
  printf 'choreofs_sio_tx_poll_role1_total_us_addr=%s us=0x%s\n' "$choreofs_sio_tx_poll_role1_total_us_addr" "$choreofs_sio_tx_poll_role1_total_us"
  if [[ -n "$appkit_wasi_resume_count_addr" ]]; then
    printf 'appkit_wasi_resume_count_addr=%s count=0x%s\n' "$appkit_wasi_resume_count_addr" "$appkit_wasi_resume_count"
    printf 'appkit_wasi_resume_last_us_addr=%s us=0x%s\n' "$appkit_wasi_resume_last_us_addr" "$appkit_wasi_resume_last_us"
    printf 'appkit_wasi_resume_total_us_addr=%s us=0x%s\n' "$appkit_wasi_resume_total_us_addr" "$appkit_wasi_resume_total_us"
    printf 'appkit_wasi_resume_max_us_addr=%s us=0x%s\n' "$appkit_wasi_resume_max_us_addr" "$appkit_wasi_resume_max_us"
    printf 'appkit_wasi_request_send_count_addr=%s count=0x%s\n' "$appkit_wasi_request_send_count_addr" "$appkit_wasi_request_send_count"
    printf 'appkit_wasi_request_send_last_us_addr=%s us=0x%s\n' "$appkit_wasi_request_send_last_us_addr" "$appkit_wasi_request_send_last_us"
    printf 'appkit_wasi_request_send_total_us_addr=%s us=0x%s\n' "$appkit_wasi_request_send_total_us_addr" "$appkit_wasi_request_send_total_us"
    printf 'appkit_wasi_request_send_max_us_addr=%s us=0x%s\n' "$appkit_wasi_request_send_max_us_addr" "$appkit_wasi_request_send_max_us"
    printf 'appkit_wasi_completion_recv_count_addr=%s count=0x%s\n' "$appkit_wasi_completion_recv_count_addr" "$appkit_wasi_completion_recv_count"
    printf 'appkit_wasi_completion_recv_last_us_addr=%s us=0x%s\n' "$appkit_wasi_completion_recv_last_us_addr" "$appkit_wasi_completion_recv_last_us"
    printf 'appkit_wasi_completion_recv_total_us_addr=%s us=0x%s\n' "$appkit_wasi_completion_recv_total_us_addr" "$appkit_wasi_completion_recv_total_us"
    printf 'appkit_wasi_completion_recv_max_us_addr=%s us=0x%s\n' "$appkit_wasi_completion_recv_max_us_addr" "$appkit_wasi_completion_recv_max_us"
    printf 'appkit_wasi_complete_count_addr=%s count=0x%s\n' "$appkit_wasi_complete_count_addr" "$appkit_wasi_complete_count"
    printf 'appkit_wasi_complete_last_us_addr=%s us=0x%s\n' "$appkit_wasi_complete_last_us_addr" "$appkit_wasi_complete_last_us"
    printf 'appkit_wasi_complete_total_us_addr=%s us=0x%s\n' "$appkit_wasi_complete_total_us_addr" "$appkit_wasi_complete_total_us"
    printf 'appkit_wasi_complete_max_us_addr=%s us=0x%s\n' "$appkit_wasi_complete_max_us_addr" "$appkit_wasi_complete_max_us"
  fi
fi
printf 'epf_core0_epoch_addr=%s epoch=0x%s\n' "$epf_core0_epoch_addr" "$epf_core0_epoch"
printf 'epf_core0_kind_addr=%s kind=0x%s\n' "$epf_core0_kind_addr" "$epf_core0_kind"
printf 'epf_core0_reason_addr=%s reason=0x%s\n' "$epf_core0_reason_addr" "$epf_core0_reason"
printf 'epf_core0_arg0_addr=%s arg0=0x%s\n' "$epf_core0_arg0_addr" "$epf_core0_arg0"
printf 'epf_core0_arg1_addr=%s arg1=0x%s\n' "$epf_core0_arg1_addr" "$epf_core0_arg1"
printf 'epf_core0_arg2_addr=%s arg2=0x%s\n' "$epf_core0_arg2_addr" "$epf_core0_arg2"
printf 'epf_core0_fuel_addr=%s fuel=0x%s\n' "$epf_core0_fuel_addr" "$epf_core0_fuel"
printf 'epf_core1_epoch_addr=%s epoch=0x%s\n' "$epf_core1_epoch_addr" "$epf_core1_epoch"
printf 'epf_core1_kind_addr=%s kind=0x%s\n' "$epf_core1_kind_addr" "$epf_core1_kind"
printf 'epf_core1_reason_addr=%s reason=0x%s\n' "$epf_core1_reason_addr" "$epf_core1_reason"
printf 'epf_core1_arg0_addr=%s arg0=0x%s\n' "$epf_core1_arg0_addr" "$epf_core1_arg0"
printf 'epf_core1_arg1_addr=%s arg1=0x%s\n' "$epf_core1_arg1_addr" "$epf_core1_arg1"
printf 'epf_core1_arg2_addr=%s arg2=0x%s\n' "$epf_core1_arg2_addr" "$epf_core1_arg2"
printf 'epf_core1_fuel_addr=%s fuel=0x%s\n' "$epf_core1_fuel_addr" "$epf_core1_fuel"
printf 'epf_load_epoch_addr=%s epoch=0x%s\n' "$epf_load_epoch_addr" "$epf_load_epoch"
printf 'epf_image_digest_addr=%s digest=0x%s\n' "$epf_image_digest_addr" "$epf_image_digest"
printf 'epf_load_reason_addr=%s reason=0x%s\n' "$epf_load_reason_addr" "$epf_load_reason"
printf 'epf_active_target_kind_addr=%s target=0x%s\n' "$epf_active_target_kind_addr" "$epf_active_target_kind"
printf 'epf_active_policy_id_addr=%s policy=0x%s\n' "$epf_active_policy_id_addr" "$epf_active_policy_id"
printf 'epf_policy_timer_irq_ready_addr=%s ready=0x%s\n' "$epf_policy_timer_irq_ready_addr" "$epf_policy_timer_irq_ready"
printf 'epf_policy_timer_fact_kind_addr=%s kind=0x%s\n' "$epf_policy_timer_fact_kind_addr" "$epf_policy_timer_fact_kind"
printf 'epf_policy_timer_fact_arg0_addr=%s arg0=0x%s\n' "$epf_policy_timer_fact_arg0_addr" "$epf_policy_timer_fact_arg0"
printf 'epf_policy_timer_fact_fuel_addr=%s fuel=0x%s\n' "$epf_policy_timer_fact_fuel_addr" "$epf_policy_timer_fact_fuel"
printf 'epf_image_ingress_addr=%s ingress=0x%s\n' "$epf_image_ingress_addr" "$epf_image_ingress"
printf 'epf_bytecode_state_addr=%s state=0x%s\n' "$epf_bytecode_state_addr" "$epf_bytecode_state"
printf 'epf_core0_tap_spool_len_addr=%s len=0x%s\n' "$epf_core0_tap_spool_len_addr" "$epf_core0_tap_spool_len"
printf 'epf_core0_tap_spool_read_addr=%s read=0x%s\n' "$epf_core0_tap_spool_read_addr" "$epf_core0_tap_spool_read"
printf 'epf_tap_spool_len_addr=%s len=0x%s\n' "$epf_tap_spool_len_addr" "$epf_tap_spool_len"
printf 'epf_tap_spool_read_addr=%s read=0x%s\n' "$epf_tap_spool_read_addr" "$epf_tap_spool_read"
printf 'baker_clock_tick_ctrl=0x%s\n' "$watchdog_tick"
printf 'baker_clock_clk_ref_ctrl=0x%s\n' "$clk_ref_ctrl"
printf 'baker_clock_clk_ref_selected=0x%s\n' "$clk_ref_selected"
printf 'baker_clock_clk_sys_ctrl=0x%s\n' "$clk_sys_ctrl"
printf 'baker_clock_clk_sys_selected=0x%s\n' "$clk_sys_selected"
printf 'baker_clock_clk_peri_ctrl=0x%s\n' "$clk_peri_ctrl"
printf 'baker_clock_clk_peri_selected=0x%s\n' "$clk_peri_selected"
printf 'baker_clock_xosc_status=0x%s\n' "$xosc_status"
printf 'baker_clock_pll_sys_cs=0x%s\n' "$pll_sys_cs"
printf 'baker_clock_pll_sys_fbdiv=0x%s\n' "$pll_sys_fbdiv"
printf 'baker_clock_pll_sys_prim=0x%s\n' "$pll_sys_prim"
print_sio_trace() {
  local name="$1"
  local base_addr="$2"
  local count_hex="$3"
  local count_dec="$((16#$count_hex))"
  if (( count_dec > 16 )); then
    count_dec=16
  fi
  local trace_idx=0
  while (( trace_idx < count_dec )); do
    local trace_addr
    local trace_word
    trace_addr="$(printf '0x%x' "$((base_addr + trace_idx * 4))")"
    trace_word="$(read_word "$trace_addr")"
    printf '%s[%d]_addr=%s value=0x%s\n' "$name" "$trace_idx" "$trace_addr" "$trace_word"
    trace_idx=$((trace_idx + 1))
  done
}

if [[ "$pattern" != "session-mismatch" && "$print_choreofs_markers" == "1" ]]; then
  print_sio_trace choreofs_sio_trace_core0 "$choreofs_sio_trace_core0_addr" "$choreofs_sio_trace_core0_count"
  print_sio_trace choreofs_sio_trace_core1 "$choreofs_sio_trace_core1_addr" "$choreofs_sio_trace_core1_count"
fi

if [[ "$skip_result_check" != "1" && "$result" != "$expected_result" ]]; then
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
  if [[ "$pattern" != "session-mismatch" ]]; then
    echo "Baker hardware pattern $pattern ok"
    exit 0
  fi
else
  if [[ "$stage" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: failure stage was set" >&2
    exit 1
  fi
if [[ "$pattern" == "session-mismatch" || "$pattern" == "capacity-fault" ]]; then
  if [[ "$core0_stage" != "48490004" && "$core0_stage" != "4849000a" ]]; then
    echo "Baker hardware pattern $pattern failed: core0 did not stay in a running scheduler stage" >&2
    exit 1
    fi
  elif [[ "$runtime_ready_core" == "core0" ]]; then
    if [[ "$core0_stage" != "4849000a" ]]; then
      echo "Baker hardware pattern $pattern failed: core0 did not reach runtime-ready marker" >&2
      exit 1
    fi
  else
    if [[ "$core0_stage" != "48490004" && "$core0_stage" != "4849000a" ]]; then
      echo "Baker hardware pattern $pattern failed: core0 did not stay in a running scheduler stage" >&2
      exit 1
    fi
    if [[ "$core1_stage" != "4849000a" ]]; then
      echo "Baker hardware pattern $pattern failed: core1 did not reach runtime-ready marker" >&2
      exit 1
    fi
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
fi

if [[ "$pattern" == "session-mismatch" || "$pattern" == "capacity-fault" ]]; then
  if [[ "$epf_load_epoch" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF bytecode image was not loaded" >&2
    exit 1
  fi
  if [[ "$epf_tap_spool_len" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: no TapEvent was retained for EPF" >&2
    exit 1
  fi
  if [[ "$epf_bytecode_state" != "4c4f4144" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF bytecode state is not Loaded" >&2
    exit 1
  fi
  if [[ "$epf_image_digest" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF image digest was not recorded" >&2
    exit 1
  fi
  if [[ "$epf_load_reason" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF load reason is not success" >&2
    exit 1
  fi
  if [[ "$epf_core1_epoch" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF marker epoch was not advanced on core1" >&2
    exit 1
  fi
  if [[ "$epf_core1_fuel" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF VM fuel was not consumed" >&2
    exit 1
  fi
fi

if [[ "$pattern" == "session-mismatch" ]]; then
  if [[ "$epf_core1_kind" != "00000205" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF evidence kind is not TransportMismatch" >&2
    exit 1
  fi
  if [[ "$epf_core1_reason" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF reason is not SessionMismatch" >&2
    exit 1
  fi
  epf_expected_session_dec="$((16#$epf_core1_arg0))"
  epf_observed_session_dec="$((16#$epf_core1_arg1))"
  if (( epf_expected_session_dec == 0 || epf_observed_session_dec == 0 )); then
    echo "Baker hardware pattern $pattern failed: EPF session ids were not recorded" >&2
    exit 1
  fi
  if (( (epf_expected_session_dec ^ 0x11110000) != epf_observed_session_dec )); then
    echo "Baker hardware pattern $pattern failed: observed session did not match the deliberate skew" >&2
    exit 1
  fi
  if [[ "$epf_core1_arg2" != "02050001" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF typed TransportMismatch/Session tag mismatch" >&2
    exit 1
  fi
fi

if [[ "$pattern" == "capacity-fault" ]]; then
  if [[ "$epf_core1_kind" != "00000207" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF evidence kind is not TransportFault" >&2
    exit 1
  fi
  if [[ "$epf_core1_reason" != "00000003" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF reason is not Capacity" >&2
    exit 1
  fi
  if [[ "$epf_core1_arg0" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF capacity site was not recorded" >&2
    exit 1
  fi
  if [[ "$epf_core1_arg1" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF capacity lane is not lane 0" >&2
    exit 1
  fi
  if [[ "$epf_core1_arg2" != "02070003" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF typed TransportFault/Capacity tag mismatch" >&2
    exit 1
  fi
fi

if [[ "$pattern" == "epf-policy-timer" ]]; then
  if [[ "$epf_load_epoch" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF policy image was not loaded" >&2
    exit 1
  fi
  if [[ "$epf_bytecode_state" != "4c4f4144" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF bytecode state is not Loaded" >&2
    exit 1
  fi
  if [[ "$epf_load_reason" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF load reason is not success" >&2
    exit 1
  fi
  if [[ "$epf_active_target_kind" != "00000001" || "$epf_active_policy_id" != "00000039" ]]; then
    echo "Baker hardware pattern $pattern failed: active EPF image is not Policy(57)" >&2
    exit 1
  fi
  if [[ "$epf_image_ingress" != "43484f52" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF policy image was not delivered by choreography" >&2
    exit 1
  fi
  if [[ "$epf_policy_timer_irq_ready" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: timer IRQ fact was not observed at resolver entry" >&2
    exit 1
  fi
  if [[ "$epf_policy_timer_fact_kind" != "00000057" || "$epf_policy_timer_fact_arg0" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF policy VM did not read the timer TapEvent fact" >&2
    exit 1
  fi
  if [[ "$epf_policy_timer_fact_fuel" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: EPF timer fact VM did not consume fuel" >&2
    exit 1
  fi
  if [[ "$epf_core0_epoch" == "00000000" || "$epf_core1_epoch" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: both cores did not record EPF observe output" >&2
    exit 1
  fi
  if [[ "$epf_core0_fuel" == "00000000" || "$epf_core1_fuel" == "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: both cores did not consume EPF VM fuel" >&2
    exit 1
  fi
fi

watchdog_tick_dec="$((16#$watchdog_tick))"
clk_ref_selected_dec="$((16#$clk_ref_selected))"
clk_sys_ctrl_dec="$((16#$clk_sys_ctrl))"
clk_sys_selected_dec="$((16#$clk_sys_selected))"
clk_peri_ctrl_dec="$((16#$clk_peri_ctrl))"
clk_peri_selected_dec="$((16#$clk_peri_selected))"
xosc_status_dec="$((16#$xosc_status))"
pll_sys_cs_dec="$((16#$pll_sys_cs))"
pll_sys_fbdiv_dec="$((16#$pll_sys_fbdiv))"
pll_sys_prim_dec="$((16#$pll_sys_prim))"
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
if (( (pll_sys_cs_dec & 0x80000000) == 0 || pll_sys_fbdiv_dec != 125 )); then
  echo "Baker hardware pattern $pattern failed: PLL_SYS is not locked at the 125MHz feedback divider" >&2
  exit 1
fi
if (( ((pll_sys_prim_dec >> 16) & 0x7) != 6 || ((pll_sys_prim_dec >> 12) & 0x7) != 2 )); then
  echo "Baker hardware pattern $pattern failed: PLL_SYS post dividers are not 6 and 2" >&2
  exit 1
fi
if (( (clk_sys_ctrl_dec & 0x1) != 1 || ((clk_sys_ctrl_dec >> 5) & 0x7) != 0 || (clk_sys_selected_dec & 0x2) == 0 )); then
  echo "Baker hardware pattern $pattern failed: clk_sys did not select PLL_SYS" >&2
  exit 1
fi
if (( (clk_peri_ctrl_dec & 0x800) == 0 || ((clk_peri_ctrl_dec >> 5) & 0x7) != 0 || (clk_peri_selected_dec & 0x1) == 0 )); then
  echo "Baker hardware pattern $pattern failed: clk_peri did not select clk_sys" >&2
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

require_nonzero_counter() {
  local name="$1"
  local value="$2"
  if (( 16#$value == 0 )); then
    echo "Baker hardware pattern $pattern failed: $name stayed zero" >&2
    exit 1
  fi
}

require_choreofs_sio_cross_core() {
  require_nonzero_counter choreofs_sio_core0_to_core1_tx "$choreofs_sio_core0_to_core1_tx"
  require_nonzero_counter choreofs_sio_core0_to_core1_rx "$choreofs_sio_core0_to_core1_rx"
  require_nonzero_counter choreofs_sio_core1_to_core0_tx "$choreofs_sio_core1_to_core0_tx"
  require_nonzero_counter choreofs_sio_core1_to_core0_rx "$choreofs_sio_core1_to_core0_rx"
  require_nonzero_counter choreofs_sio_tx_poll "$choreofs_sio_tx_poll_count"
  require_nonzero_counter choreofs_reply_send_fd_write_object "$choreofs_reply_send_fd_write_object_count"
  require_nonzero_counter choreofs_reply_send_poll_oneoff "$choreofs_reply_send_poll_oneoff_count"
}

require_appkit_wasi_metrics() {
  if [[ -z "$appkit_wasi_resume_count_addr" ]]; then
    echo "Baker hardware pattern $pattern failed: appkit WASI timing symbols are missing" >&2
    exit 1
  fi
  require_nonzero_counter appkit_wasi_resume "$appkit_wasi_resume_count"
  require_nonzero_counter appkit_wasi_request_send "$appkit_wasi_request_send_count"
  require_nonzero_counter appkit_wasi_completion_recv "$appkit_wasi_completion_recv_count"
  require_nonzero_counter appkit_wasi_complete "$appkit_wasi_complete_count"
}

require_choreofs_reply_future_poll_metrics() {
  if [[ -z "$choreofs_reply_send_future_poll_count_addr" ]]; then
    echo "Baker hardware pattern $pattern failed: reply-send future poll symbols are missing" >&2
    exit 1
  fi
  if [[ -z "$choreofs_reply_encode_count_addr" ]]; then
    echo "Baker hardware pattern $pattern failed: reply encode symbols are missing" >&2
    exit 1
  fi
  require_nonzero_counter choreofs_reply_send_future_poll "$choreofs_reply_send_future_poll_count"
  require_nonzero_counter choreofs_reply_encode "$choreofs_reply_encode_count"
}

if [[ "$pattern" == "choreofs-traffic" ]]; then
  require_choreofs_sio_cross_core
  require_appkit_wasi_metrics
  if [[ "$choreofs_engine_error_code" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: ChoreoFS error marker was set" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_fd_write_count" != "00000007" ]]; then
    echo "Baker hardware pattern $pattern failed: fd_write count mismatch" >&2
    exit 1
  fi
  if [[ "$choreofs_poll_count" != "00000007" ]]; then
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
  require_choreofs_sio_cross_core
  require_appkit_wasi_metrics
  require_choreofs_reply_future_poll_metrics
  if [[ "$choreofs_engine_error_code" != "00000000" ]]; then
    echo "Baker hardware pattern $pattern failed: ChoreoFS error marker was set" >&2
    exit 1
  fi
  if [[ "$choreofs_path_open_count" != "00000001" ]]; then
    echo "Baker hardware pattern $pattern failed: path_open count mismatch" >&2
    exit 1
  fi
  choreofs_fd_write_count_dec="$((16#$choreofs_fd_write_count))"
  choreofs_poll_count_dec="$((16#$choreofs_poll_count))"
  if (( choreofs_fd_write_count_dec < 7 )); then
    echo "Baker hardware pattern $pattern failed: fd_write count did not reach one visual cycle" >&2
    exit 1
  fi
  if (( choreofs_poll_count_dec < 7 )); then
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
