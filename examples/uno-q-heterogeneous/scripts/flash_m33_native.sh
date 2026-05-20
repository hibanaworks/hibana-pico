#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
ADB="${ADB:-adb}"
REMOTE_ELF="${UNO_Q_REMOTE_ELF:-/tmp/uno-q-m33-native-kernel.elf}"
OPENOCD="${UNO_Q_OPENOCD:-/opt/openocd/bin/openocd}"
OPENOCD_DIR="${UNO_Q_OPENOCD_DIR:-/opt/openocd}"

if [[ "${UNO_Q_SKIP_BUILD:-0}" == "1" ]]; then
  ELF="${REPO_ROOT}/target/thumbv8m.main-none-eabi/release/uno-q-m33-native-kernel"
else
  ELF="$("${SCRIPT_DIR}/build_m33_native.sh" | tail -n 1)"
fi

"${ADB}" push "${ELF}" "${REMOTE_ELF}"
"${ADB}" shell "${OPENOCD} -d0 -s ${OPENOCD_DIR} -f openocd_gpiod.cfg -c 'reset_config srst_only srst_push_pull; init; reset halt; program ${REMOTE_ELF} verify reset; shutdown'"
