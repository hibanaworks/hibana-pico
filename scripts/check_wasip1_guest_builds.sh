#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed | rg -q '^wasm32-wasip1$'; then
  echo "wasm32-wasip1 target is not installed; run: rustup target add wasm32-wasip1" >&2
  exit 1
fi

CARGO_TARGET_DIR="$ROOT/target/wasip1-apps" \
  cargo build \
    --manifest-path apps/wasip1/swarm-node-apps/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

CARGO_TARGET_DIR="$ROOT/target/wasip1-apps" \
  cargo build \
    --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
    --target wasm32-wasip1 \
    --release \
    --bins

embedded_led_bins=(
  wasip1-led-fd-write
  wasip1-led-blink
  wasip1-led-chaser
  wasip1-led-ordinary-std-chaser
  wasip1-led-bad-order
  wasip1-led-invalid-fd
  wasip1-led-bad-payload
  wasip1-led-choreofs-open
  wasip1-led-choreofs-bad-path
  wasip1-led-choreofs-bad-payload
  wasip1-led-choreofs-wrong-object
)

for bin in "${embedded_led_bins[@]}"; do
  CARGO_TARGET_DIR="$ROOT/target/wasip1-apps" \
  RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
    cargo build \
      --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
      --target wasm32-wasip1 \
      --release \
      --bin "$bin"
done

artifact_dir="$ROOT/target/wasip1-apps/wasm32-wasip1/release"
artifacts=(
  swarm-coordinator
  swarm-sensor
  swarm-actuator
  swarm-gateway
  wasip1-stdout
  wasip1-stderr
  wasip1-stdin
  wasip1-clock
  wasip1-random
  wasip1-exit
  wasip1-timer
  wasip1-trap
  wasip1-infinite-loop
  wasip1-led-fd-write
  wasip1-led-blink
  wasip1-led-chaser
  wasip1-led-bad-order
  wasip1-led-invalid-fd
  wasip1-led-bad-payload
  wasip1-led-choreofs-open
  wasip1-led-choreofs-bad-path
  wasip1-led-choreofs-bad-payload
  wasip1-led-choreofs-wrong-object
  wasip1-led-ordinary-std-chaser
  wasip1-std-core-coverage
  wasip1-std-choreofs-read
  wasip1-std-choreofs-append
  wasip1-std-bad-path
  wasip1-std-choreofs-static-write
  wasip1-std-sock-send-recv
  wasip1-std-sock-accept-send-recv
  wasip1-std-sock-accept-bad
  wasip1-std-stream-control
  wasip1-memory-grow-ok
  wasip1-memory-grow-stale-lease
)

for artifact in "${artifacts[@]}"; do
  wasm="$artifact_dir/$artifact.wasm"
  if [[ ! -s "$wasm" ]]; then
    echo "missing or empty WASI P1 guest artifact: $wasm" >&2
    exit 1
  fi
  if ! rg -a -q 'wasi_snapshot_preview1' "$wasm"; then
    echo "WASI P1 guest artifact lacks preview1 imports: $wasm" >&2
    exit 1
  fi
  if rg -a -q 'wasi:|wasm32-wasip2|wasip2|wasi_snapshot_preview2|preview2|wit-bindgen|wit_component|component-model' "$wasm"; then
    echo "WASI P1 guest artifact contains forbidden P2/WIT/Component surface: $wasm" >&2
    exit 1
  fi
done

if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-timer.wasm"; then
  echo "WASI P1 timer smoke artifact lacks poll_oneoff: $artifact_dir/wasip1-timer.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana wasip1 trap' "$artifact_dir/wasip1-trap.wasm"; then
  echo "WASI P1 trap smoke artifact lacks trap marker: $artifact_dir/wasip1-trap.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-fd-write.wasm"; then
  echo "WASI P1 LED fd_write artifact lacks fd_write: $artifact_dir/wasip1-led-fd-write.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-blink.wasm"; then
  echo "WASI P1 LED blink artifact lacks fd_write: $artifact_dir/wasip1-led-blink.wasm" >&2
  exit 1
fi
if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-led-blink.wasm"; then
  echo "WASI P1 LED blink artifact lacks poll_oneoff: $artifact_dir/wasip1-led-blink.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-chaser.wasm"; then
  echo "WASI P1 LED chaser artifact lacks fd_write: $artifact_dir/wasip1-led-chaser.wasm" >&2
  exit 1
