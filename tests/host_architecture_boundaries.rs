fn assert_absent(path: &str, source: &str, forbidden: &[&str]) {
    for needle in forbidden {
        assert!(
            !source.contains(needle),
            "{path} must not contain architecture-boundary marker {needle:?}"
        );
    }
}

fn assert_present(path: &str, source: &str, required: &[&str]) {
    for needle in required {
        assert!(
            source.contains(needle),
            "{path} must contain architecture-boundary marker {needle:?}"
        );
    }
}

#[test]
fn crate_public_surface_hides_project_tree_behind_proof_facade() {
    let lib = include_str!("../src/lib.rs");
    let proof = include_str!("../src/proof/baker_link.rs");

    assert_present("src/lib.rs", lib, &["pub mod proof;", "mod projects;"]);
    assert_absent(
        "src/lib.rs",
        lib,
        &["pub mod projects;", "doc(hidden)", "cfg_attr(doc"],
    );
    assert_present(
        "src/proof/baker_link.rs",
        proof,
        &["pub use crate::projects::baker_link_led"],
    );
    assert_absent(
        "src/proof/baker_link.rs",
        proof,
        &["doc(hidden)", "compat", "legacy"],
    );
}

#[test]
fn choreography_sources_do_not_depend_on_kernel_machine_or_projects() {
    const SOURCES: &[(&str, &str)] = &[
        (
            "src/choreography/protocol/mod.rs",
            include_str!("../src/choreography/protocol/mod.rs"),
        ),
        (
            "src/choreography/protocol/labels.rs",
            include_str!("../src/choreography/protocol/labels.rs"),
        ),
        (
            "src/choreography/protocol/route.rs",
            include_str!("../src/choreography/protocol/route.rs"),
        ),
        (
            "src/choreography/protocol/control.rs",
            include_str!("../src/choreography/protocol/control.rs"),
        ),
        (
            "src/choreography/protocol/wasi.rs",
            include_str!("../src/choreography/protocol/wasi.rs"),
        ),
        (
            "src/choreography/protocol/device.rs",
            include_str!("../src/choreography/protocol/device.rs"),
        ),
        (
            "src/choreography/protocol/management.rs",
            include_str!("../src/choreography/protocol/management.rs"),
        ),
        (
            "src/choreography/protocol/remote.rs",
            include_str!("../src/choreography/protocol/remote.rs"),
        ),
        (
            "src/choreography/protocol/network.rs",
            include_str!("../src/choreography/protocol/network.rs"),
        ),
        (
            "src/choreography/protocol/swarm.rs",
            include_str!("../src/choreography/protocol/swarm.rs"),
        ),
        (
            "src/choreography/local.rs",
            include_str!("../src/choreography/local.rs"),
        ),
        (
            "src/projects/baker_link_led/choreography.rs",
            include_str!("../src/projects/baker_link_led/choreography.rs"),
        ),
        (
            "src/projects/baker_link_led/resolver.rs",
            include_str!("../src/projects/baker_link_led/resolver.rs"),
        ),
        (
            "src/choreography/swarm.rs",
            include_str!("../src/choreography/swarm.rs"),
        ),
    ];

    for (path, source) in SOURCES {
        assert_absent(
            path,
            source,
            &[
                "crate::kernel",
                "crate::machine",
                "crate::projects",
                "hibana_pico::kernel",
                "hibana_pico::machine",
                "hibana_pico::projects",
            ],
        );
    }
}

#[test]
fn projects_do_not_define_protocol_or_authority_vocabulary() {
    const PROJECT_SOURCES: &[(&str, &str)] = &[
        (
            "src/projects/baker_link_led/main.rs",
            include_str!("../src/projects/baker_link_led/main.rs"),
        ),
        (
            "src/projects/baker_link_led/runtime.rs",
            include_str!("../src/projects/baker_link_led/runtime.rs"),
        ),
        (
            "src/projects/baker_link_led/guest.rs",
            include_str!("../src/projects/baker_link_led/guest.rs"),
        ),
        (
            "src/projects/baker_link_led/manifest.rs",
            include_str!("../src/projects/baker_link_led/manifest.rs"),
        ),
        (
            "src/projects/pico2w_swarm/runtime/mod.rs",
            include_str!("../src/projects/pico2w_swarm/runtime/mod.rs"),
        ),
        (
            "src/projects/pico2w_swarm/runtime/roles.rs",
            include_str!("../src/projects/pico2w_swarm/runtime/roles.rs"),
        ),
        (
            "src/projects/rp2040_sio_smoke/main.rs",
            include_str!("../src/projects/rp2040_sio_smoke/main.rs"),
        ),
    ];

    for (path, source) in PROJECT_SOURCES {
        assert_absent(
            path,
            source,
            &[
                "pub const LABEL_",
                "impl ResourceKind",
                "impl ControlResourceKind",
                "struct RouteControl",
                "ControlOp::RouteDecision",
                "ControlOp::Topology",
                "ControlOp::Tx",
                "TopologyBeginKind",
                "TxCommitKind",
                "StateRestoreKind",
            ],
        );
    }
}

