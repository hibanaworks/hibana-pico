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
fn public_root_is_the_capsule_surface_only() {
    let lib = include_str!("../src/lib.rs");

    assert_present(
        "src/lib.rs",
        lib,
        &[
            "pub mod appkit;",
            "pub mod choreography;",
            "pub mod site;",
            "mod kernel;",
        ],
    );
    assert_absent(
        "src/lib.rs",
        lib,
        &[
            "pub mod board;",
            "pub mod proof;",
            "pub mod kernel;",
            "pub mod machine;",
            "pub mod port;",
            "pub mod projects;",
            "mod machine;",
            "mod port;",
            "mod projects;",
        ],
    );
}

#[test]
fn appkit_has_capsule_shape_without_legacy_facades() {
    let appkit = include_str!("../src/appkit.rs");

    assert_present(
        "src/appkit.rs",
        appkit,
        &[
            "pub trait Capsule",
            "fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe>;",
            "pub trait LogicalImage",
            "const REQUESTED_ROLES: RoleSet;",
            "fn wasi_guest_storage<'guest, const ROLE: u8>() -> WasiGuestStorage<'guest>;",
            "fn wasi_budget<const ROLE: u8>() -> BudgetRun",
            "drive_canonical_wasi_engine",
            "self.wasi_guest_bytes.is_some()",
            "pub struct WasiGuestArena",
            "pub struct WasiGuestStorage<'guest>",
            "Guest::init_in_place(ptr, module)?;",
            "pub struct CarrierKind",
            "pub struct PeerImageSet",
            "type Carrier<'a>: hibana::integration::Transport + 'a",
            "fn carrier<'a>() -> Self::Carrier<'a>;",
            "fn visit_requested_projected_roles<C, V>",
            "pub trait Placement",
            "pub enum RoleKind",
            "pub struct RoleKindCounts",
            "fn role_kind(role: u8) -> RoleKind",
            "pub trait ArtifactBundle",
            "pub trait Localside",
            "pub struct EngineCtx<'endpoint, 'guest, C: Capsule, const ROLE: u8>",
            "pub const fn role(&self) -> u8",
            "pub fn run<I, C>",
            "I::Exit<C::Report>",
            "pub trait FromRunReport",
            "pub struct EndpointCarrierFacts",
            "pub const fn session_id(self) -> u32",
            "pub struct RoleEndpointCtx<'a, C: Capsule, const ROLE: u8>",
            "pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE>",
            "pub fn validate_requested_roles<C, I>()",
            "I::REQUESTED_ROLES.is_subset_of(projection.roles)",
            "pub struct RunReport",
            "pub const fn validated_role_count(&self) -> u8",
            "pub const fn attached_endpoint_count(&self) -> u8",
            "pub const fn attached_role_kinds(&self) -> RoleKindCounts",
            "pub const fn wasi_imports(&self) -> WasiImports",
            "pub const fn wasi_completion_pair_count(&self) -> u8",
            "pub const fn manifest(&self) -> ImageManifest",
            "pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts",
            "pub struct ImageManifest",
            "pub capsule_fingerprint: [u64; 2]",
            "pub placement_fingerprint: [u64; 2]",
            "pub label_universe_fingerprint: [u64; 2]",
            "pub choreography_session_id: u32",
            "pub peer_image_ids: [ImageId; 8]",
            "pub peer_image_count: u8",
            "pub fn can_attach_peer(&self, peer: &Self) -> bool",
            "pub policies: [u16; 16]",
            "pub policy_count: u16",
            "pub control_ops: [u8; 16]",
            "pub control_tap_ids: [u16; 16]",
            "pub control_count: u16",
            "pub struct LaneSet",
            "pub struct WasiImports",
            "pub wasi_completion_pair_count: u8",
            "pub struct ChoreoFsObject",
            "pub struct ChoreoFsObjectSet",
            "pub struct ChoreoFsFacts",
            "pub struct LedgerFacts",
            "pub struct DriverFacts",
        ],
    );
    assert_absent(
        "src/appkit.rs",
        appkit,
        &[
            "pub mod support",
            "pub struct Choreo<",
            "pub trait App",
            "type Program:",
            "pub struct GuestArtifact",
            "pub enum ArtifactError",
            "pub const fn guest_artifact",
            "pub async fn drive_wasi_guest",
            "pub fn drive_wasi_guest",
            "pub async fn drive_wasi_guest_imports",
            "drive_wasi_guest_imports",
            "ObjectSpec",
            "ObjectSpecSet",
            "AttachedImage",
            "fn run(attached",
            "BoardRuntime",
            "BoardRun",
            "appkit::Program",
            "pub trait ProjectedRoleVisitor",
            "pub fn visit_projected_role",
            "fn visit_projected_roles<V>(",
            "pub struct InProcessCarrier",
            "pub struct InProcessTx",
            "pub struct InProcessRx",
            "pub struct CarrierAttachState",
            "pub struct AttachedCarrierFrame",
            "pub fn push_attached_frame",
            "pub fn pop_attached_frame",
            "pub fn requeue_attached_frame",
            "RefCell",
            "site::carrier::IN_PROCESS",
            "has_in_process_carrier",
            "baker",
            "Baker",
            "protocol inference",
            "route mismatch recovery",
            "timeout heuristic",
            "host FS fallback",
            "direct syscall completion",
            "Rp2040Sio",
            "SioTransport",
            "ActiveWasiGuestStorage",
            "InlineWasiGuestStorage",
            "StaticWasiGuestStorage",
            "MaybeUninit<crate::kernel::engine::wasm::Guest",
            "Box<crate::kernel::engine::wasm::Guest",
            "Box<dyn Future",
            "Vec<ScheduledTask",
            "Box::pin",
            "std::vec![",
            "Guest::new(bytes)",
            "feature = \"platform-host-native\"",
            "platform-host-native",
            "feature = \"platform-linux\"",
            "platform-linux",
            "platform-cortex-m",
        ],
    );
}

