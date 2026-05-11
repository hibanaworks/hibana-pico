use hibana_pico::kernel::metrics::{NO_P2_METRICS, SINGLE_NODE_METRICS, SWARM_METRICS};

#[test]
fn plan_pico_single_node_measurements_stay_bounded() {
    let metrics = SINGLE_NODE_METRICS;

    assert_eq!(metrics.sio_role_capacity, 4);
    assert_eq!(metrics.sio_queue_capacity, 8);
    assert_eq!(metrics.sio_frame_payload_capacity, 96);
    assert!(metrics.sio_frame_size <= 128);
    assert!(metrics.syscall_request_size <= 64);
    assert!(metrics.syscall_response_size <= 64);
    assert_eq!(metrics.syscall_buffer_capacity, 30);
    assert!(metrics.timer_table_size <= 192);
    assert!(metrics.interrupt_resolver_size <= 320);
    assert!(metrics.interrupt_resolver_rejection_telemetry_size <= 16);
    assert!(metrics.budget_controller_size <= 32);
    assert!(metrics.memory_lease_table_size <= 128);
    assert!(metrics.memory_lease_rejection_telemetry_size <= 16);
    assert!(metrics.pico_fd_view_size <= 256);
    assert!(metrics.pico_fd_rejection_telemetry_size <= 16);
    assert!(metrics.app_stream_table_size <= 128);
    assert!(metrics.app_lease_table_size <= 128);
    assert!(metrics.endpoint_size <= 32);
    assert!(metrics.role_program_size <= 16);
    assert!(metrics.static_arg_env_size <= 256);
    assert!(metrics.image_slot_table_size <= 2048);
    assert!(metrics.management_rejection_telemetry_size <= 16);
}

#[test]
fn plan_pico_swarm_measurements_stay_bounded() {
    let metrics = SWARM_METRICS;

    assert_eq!(metrics.wifi_frame_header_len, 28);
    assert_eq!(metrics.wifi_frame_payload_capacity, 96);
    assert_eq!(metrics.wifi_auth_tag_len, 4);
    assert_eq!(metrics.wifi_frame_max_wire_len, 128);
    assert!(metrics.wifi_frame_size <= 128);
    assert_eq!(metrics.fragmentation_header_len, 6);
    assert_eq!(metrics.fragmentation_chunk_capacity, 90);
    assert!(metrics.fragmentation_buffer_size <= 320);
    assert_eq!(metrics.qemu_cyw_max_roles, 6);
    assert!(metrics.qemu_cyw_transport_size <= 64);
    assert!(metrics.host_swarm_medium_size <= 2048);
    assert!(metrics.host_swarm_role_transport_size <= 128);
    assert!(metrics.neighbor_table_size <= 64);
    assert!(metrics.remote_object_table_size <= 256);
    assert!(metrics.remote_object_rejection_telemetry_size <= 16);
    assert!(metrics.replay_window_size <= 8);
    assert!(metrics.swarm_drop_telemetry_size <= 16);
    assert_eq!(metrics.wifi_ping_pong_nodes, 2);
    assert_eq!(metrics.wifi_ping_pong_messages, 2);
    assert_eq!(metrics.remote_fd_read_messages, 2);
    assert_eq!(metrics.remote_actuator_command_messages, 2);
    assert_eq!(metrics.packet_loss_redelivery_frames, 1);
    assert_eq!(metrics.provisioning_join_messages, 4);
    assert_eq!(metrics.leave_revoke_messages, 4);
    assert_eq!(metrics.qemu_swarm_default_nodes, 6);
    assert_eq!(metrics.qemu_swarm_default_sensor_nodes, 5);
    assert_eq!(metrics.qemu_swarm_sample_count, 5);
    assert_eq!(metrics.qemu_swarm_wasip1_fd_write_count, 5);
    assert_eq!(metrics.qemu_swarm_aggregate_ack_count, 5);
    assert_eq!(metrics.qemu_swarm_base_sample_value, 0x0000_a5a5);
    assert_eq!(metrics.qemu_swarm_default_aggregate, 0x0003_3c43);
}