fi
if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-led-chaser.wasm"; then
  echo "WASI P1 LED chaser artifact lacks poll_oneoff: $artifact_dir/wasip1-led-chaser.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-bad-order.wasm"; then
  echo "WASI P1 LED bad-order artifact lacks fd_write: $artifact_dir/wasip1-led-bad-order.wasm" >&2
  exit 1
fi
if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-led-bad-order.wasm"; then
  echo "WASI P1 LED bad-order artifact lacks poll_oneoff: $artifact_dir/wasip1-led-bad-order.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-invalid-fd.wasm"; then
  echo "WASI P1 LED invalid-fd artifact lacks fd_write: $artifact_dir/wasip1-led-invalid-fd.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-bad-payload.wasm"; then
  echo "WASI P1 LED bad-payload artifact lacks fd_write: $artifact_dir/wasip1-led-bad-payload.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-ordinary-std-chaser.wasm"; then
  echo "WASI P1 LED ordinary std chaser artifact lacks fd_write: $artifact_dir/wasip1-led-ordinary-std-chaser.wasm" >&2
  exit 1
fi
if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-led-ordinary-std-chaser.wasm"; then
  echo "WASI P1 LED ordinary std chaser artifact lacks poll_oneoff: $artifact_dir/wasip1-led-ordinary-std-chaser.wasm" >&2
  exit 1
fi
for choreofs_led in \
  wasip1-led-choreofs-open \
  wasip1-led-choreofs-bad-path \
  wasip1-led-choreofs-bad-payload \
  wasip1-led-choreofs-wrong-object
do
  if ! rg -a -q 'path_open' "$artifact_dir/$choreofs_led.wasm"; then
    echo "WASI P1 Baker ChoreoFS LED artifact lacks path_open: $artifact_dir/$choreofs_led.wasm" >&2
    exit 1
  fi
done
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-led-choreofs-open.wasm"; then
  echo "WASI P1 Baker ChoreoFS LED artifact lacks fd_write: $artifact_dir/wasip1-led-choreofs-open.wasm" >&2
  exit 1
fi
if ! rg -a -q 'poll_oneoff' "$artifact_dir/wasip1-led-choreofs-open.wasm"; then
  echo "WASI P1 Baker ChoreoFS LED artifact lacks poll_oneoff: $artifact_dir/wasip1-led-choreofs-open.wasm" >&2
  exit 1
fi
if rg -a -q 'hibana ordinary std wasip1 chaser' "$artifact_dir/wasip1-led-ordinary-std-chaser.wasm"; then
  echo "WASI P1 LED ordinary std chaser artifact must not rely on the old traffic marker: $artifact_dir/wasip1-led-ordinary-std-chaser.wasm" >&2
  exit 1
fi
if ! rg -a -q 'environ_get|args_get|proc_exit' "$artifact_dir/wasip1-led-ordinary-std-chaser.wasm"; then
  echo "WASI P1 LED ordinary std chaser artifact lacks Rust std WASI start imports: $artifact_dir/wasip1-led-ordinary-std-chaser.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana std core coverage' "$artifact_dir/wasip1-std-core-coverage.wasm"; then
  echo "WASI P1 std core coverage artifact lacks stdout marker: $artifact_dir/wasip1-std-core-coverage.wasm" >&2
  exit 1
fi
if ! rg -a -q 'memory.grow' "$artifact_dir/wasip1-std-core-coverage.wasm"; then
  echo "WASI P1 std core coverage artifact lacks memory.grow marker: $artifact_dir/wasip1-std-core-coverage.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-std-core-coverage.wasm"; then
  echo "WASI P1 std core coverage artifact lacks fd_write: $artifact_dir/wasip1-std-core-coverage.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-choreofs-read.wasm"; then
  echo "WASI P1 std ChoreoFS artifact lacks path_open: $artifact_dir/wasip1-std-choreofs-read.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_read' "$artifact_dir/wasip1-std-choreofs-read.wasm"; then
  echo "WASI P1 std ChoreoFS artifact lacks fd_read: $artifact_dir/wasip1-std-choreofs-read.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana choreofs read' "$artifact_dir/wasip1-std-choreofs-read.wasm"; then
  echo "WASI P1 std ChoreoFS artifact lacks stdout marker: $artifact_dir/wasip1-std-choreofs-read.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-choreofs-append.wasm"; then
  echo "WASI P1 std ChoreoFS append artifact lacks path_open: $artifact_dir/wasip1-std-choreofs-append.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_write' "$artifact_dir/wasip1-std-choreofs-append.wasm"; then
  echo "WASI P1 std ChoreoFS append artifact lacks fd_write: $artifact_dir/wasip1-std-choreofs-append.wasm" >&2
  exit 1