#[test]
fn choreography_is_protocol_vocabulary_only() {
    let choreography_mod = include_str!("../src/choreography/mod.rs");
    let protocol_mod = include_str!("../src/choreography/protocol/mod.rs");

    assert_present(
        "src/choreography/mod.rs",
        choreography_mod,
        &["pub mod protocol;"],
    );
    assert_absent(
        "src/choreography/mod.rs",
        choreography_mod,
        &[
            "pub mod fragment;",
            "pub mod local;",
            "pub mod proof;",
            "appkit::Choreo",
            "appkit::Program",
            "trait Fragment",
            "CAPS",
            "WASI_IMPORTS",
            "crate::kernel",
            "crate::machine",
            "crate::port",
            "crate::projects",
        ],
    );
    assert_absent(
        "src/choreography/protocol/mod.rs",
        protocol_mod,
        &[
            "mod network;",
            "mod remote;",
            "mod swarm;",
            "crate::kernel",
            "crate::machine",
            "crate::port",
            "crate::projects",
            "appkit runtime state",
        ],
    );
}

#[test]
fn site_exposes_site_facts_not_protocol_authority() {
    let site = include_str!("../src/site.rs");

    assert_present("src/site.rs", site, &["pub struct Local<Image>"]);
    assert_absent(
        "src/site.rs",
        site,
        &[
            "pub mod host",
            "pub mod linux",
            "pub mod mcu",
            "pub mod rp2040",
            "pub mod swarm",
            "pub mod process",
            "pub mod bare",
            "pub struct Native<Image>",
            "pub struct Core<const CORE: u16, Image>",
            "pub const TCP",
            "pub const UDP",
            "pub const UART",
            "pub const USB",
            "IN_PROCESS",
            "pub mod carrier",
            "SioTransport",
            "core_id()",
            "EngineReq",
            "GuestLedger",
            "WASI import dispatch",
            "protocol authority",
            "authorize WASI",
            "complete WASI",
            "route mismatch recovery",
            "timeout heuristic",
        ],
    );
}