#[test]
fn plan_pico_cyw43439_firmware_artifact_is_picosdk_sourced_and_disassembled() {
    let firmware = std::fs::read("firmware/cyw43/w43439A0_7_95_49_00_firmware.bin")
        .expect("run scripts/extract_cyw43_firmware.py before plan gate");
    let clm = std::fs::read("firmware/cyw43/w43439A0_7_95_49_00_clm.bin")
        .expect("run scripts/extract_cyw43_firmware.py before plan gate");
    let manifest = std::fs::read_to_string("firmware/cyw43/w43439A0_7_95_49_00.manifest.json")
        .expect("run scripts/extract_cyw43_firmware.py before plan gate");
    let disasm = std::fs::read_to_string(
        "firmware/cyw43/w43439A0_7_95_49_00_firmware.thumb.disasm.head.txt",
    )
    .expect("run scripts/disassemble_cyw43_firmware.sh before plan gate");

    assert_eq!(firmware.len(), 224_190);
    assert_eq!(clm.len(), 984);
    assert!(manifest.contains("a1438dff1d38bd9c65dbd693f0e5db4b9ae91779"));
    assert!(manifest.contains("dd7568229f3bf7a37737b9e1ef250c26efe75b23"));
    assert!(manifest.contains("\"fnv1a32\": \"0xfa231a9f\""));
    assert!(manifest.contains("\"fnv1a32\": \"0x5178f94d\""));
    assert!(disasm.contains("Disassembly of section .text"));
    assert!(disasm.contains("_binary_firmware_cyw43_w43439A0_7_95_49_00_firmware_bin_start"));
}

#[test]
fn plan_pico_no_p2_measurements_stay_absent() {
    let metrics = NO_P2_METRICS;

    assert_eq!(metrics.wasip1_full_subset_import_count, 46);
    assert_eq!(metrics.wasip1_static_args_capacity, 4);
    assert_eq!(metrics.wasip1_static_env_capacity, 4);
    assert_eq!(metrics.wasip1_static_arg_bytes_capacity, 64);
    assert_eq!(metrics.wasip1_static_env_bytes_capacity, 128);
    assert!(metrics.network_object_table_size <= 256);
    assert!(metrics.network_object_rejection_telemetry_size <= 16);
    assert_eq!(metrics.network_datagram_payload_capacity, 48);
    assert_eq!(metrics.network_stream_payload_capacity, 48);
    assert_eq!(metrics.component_model_loader_bytes, 0);
    assert_eq!(metrics.wit_runtime_table_bytes, 0);
    assert_eq!(metrics.p2_resource_table_bytes, 0);
}

#[test]
fn plan_pico_source_tree_keeps_no_p2_runtime_surface() {
    const NEEDLES: &[&str] = &[
        "wasi:cli",
        "wasi:clocks",
        "wasi:filesystem",
        "wasi:http",
        "wasi:io",
        "wasi:random",
        "wasi:sockets",
        "wasi/",
        "wasm32-wasip2",
        "wasip2",
        "wasi_snapshot_preview2",
        "preview2",
        "wit-bindgen",
        "wit_component",
        "component-model",
    ];
    const SOURCES: &[(&str, &str)] = &[
        ("Cargo.toml", include_str!("../Cargo.toml")),
        ("README.md", include_str!("../README.md")),
        ("src/kernel/app.rs", include_str!("../src/kernel/app.rs")),
        (
            "src/port/host_queue.rs",
            include_str!("../src/port/host_queue.rs"),
        ),
        (
            "src/kernel/budget.rs",
            include_str!("../src/kernel/budget.rs"),
        ),
        (
            "src/machine/rp2350/cyw43439.rs",
            include_str!("../src/machine/rp2350/cyw43439.rs"),
        ),
        (
            "src/kernel/device/gpio.rs",
            include_str!("../src/kernel/device/gpio.rs"),
        ),
        ("src/lib.rs", include_str!("../src/lib.rs")),
        (
            "src/kernel/metrics.rs",
            include_str!("../src/kernel/metrics.rs"),
        ),
        (
            "src/kernel/mgmt/mod.rs",
            include_str!("../src/kernel/mgmt/mod.rs"),
        ),
        (
            "src/kernel/network/mod.rs",
            include_str!("../src/kernel/network/mod.rs"),
        ),
        (
            "src/kernel/policy.rs",
            include_str!("../src/kernel/policy.rs"),
        ),
        (
            "src/kernel/remote/mod.rs",
            include_str!("../src/kernel/remote/mod.rs"),
        ),
        (
            "src/kernel/resolver.rs",
            include_str!("../src/kernel/resolver.rs"),
        ),
        (
            "src/kernel/guest_ledger.rs",
            include_str!("../src/kernel/guest_ledger.rs"),
        ),
        (
            "src/kernel/swarm/mod.rs",
            include_str!("../src/kernel/swarm/mod.rs"),
        ),
        (
            "src/choreography/protocol/mod.rs",
            include_str!("../src/choreography/protocol/mod.rs"),
        ),
        (
            "src/kernel/device/timer.rs",
            include_str!("../src/kernel/device/timer.rs"),
        ),
        (
            "src/port/transport.rs",
            include_str!("../src/port/transport.rs"),
        ),
        (
            "src/kernel/wasi/mod.rs",
            include_str!("../src/kernel/wasi/mod.rs"),
        ),
        (
            "src/kernel/engine/wasm/mod.rs",
            include_str!("../src/kernel/engine/wasm/mod.rs"),
        ),
    ];

    for (path, source) in SOURCES {
        for needle in NEEDLES {
            assert!(
                !source.contains(needle),
                "{path} contains forbidden No-P2 runtime surface marker {needle:?}"
            );
        }
    }
}

