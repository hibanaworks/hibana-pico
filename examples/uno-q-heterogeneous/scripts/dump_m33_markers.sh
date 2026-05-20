#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

ADB="${ADB:-adb}"
OPENOCD="${UNO_Q_OPENOCD:-/opt/openocd/bin/openocd}"
OPENOCD_DIR="${UNO_Q_OPENOCD_DIR:-/opt/openocd}"
ELF="${UNO_Q_M33_ELF:-${REPO_ROOT}/target/thumbv8m.main-none-eabi/release/uno-q-m33-native-kernel}"
RUN_MS="${UNO_Q_M33_MARKER_RUN_MS:-750}"
RESET="${UNO_Q_M33_MARKER_RESET:-1}"

HOST="$(rustc -vV | sed -n 's/^host: //p')"
NM="${UNO_Q_RUST_NM:-$(rustc --print sysroot)/lib/rustlib/${HOST}/bin/llvm-nm}"
if [[ ! -x "${NM}" ]]; then
  NM="$(command -v rust-nm)"
fi

if [[ ! -f "${ELF}" ]]; then
  echo "missing M33 ELF: ${ELF}" >&2
  echo "run examples/uno-q-heterogeneous/scripts/build_m33_native.sh first" >&2
  exit 1
fi

markers=(
  HIBANA_M33_BOOT_STAGE
  HIBANA_M33_TIMER_TICKS
  HIBANA_M33_SCAN_TICKS
  HIBANA_M33_SCAN_INDEX
  HIBANA_M33_LIT_TICKS
  HIBANA_M33_LAST_LIT_LED
  HIBANA_M33_MATRIX_WORD0
  HIBANA_M33_MATRIX_BITS
  HIBANA_M33_FACE_UPDATES
  HIBANA_M33_LAST_FACE
  HIBANA_M33_BOARD_POLLS
  HIBANA_M33_ROLE_STEP
  HIBANA_M33_PANIC_LINE
  HIBANA_M33_PANIC_COLUMN
  HIBANA_M33_USART1_RX_BYTES
  HIBANA_M33_LPUART1_RX_BYTES
  HIBANA_M33_USART1_TX_BYTES
  HIBANA_M33_LPUART1_TX_BYTES
  HIBANA_M33_LAST_RX_UART
  HIBANA_M33_TX_READY_MASK
  HIBANA_M33_USART1_ISR
  HIBANA_M33_LPUART1_ISR
  HIBANA_M33_USART1_ORE
  HIBANA_M33_LPUART1_ORE
  HIBANA_M33_RX_RING_PUMPED
  HIBANA_M33_RX_RING_DROPS
  HIBANA_M33_HINT_POLLS
  HIBANA_M33_LAST_HINT_LANE
  HIBANA_M33_RX_BYTES
  HIBANA_M33_RX_FRAMES
  HIBANA_M33_TX_FRAMES
  HIBANA_M33_LAST_RX
  HIBANA_M33_LAST_RX_PAYLOAD01
  HIBANA_M33_LAST_TX
)

nm_output="$("${NM}" -P "${ELF}")"
read_cmds=""
printf 'M33 marker addresses from %s\n' "${ELF}"
for marker in "${markers[@]}"; do
  addr="$(awk -v marker="${marker}" '$1 == marker { print $3; exit }' <<<"${nm_output}")"
  if [[ -z "${addr}" ]]; then
    echo "missing marker symbol: ${marker}" >&2
    exit 1
  fi
  printf '  %-28s 0x%s\n' "${marker}" "${addr}"
  read_cmds+="echo ${marker}=[read_memory 0x${addr} 32 1]; "
done

if [[ "${RESET}" == "1" ]]; then
  openocd_cmd="reset_config srst_only srst_push_pull; init; reset run; sleep ${RUN_MS}; halt; ${read_cmds} resume; shutdown"
else
  openocd_cmd="reset_config srst_only srst_push_pull; init; halt; ${read_cmds} resume; shutdown"
fi

"${ADB}" shell "${OPENOCD} -d0 -s ${OPENOCD_DIR} -f openocd_gpiod.cfg -c '${openocd_cmd}'"
