#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo test --test host_measurement_gates
cargo test --test host_feature_profiles
cargo test --test host_feature_profiles --features profile-rp2040-pico-min
cargo test --test host_feature_profiles --features profile-rp2040-picow-swarm-min
cargo test --test host_feature_profiles --features profile-rp2350-pico2w-swarm-min
cargo test --test host_feature_profiles --features profile-host-linux-wasip1-full
cargo test --manifest-path apps/wasip1/hibana-wasi-guest/Cargo.toml
HIBANA_PICO_ENFORCE_PRACTICAL=1 bash ./scripts/check_pico_demo_budget.sh
cargo test --lib kernel::budget::tests
cargo test --lib kernel::engine::wasm::vm::tests::fuel_exhaustion_becomes_budget_expired_event
cargo test --lib kernel::resolver::tests::resolved_ready_facts_are_consumed_once
cargo test --lib kernel::resolver::tests::budget_timer_expiry_is_readiness_not_direct_kill
cargo test --lib kernel::resolver::tests::gpio_wait_fence_revokes_subscription_before_old_edge_can_progress
cargo test --lib kernel::network::tests::network_object_table_revoke_and_quiesce_reject
cargo test --lib kernel::network::tests::authenticated_network_object_control_rejects_forged_or_stale_grants
cargo test --lib kernel::policy::tests::policy_slot_table_requires_explicit_allowed_slot
cargo test --lib choreography::protocol::tests::route_control_arm_ids_are_distinct_and_scope_preserving
cargo test --lib choreography::protocol::tests::management_payloads_round_trip
cargo test --lib kernel::mgmt::tests::image_activation_boundary_requires_interrupt_and_remote_quiescence
cargo test --lib kernel::mgmt::tests::authenticated_management_install_rejects_forged_or_stale_grants
cargo test --lib kernel::remote::tests::authenticated_remote_object_control_rejects_forged_or_stale_grants
cargo test --lib machine::rp2040::baker_link::tests
cargo test --lib kernel::wasi::tests::pico_fd_view_rejects_invalid_stale_closed_and_wrong_rights
cargo test --lib kernel::wasi::tests::pico_fd_view_tracks_interrupt_subscription_control_grant
cargo test --lib kernel::wasi::tests::pico_fd_view_tracks_gateway_route_metadata
cargo test --lib kernel::wasi::tests::memory_lease_table_memory_grow_fence_rejects_stale_read_write_and_old_epoch
cargo test --lib kernel::engine::wasm::vm::tests::fuel_exhaustion_becomes_budget_expired_event
cargo test --lib kernel::resolver::tests::timer_irq_resolves_only_after_a_matching_sleep_request_is_due
cargo test --lib kernel::resolver::tests::gpio_edges_reject_until_a_wait_is_registered
cargo test --test host_management_hotswap host_backend_management_install_requires_mem_fence_before_activate
cargo test --test host_baker_led_fd baker_link_abort
cargo test --test host_swarm_plan swarm_fragmentation_is_explicit_bounded_and_reassembles_secure_frames
cargo test --test host_swarm_plan swarm_auth_and_replay_failures_drop_and_update_telemetry_without_payload_authority
cargo test --test host_swarm_plan swarm_transport_copies_payload_and_does_not_share_node_memory
cargo test --test host_swarm_plan two_node_wifi_ping_pong_is_wired_through_hibana_over_swarm_transport
cargo test --test host_swarm_plan phone_local_provisioning_triggers_wifi_join_but_swarm_grant_is_runtime_authority
cargo test --test host_swarm_plan ble_provisioning_installs_local_config_but_swarm_join_remains_runtime_authority
cargo test --test host_swarm_plan swarm_leave_revoke_choreography_quiesces_objects_leases_and_neighbors
cargo test --test host_swarm_plan swarm_policy_route_selects_app_scope_from_budget_telemetry
cargo test --test host_swarm_plan wifi_packet_loss_does_not_create_semantic_fallback_and_requires_explicit_redelivery
cargo test --test host_swarm_plan one_choreography_connects_all_swarm_nodes_with_sample_wasi_and_aggregate
cargo test --test host_swarm_plan six_process_swarm_choreography_connects_coordinator_and_five_sensors
cargo test --test host_swarm_plan --features profile-host-qemu-swarm production_qemu_swarm_routes_network_objects_over_swarm_transport
cargo test --test host_swarm_plan one_choreography_connects_sensor_actuator_and_gateway_telemetry
cargo test --test host_swarm_plan swarm_wrong_payload_and_wrong_localside_label_reject
cargo test --test host_swarm_plan remote_object_control_selects_explicit_route_arm_without_bridge
cargo test --test host_swarm_plan remote_management_object_control_selects_management_route_arm_without_bridge
cargo test --test host_swarm_plan remote_telemetry_object_control_selects_telemetry_route_arm_without_bridge
cargo test --test host_swarm_plan wasi_fd_selects_network_datagram_route_without_p2_or_bridge
cargo test --test host_swarm_plan wasi_fd_selects_network_stream_route_without_p2_or_bridge
cargo test --test host_swarm_plan remote_management_image_install_is_wired_through_hibana_over_swarm_transport
cargo test --test host_swarm_plan remote_management_rejects_bad_image_over_swarm_transport
cargo test --test host_swarm_plan remote_management_activation_emits_node_image_update_to_swarm_observer
cargo test --test host_wasip1_syscalls generic_wasi_fd_read_stat_close_are_lease_and_choreography_wired
cargo test --test host_wasip1_syscalls generic_wasi_poll_oneoff_is_choreography_wired
cargo test --test host_baker_led_fd
bash ./scripts/check_wasip1_guest_builds.sh
cargo build \
  --target thumbv6m-none-eabi \
  --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts"