#[test]
fn plan_pico_source_tree_keeps_no_bridge_runtime_surface() {
    const NEEDLES: &[&str] = &[
        "PicoBridge",
        "BridgeAdvance",
        "typed phase bridge",
        "bridge_state_size",
        "host_bridge",
        "send_packet_to_remote",
        "fd.is_remote(",
    ];
    const SOURCES: &[(&str, &str)] = &[
        ("src/kernel/app.rs", include_str!("../src/kernel/app.rs")),
        (
            "src/port/host_queue.rs",
            include_str!("../src/port/host_queue.rs"),
        ),
        (
            "src/kernel/budget.rs",
            include_str!("../src/kernel/budget.rs"),
        ),
        (
            "src/machine/rp2350/cyw43439.rs",
            include_str!("../src/machine/rp2350/cyw43439.rs"),
        ),
        (
            "src/kernel/device/gpio.rs",
            include_str!("../src/kernel/device/gpio.rs"),
        ),
        ("src/lib.rs", include_str!("../src/lib.rs")),
        (
            "src/kernel/metrics.rs",
            include_str!("../src/kernel/metrics.rs"),
        ),
        (
            "src/kernel/mgmt/mod.rs",
            include_str!("../src/kernel/mgmt/mod.rs"),
        ),
        (
            "src/kernel/network/mod.rs",
            include_str!("../src/kernel/network/mod.rs"),
        ),
        (
            "src/kernel/policy.rs",
            include_str!("../src/kernel/policy.rs"),
        ),
        (
            "src/kernel/remote/mod.rs",
            include_str!("../src/kernel/remote/mod.rs"),
        ),
        (
            "src/kernel/resolver.rs",
            include_str!("../src/kernel/resolver.rs"),
        ),
        (
            "src/kernel/guest_ledger.rs",
            include_str!("../src/kernel/guest_ledger.rs"),
        ),
        (
            "src/kernel/swarm/mod.rs",
            include_str!("../src/kernel/swarm/mod.rs"),
        ),
        (
            "src/choreography/protocol/mod.rs",
            include_str!("../src/choreography/protocol/mod.rs"),
        ),
        (
            "src/kernel/device/timer.rs",
            include_str!("../src/kernel/device/timer.rs"),
        ),
        (
            "src/port/transport.rs",
            include_str!("../src/port/transport.rs"),
        ),
        (
            "src/kernel/wasi/mod.rs",
            include_str!("../src/kernel/wasi/mod.rs"),
        ),
        (
            "src/kernel/engine/wasm/mod.rs",
            include_str!("../src/kernel/engine/wasm/mod.rs"),
        ),
    ];

    for (path, source) in SOURCES {
        for needle in NEEDLES {
            assert!(
                !source.contains(needle),
                "{path} contains forbidden bridge/relay runtime surface marker {needle:?}"
            );
        }
    }
}