fi
if ! rg -a -q 'fd_read' "$artifact_dir/wasip1-std-choreofs-append.wasm"; then
  echo "WASI P1 std ChoreoFS append artifact lacks fd_read: $artifact_dir/wasip1-std-choreofs-append.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana choreofs append' "$artifact_dir/wasip1-std-choreofs-append.wasm"; then
  echo "WASI P1 std ChoreoFS append artifact lacks stdout marker: $artifact_dir/wasip1-std-choreofs-append.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-bad-path.wasm"; then
  echo "WASI P1 std bad-path artifact lacks path_open: $artifact_dir/wasip1-std-bad-path.wasm" >&2
  exit 1
fi
if ! rg -a -q 'forbidden path must reject' "$artifact_dir/wasip1-std-bad-path.wasm"; then
  echo "WASI P1 std bad-path artifact lacks typed-reject marker: $artifact_dir/wasip1-std-bad-path.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-choreofs-static-write.wasm"; then
  echo "WASI P1 std static-write artifact lacks path_open: $artifact_dir/wasip1-std-choreofs-static-write.wasm" >&2
  exit 1
fi
if ! rg -a -q 'readonly static write must reject' "$artifact_dir/wasip1-std-choreofs-static-write.wasm"; then
  echo "WASI P1 std static-write artifact lacks typed-reject marker: $artifact_dir/wasip1-std-choreofs-static-write.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-sock-send-recv.wasm"; then
  echo "WASI P1 std sock artifact lacks path_open: $artifact_dir/wasip1-std-sock-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_send' "$artifact_dir/wasip1-std-sock-send-recv.wasm"; then
  echo "WASI P1 std sock artifact lacks sock_send: $artifact_dir/wasip1-std-sock-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_recv' "$artifact_dir/wasip1-std-sock-send-recv.wasm"; then
  echo "WASI P1 std sock artifact lacks sock_recv: $artifact_dir/wasip1-std-sock-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_shutdown' "$artifact_dir/wasip1-std-sock-send-recv.wasm"; then
  echo "WASI P1 std sock artifact lacks sock_shutdown: $artifact_dir/wasip1-std-sock-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana network datagram ping pong' "$artifact_dir/wasip1-std-sock-send-recv.wasm"; then
  echo "WASI P1 std sock artifact lacks stdout marker: $artifact_dir/wasip1-std-sock-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_accept' "$artifact_dir/wasip1-std-sock-accept-send-recv.wasm"; then
  echo "WASI P1 std listener accept artifact lacks sock_accept: $artifact_dir/wasip1-std-sock-accept-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_send' "$artifact_dir/wasip1-std-sock-accept-send-recv.wasm"; then
  echo "WASI P1 std listener accept artifact lacks sock_send: $artifact_dir/wasip1-std-sock-accept-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_recv' "$artifact_dir/wasip1-std-sock-accept-send-recv.wasm"; then
  echo "WASI P1 std listener accept artifact lacks sock_recv: $artifact_dir/wasip1-std-sock-accept-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_shutdown' "$artifact_dir/wasip1-std-sock-accept-send-recv.wasm"; then
  echo "WASI P1 std listener accept artifact lacks sock_shutdown: $artifact_dir/wasip1-std-sock-accept-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana listener accept fd ping pong' "$artifact_dir/wasip1-std-sock-accept-send-recv.wasm"; then
  echo "WASI P1 std listener accept artifact lacks stdout marker: $artifact_dir/wasip1-std-sock-accept-send-recv.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_accept' "$artifact_dir/wasip1-std-sock-accept-bad.wasm"; then
  echo "WASI P1 std bad sock artifact lacks sock_accept: $artifact_dir/wasip1-std-sock-accept-bad.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_accept must reject' "$artifact_dir/wasip1-std-sock-accept-bad.wasm"; then
  echo "WASI P1 std bad sock artifact lacks typed-reject marker: $artifact_dir/wasip1-std-sock-accept-bad.wasm" >&2
  exit 1