for baker_choreofs_feature in \
  baker-choreofs-demo \
  baker-choreofs-bad-path-demo \
  baker-choreofs-bad-payload-demo \
  baker-choreofs-wrong-object-demo
do
  cargo build \
    --target thumbv6m-none-eabi \
    --release \
    --bin hibana-pico-baker-led-demo \
    --features "profile-rp2040-pico-min embed-wasip1-artifacts ${baker_choreofs_feature}"
done
cargo build \
  --target thumbv6m-none-eabi \
  --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-control-min baker-abort-safe-demo"
cargo build \
  --target thumbv6m-none-eabi \
  --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-control-min baker-recoverable-abort-demo"
cargo build \
  --target thumbv8m.main-none-eabi \
  --release \
  --bin hibana-pico2w-swarm-demo \
  --bin hibana-pico2w-swarm-coordinator \
  --bin hibana-pico2w-swarm-sensor \
  --bin hibana-pico2w-swarm-coordinator-6 \
  --bin hibana-pico2w-swarm-sensor-2 \
  --bin hibana-pico2w-swarm-sensor-3 \
  --bin hibana-pico2w-swarm-sensor-4 \
  --bin hibana-pico2w-swarm-sensor-5 \
  --bin hibana-pico2w-swarm-sensor-6 \
  --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"

if [[ "${HIBANA_PICO_SKIP_QEMU_SWARM:-0}" != 1 ]]; then
  qemu_swarm_bin="${HIBANA_PICO_QEMU_BIN:-${QEMU_BIN:-}}"
  if [[ -z "$qemu_swarm_bin" ]]; then
    for candidate in \
      ../qemu-rp2040/build/qemu-system-arm \
      ../qemu-upstream/build/qemu-system-arm
    do
      if [[ -x "$candidate" ]]; then
        qemu_swarm_bin="$candidate"
        break
      fi
    done
  fi

  if [[ -z "$qemu_swarm_bin" || ! -x "$qemu_swarm_bin" ]]; then
    echo "plan_pico gate failed: missing patched qemu-system-arm for 6-process Pico 2 W swarm runner" >&2
    echo "set HIBANA_PICO_QEMU_BIN=/path/to/qemu-system-arm or QEMU_BIN=/path/to/qemu-system-arm" >&2
    echo "set HIBANA_PICO_SKIP_QEMU_SWARM=1 only for non-QEMU local environments" >&2
    exit 1
  fi

  HIBANA_PICO_SKIP_BUILD=1 \
  HIBANA_PICO_MINIMAL_KERNELS=1 \
  HIBANA_PICO_SWARM_NODES=6 \
    bash ./scripts/run_pico2w_swarm_qemu.sh "$qemu_swarm_bin"
fi

search_paths=(Cargo.toml README.md src apps)
if [ -d fixtures ]; then
  search_paths+=(fixtures)
fi

if rg -n -S 'wasi:(cli|clocks|filesystem|http|io|random|sockets)|wasi/|wasm32-wasip2|wasip2|wasi_snapshot_preview2|preview2|wit-bindgen|wit_component|component-model' \
  "${search_paths[@]}"; then
  echo "plan_pico gate failed: found forbidden WASI P2 / WIT / Component Model runtime surface" >&2
  exit 1
fi

if rg -n -S 'PicoBridge|BridgeAdvance|typed phase bridge|bridge_state_size|host_bridge|send_packet_to_remote|fd\.is_remote\(' \
  Cargo.toml README.md src; then
  echo "plan_pico gate failed: found forbidden bridge/relay runtime surface" >&2
  exit 1
fi

if rg -n -S 'PicoWasiBridge' plan.md README.md src; then
  echo "plan_pico gate failed: Pico WASI import owner must not use bridge naming" >&2
  exit 1
fi

echo "plan_pico gates ok"