#[test]
fn plan_pico_source_tree_keeps_removed_compatibility_names_out() {
    const NEEDLES: &[&str] = &[
        "Authorizer",
        "NetworkFd",
        "ListenerFd",
        "CompatibilityTier",
        "PicoFdResource",
        "PicoFdTable",
        "PicoFdEntry",
        "grant_routed",
        "remote_capability_table_size",
        "remote_capability_rejection_telemetry_size",
    ];
    const SOURCES: &[(&str, &str)] = &[
        (
            "src/kernel/features.rs",
            include_str!("../src/kernel/features.rs"),
        ),
        (
            "src/kernel/metrics.rs",
            include_str!("../src/kernel/metrics.rs"),
        ),
        (
            "src/kernel/mgmt/mod.rs",
            include_str!("../src/kernel/mgmt/mod.rs"),
        ),
        (
            "src/kernel/network/mod.rs",
            include_str!("../src/kernel/network/mod.rs"),
        ),
        (
            "src/kernel/remote/mod.rs",
            include_str!("../src/kernel/remote/mod.rs"),
        ),
        (
            "src/kernel/guest_ledger.rs",
            include_str!("../src/kernel/guest_ledger.rs"),
        ),
        (
            "src/kernel/wasi/mod.rs",
            include_str!("../src/kernel/wasi/mod.rs"),
        ),
        (
            "src/kernel/wasi/host_runner.rs",
            include_str!("../src/kernel/wasi/host_runner.rs"),
        ),
    ];

    for (path, source) in SOURCES {
        for needle in NEEDLES {
            assert!(
                !source.contains(needle),
                "{path} contains removed compatibility/control-bypass marker {needle:?}"
            );
        }
    }
}

#[test]
fn plan_wasi_vm_hot_path_keeps_control_structure_scans_out() {
    let vm = include_str!("../src/kernel/engine/wasm/vm.rs");

    assert!(
        vm.contains("CoreControlTarget"),
        "Wasm VM should materialize decoded control targets before execution"
    );
    assert!(
        vm.contains("decode_core_control_targets"),
        "Wasm VM should decode control targets before execution"
    );
    assert!(
        !vm.contains("cfg!(any(test"),
        "Wasm VM capacity must be selected by explicit profile features, not by test builds"
    );
    assert!(
        vm.contains("feature = \"wasm-engine-wasip1-std-profile\") {\n    16"),
        "embedded std profile must keep active control-stack capacity measured, not padded"
    );
    assert!(
        vm.contains("feature = \"wasm-engine-wasip1-std-profile\") {\n    56"),
        "embedded std profile must keep active control-target capacity measured, not padded"
    );
    for needle in [
        concat!("find", "_matching"),
        concat!("find", "_matching_end"),
        concat!("find", "_matching_else_or_end"),
    ] {
        assert!(
            !vm.contains(needle),
            "Wasm VM hot path must not rescan raw control structure with {needle:?}"
        );
    }
}

#[test]
fn plan_wasi_engine_facade_keeps_placement_and_vm_internals_private() {
    let facade = include_str!("../src/kernel/engine/wasm/mod.rs");
    let vm = include_str!("../src/kernel/engine/wasm/vm.rs");
    let baker_guest = include_str!("../src/projects/baker_link_led/guest.rs");

    assert!(
        !facade.contains("write_new_in_place"),
        "engine facade must expose Guest::new and Guest::resume, not an embedded placement constructor"
    );
    assert!(
        !vm.contains("write_new_in_place") && !vm.contains("parse_in_place"),
        "VM placement internals must not be a second construction path"
    );
    assert!(
        !facade.contains("pub fn place_in_static_slot")
            && facade.contains("pub(crate) fn place_in_static_slot"),
        "Pico static-slot capacity may exist only as crate-internal engine capacity"
    );
    assert!(
        baker_guest.contains("Guest::place_in_static_slot"),
        "Baker loader may use only the crate-internal static-slot capacity, not a public placement API"
    );
    for needle in [
        "pub enum VmEvent",
        "pub struct Vm",
        "pub struct FdWriteCall",
        "pub struct PathCall",
        "pub struct SocketCall",
        "pub struct Module",
        "pub struct Interpreter",
        "pub fn complete_fd_write",
    ] {
        assert!(
            !vm.contains(needle),
            "private VM implementation leaks facade-internal surface marker {needle:?}"
        );
    }
}

