#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

QEMU_BIN="${1:-${QEMU_BIN:-}}"
if [[ -z "$QEMU_BIN" ]]; then
  echo "usage: $0 /path/to/qemu-system-arm" >&2
  echo "or set QEMU_BIN=/path/to/qemu-system-arm" >&2
  exit 1
fi
if [[ ! -x "$QEMU_BIN" ]]; then
  echo "qemu-system-arm is not executable: $QEMU_BIN" >&2
  exit 1
fi
if ! "$QEMU_BIN" -machine help 2>/dev/null | grep -q "raspberrypi-pico2w"; then
  echo "qemu-system-arm does not expose the patched raspberrypi-pico2w machine: $QEMU_BIN" >&2
  echo "apply qemu/patches and qemu/overlay to an upstream QEMU checkout, then pass that build here" >&2
  exit 1
fi

NODE_COUNT="${HIBANA_PICO_SWARM_NODES:-6}"
if (( NODE_COUNT < 2 || NODE_COUNT > 6 )); then
  echo "HIBANA_PICO_SWARM_NODES must be between 2 and 6" >&2
  exit 1
fi
if [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" == 1 && "$NODE_COUNT" != 6 ]]; then
  echo "HIBANA_PICO_MINIMAL_KERNELS=1 currently targets the default 6-node swarm" >&2
  exit 1
fi

if [[ "${HIBANA_PICO_SKIP_BUILD:-0}" != 1 ]]; then
  bash ./scripts/check_wasip1_guest_builds.sh
  if [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" == 1 ]]; then
    cargo build \
      --target thumbv8m.main-none-eabi \
      --release \
      --bin hibana-pico2w-swarm-coordinator-6 \
      --bin hibana-pico2w-swarm-sensor-2 \
      --bin hibana-pico2w-swarm-sensor-3 \
      --bin hibana-pico2w-swarm-sensor-4 \
      --bin hibana-pico2w-swarm-sensor-5 \
      --bin hibana-pico2w-swarm-sensor-6 \
      --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
  elif [[ "${HIBANA_PICO_SPLIT_KERNELS:-0}" == 1 ]]; then
    cargo build \
      --target thumbv8m.main-none-eabi \
      --release \
      --bin hibana-pico2w-swarm-coordinator \
      --bin hibana-pico2w-swarm-sensor \
      --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
  else
    cargo build \
      --target thumbv8m.main-none-eabi \
      --release \
      --bin hibana-pico2w-swarm-demo \
      --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
  fi
fi

DEFAULT_KERNEL="$ROOT/target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-demo"
DEFAULT_COORD_KERNEL="$DEFAULT_KERNEL"
DEFAULT_SENSOR_KERNEL="$DEFAULT_KERNEL"
if [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" == 1 ]]; then
  DEFAULT_COORD_KERNEL="$ROOT/target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-coordinator-6"
  DEFAULT_SENSOR_KERNEL=
elif [[ "${HIBANA_PICO_SPLIT_KERNELS:-0}" == 1 ]]; then
  DEFAULT_COORD_KERNEL="$ROOT/target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-coordinator"
  DEFAULT_SENSOR_KERNEL="$ROOT/target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-sensor"
fi
COORD_KERNEL="${HIBANA_PICO_COORD_KERNEL:-${HIBANA_PICO_KERNEL:-$DEFAULT_COORD_KERNEL}}"
SENSOR_KERNEL="${HIBANA_PICO_SENSOR_KERNEL:-${HIBANA_PICO_KERNEL:-$DEFAULT_SENSOR_KERNEL}}"
if [[ ! -f "$COORD_KERNEL" ]]; then
  echo "missing Pico 2 W coordinator kernel: $COORD_KERNEL" >&2
  exit 1
fi
if [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" != 1 && ! -f "$SENSOR_KERNEL" ]]; then
  echo "missing Pico 2 W sensor kernel: $SENSOR_KERNEL" >&2
  exit 1
fi

TIMEOUT_BIN="${TIMEOUT_BIN:-}"
if [[ -z "$TIMEOUT_BIN" ]]; then
  if command -v timeout >/dev/null 2>&1; then
    TIMEOUT_BIN="$(command -v timeout)"
  elif command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_BIN="$(command -v gtimeout)"
  else
    echo "requires timeout or gtimeout; install coreutils on macOS" >&2
    exit 1
  fi
fi

RUN_TIMEOUT="${HIBANA_PICO_QEMU_TIMEOUT:-10s}"
PORT_BASE="${HIBANA_PICO_PORT_BASE:-39000}"
SENSOR_BOOT_WAIT="${HIBANA_PICO_SENSOR_BOOT_WAIT:-0.5}"
MESH_SPOOF_PROBE="${HIBANA_PICO_QEMU_MESH_SPOOF_PROBE:-1}"
MESH_SPOOF_PROBE_TIMEOUT="${HIBANA_PICO_QEMU_MESH_SPOOF_PROBE_TIMEOUT:-5}"
LOG_DIR="${HIBANA_PICO_LOG_DIR:-$(mktemp -d /tmp/hibana-pico2w-swarm.XXXXXX)}"
mkdir -p "$LOG_DIR"

COORD_LOG="$LOG_DIR/coordinator.log"
: > "$COORD_LOG"

run_node() {
  local role="$1"
  local node_id="$2"
  local log="$3"
  local kernel="$4"
  local qemu_log_args=()

  if [[ "$MESH_SPOOF_PROBE" == 1 ]]; then
    qemu_log_args=(-d guest_errors -D "$log.qemu")
    : > "$log.qemu"
  fi

  "$TIMEOUT_BIN" "$RUN_TIMEOUT" "$QEMU_BIN" \
    "${qemu_log_args[@]}" \
    -display none \
    -serial stdio \
    -monitor none \
    -M raspberrypi-pico2w \
    -global "cyw43439-wifi.radio-port-base=$PORT_BASE" \
    -global "cyw43439-wifi.node-role=$role" \
    -global "cyw43439-wifi.node-id=$node_id" \
    -global "cyw43439-wifi.node-count=$NODE_COUNT" \
    -kernel "$kernel" \
    > "$log" 2>&1
}

inject_mesh_spoof_probe() {
  if [[ "$MESH_SPOOF_PROBE" != 1 || "$NODE_COUNT" -lt 4 ]]; then
    return
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required for QEMU mesh spoof probe" >&2
    exit 1
  fi

  if ! python3 - "$PORT_BASE" "$LOG_DIR/sensor-4.log.qemu" "$MESH_SPOOF_PROBE_TIMEOUT" <<'PY'
import socket
import sys
import time

port_base = int(sys.argv[1])
qemu_log = sys.argv[2]
timeout = float(sys.argv[3])
dst_node = 4
expected = (
    "mesh frame node mismatch",
    "mesh packet dst",
)
expected_alias = "mesh source address is not loopback peer"

alias_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    alias_sock.bind(("127.0.0.2", port_base + 1))
except OSError as error:
    print(
        f"warning: cannot bind 127.0.0.2:{port_base + 1} for source-address spoof probe: {error}",
        file=sys.stderr,
    )
    if sys.platform == "darwin":
        print("warning: skipping QEMU mesh spoof probe on Darwin loopback alias limits", file=sys.stderr)
        with open(f"{qemu_log}.probe-skipped", "w", encoding="utf-8") as marker:
            marker.write("darwin-loopback-alias\n")
        sys.exit(0)
    alias_sock = None
else:
    expected = (expected_alias,) + expected

spoof_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    spoof_sock.bind(("127.0.0.1", port_base + 1))
except OSError as error:
    print(f"cannot bind 127.0.0.1:{port_base + 1} before coordinator start: {error}", file=sys.stderr)
    sys.exit(1)

def packet(packet_dst, frame_src, frame_dst):
    return bytes([
        packet_dst, 6,
        0, 0,
        frame_src >> 8, frame_src & 0xff,
        frame_dst >> 8, frame_dst & 0xff,
    ])

probes = (
    (spoof_sock, packet(dst_node, 2, dst_node)),
    (spoof_sock, packet(dst_node + 1, 1, dst_node + 1)),
)
if alias_sock is not None:
    probes = ((alias_sock, packet(dst_node, 1, dst_node)),) + probes
deadline = time.monotonic() + timeout
while time.monotonic() < deadline:
    for sock, payload in probes:
        sock.sendto(payload, ("127.0.0.1", port_base + dst_node))
    time.sleep(0.05)
    try:
        with open(qemu_log, "r", encoding="utf-8") as log:
            content = log.read()
            if all(rejection in content for rejection in expected):
                sys.exit(0)
    except FileNotFoundError:
        pass

sys.exit(1)
PY
  then
    echo "failed to inject QEMU mesh spoof probe" >&2
    exit 1
  fi
}

sensor_kernel_for() {
  local node_id="$1"
  local var="HIBANA_PICO_SENSOR_${node_id}_KERNEL"
  local override="${!var:-}"
  if [[ -n "$override" ]]; then
    printf '%s\n' "$override"
  elif [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" == 1 ]]; then
    printf '%s\n' "$ROOT/target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-sensor-$node_id"
  else
    printf '%s\n' "$SENSOR_KERNEL"
  fi
}

set +e
sensor_pids=()
for node_id in $(seq 2 "$NODE_COUNT"); do
  sensor_log="$LOG_DIR/sensor-$node_id.log"
  : > "$sensor_log"
  node_kernel="$(sensor_kernel_for "$node_id")"
  if [[ ! -f "$node_kernel" ]]; then
    echo "missing Pico 2 W sensor-$node_id kernel: $node_kernel" >&2
    exit 1
  fi
  run_node 1 "$node_id" "$sensor_log" "$node_kernel" &
  sensor_pids+=("$!")
done
sleep "$SENSOR_BOOT_WAIT"
inject_mesh_spoof_probe
run_node 0 1 "$COORD_LOG" "$COORD_KERNEL" &
coord_pid=$!
wait "$coord_pid"
coord_status=$?
sensor_status=0
for pid in "${sensor_pids[@]}"; do
  wait "$pid"
  status=$?
  if (( status != 124 && status != 0 )); then
    sensor_status="$status"
  fi
done
set -e

printf 'logs: %s\n' "$LOG_DIR"
printf 'node count: %s\n' "$NODE_COUNT"
printf 'radio port base: %s\n' "$PORT_BASE"
printf 'coordinator kernel: %s\n' "$COORD_KERNEL"
if [[ "${HIBANA_PICO_MINIMAL_KERNELS:-0}" == 1 ]]; then
  printf '%s\n' 'sensor kernels: per-node minimal projection images'
else
  printf 'sensor kernel: %s\n' "$SENSOR_KERNEL"
fi
printf 'coordinator status: %s\n' "$coord_status"
printf 'sensor aggregate status: %s\n' "$sensor_status"
printf '%s\n' '--- coordinator ---'
cat "$COORD_LOG"
for node_id in $(seq 2 "$NODE_COUNT"); do
  printf '%s\n' "--- sensor-$node_id ---"
  cat "$LOG_DIR/sensor-$node_id.log"
done

if ! grep -q "hibana pico2w cyw43439 swarm ok" "$COORD_LOG"; then
  echo "missing coordinator success line" >&2
  exit 1
fi
if ! grep -q "completed sensors 0x$(printf '%08x' "$((NODE_COUNT - 1))")" "$COORD_LOG"; then
  echo "missing coordinator completed sensor count" >&2
  exit 1
fi
aggregate=0
for node_id in $(seq 2 "$NODE_COUNT"); do
  aggregate=$((aggregate + 0x0000a5a5 + node_id - 2))
done
aggregate_hex="$(printf '0x%08x' "$aggregate")"
if ! grep -q "swarm aggregate $aggregate_hex" "$COORD_LOG"; then
  echo "missing coordinator aggregate line $aggregate_hex" >&2
  exit 1
fi
for node_id in $(seq 2 "$NODE_COUNT"); do
  value=$((0x0000a5a5 + node_id - 2))
  value_hex="$(printf '0x%08x' "$value")"
  sensor_log="$LOG_DIR/sensor-$node_id.log"
  if ! grep -q "sent sample $value_hex" "$sensor_log"; then
    echo "missing sensor-$node_id reply line $value_hex" >&2
    exit 1
  fi
  if ! grep -q "wasip1 guest fd_write done" "$sensor_log"; then
    echo "missing sensor-$node_id WASI guest fd_write line" >&2
    exit 1
  fi
  if ! grep -q "aggregate accepted $aggregate_hex" "$sensor_log"; then
    echo "missing sensor-$node_id aggregate accepted line $aggregate_hex" >&2
    exit 1
  fi
  if ! grep -q "wasip1 guest exchange done node 0x$(printf '%08x' "$node_id")" "$COORD_LOG"; then
    echo "missing coordinator WASI guest exchange line for node $node_id" >&2
    exit 1
  fi
  if ! grep -q "wasip1 fd_write node 0x$(printf '%08x' "$node_id"): hibana swarm sensor" "$COORD_LOG"; then
    echo "missing coordinator Rust-built sensor WASI P1 artifact marker for node $node_id" >&2
    exit 1
  fi
done

if [[ "$NODE_COUNT" == 6 ]]; then
  if ! grep -q "remote actuator route ack node 0x00000003" "$COORD_LOG"; then
    echo "missing coordinator remote actuator route ack line" >&2
    exit 1
  fi
  if ! grep -q "network datagram fd ack node 0x00000004" "$COORD_LOG"; then
    echo "missing coordinator network datagram fd ack line" >&2
    exit 1
  fi
  if ! grep -q "network stream fd ack node 0x00000004" "$COORD_LOG"; then
    echo "missing coordinator network stream fd ack line" >&2
    exit 1
  fi
  if ! grep -q "remote management image updated node 0x00000005" "$COORD_LOG"; then
    echo "missing coordinator remote management image update line" >&2
    exit 1
  fi

  if ! grep -q "actuator set value" "$LOG_DIR/sensor-3.log"; then
    echo "missing sensor-3 actuator line" >&2
    exit 1
  fi
  if ! grep -q "gateway telemetry sent node 0x00000003" "$LOG_DIR/sensor-3.log"; then
    echo "missing sensor-3 gateway telemetry line" >&2
    exit 1
  fi
  if ! grep -q "gateway telemetry accepted node 0x00000003" "$LOG_DIR/sensor-4.log"; then
    echo "missing sensor-4 gateway telemetry acceptance line" >&2
    exit 1
  fi
  if ! grep -q "network datagram fd accepted" "$LOG_DIR/sensor-4.log"; then
    echo "missing sensor-4 network datagram fd line" >&2
    exit 1
  fi
  if ! grep -q "network stream fd accepted" "$LOG_DIR/sensor-4.log"; then
    echo "missing sensor-4 network stream fd line" >&2
    exit 1
  fi
  if [[ "$MESH_SPOOF_PROBE" == 1 && ! -f "$LOG_DIR/sensor-4.log.qemu.probe-skipped" ]]; then
    if ! grep -q "mesh source address is not loopback peer" "$LOG_DIR/sensor-4.log.qemu"; then
      echo "missing sensor-4 QEMU mesh alias-source rejection line" >&2
      exit 1
    fi
    if ! grep -q "mesh frame node mismatch" "$LOG_DIR/sensor-4.log.qemu"; then
      echo "missing sensor-4 QEMU mesh frame-source rejection line" >&2
      exit 1
    fi
    if ! grep -q "mesh packet dst" "$LOG_DIR/sensor-4.log.qemu"; then
      echo "missing sensor-4 QEMU mesh packet-destination rejection line" >&2
      exit 1
    fi
  fi
  if ! grep -q "remote management image activated" "$LOG_DIR/sensor-5.log"; then
    echo "missing sensor-5 remote management activation line" >&2
    exit 1
  fi
fi