fi
if ! rg -a -q 'path_open' "$artifact_dir/wasip1-std-stream-control.wasm"; then
  echo "WASI P1 std stream control artifact lacks path_open: $artifact_dir/wasip1-std-stream-control.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_send' "$artifact_dir/wasip1-std-stream-control.wasm"; then
  echo "WASI P1 std stream control artifact lacks sock_send: $artifact_dir/wasip1-std-stream-control.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_recv' "$artifact_dir/wasip1-std-stream-control.wasm"; then
  echo "WASI P1 std stream control artifact lacks sock_recv: $artifact_dir/wasip1-std-stream-control.wasm" >&2
  exit 1
fi
if ! rg -a -q 'sock_shutdown' "$artifact_dir/wasip1-std-stream-control.wasm"; then
  echo "WASI P1 std stream control artifact lacks sock_shutdown: $artifact_dir/wasip1-std-stream-control.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana network stream control ping pong' "$artifact_dir/wasip1-std-stream-control.wasm"; then
  echo "WASI P1 std stream control artifact lacks stdout marker: $artifact_dir/wasip1-std-stream-control.wasm" >&2
  exit 1
fi
if rg -q '#!\[no_main\]|__main_void' apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-led-ordinary-std-chaser.rs; then
  echo "WASI P1 LED ordinary std chaser source must be ordinary fn main without an extra __main_void trampoline" >&2
  exit 1
fi
if rg -q '#!\[no_main\]|__main_void' apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-core-coverage.rs; then
  echo "WASI P1 std core coverage source must be ordinary fn main without an extra __main_void trampoline" >&2
  exit 1
fi
if ! rg -a -q 'hibana wasip1 memory grow ok' "$artifact_dir/wasip1-memory-grow-ok.wasm"; then
  echo "WASI P1 memory grow ok artifact lacks marker: $artifact_dir/wasip1-memory-grow-ok.wasm" >&2
  exit 1
fi
if ! rg -a -q 'hibana memgrow stale lease' "$artifact_dir/wasip1-memory-grow-stale-lease.wasm"; then
  echo "WASI P1 memory grow stale lease artifact lacks marker: $artifact_dir/wasip1-memory-grow-stale-lease.wasm" >&2
  exit 1
fi
if ! rg -a -q 'memory.grow' "$artifact_dir/wasip1-memory-grow-ok.wasm"; then
  echo "WASI P1 memory grow ok artifact lacks memory.grow name marker: $artifact_dir/wasip1-memory-grow-ok.wasm" >&2
  exit 1
fi
if ! rg -a -q 'memory.grow' "$artifact_dir/wasip1-memory-grow-stale-lease.wasm"; then
  echo "WASI P1 memory grow stale lease artifact lacks memory.grow name marker: $artifact_dir/wasip1-memory-grow-stale-lease.wasm" >&2
  exit 1
fi

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    rust_built_wasip1_smoke_artifacts_cover_timer_trap_and_infinite_loop \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    rust_built_swarm_wasip1_artifacts_exercise_localside_choreography \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    rust_built_swarm_wasip1_artifacts_exercise_one_global_swarm_choreography \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    rust_built_wasip1_artifact_installs_as_hotswap_image_and_requires_fence \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    rust_built_wasip1_memory_grow_artifacts_exercise_fence_and_stale_lease_rejection \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_baker_led_fd \
    --features baker-ordinary-std-demo \
    baker_link_ordinary_std_wasip1_app_fits_embedded_std_start_profile_when_sized \
    -- \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_ordinary_std_core_coverage_runs_on_host_full_profile \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_std_choreofs_app_uses_resource_store_through_host_full_runner \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_std_choreofs_append_app_writes_and_reads_resource_store \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_bad_std_path_app_rejects_before_hidden_host_fs \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_bad_std_static_write_rejects_at_choreofs_control \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_std_sock_app_uses_network_object_without_p2 \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_std_sock_accept_app_mints_network_object_without_socket_authority \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_std_stream_control_app_uses_network_object_without_socket_authority \
    -- \
    --ignored \
    --exact

HIBANA_WASIP1_GUEST_DIR="$artifact_dir" \
  cargo test \
    --test host_wasip1_artifacts \
    --features profile-host-linux-wasip1-full \
    rust_built_bad_std_sock_accept_rejects_without_listener_route \
    -- \
    --ignored \
    --exact

echo "wasip1 swarm guest builds ok"