#[test]
fn baker_project_uses_common_choreography_fragments_without_warning_suppression() {
    let baker = include_str!("../src/projects/baker_link_led/choreography.rs");
    let runtime = include_str!("../src/projects/baker_link_led/runtime.rs");
    let device_session = include_str!("../src/projects/baker_link_led/device_session.rs");
    let kernel_session = include_str!("../src/projects/baker_link_led/kernel_session.rs");
    let engine_session = include_str!("../src/projects/baker_link_led/engine_session.rs");
    let storage = include_str!("../src/projects/baker_link_led/storage.rs");
    let stages = include_str!("../src/projects/baker_link_led/stages.rs");
    let guest = include_str!("../src/projects/baker_link_led/guest.rs");
    let fragments = include_str!("../src/choreography/protocol/fragments.rs");

    for source in [
        runtime,
        device_session,
        kernel_session,
        engine_session,
        storage,
        stages,
        guest,
    ] {
        assert!(
            !source.contains("allow(dead_code") && !source.contains("allow(unused_imports"),
            "Baker proof modules must be split or cfg-gated instead of suppressing responsibility leaks"
        );
    }
    assert!(
        fragments.contains("local_fd_write_gpio_cycle")
            && fragments.contains("local_path_open_cycle")
            && fragments.contains("local_poll_timer_cycle")
            && fragments.contains("ChoreoFsOpenAdmitRouteMsg")
            && fragments.contains("ChoreoFsOpenRejectRouteMsg"),
        "common syscall/device choreography fragments must live in the shared protocol layer"
    );
    let choreofs = include_str!("../src/kernel/choreofs.rs");
    assert!(
        !choreofs.contains("open_with_ledger")
            && !choreofs.contains("open_wasip1_path_with_ledger")
            && !choreofs.contains("apply_fd_cap_mint")
            && !choreofs.contains("grant_preopen_root"),
        "ChoreoFS must remain an object identity store; fd materialization belongs to projected Kernel localside after hibana route control"
    );
    for needle in [
        "macro_rules! fd_write_cycle",
        "macro_rules! path_open_cycle",
        "macro_rules! poll_cycle",
        "abort_safe_gpio_cycle",
    ] {
        assert!(
            !baker.contains(needle),
            "Baker choreography should assemble common fragments, not duplicate {needle:?}"
        );
    }
}

#[test]
fn plan_pico_document_keeps_wasi_import_trampoline_no_bridge_naming() {
    let plan = include_str!("../plan.md");

    assert!(
        !plan.contains("PicoWasiBridge"),
        "plan.md must not describe the WASI import trampoline as a bridge owner"
    );
    assert!(
        plan.contains("WASI P1 import trampoline"),
        "plan.md should name the WASI import boundary without bridge naming"
    );
}