#[test]
fn wasip1_guest_examples_live_under_examples_and_keep_socket_assets() {
    let root_cargo = include_str!("../Cargo.toml");
    let helper = include_str!("../examples/wasip1-guests/hibana-wasi-guest/src/lib.rs");
    let net = include_str!("../examples/wasip1-guests/hibana-wasi-guest/src/net.rs");
    let sys = include_str!("../examples/wasip1-guests/hibana-wasi-guest/src/sys.rs");
    let smoke_manifest = include_str!("../examples/wasip1-guests/wasip1-smoke-apps/Cargo.toml");
    let baker_guest_manifest = include_str!("../examples/baker-firmware/wasip1/traffic/Cargo.toml");

    assert_present(
        "examples/wasip1-guests/hibana-wasi-guest/src/lib.rs",
        helper,
        &["pub mod choreofs;", "pub mod net;"],
    );
    assert_present(
        "examples/wasip1-guests/hibana-wasi-guest/src/net.rs",
        net,
        &[
            "pub struct Datagram",
            "pub struct Stream",
            "pub struct Listener",
            "sock_send_exact",
            "sock_recv_checked",
            "sock_accept_stream",
        ],
    );
    assert_present(
        "examples/wasip1-guests/hibana-wasi-guest/src/sys.rs",
        sys,
        &[
            "fn sock_send",
            "fn sock_recv",
            "fn sock_shutdown",
            "fn sock_accept",
        ],
    );
    assert_present(
        "examples/wasip1-guests/wasip1-smoke-apps/Cargo.toml",
        smoke_manifest,
        &[
            "wasip1-std-sock-send-recv",
            "wasip1-std-sock-accept-send-recv",
            "wasip1-std-sock-accept-bad",
            "wasip1-std-stream-control",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/traffic/Cargo.toml",
        baker_guest_manifest,
        &[
            "name = \"baker-wasip1-traffic\"",
            "wasip1-led-choreofs-traffic-cycle",
            "../../../wasip1-guests/hibana-wasi-guest",
        ],
    );
    assert_absent(
        "Cargo.toml",
        root_cargo,
        &[
            "apps/wasip1/hibana-wasi-guest",
            "apps/wasip1/swarm-node-apps",
            "apps/wasip1/wasip1-smoke-apps",
        ],
    );
}

#[test]
fn private_baker_artifact_contains_two_logical_images_without_runtime_escape() {
    let baker = include_str!("../examples/baker-firmware/src/lib.rs");
    let traffic_bin = include_str!("../examples/baker-firmware/src/bin/traffic.rs");
    let choreofs_bin = include_str!("../examples/baker-firmware/src/bin/choreofs_traffic.rs");
    let choreofs_loop_bin =
        include_str!("../examples/baker-firmware/src/bin/choreofs_traffic_loop.rs");
    let choreofs_wasi_guest = include_str!(
        "../examples/baker-firmware/wasip1/traffic/src/bin/wasip1-led-choreofs-traffic-cycle.rs"
    );
    let fail_safe_bin = include_str!("../examples/baker-firmware/src/bin/fail_safe.rs");
    let recovery_bin = include_str!("../examples/baker-firmware/src/bin/recovery.rs");
    let many_reentry_bin = include_str!("../examples/baker-firmware/src/bin/many_reentry.rs");
    let endpoint_poison_bin = include_str!("../examples/baker-firmware/src/bin/endpoint_poison.rs");

    assert_present(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "rp2040_sio::core_id()",
            "pub struct DriverImage;",
            "pub struct EngineImage;",
            "pub struct SioTransport",
            "pub trait BakerCapsuleFacts",
            "appkit::run::<DriverImage, C>",
            "appkit::run::<EngineImage, C>",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "RunCtx",
            "project_role",
            "g::Role<2>",
            "RoleSet::from_bits(0b101)",
            "direct syscall completion",
            "site::rp2040",
            "core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), stage)",
            "option_env!(\"HIBANA_BAKER_PATTERN\")",
            "run_selected_pattern",
            "fn main()",
            "impl appkit::Capsule for",
            "const GREEN_LED: appkit::ChoreoFsObject",
            "const YELLOW_LED: appkit::ChoreoFsObject",
            "const RED_LED: appkit::ChoreoFsObject",
            "BakerChoreoFsRouteContinue",
            "BakerChoreoFsRouteBreak",
            "baker_drive_wasi_engine",
            "baker_choreofs_driver",
            "baker_control_engine_one_cycle",
            "baker_control_driver_one_cycle",
            "baker_many_reentry_engine",
            "baker_many_reentry_driver",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/traffic.rs",
        traffic_bin,
        &[
            "impl appkit::Capsule for Traffic",
            "fn choreography()",
            "impl appkit::Localside<Traffic> for TrafficLocal",
            "baker_firmware::run::<Traffic>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/choreofs_traffic.rs",
        choreofs_bin,
        &[
            "const GREEN_LED: appkit::ChoreoFsObject",
            "const YELLOW_LED: appkit::ChoreoFsObject",
            "const RED_LED: appkit::ChoreoFsObject",
            "impl appkit::Capsule for ChoreoFsTraffic",
            "fn choreography()",
            "impl appkit::Localside<ChoreoFsTraffic> for ChoreoFsTrafficLocal",
            "baker_firmware::run::<ChoreoFsTraffic>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/choreofs_traffic_loop.rs",
        choreofs_loop_bin,
        &[
            "const GREEN_LED: appkit::ChoreoFsObject",
            "const YELLOW_LED: appkit::ChoreoFsObject",
            "const RED_LED: appkit::ChoreoFsObject",
            "impl appkit::Capsule for ChoreoFsTrafficLoop",
            "const VISUAL_READY_CYCLES: u32 = 1",
            "impl appkit::Localside<ChoreoFsTrafficLoop> for ChoreoFsTrafficLoopLocal",
            "baker_firmware::run::<ChoreoFsTrafficLoop>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/choreofs_traffic.rs",
        choreofs_bin,
        &[
            "async fn drive_wasi_engine",
            "async fn drive_choreofs_driver",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/choreofs_traffic_loop.rs",
        choreofs_loop_bin,
        &[
            "async fn drive_wasi_engine",
            "async fn drive_choreofs_driver",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/traffic/src/bin/wasip1-led-choreofs-traffic-cycle.rs",
        choreofs_wasi_guest,
        &[
            "use hibana_wasi_guest::baker::{Led, sleep_ms};",
            "fn main()",
            "Led::open(\"/device/led/green\")",
            "Led::open(\"/device/led/yellow\")",
            "Led::open(\"/device/led/red\")",
            "set_and_wait(&green, true)",
            "set_and_wait(&yellow, true)",
            "set_and_wait(&red, true)",
        ],
    );
    assert_absent(
        "examples/baker-firmware/wasip1/traffic/src/bin/wasip1-led-choreofs-traffic-cycle.rs",
        choreofs_wasi_guest,
        &[
            "#![no_std]",
            "unsafe extern",
            "wasi_snapshot_preview1",
            "fn path_open",
            "fn fd_write",
            "fn poll_oneoff",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/fail_safe.rs",
        fail_safe_bin,
        &[
            "impl appkit::Capsule for FailSafe",
            "const SUCCESS_RESULT: u32 = baker_firmware::RESULT_FAIL_SAFE_OK",
            "impl appkit::Localside<FailSafe> for FailSafeLocal",
            "baker_firmware::run::<FailSafe>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/recovery.rs",
        recovery_bin,
        &[
            "impl appkit::Capsule for Recovery",
            "const SUCCESS_RESULT: u32 = baker_firmware::RESULT_RECOVERY_OK",
            "impl appkit::Localside<Recovery> for RecoveryLocal",
            "baker_firmware::run::<Recovery>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/many_reentry.rs",
        many_reentry_bin,
        &[
            "impl appkit::Capsule for ManyReentry",
            "EngineAbortBeginControl",
            "EngineAbortFenceControl",
            "EngineAbortAckControl",
            "impl appkit::Localside<ManyReentry> for ManyReentryLocal",
            "baker_firmware::run::<ManyReentry>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/endpoint_poison.rs",
        endpoint_poison_bin,
        &[
            "impl appkit::Capsule for EndpointPoison",
            "impl appkit::Localside<EndpointPoison> for EndpointPoisonLocal",
            "record_endpoint_error(&error)",
            "poisoned generation must not produce a flow continuation",
            "baker_firmware::run::<EndpointPoison>()",
        ],
    );
}

#[test]
fn heterogeneous_example_projects_one_capsule_into_separate_logical_images() {
    let hetero = include_str!("../examples/heterogeneous-split-example/src/lib.rs");
    let linux = include_str!("../examples/heterogeneous-split-example/src/bin/linux-control.rs");
    let m33 = include_str!("../examples/heterogeneous-split-example/src/bin/m33-realtime.rs");
    let rp2040 = include_str!("../examples/heterogeneous-split-example/src/bin/rp2040-io.rs");

    assert_present(
        "examples/heterogeneous-split-example/src/lib.rs",
        hetero,
        &[
            "pub struct Control;",
            "pub fn assert_peer_manifests()",
            "pub struct LinuxControl;",
            "pub struct M33Realtime;",
            "pub struct Rp2040Io;",
            "impl appkit::Capsule for Control",
            "impl appkit::LogicalImage<Control> for site::Local<image::LinuxControl>",
            "impl appkit::LogicalImage<Control> for site::Local<image::M33Realtime>",
            "impl appkit::LogicalImage<Control> for site::Local<image::Rp2040Io>",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(2);",
            "can_attach_peer",
        ],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/linux-control.rs",
        linux,
        &[
            "appkit::run::<",
            "site::Local<heterogeneous_split_example::image::LinuxControl>",
            "heterogeneous_split_example::Control",
        ],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/m33-realtime.rs",
        m33,
        &[
            "appkit::run::<",
            "site::Local<heterogeneous_split_example::image::M33Realtime>",
            "heterogeneous_split_example::Control",
        ],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/rp2040-io.rs",
        rp2040,
        &[
            "appkit::run::<",
            "site::Local<heterogeneous_split_example::image::Rp2040Io>",
            "heterogeneous_split_example::Control",
        ],
    );
    assert_absent(
        "examples/heterogeneous-split-example/src/lib.rs",
        hetero,
        &[
            "macro_rules!",
            "site::linux",
            "site::mcu",
            "site::rp2040",
            "pub mod carrier",
            "RefCell",
            "Vec<",
            "direct syscall completion",
            "pub fn run_linux_control",
            "pub fn run_m33_realtime",
            "pub fn run_rp2040_io",
        ],
    );
}

#[test]
fn cargo_uses_published_hibana_release_and_no_demo_meaning_features() {
    let cargo = include_str!("../Cargo.toml");

    assert_present(
        "Cargo.toml",
        cargo,
        &["hibana = { version = \"0.4.1\", default-features = false }"],
    );
    assert_absent(
        "Cargo.toml",
        cargo,
        &[
            "hibana = { path = \"../hibana\"",
            "[patch.crates-io]",
            "baker-choreofs-demo",
            "baker-choreofs-bad-path-demo",
            "baker-choreofs-bad-payload-demo",
            "baker-choreofs-wrong-object-demo",
            "baker-abort-safe-demo",
            "baker-recoverable-abort-demo",
            "appkit build",
            "proc_macro choreography",
            "platform-host-native",
            "platform-linux",
            "platform-cortex-m",
        ],
    );
}

#[test]
fn plan_fixes_failure_deadline_cancellation_as_fail_closed_evidence() {
    let plan = include_str!("../plan.md");
    let lib = include_str!("../src/lib.rs");
    let appkit = include_str!("../src/appkit.rs");

    assert_present(
        "plan.md",
        plan,
        &[
            "### Failure / Deadline / Cancellation Constitution",
            "Committed Hibana wait semantics are `Progress | Fault`.",
            "Rust public APIs expose committed progress as `Ok(progress) | Err(domain evidence)`.",
            "Committed Fault is terminal evidence, not a route arm.",
            "Hibana also has non-consuming preview/probe points.",
            "A preview/probe mismatch is not protocol progress",
            "Timeout is not a public API.",
            "Deadline is an operational fuse.",
            "A protocol-visible timeout must be written as choreography: Timer / clock /",
            "Public Hibana exposes only these error evidence envelopes:",
            "EndpointError",
            "ResolverError",
            "AttachError",
            "There is no wide `HibanaError` for localside.",
            "Retry after an operational fault is a new choreography instance / new session generation.",
            "Failure never authorizes hidden progress.",
        ],
    );
    assert_present(
        "plan.md",
        plan,
        &[
            "### Failure / Deadline / Cancellation Gate",
            "wait semantics are `Progress | Fault`",
            "no public timeout API",
            "no public cancel / reconnect / same-generation recovery API",
            "no public wide `HibanaError`",
            "no public `EndpointErrorKind` / `ResolverErrorKind` / `AttachErrorKind` decision surface",
            "preview/probe `Err` is non-progress and cannot select hidden progress",
            "operational deadline expiry poisons the current session generation",
            "protocol-visible timeout uses resolver-selected explicit route arm",
        ],
    );

    for (path, source) in [("src/lib.rs", lib), ("src/appkit.rs", appkit)] {
        assert_absent(
            path,
            source,
            &[
                "HibanaError",
                "pub enum EndpointErrorKind",
                "pub struct EndpointErrorKind",
                "pub type EndpointErrorKind",
                "pub enum ResolverErrorKind",
                "pub struct ResolverErrorKind",
                "pub type ResolverErrorKind",
                "pub enum AttachErrorKind",
                "pub struct AttachErrorKind",
                "pub type AttachErrorKind",
                "recv_timeout",
                "send_timeout",
                "offer_timeout",
                "decode_timeout",
                "try_recover",
                "ignore_fault",
                "reconnect",
                "pub fn cancel",
                "pub async fn cancel",
            ],
        );
    }
}