#[test]
fn machine_sources_do_not_define_protocol_authority_or_runtime_managers() {
    const MACHINE_SOURCES: &[(&str, &str)] = &[
        (
            "src/machine/rp2040/baker_link.rs",
            include_str!("../src/machine/rp2040/baker_link.rs"),
        ),
        (
            "src/machine/rp2040/sio.rs",
            include_str!("../src/machine/rp2040/sio.rs"),
        ),
        (
            "src/machine/rp2040/timer.rs",
            include_str!("../src/machine/rp2040/timer.rs"),
        ),
        (
            "src/machine/rp2040/uart.rs",
            include_str!("../src/machine/rp2040/uart.rs"),
        ),
        (
            "src/machine/rp2350/cyw43439.rs",
            include_str!("../src/machine/rp2350/cyw43439.rs"),
        ),
        (
            "src/machine/rp2350/sio.rs",
            include_str!("../src/machine/rp2350/sio.rs"),
        ),
        (
            "src/machine/rp2350/uart.rs",
            include_str!("../src/machine/rp2350/uart.rs"),
        ),
    ];

    for (path, source) in MACHINE_SOURCES {
        assert_absent(
            path,
            source,
            &[
                "pub const LABEL_",
                "ControlOp::",
                "RouteDecision",
                "TopologyBegin",
                "TopologyCommit",
                "TxCommit",
                "TxAbort",
                "runtime recovery manager",
                "runtime topology manager",
                "runtime transaction manager",
                "fd.is_remote",
                "GuestLedger",
                "apply_fd_cap_mint",
                "apply_fd_cap_grant",
                "baker_link_pico_min_ledger",
                "mint_baker_link_choreofs_fd",
            ],
        );
    }
}

#[test]
fn baker_project_layer_owns_baker_fd_materialization_not_machine_manifest() {
    let machine = include_str!("../src/machine/rp2040/baker_link.rs");
    let project_manifest = include_str!("../src/projects/baker_link_led/manifest.rs");
    let project_ledger = include_str!("../src/projects/baker_link_led/ledger.rs");
    let project_engine_session = include_str!("../src/projects/baker_link_led/engine_session.rs");
    let project_kernel_session = include_str!("../src/projects/baker_link_led/kernel_session.rs");
    let labels = include_str!("../src/choreography/protocol/labels.rs");
    let engine_facade = include_str!("../src/kernel/engine/wasm/mod.rs");
    let engine_vm = include_str!("../src/kernel/engine/wasm/vm.rs");

    assert_absent(
        "src/machine/rp2040/baker_link.rs",
        machine,
        &[
            "GuestLedger",
            "apply_fd_cap_mint",
            "apply_fd_cap_grant",
            "baker_link_pico_min_ledger",
            "mint_baker_link_choreofs_fd",
            "ChoreoFsStore",
            "GpioFdWriteRoute",
            "BAKER_LINK_LED_FD",
            "BAKER_LINK_LED_FDS",
            "BAKER_LINK_LED_RESOURCE_PATHS",
            "BAKER_LINK_TRAFFIC_LIGHT_PATTERN",
            "baker_link_led_resource_store",
            "resolve_baker_link_choreofs_object",
            "baker_link_led_fd_write_route",
        ],
    );
    assert_present(
        "src/machine/rp2040/baker_link.rs",
        machine,
        &[
            "BAKER_LINK_USER_LED_PINS",
            "BAKER_LINK_SAFE_GPIO_LEVELS",
            "impl BoardSafeState for BakerLinkBoard",
        ],
    );
    assert_present(
        "src/projects/baker_link_led/manifest.rs",
        project_manifest,
        &[
            "pub const BAKER_LINK_LED_FDS",
            "pub const BAKER_LINK_LED_RESOURCE_PATHS",
            "pub fn baker_link_led_resource_store",
            "pub fn resolve_baker_link_choreofs_object",
            "pub const fn baker_link_led_fd_write_route",
            "pub const BAKER_LINK_TRAFFIC_LIGHT_PATTERN",
        ],
    );
    assert_present(
        "src/projects/baker_link_led/ledger.rs",
        project_ledger,
        &[
            "pub fn baker_link_pico_min_ledger",
            "pub fn mint_baker_link_choreofs_fd",
            "pub fn resolve_baker_link_choreofs_path",
            "apply_fd_cap_mint",
            "apply_fd_cap_grant",
        ],
    );
    assert_present(
        "src/projects/baker_link_led/kernel_session.rs",
        project_kernel_session,
        &["ChoreoFsOpenAdmitRouteMsg", "mint_baker_link_choreofs_fd"],
    );
    assert_present(
        "src/projects/baker_link_led/engine_session.rs",
        project_engine_session,
        &["ChoreoFsOpenAdmitRouteMsg", "PathOpen"],
    );
    assert_absent(
        "src/choreography/protocol/labels.rs",
        labels,
        &["LABEL_BAKER_FD_WRITE"],
    );
    assert_absent(
        "src/kernel/engine/wasm",
        engine_facade,
        &["baker-abort-safe-demo"],
    );
    assert_absent(
        "src/kernel/engine/wasm",
        engine_vm,
        &["baker-abort-safe-demo"],
    );
}

