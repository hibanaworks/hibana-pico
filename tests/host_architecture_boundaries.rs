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

fn appkit_public_source() -> &'static str {
    include_str!("../src/appkit/mod.rs")
}

fn appkit_sources() -> String {
    [
        include_str!("../src/appkit/mod.rs"),
        include_str!("../src/appkit/internal.rs"),
    ]
    .join("\n")
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
fn gate_scans_current_guest_layout_and_ignores_nested_targets() {
    let ignore = include_str!("../.gitignore");
    let gate = include_str!("../scripts/check_plan_pico_gates.sh");
    let pico_nod_app_gate = include_str!("../scripts/check_pico_nod_app.sh");
    let pico_nod_release_gate = include_str!("../scripts/check_pico_nod_release_readiness.sh");
    let wasip1_gate = include_str!("../scripts/check_wasip1_guest_builds.sh");
    let section_gate = include_str!("../scripts/check_baker_section_budgets.sh");

    assert_present(
        ".gitignore",
        ignore,
        &["target/", "/examples/pico-nod/apple/PicoNodApp/.build/"],
    );
    assert_absent(
        ".gitignore",
        ignore,
        &["/apps/wasip1/hibana-wasi-guest/target/"],
    );
    assert_present(
        "scripts/check_plan_pico_gates.sh",
        gate,
        &[
            "src examples guest Cargo.toml",
            "src tests examples guest",
            "bash ./scripts/check_baker_section_budgets.sh",
            "bash ./scripts/check_pico_nod_app.sh",
            "cargo check --workspace --exclude uno-q-heterogeneous --all-targets",
            "--glob '!examples/uno-q-heterogeneous/**'",
            "cargo test -p pico-nod-example",
            "cargo test -p xbot-example",
            "git ls-files --others --exclude-standard | rg -n '/target/'",
            "cargo check -p heterogeneous-split-example --target thumbv6m-none-eabi --bin rp2040-io",
            "(^|[(,])\\s*_[A-Za-z0-9_]+\\s*:",
        ],
    );
    assert_absent("scripts/check_plan_pico_gates.sh", gate, &[" apps"]);
    assert_present(
        "scripts/check_pico_nod_app.sh",
        pico_nod_app_gate,
        &[
            "examples/pico-nod/apple/PicoNodApp",
            "swift test",
            "swift build -c release",
            "xcodebuild",
            "-destination 'generic/platform=iOS'",
            "CODE_SIGNING_ALLOWED=NO",
            "Metadata extraction skipped",
            "warning:|error:",
        ],
    );
    assert_present(
        "scripts/check_pico_nod_release_readiness.sh",
        pico_nod_release_gate,
        &[
            "PICO_NOD_APPLE_TEAM_ID",
            "PICO_NOD_BUNDLE_ID",
            "PICO_NOD_APNS_KEY_ID",
            "PICO_NOD_APNS_PRIVATE_KEY_PATH",
            "PICO_NOD_STORE_ISSUER_ID",
            "PICO_NOD_STORE_PRIVATE_KEY_PATH",
            "PICO_NOD_TLS_TERMINATION",
            "PICO_NOD_EXTERNAL_ACTION_ENDPOINT",
            "PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH",
            "xcodebuild first launch setup",
            "pico-nod release readiness: not ready",
        ],
    );
    assert_present(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_gate,
        &[
            "expected_wasms=(",
            "wasip1-led-choreofs-traffic-cycle.wasm",
            "wasip1-led-choreofs-traffic-once.wasm",
            "rm -rf \"$target_dir\"",
            "mkdir -p \"$target_dir\"",
            "--initial-memory=65536",
            "--max-memory=65536",
            "-zstack-size=4096",
            "diff -u \"$expected_list\" \"$actual_list\"",
        ],
    );
    assert_absent(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_gate,
        &["sock_(accept|recv|send|shutdown)"],
    );
    assert_present(
        "scripts/check_baker_section_budgets.sh",
        section_gate,
        &[
            "budget_for_bin()",
            "baker-choreofs-traffic-loop",
            "section_size()",
            "check_budget \"$bin\" \".text\"",
            "check_budget \"$bin\" \".rodata\"",
            "check_budget \"$bin\" \".data\"",
            "check_budget \"$bin\" \".bss\"",
            "flash(.text+.rodata+.data)",
            "section-budget bin=%s",
        ],
    );
}

#[test]
fn appkit_host_runs_deadline_clock_from_wall_time_not_poll_count() {
    let appkit = include_str!("../src/appkit/internal.rs");

    assert_present(
        "src/appkit/internal.rs",
        appkit,
        &[
            "struct HostMonotonicClock",
            "start: std::time::Instant",
            "self.start.elapsed().as_millis()",
            "type AppkitAttachClock = HostMonotonicClock",
            "type AppkitAttachClock = hibana::integration::runtime::CounterClock",
            "fn new_appkit_attach_clock() -> AppkitAttachClock",
            "let clock = new_appkit_attach_clock();",
            "new_appkit_attach_clock(),",
        ],
    );
}

#[test]
fn uno_q_uart_carrier_defaults_to_hardware_safe_byte_pacing() {
    let uno_q = include_str!("../examples/uno-q-heterogeneous/src/lib.rs");

    assert_present(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[
            "UNO_Q_HIBANA_UART_BYTE_US",
            ".unwrap_or(10_000)",
            "UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS",
            "UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS",
        ],
    );
    assert_absent(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[".unwrap_or(1_000)", ".unwrap_or(5_000)"],
    );
}

#[test]
fn appkit_has_capsule_shape_without_legacy_facades() {
    let appkit_public = appkit_public_source();
    let appkit = appkit_sources();

    assert_present(
        "src/appkit/mod.rs",
        appkit_public,
        &[
            "mod internal;",
            "pub use crate::choreography::protocol::BuiltInLabelUniverse as BuiltInUniverse;",
            "pub use internal::{",
            "Capsule",
            "LogicalImage",
            "Placement",
            "ArtifactBundle",
            "Localside",
            "run",
        ],
    );
    assert_absent(
        "src/appkit/mod.rs",
        appkit_public,
        &[
            "pub mod",
            "pub struct",
            "pub enum",
            "pub trait",
            "pub fn",
            "pub use internal::*",
        ],
    );

    assert_present(
        "src/appkit",
        &appkit,
        &[
            "pub trait Capsule",
            "fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe>;",
            "pub trait LogicalImage",
            "const REQUESTED_ROLES: RoleSet;",
            "pub trait WasiGuestImage",
            "fn wasi_guest_lease<'guest, const ROLE: u8>() -> WasiGuestLease<'guest>;",
            "fn wasi_budget<const ROLE: u8>() -> BudgetRun",
            "pub trait ArtifactGuestStorage",
            "NoWasi` never leases storage",
            "drive_canonical_wasi_engine",
            "self.wasi_guest_bytes.is_some()",
            "pub struct WasiGuestArena",
            "pub fn lease<'guest>(&'guest mut self)",
            "pub struct WasiGuestLease<'guest>",
            "Guest::init_in_place(ptr, module)?;",
            "pub struct CarrierKind",
            "pub struct PeerImageSet",
            "type Carrier<'a>: hibana::integration::Transport + 'a",
            "fn carrier<'a>() -> Self::Carrier<'a>",
            "fn visit_requested_projected_roles<C, V>",
            "pub trait Placement",
            "pub enum RoleKind",
            "pub struct RoleKindCounts",
            "fn role_kind(role: u8) -> RoleKind",
            "pub trait ArtifactBundle",
            "pub trait Localside",
            "Static WASI import",
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
            "pub capacity_overflow: bool",
            "hibana projection metadata exceeded appkit linked metadata capacity",
            "let config = hibana::integration::runtime::Config::from_resources(",
            "attach_tap,",
            "rendezvous_slab,",
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
        "src/appkit",
        &appkit,
        &[
            "pub mod support",
            "pub struct Choreo<",
            "pub trait App",
            "type Program:",
            "pub struct GuestArtifact",
            "pub enum ArtifactError",
            "parse_wasip1_imports",
            "artifact.validate(image_projection.wasi_imports)",
            "logical image artifact must be a WASI Preview 1 artifact or explicit NoWasi",
            "UnsupportedWasiImport",
            "configured_range_end()",
            "0..lane_range_end",
            "endpoint_slots",
            "EmbeddedTaskContextSlot",
            "Context<'static>",
            "context: EmbeddedTaskContextSlot",
            "embedded_task_context(",
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
            "OPERATIONAL_DEADLINE_TICKS",
            "operational deadline fuse into `hibana::integration::runtime::Config`",
            "Rp2040Sio",
            "SioTransport",
            "ActiveWasiGuestLease",
            "InlineWasiGuestLease",
            "StaticWasiGuestLease",
            "MaybeUninit<crate::kernel::engine::wasm::Guest",
            "poll_embedded_future_to_completion",
            "Box<crate::kernel::engine::wasm::Guest",
            "Box<dyn Future",
            "Vec<ScheduledTask",
            "Box::pin",
            "std::vec![",
            "Guest::new(bytes)",
            "pub fn storage<'guest>(&'static self)",
            "pub fn storage<'guest>(&'guest mut self)",
            "pub unsafe fn storage_from_owner",
            "storage_from_owner(",
            "unsafe impl Sync for WasiGuestArena",
            "AtomicBool",
            "compare_exchange(false, true",
            "occupied.store(false",
            "feature = \"platform-host-native\"",
            "platform-host-native",
            "feature = \"platform-linux\"",
            "platform-linux",
            "platform-cortex-m",
        ],
    );
}

#[test]
fn appkit_projection_caps_use_numeric_message_facts_for_wasi_validation() {
    let appkit = appkit_sources();

    assert_present(
        "src/appkit",
        &appkit,
        &[
            "let engine_req_import = wasi_import_for_engine_req_label(spec.label);",
            "if is_engine_ret_label(spec.label)",
            "self.has_loop_continue_head_eff(spec.eff_index)",
            "self.has_loop_break_head_eff(spec.eff_index)",
        ],
    );
    assert_absent(
        "src/appkit",
        &appkit,
        &[
            "ProjectionTypeFingerprint::of::<EngineReq>()",
            "ProjectionTypeFingerprint::of::<EngineRet>()",
            "spec.payload_type == engine_req",
            "spec.payload_type == engine_ret",
        ],
    );
}

#[test]
fn appkit_manifest_peer_attach_does_not_use_host_type_fingerprints_as_authority() {
    let appkit = appkit_sources();
    let can_attach = appkit
        .split("pub fn can_attach_peer(&self, peer: &Self) -> bool {")
        .nth(1)
        .and_then(|tail| tail.split("pub const fn peer_images").next())
        .expect("ImageManifest::can_attach_peer body");

    assert_present(
        "src/appkit",
        can_attach,
        &[
            "self.choreography_fingerprint == peer.choreography_fingerprint",
            "self.choreography_session_id == peer.choreography_session_id",
            "self.projected_role_set == peer.projected_role_set",
            "self.peer_images().contains(peer.logical_image_id)",
        ],
    );
    assert_absent(
        "src/appkit",
        can_attach,
        &[
            "self.capsule_fingerprint == peer.capsule_fingerprint",
            "self.placement_fingerprint == peer.placement_fingerprint",
            "self.label_universe_fingerprint == peer.label_universe_fingerprint",
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
fn wasip1_guest_programs_are_separate_from_examples_and_follow_core_wasi_allowlist() {
    let root_cargo = include_str!("../Cargo.toml");
    let helper = include_str!("../guest/hibana-wasip1-guest/src/lib.rs");
    let sys = include_str!("../guest/hibana-wasip1-guest/src/sys.rs");
    let program_manifest = include_str!("../guest/wasip1-programs/Cargo.toml");
    let baker_guest_manifest = include_str!("../examples/baker-firmware/wasip1/guest/Cargo.toml");
    let wasip1_build_script = include_str!("../scripts/check_wasip1_guest_builds.sh");

    assert_present(
        "guest/hibana-wasip1-guest/src/lib.rs",
        helper,
        &["pub mod choreofs;", "pub mod time;"],
    );
    assert_absent(
        "guest/hibana-wasip1-guest/src/lib.rs",
        helper,
        &["pub mod baker;", "pub mod net;"],
    );
    assert_present(
        "guest/hibana-wasip1-guest/src/sys.rs",
        sys,
        &["fn path_open", "fn fd_write", "fn poll_oneoff"],
    );
    assert_absent(
        "guest/hibana-wasip1-guest/src/sys.rs",
        sys,
        &[
            "fn sock_send",
            "fn sock_recv",
            "fn sock_shutdown",
            "fn sock_accept",
        ],
    );
    assert_present(
        "guest/wasip1-programs/Cargo.toml",
        program_manifest,
        &[
            "name = \"hibana-pico-wasip1-programs\"",
            "wasip1-std-choreofs-read",
            "wasip1-std-choreofs-append",
            "wasip1-std-choreofs-static-write",
            "wasip1-memory-grow-ok",
        ],
    );
    assert_absent(
        "guest/wasip1-programs/Cargo.toml",
        program_manifest,
        &[
            "wasip1-led-",
            "wasip1-std-sock",
            "wasip1-std-stream-control",
            "baker",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/guest/Cargo.toml",
        baker_guest_manifest,
        &[
            "name = \"baker-wasip1-guest\"",
            "wasip1-led-choreofs-traffic-cycle",
            "../../../../guest/hibana-wasip1-guest",
        ],
    );
    assert_absent(
        "Cargo.toml",
        root_cargo,
        &[
            "apps/wasip1/hibana-wasip1-guest",
            "apps/wasip1/swarm-node-apps",
            "guest/swarm-node-apps",
            "apps/wasip1/wasip1-programs",
            "examples/wasip1-guests",
        ],
    );
    assert_absent(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_build_script,
        &[
            "guest/swarm-node-apps",
            "swarm-actuator.wasm",
            "swarm-coordinator.wasm",
            "swarm-gateway.wasm",
            "swarm-sensor.wasm",
        ],
    );
}

#[test]
fn private_baker_artifact_contains_two_logical_images_without_runtime_escape() {
    let baker_manifest = include_str!("../examples/baker-firmware/Cargo.toml");
    let baker = include_str!("../examples/baker-firmware/src/lib.rs");
    let traffic_bin = include_str!("../examples/baker-firmware/src/bin/traffic.rs");
    let choreofs_bin = include_str!("../examples/baker-firmware/src/bin/choreofs_traffic.rs");
    let choreofs_loop_bin =
        include_str!("../examples/baker-firmware/src/bin/choreofs_traffic_loop.rs");
    let baker_wasi_guest_lib = include_str!("../examples/baker-firmware/wasip1/guest/src/lib.rs");
    let choreofs_wasi_guest = include_str!(
        "../examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs"
    );
    let choreofs_wasi_once_guest = include_str!(
        "../examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs"
    );
    let fail_safe_bin = include_str!("../examples/baker-firmware/src/bin/fail_safe.rs");
    let recovery_bin = include_str!("../examples/baker-firmware/src/bin/recovery.rs");
    let many_reentry_bin = include_str!("../examples/baker-firmware/src/bin/many_reentry.rs");
    let endpoint_poison_bin = include_str!("../examples/baker-firmware/src/bin/endpoint_poison.rs");
    let timer_route_bin = include_str!("../examples/baker-firmware/src/bin/timer_route.rs");
    let baker_hardware_script = include_str!("../scripts/run_baker_link_hardware_pattern.sh");
    let readme = include_str!("../README.md");

    assert_present(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "static BAKER_BOOT2_W25Q080: [u8; 256]",
            "CLOCKS_CLK_SYS_RESUS_CTRL",
            "PLL_SYS_FBDIV_125",
            "PLL_SYS_POSTDIV_125MHZ",
            "CLOCKS_CLK_SYS_SELECTED_AUX",
            "CLOCKS_CLK_PERI_SELECTED_CLK_SYS",
            "write_volatile(CLOCKS_CLK_SYS_CTRL",
            "WATCHDOG_TICK_ENABLE | (BAKER_TIMER_TICK_CYCLES & 0x01ff)",
            "rp2040_sio::core_id()",
            "pub struct DriverImage;",
            "pub struct EngineImage;",
            "pub struct SioTransport",
            "fn open<'a>(",
            "lane: u8",
            "SioRx::new(local_role, session_id, lane)",
            "pending: Option<PendingTxFrame>",
            "struct SioRxAccumulator",
            "static mut SIO_RX_ACCUM_CORE0",
            "static mut SIO_RX_ACCUM_CORE1",
            "fn rx_accumulator(local_role: u8) -> *mut SioRxAccumulator",
            "sio_rx_accumulator_is_local_role_owned_across_lanes",
            "BAKER_ENGINE_WASI_GUEST_ARENA",
            "baker_engine_wasi_guest_lease",
            "addr_of_mut!(BAKER_ENGINE_WASI_GUEST_ARENA)",
            "arena.lease()",
            "fifo::try_push(word)",
            "fifo::try_pop()",
            "context.waker().wake_by_ref()",
            "if frame.session_id != rx.session_id || frame.lane != rx.lane",
            "store_demux_frame",
            "take_demux_frame(rx.local_role, rx.session_id, rx.lane)",
            ".hint_frame_label",
            ".take()",
            "rx.delivered = true;",
            "core::task::Poll::Ready(Ok(hibana::integration::wire::Payload::new(",
            "delivered_sio_payload_emits_route_hint_once",
            "staged_sio_payload_emits_route_hint_before_delivery",
            "pub trait BakerCapsuleFacts",
            "pub fn baker_timer_route_resolver_ready(timeout_ms: u64) -> bool",
            "appkit::run::<DriverImage, C>",
            "appkit::run::<EngineImage, C>",
        ],
    );
    assert_absent(
        "examples/baker-firmware/Cargo.toml",
        baker_manifest,
        &["rp2040-boot2"],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "rp2040_boot2::",
            "RunCtx",
            "project_role",
            "g::Role<2>",
            "RoleSet::from_bits(0b101)",
            "pending_words: [u32; SIO_FRAME_WORDS]",
            "direct syscall completion",
            "site::rp2040",
            "core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), stage)",
            "option_env!(\"HIBANA_BAKER_PATTERN\")",
            "run_selected_pattern",
            "fn main()",
            "_lane: u8",
            "impl appkit::Capsule for",
            "const GREEN_LED: appkit::ChoreoFsObject",
            "const YELLOW_LED: appkit::ChoreoFsObject",
            "const RED_LED: appkit::ChoreoFsObject",
            "BakerChoreoFsRouteContinue",
            "BakerChoreoFsRouteBreak",
            "baker_drive_wasi_engine",
            "baker_choreofs_driver",
            "fifo::push_blocking",
            "fifo::pop_blocking",
            "pub fn push_blocking",
            "pub fn pop_blocking",
            "stage_route_hint_from_fifo",
            ".or_else(|| stage_route_hint_from_fifo(rx))",
            "static BAKER_WASI_GUEST_ARENA",
            "fn baker_wasi_guest_lease",
            "baker_control_engine_one_cycle",
            "baker_control_driver_one_cycle",
            "baker_many_reentry_engine",
            "baker_many_reentry_driver",
            "pub fn baker_timer_route_arm",
            "pub fn baker_timer_route_ready",
            "pub fn baker_timer_route_finish",
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
            "wasip1-led-choreofs-traffic-once.wasm",
            "driver_proc_exit(&mut ctx).await?",
            "LABEL_WASI_PROC_EXIT",
            "baker_firmware::run::<ChoreoFsTraffic>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/choreofs_traffic.rs",
        choreofs_bin,
        &[
            "WasiImportLoopContinue",
            "WasiImportLoopBreak",
            "g::route(",
            "REENTRY_CYCLES",
            "offer_engine_req",
            "ctx.endpoint().offer().await",
            "wasip1-led-choreofs-traffic-cycle.wasm",
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
            "WasiImportLoopContinue",
            "WasiImportLoopBreak",
            "g::route(",
            "wasip1-led-choreofs-traffic-cycle.wasm",
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
        "examples/baker-firmware/wasip1/guest/src/lib.rs",
        baker_wasi_guest_lib,
        &[
            "#![no_std]",
            "use hibana_wasip1_guest::{Error, Result, choreofs, time};",
            "const DEVICE_PREOPEN_FD: u32 = 9;",
            "const LED_PATH_PREFIX: &str = \"device/led/\";",
            "pub struct Led",
            "time::sleep_ms(ms)",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs",
        choreofs_wasi_guest,
        &[
            "use baker_wasip1_guest::{Led, sleep_ms};",
            "fn main()",
            "Led::open(\"/device/led/green\")",
            "Led::open(\"/device/led/yellow\")",
            "Led::open(\"/device/led/red\")",
            "set_and_wait(&green, true)",
            "set_and_wait(&yellow, true)",
            "set_and_wait(&red, true)",
            "sleep_ms(STEP_MS)",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs",
        choreofs_wasi_once_guest,
        &[
            "use baker_wasip1_guest::{Led, sleep_ms};",
            "fn main()",
            "Led::open(\"/device/led/green\")",
            "Led::open(\"/device/led/yellow\")",
            "Led::open(\"/device/led/red\")",
            "set_and_wait(&green, true)",
            "set_and_wait(&yellow, true)",
            "set_and_wait(&red, true)",
            "sleep_ms(STEP_MS)",
        ],
    );
    assert_absent(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs",
        choreofs_wasi_once_guest,
        &["loop {\n        set_and_wait"],
    );
    assert_absent(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs",
        choreofs_wasi_guest,
        &[
            "#![no_std]",
            "struct Led",
            "LED_PATH_PREFIX",
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
    assert_present(
        "examples/baker-firmware/src/bin/timer_route.rs",
        timer_route_bin,
        &[
            "impl appkit::Capsule for TimerRoute",
            "fn timer_route_resolver",
            "ResourceKind",
            "signals::core::TAG",
            "baker_firmware::baker_timer_route_resolver_ready(100)",
            "Ok(RouteResolution::Defer)",
            "Ok(RouteResolution::Arm(1))",
            "registry.policy::<TIMER_ROUTE_POLICY, 0>",
            "registry.policy::<TIMER_ROUTE_POLICY, 1>",
            "ctx.endpoint().flow::<TimerExpiredRoute>()?",
            "route.send(()).await?",
            "ctx.endpoint().offer().await?",
            "branch.decode::<TimerExpired>().await?",
            "let done = ctx.endpoint().recv::<TimerRouteDone>().await?;",
            "let ack = ctx.endpoint().recv::<TimerRouteAck>().await?;",
            "if done != 1",
            "baker_firmware::run::<TimerRoute>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/timer_route.rs",
        timer_route_bin,
        &[
            "AtomicBool",
            "Ordering",
            "TIMER_FACT_READY",
            "TimerFiredFact",
            "baker_firmware::baker_timer_route_arm(100)",
            "baker_firmware::baker_timer_route_ready()",
            "baker_firmware::baker_timer_route_finish()",
            "baker_firmware::baker_poll_delay(100)",
            "fact.send(&1).await?",
            "record_choreofs_driver_trace(0x5452_011",
            "record_choreofs_engine_error_code(0x5452_4",
            "record_choreofs_engine_error_code(0x5452_7",
            "compare_exchange",
            "fetch_",
            "load(Ordering",
            "store(true, Ordering",
        ],
    );
    assert_present(
        "scripts/run_baker_link_hardware_pattern.sh",
        baker_hardware_script,
        &[
            "timer-route)",
            "bin_name=\"baker-timer-route\"",
            "expected_result=\"48495452\"",
            "deadline-fault)",
            "endpoint-poison)",
        ],
    );
    assert_present(
        "README.md",
        readme,
        &[
            "Transport::open(local_role, session_id, lane)",
            "stores it in SIO frame metadata",
            "`poll_send` and `poll_recv` do not spin inside FIFO push/pop loops",
            "Carrier state is owned by the physical endpoint/core that consumes the stream",
            "The rule is ownership first",
            "ownership can express the state, that is the design",
            "Do not replace ownership",
            "second-line primitive",
            "truly shared concurrently",
            "made single-owner without adding more",
            "read-modify-write atomics",
            "simplest and fastest ownership primitive",
            "RP2040/thumbv6m SIO does",
            "core-owned and structured without atomic slot ownership",
            "embedded WASI guest arena uses a single-owner arena lease",
            "atomics are never a hidden",
            "portability requirement for bare-metal images",
            "arena is intentionally not `Sync`",
            "separate owner arena for each logical image",
            "`NoWasi` logical image must not lease guest storage",
            "recv_frame_hint",
            "route-observation hint-drain",
            "Static WASI import tables are",
            "not admission authority",
            "not reject a `WasiImage` because static imports exceed",
            "An import becomes meaningful only when the guest actually calls it",
            "`appkit` itself is also a curated facade",
            "implementation modules under `src/appkit/` remain",
            "bash ./scripts/check_baker_section_budgets.sh",
            "gates `.text`, `.rodata`, `.data`, `.bss`, and flash-size totals",
        ],
    );
}

#[test]
fn pico_nod_example_keeps_public_ingress_and_external_commit_under_choreography() {
    let manifest = include_str!("../examples/pico-nod/Cargo.toml");
    let plan = include_str!("../examples/pico-nod/plan.md");
    let app_manifest = include_str!("../examples/pico-nod/apple/PicoNodApp/Package.swift");
    let app_project =
        include_str!("../examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj/project.pbxproj");
    let app_scheme = include_str!(
        "../examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj/xcshareddata/xcschemes/PicoNodApp.xcscheme"
    );
    let app_core = include_str!(
        "../examples/pico-nod/apple/PicoNodApp/Sources/PicoNodAppCore/PicoNodAppCore.swift"
    );
    let app_ui =
        include_str!("../examples/pico-nod/apple/PicoNodApp/Sources/PicoNodApp/PicoNodApp.swift");
    let app_tests = include_str!(
        "../examples/pico-nod/apple/PicoNodApp/Tests/PicoNodAppCoreTests/PicoNodAppCoreTests.swift"
    );
    let release_gate = include_str!("../scripts/check_pico_nod_release_readiness.sh");
    let archive_script = include_str!("../scripts/archive_pico_nod_app.sh");
    let acceptor = include_str!("../examples/pico-nod/src/acceptor.rs");
    let acceptor_bin = include_str!("../examples/pico-nod/src/bin/pico-nod-http-acceptor.rs");
    let lib = include_str!("../examples/pico-nod/src/lib.rs");
    let ingress = include_str!("../examples/pico-nod/src/ingress.rs");
    let approval = include_str!("../examples/pico-nod/src/approval.rs");
    let apns = include_str!("../examples/pico-nod/src/apns.rs");
    let billing = include_str!("../examples/pico-nod/src/billing.rs");
    let commit = include_str!("../examples/pico-nod/src/commit.rs");
    let release = include_str!("../examples/pico-nod/src/release.rs");
    let app_store_review = include_str!("../examples/pico-nod/release/app-store-review.md");
    let privacy_labels = include_str!("../examples/pico-nod/release/privacy-labels.md");
    let operations_runbook = include_str!("../examples/pico-nod/release/operations-runbook.md");
    let support = include_str!("../examples/pico-nod/src/support.rs");
    let security = include_str!("../examples/pico-nod/tests/security.rs");

    assert_present(
        "examples/pico-nod/Cargo.toml",
        manifest,
        &[
            "name = \"pico-nod-example\"",
            "hibana-pico",
            "wasm-engine-core",
            "wasip1-sys-fd-write",
            "wasip1-sys-path-open",
        ],
    );
    assert_present(
        "examples/pico-nod/plan.md",
        plan,
        &[
            "public bytes",
            "-> WASI P1 ingress",
            "CommitBoundary is the only external side-effect path",
            "Pico Nod has no database.",
            "shared mutable authority",
            "The HTTP/TLS acceptor can:",
            "Device compromise is a root-trust loss for that device",
            "external_unknown_outcome_fences_without_idempotency_evidence",
            "billing_entitlement_is_fact_not_approval_or_commit",
            "support_actions_are_intents_not_admin_direct_paths",
            "The repository is not App Store ready until",
            "The repository is not production-server ready until",
            "Minimal HTTP byte acceptor",
            "scripts/check_pico_nod_release_readiness.sh",
            "scripts/archive_pico_nod_app.sh",
            "examples/pico-nod/release/app-store-review.md",
            "examples/pico-nod/release/privacy-labels.md",
            "examples/pico-nod/release/operations-runbook.md",
            "PicoNodApp.xcodeproj",
            "must fail closed when production identifiers",
            "pico-nod-http-acceptor -- --preflight",
            "production mode must not",
        ],
    );
    assert_present(
        "scripts/check_pico_nod_release_readiness.sh",
        release_gate,
        &[
            "PICO_NOD_APPLE_TEAM_ID",
            "PICO_NOD_BUNDLE_ID",
            "PICO_NOD_APNS_KEY_ID",
            "PICO_NOD_APNS_PRIVATE_KEY_PATH",
            "PICO_NOD_STORE_ISSUER_ID",
            "PICO_NOD_STORE_PRIVATE_KEY_PATH",
            "PICO_NOD_TLS_TERMINATION",
            "PICO_NOD_EXTERNAL_ACTION_ENDPOINT",
            "PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH",
            "xcodebuild first launch setup",
            "PicoNodApp.xcodeproj/project.pbxproj",
            "LaunchScreen.storyboard",
            "release/app-store-review.md",
            "release/privacy-labels.md",
            "release/operations-runbook.md",
            "archive_pico_nod_app.sh",
            "pico-nod release readiness: not ready",
        ],
    );
    assert_present(
        "scripts/archive_pico_nod_app.sh",
        archive_script,
        &[
            "PICO_NOD_APPLE_TEAM_ID",
            "PICO_NOD_BUNDLE_ID",
            "-project \"$PROJECT\"",
            "-scheme PicoNodApp",
            "-destination 'generic/platform=iOS'",
            "-exportArchive",
            "app-store-connect",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/Package.swift",
        app_manifest,
        &[
            "name: \"PicoNodApp\"",
            ".iOS(.v17)",
            ".macOS(.v14)",
            "Assets.xcassets",
            "PrivacyInfo.xcprivacy",
            "PicoNod.entitlements",
            "LaunchScreen.storyboard",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj/project.pbxproj",
        app_project,
        &[
            "PBXNativeTarget",
            "PicoNodApp.app",
            "XCLocalSwiftPackageReference",
            "PicoNodAppCore",
            "CODE_SIGN_ENTITLEMENTS = Sources/PicoNodApp/PicoNod.entitlements;",
            "ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;",
            "Assets.xcassets in Resources",
            "GENERATE_INFOPLIST_FILE = YES;",
            "INFOPLIST_KEY_UILaunchStoryboardName = LaunchScreen;",
            "INFOPLIST_KEY_UISupportedInterfaceOrientations",
            "SUPPORTED_PLATFORMS = \"iphoneos iphonesimulator macosx\";",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj/xcshareddata/xcschemes/PicoNodApp.xcscheme",
        app_scheme,
        &[
            "BuildableName = \"PicoNodApp.app\"",
            "BlueprintName = \"PicoNodApp\"",
            "buildForArchiving = \"YES\"",
            "ArchiveAction",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/Sources/PicoNodAppCore/PicoNodAppCore.swift",
        app_core,
        &[
            "import CryptoKit",
            "Curve25519.Signing.PrivateKey",
            "ApprovalAction",
            "ApprovalEvidence",
            "displayedHash",
            "verify(_ evidence:",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/Sources/PicoNodApp/PicoNodApp.swift",
        app_ui,
        &[
            "import SwiftUI",
            "Button(\"Nod\")",
            "Button(\"Reject\")",
            "Button(\"Fence\")",
            "displayed.displayedHash",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/Tests/PicoNodAppCoreTests/PicoNodAppCoreTests.swift",
        app_tests,
        &[
            "nodEvidenceVerifiesForDisplayedIntent",
            "tamperedActionDoesNotVerify",
            "malformedEvidenceFailsClosed",
        ],
    );
    assert_present(
        "examples/pico-nod/src/acceptor.rs",
        acceptor,
        &[
            "pub struct HttpTlsAcceptor",
            "POST /intent",
            "content-length:",
            "ChunkedUnsupported",
            "cannot_hold_credentials(&self) -> bool",
            "cannot_select_routes(&self) -> bool",
            "cannot_commit_external_actions(&self) -> bool",
        ],
    );
    assert_present(
        "examples/pico-nod/apple/PicoNodApp/Sources/PicoNodApp/Assets.xcassets/AppIcon.appiconset/Contents.json",
        include_str!(
            "../examples/pico-nod/apple/PicoNodApp/Sources/PicoNodApp/Assets.xcassets/AppIcon.appiconset/Contents.json"
        ),
        &["ios-marketing", "pico-nod-1024.png", "iphone", "ipad"],
    );
    assert_present(
        "examples/pico-nod/src/bin/pico-nod-http-acceptor.rs",
        acceptor_bin,
        &[
            "TcpListener::bind",
            "HttpTlsAcceptor::new",
            "WasiIngress::normalize_public_request",
            "127.0.0.1:8787",
            "--preflight",
            "--production",
            "release_preflight(&args)?",
            "is_loopback_bind_address",
            "production bind address must be loopback",
            "production configuration is incomplete",
        ],
    );
    assert_present(
        "examples/pico-nod/src/lib.rs",
        lib,
        &[
            "pub struct PicoNodCapsule;",
            "pub struct PicoNodUniverse;",
            "PICO_NOD_APPROVAL_POLICY",
            "impl appkit::Capsule for PicoNodCapsule",
            "g::route(",
            "site::Local<image::WasiIngressProcess>",
            "site::Local<image::CommitBoundaryProcess>",
            "site::Local<image::HostProofProcess>",
            "type Artifact = appkit::WasiImage<'static>;",
            "type Artifact = appkit::NoWasi;",
            "pub mod release;",
        ],
    );
    assert_absent(
        "examples/pico-nod/src/lib.rs",
        lib,
        &[
            "macro_rules!",
            "with_policy",
            "project_role",
            "InProcessCarrier",
            "RefCell",
            "Atomic",
            "std::",
        ],
    );
    assert_present(
        "examples/pico-nod/src/ingress.rs",
        ingress,
        &[
            "pub struct WasiIngress;",
            "normalize_public_request",
            "cannot_hold_credentials() -> bool",
            "cannot_select_routes() -> bool",
        ],
    );
    assert_present(
        "examples/pico-nod/src/approval.rs",
        approval,
        &[
            "pub struct ApprovalBoundary",
            "ApprovalDecision::Nod",
            "ApprovalDecision::Reject",
            "ApprovalDecision::Fence",
            "displayed_hash(request)",
            "sign_approval(",
        ],
    );
    assert_present(
        "examples/pico-nod/src/apns.rs",
        apns,
        &[
            "pub struct ApnsBoundary",
            "pub trait ApnsProvider",
            "DeviceDeliveryCap",
            "cannot_approve(&self) -> bool",
            "cannot_select_routes(&self) -> bool",
            "cannot_commit_external_actions(&self) -> bool",
        ],
    );
    assert_present(
        "examples/pico-nod/src/billing.rs",
        billing,
        &[
            "pub struct BillingBoundary",
            "pub struct EntitlementFact",
            "EntitlementState::Unknown",
            "require_paid_feature",
            "cannot_approve(&self) -> bool",
            "cannot_commit_external_actions(&self) -> bool",
        ],
    );
    assert_present(
        "examples/pico-nod/src/commit.rs",
        commit,
        &[
            "pub struct CommitBoundary",
            "pub trait ExternalActionApi",
            "CommitOutcome::DuplicateCommitted",
            "UnknownWithoutIdempotencyEvidence",
            "FailedClosed",
        ],
    );
    assert_present(
        "examples/pico-nod/src/release.rs",
        release,
        &[
            "RELEASE_REQUIREMENTS",
            "RELEASE_FILE_REQUIREMENTS",
            "RELEASE_ARTIFACTS",
            "PICO_NOD_APPLE_TEAM_ID",
            "PICO_NOD_BUNDLE_ID",
            "PICO_NOD_APNS_KEY_ID",
            "PICO_NOD_STORE_ISSUER_ID",
            "PICO_NOD_TLS_TERMINATION",
            "PICO_NOD_EXTERNAL_ACTION_ENDPOINT",
        ],
    );
    assert_present(
        "examples/pico-nod/release/app-store-review.md",
        app_store_review,
        &[
            "reviewer",
            "Nod",
            "Reject",
            "Fence",
            "The app is an approval device",
        ],
    );
    assert_present(
        "examples/pico-nod/release/privacy-labels.md",
        privacy_labels,
        &[
            "No tracking.",
            "No analytics SDK.",
            "No external action API credentials in the app.",
            "Audit output must not contain",
        ],
    );
    assert_present(
        "examples/pico-nod/release/operations-runbook.md",
        operations_runbook,
        &[
            "PICO_NOD_TLS_TERMINATION=external-loopback",
            "public TLS",
            "loopback pico-nod-http-acceptor",
            "UnknownWithoutIdempotencyEvidence",
            "Choreography remains the authority.",
        ],
    );
    assert_present(
        "examples/pico-nod/deploy/env.example",
        include_str!("../examples/pico-nod/deploy/env.example"),
        &[
            "PICO_NOD_TLS_TERMINATION=external-loopback",
            "PICO_NOD_APNS_PRIVATE_KEY_PATH=",
            "PICO_NOD_STORE_PRIVATE_KEY_PATH=",
            "PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH=",
        ],
    );
    assert_present(
        "examples/pico-nod/deploy/launchd/com.hibana.pico-nod.plist",
        include_str!("../examples/pico-nod/deploy/launchd/com.hibana.pico-nod.plist"),
        &[
            "com.hibana.pico-nod",
            "--production",
            "127.0.0.1:8787",
            "KeepAlive",
        ],
    );
    assert_present(
        "examples/pico-nod/src/support.rs",
        support,
        &[
            "pub enum SupportAction",
            "pub struct SupportIntent",
            "ActionKind::LocalCommand",
            "WasiIngress::normalize_public_request",
            "cannot_commit_without_approval(&self) -> bool",
            "cannot_select_routes(&self) -> bool",
        ],
    );
    assert_present(
        "examples/pico-nod/tests/security.rs",
        security,
        &[
            "pico_nod_capsule_projects_role_split_and_approval_route",
            "wasi_ingress_output_is_candidate_evidence_not_authority",
            "approval_display_hash_mismatch_rejected",
            "external_failed_closed_records_terminal_fault_without_commit",
            "http_tls_acceptor_forwards_bounded_body_to_wasi_ingress_only",
            "http_tls_acceptor_rejects_unbounded_or_implicit_http_shapes",
            "lost_commit_ack_does_not_commit_twice",
            "no_database_contract_keeps_global_revoke_outside_protocol",
            "apns_delivery_success_does_not_approve_or_commit",
            "billing_entitlement_is_fact_not_approval_or_commit",
            "support_actions_are_intents_not_admin_direct_paths",
            "release_requirements_cover_app_store_and_server_operation",
            "production_server_preflight_fails_closed_without_release_configuration",
            "production_server_preflight_rejects_unreadable_credential_paths",
            "production_server_rejects_public_clear_http_bind_even_when_configured",
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
            "struct ExampleEdgeSlots",
            "static mut EXAMPLE_FRAME_0_TO_1",
            "fn edge_slots_for_send(local_role: u8, peer: u8)",
            "fn edge_slots_for_recv(local_role: u8)",
            "lane: u8",
            "outgoing.lane() != tx.lane",
            "edge.slot_mut(rx.lane)",
            "edge_slots_are_lane_scoped",
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
    assert_absent(
        "examples/heterogeneous-split-example/src/lib.rs",
        hetero,
        &[
            "WASI_GUEST_ARENA",
            "wasi_guest_lease",
            "storage_from_owner(",
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
            "#![cfg_attr(target_os = \"none\", no_std)]",
            "#![cfg_attr(target_os = \"none\", no_main)]",
            "#[panic_handler]",
            "pub extern \"C\" fn main() -> !",
            "appkit::run::<",
            "site::Local<heterogeneous_split_example::image::M33Realtime>",
            "heterogeneous_split_example::Control",
        ],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/rp2040-io.rs",
        rp2040,
        &[
            "#![cfg_attr(target_os = \"none\", no_std)]",
            "#![cfg_attr(target_os = \"none\", no_main)]",
            "#[panic_handler]",
            "pub extern \"C\" fn main() -> !",
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
            "_lane: u8",
            "AtomicBool",
            "AtomicU8",
            "UnsafeCell",
            "Ordering",
            "compare_exchange",
            "HETEROGENEOUS_WASI_GUEST_ARENA",
            "SLOT_WRITING",
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
fn cargo_keeps_plan_private_and_no_demo_meaning_features() {
    let cargo = include_str!("../Cargo.toml");
    let ignore = include_str!("../.gitignore");

    assert_present(
        "Cargo.toml",
        cargo,
        &[
            "exclude = [\"/plan.md\", \"/examples/**/plan.md\"]",
            "\"examples/uno-q-heterogeneous\"",
        ],
    );
    assert_present(".gitignore", ignore, &["/plan.md"]);
    assert_present(
        "Cargo.toml",
        cargo,
        &["hibana = { version = \"0.6.2\", default-features = false }"],
    );
    assert_absent(
        "Cargo.toml",
        cargo,
        &[
            "[patch.crates-io]",
            "hibana = { path = \"../hibana\" }",
            "hibana = { git",
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
fn readme_fixes_failure_deadline_cancellation_as_fail_closed_evidence() {
    let readme = include_str!("../README.md");
    let lib = include_str!("../src/lib.rs");
    let appkit = appkit_sources();

    assert_present(
        "README.md",
        readme,
        &[
            "Committed Hibana wait semantics are `Progress | Fault`.",
            "committed progress as `Ok(progress) | Err(domain evidence)`",
            "Committed Fault is terminal",
            "evidence, not a route arm",
            "Hibana also has non-consuming preview/probe points",
            "a preview/probe mismatch is",
            "not protocol progress and cannot select hidden progress",
            "Timeout is not a public API.",
            "A deadline is\nan internal fuse",
            "A protocol-visible timeout must be written as choreography: Timer / clock /",
            "EndpointError",
            "ResolverError",
            "AttachError",
            "there is no wide `HibanaError` for localside",
            "Retry after an operational fault is a new",
            "choreography instance / new session generation",
            "Failure never authorizes hidden",
            "progress.",
        ],
    );
    assert_present(
        "README.md",
        readme,
        &[
            "wait semantics are `Progress | Fault`",
            "no public timeout API",
            "no public cancel /",
            "reconnect / same-generation recovery API",
            "no public wide `HibanaError`",
            "no public `EndpointErrorKind` / `ResolverErrorKind` /",
            "`AttachErrorKind` decision surface",
            "preview/probe `Err` is non-progress and cannot select hidden progress",
            "Operational deadline expiry is different",
            "it poisons the current session generation",
            "Protocol-visible timeout uses resolver-selected explicit route arm",
        ],
    );

    for (path, source) in [("src/lib.rs", lib), ("src/appkit", appkit.as_str())] {
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

#[test]
fn embedded_runner_keeps_scheduler_and_role_future_poll_boundary_separate() {
    let appkit = appkit_sources();

    let resolver_registration = appkit
        .find("C::register_resolvers(&mut resolver_registry);")
        .expect("appkit attach registers capsule resolvers");
    let role_future_start = appkit
        .find("visit_requested_projected_roles::<C, _>(program, I::REQUESTED_ROLES, &mut visitor);")
        .expect("appkit attach starts projected role futures");
    assert!(
        resolver_registration < role_future_start,
        "bare-metal resolver registration must happen before role futures can run forever"
    );

    assert_present(
        "src/appkit",
        &appkit,
        &[
            "#[inline(never)]\nunsafe fn poll_embedded_stored_task",
            "let task_waker = embedded_task_waker();",
            "let mut task_context = Context::from_waker(task_waker);",
            "poll_embedded_stored_task::<F, E>(future_arena, &mut task_context)",
            "fn embedded_wait_for_event() {\n    core::hint::spin_loop();\n}",
            "future: EmbeddedFutureArena<APPKIT_EMBEDDED_ROLE_FUTURE_BYTES>",
            "embedded_storage: EmbeddedAttachStorageRef<'static>",
            "self.embedded_storage",
            "run_canonical_wasi_engine_forever::<C, ImageTy, ROLE>(\n                                self.embedded_storage,",
            "let ctx_ptr = storage",
            ".future\n            .cast::<EngineCtx",
            "ctx_ptr.write(ctx)",
            "bare-metal logical images attach exactly one role",
        ],
    );
    assert_absent(
        "src/appkit",
        &appkit,
        &[
            "let mut pinned = Pin::new_unchecked(&mut *future_ptr);",
            "let poll = poll_embedded_stored_task::<F, E>;",
            "EMBEDDED_ROLE0_FUTURE_ARENA",
            "EMBEDDED_ROLE1_FUTURE_ARENA",
            "fn embedded_task_waker<const ROLE: u8>",
            "EmbeddedTaskContextSlot",
            "EmbeddedWakerSlot",
            "Context<'static>",
            "waker: EmbeddedWakerSlot",
            "embedded_task_context(",
            "fn embedded_future_arena_for_role<const ROLE: u8>",
            "run_canonical_wasi_engine_forever::<C, ImageTy, ROLE>(ctx)",
            "APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES",
            "APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES",
            "fn poll_localside_once",
            "asm!(\"wfe\"",
            "TestTimerFiredFact",
            "TEST_TIMER_FIRED_FACT_LABEL",
        ],
    );
}