#[test]
fn plan_pico_keeps_abort_out_of_loop_control_shape() {
    let plan = include_str!("../plan.md");
    assert!(
        plan.contains("Abort is not a third loop arm."),
        "plan.md must keep abort separate from Continue/Break loop control"
    );
    assert!(
        plan.contains("Abort | Normal"),
        "plan.md must describe Abort|Normal as a separate fault choice"
    );
    assert!(
        plan.contains("LoopContinue | LoopBreak"),
        "plan.md must describe Continue|Break as the only loop choice"
    );

    const FORBIDDEN_SOURCE_MARKERS: &[&str] = &[
        "abortable_loop",
        "terminal_route",
        "BakerTrafficLoopAbort",
        "LoopBreakKind as abort",
    ];
    const SOURCES: &[(&str, &str)] = &[
        (
            "src/projects/baker_link_led/choreography.rs",
            include_str!("../src/projects/baker_link_led/choreography.rs"),
        ),
        (
            "src/choreography/protocol/mod.rs",
            include_str!("../src/choreography/protocol/mod.rs"),
        ),
        (
            "src/projects/baker_link_led/runtime.rs",
            include_str!("../src/projects/baker_link_led/runtime.rs"),
        ),
        (
            "src/projects/baker_link_led/device_session.rs",
            include_str!("../src/projects/baker_link_led/device_session.rs"),
        ),
        (
            "src/projects/baker_link_led/kernel_session.rs",
            include_str!("../src/projects/baker_link_led/kernel_session.rs"),
        ),
        (
            "src/projects/baker_link_led/engine_session.rs",
            include_str!("../src/projects/baker_link_led/engine_session.rs"),
        ),
    ];

    for (path, source) in SOURCES {
        for marker in FORBIDDEN_SOURCE_MARKERS {
            assert!(
                !source.contains(marker),
                "{path} contains forbidden abort/loop conflation marker {marker:?}"
            );
        }
    }
}

#[test]
fn abort_normal_continue_break_shape_has_projection_proof() {
    let proof = include_str!("host_swarm_plan.rs");

    for needle in [
        "abort_normal_route_contains_inner_continue_break_loop_projection",
        "EngineAbortRouteControl",
        "EngineNormalRouteControl",
        "BakerTrafficLoopContinueControl",
        "BakerTrafficLoopBreakControl",
    ] {
        assert!(
            proof.contains(needle),
            "host_swarm_plan.rs must keep an explicit projection proof for Abort | (LoopContinue | LoopBreak)"
        );
    }
}

#[test]
fn plan_required_control_vocabulary_is_bound_to_protocol_types() {
    let plan = include_str!("../plan.md");
    let protocol = concat!(
        include_str!("../src/choreography/protocol/mod.rs"),
        include_str!("../src/choreography/protocol/labels.rs"),
        include_str!("../src/choreography/protocol/route.rs"),
        include_str!("../src/choreography/protocol/control.rs"),
        include_str!("../src/choreography/protocol/wasi.rs"),
        include_str!("../src/choreography/protocol/device.rs"),
        include_str!("../src/choreography/protocol/management.rs"),
    );

    for needle in [
        "RouteDecision",
        "LoopContinue",
        "LoopBreak",
        "StateSnapshot",
        "StateRestore",
        "TopologyBegin",
        "TopologyAck",
        "TopologyCommit",
        "CapDelegate",
        "AbortBegin",
        "AbortAck",
        "Fence",
        "TxCommit",
        "TxAbort",
    ] {
        assert!(
            plan.contains(needle),
            "plan.md must keep normative control vocabulary entry {needle}"
        );
    }

    for needle in [
        "ControlOp::StateSnapshot",
        "ControlOp::StateRestore",
        "ControlOp::TopologyBegin",
        "ControlOp::TopologyAck",
        "ControlOp::TopologyCommit",
        "ControlOp::TxCommit",
        "ControlOp::TxAbort",
        "pub type StateSnapshotControl",
        "pub type StateRestoreControl",
        "pub type TopologyBeginControl",
        "pub type TopologyAckControl",
        "pub type TopologyCommitControl",
        "pub type TxCommitControl",
        "pub type TxAbortControl",
        "ActivationAuthorityKind",
        "ActivationKind",
        "LABEL_ACTIVATION_AUTHORITY_CONTROL",
        "LABEL_ACTIVATION_CONTROL",
    ] {
        assert!(
            protocol.contains(needle),
            "src/choreography/protocol/ module tree must bind plan control vocabulary {needle}"
        );
    }
}

#[test]
fn publication_text_keeps_physical_wireless_claims_gated() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("Pico W is currently represented as a capacity/profile target"),
        "README must keep Pico W as pending capacity/profile until physical gates pass"
    );
    assert!(
        readme.contains("physical CYW43439 gates are still pending"),
        "README must name pending Pico W physical CYW43439 gates"
    );
    assert!(
        readme.contains("must not be read as physical Pico W"),
        "README must prevent interpreting QEMU/Baker proof as physical Pico W success"
    );
}