#[test]
fn protocol_module_tree_owns_control_vocabulary_bindings() {
    let protocol_mod = include_str!("../src/choreography/protocol/mod.rs");
    let labels = include_str!("../src/choreography/protocol/labels.rs");
    let route = include_str!("../src/choreography/protocol/route.rs");
    let control = include_str!("../src/choreography/protocol/control.rs");
    let wasi = include_str!("../src/choreography/protocol/wasi.rs");
    let device = include_str!("../src/choreography/protocol/device.rs");
    let management = include_str!("../src/choreography/protocol/management.rs");

    assert_present(
        "src/choreography/protocol/mod.rs",
        protocol_mod,
        &[
            "mod labels;",
            "mod route;",
            "mod control;",
            "mod wasi;",
            "mod device;",
            "mod management;",
            "mod remote;",
            "mod network;",
            "mod swarm;",
        ],
    );
    assert_present(
        "src/choreography/protocol/labels.rs",
        labels,
        &[
            "pub const LABEL_ENGINE_REQ",
            "pub const LABEL_TOPOLOGY_BEGIN_CONTROL",
            "pub struct EngineLabelUniverse",
        ],
    );
    assert_present(
        "src/choreography/protocol/route.rs",
        route,
        &["pub struct RouteControl", "pub type RemoteSensorRouteKind"],
    );
    assert_present(
        "src/choreography/protocol/control.rs",
        control,
        &[
            "pub type EngineNormalRouteKind",
            "pub type EngineAbortRouteControl",
            "pub type EngineNormalRouteControl",
            "pub type TopologyBeginControl",
            "pub type TopologyAckControl",
            "pub type TopologyCommitControl",
            "pub type TxCommitControl",
            "pub type TxAbortControl",
            "pub type StateSnapshotControl",
            "pub type StateRestoreControl",
            "pub type ActivationAuthorityControl",
            "pub type ActivationControl",
        ],
    );
    assert_present(
        "src/choreography/protocol/wasi.rs",
        wasi,
        &[
            "pub enum EngineReq",
            "pub enum EngineRet",
            "pub struct FdWrite",
        ],
    );
    assert_present(
        "src/choreography/protocol/device.rs",
        device,
        &["pub struct GpioSet", "pub struct UartWrite"],
    );
    assert_present(
        "src/choreography/protocol/management.rs",
        management,
        &["pub struct MgmtImageBegin", "pub struct MgmtStatus"],
    );
}

#[test]
fn source_tree_keeps_forbidden_runtime_concepts_out_of_core_surfaces() {
    const CORE_SOURCES: &[(&str, &str)] = &[
        ("src/lib.rs", include_str!("../src/lib.rs")),
        ("src/kernel/mod.rs", include_str!("../src/kernel/mod.rs")),
        (
            "src/choreography/mod.rs",
            include_str!("../src/choreography/mod.rs"),
        ),
        ("src/machine/mod.rs", include_str!("../src/machine/mod.rs")),
        (
            "src/kernel/swarm/mod.rs",
            include_str!("../src/kernel/swarm/mod.rs"),
        ),
        (
            "src/kernel/remote/mod.rs",
            include_str!("../src/kernel/remote/mod.rs"),
        ),
        (
            "src/kernel/network/mod.rs",
            include_str!("../src/kernel/network/mod.rs"),
        ),
        (
            "src/kernel/mgmt/mod.rs",
            include_str!("../src/kernel/mgmt/mod.rs"),
        ),
    ];

    for (path, source) in CORE_SOURCES {
        assert_absent(
            path,
            source,
            &[
                "bridge layer",
                "relay layer",
                "fd.is_remote",
                "runtime recovery manager",
                "runtime topology manager",
                "runtime transaction manager",
                "same-session recovery",
                "hidden fallback",
                "hidden retry",
            ],
        );
    }
}
