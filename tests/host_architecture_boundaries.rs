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
fn gate_scans_current_guest_layout_and_ignores_nested_targets() {
    let ignore = include_str!("../.gitignore");
    let gate = include_str!("../scripts/check_plan_pico_gates.sh");
    let wasip1_gate = include_str!("../scripts/check_wasip1_guest_builds.sh");

    assert_present(".gitignore", ignore, &["target/"]);
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
            "git ls-files --others --exclude-standard | rg -n '/target/'",
            "cargo check -p heterogeneous-split-example --target thumbv6m-none-eabi --bin rp2040-io",
            "(^|[(,])\\s*_[A-Za-z0-9_]+\\s*:",
        ],
    );
    assert_absent("scripts/check_plan_pico_gates.sh", gate, &[" apps"]);
    assert_present(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_gate,
        &[
            "expected_wasms=(",
            "wasip1-led-choreofs-traffic-cycle.wasm",
            "rm -rf \"$artifact_dir\"",
            "diff -u \"$expected_list\" \"$actual_list\"",
        ],
    );
    assert_absent(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_gate,
        &["sock_(accept|recv|send|shutdown)"],
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
            "pub trait WasiGuestImage",
            "fn wasi_guest_storage<'guest, const ROLE: u8>() -> WasiGuestStorage<'guest>;",
            "fn wasi_budget<const ROLE: u8>() -> BudgetRun",
            "pub trait ArtifactGuestStorage",
            "NoWasi` never leases storage",
            "drive_canonical_wasi_engine",
            "self.wasi_guest_bytes.is_some()",
            "pub struct WasiGuestArena",
            "pub unsafe fn storage_from_owner",
            "unsafe impl Sync for WasiGuestArena",
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
            "parse_wasip1_imports",
            "artifact.validate(image_projection.wasi_imports)",
            "logical image artifact must be a WASI Preview 1 artifact or explicit NoWasi",
            "UnsupportedWasiImport",
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
    let fail_safe_bin = include_str!("../examples/baker-firmware/src/bin/fail_safe.rs");
    let recovery_bin = include_str!("../examples/baker-firmware/src/bin/recovery.rs");
    let many_reentry_bin = include_str!("../examples/baker-firmware/src/bin/many_reentry.rs");
    let endpoint_poison_bin = include_str!("../examples/baker-firmware/src/bin/endpoint_poison.rs");
    let timer_route_bin = include_str!("../examples/baker-firmware/src/bin/timer_route.rs");
    let baker_hardware_script = include_str!("../scripts/run_baker_link_hardware_pattern.sh");
    let readme = include_str!("../README.md");
    let plan = include_str!("../plan.md");

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
            "baker_engine_wasi_guest_storage",
            "storage_from_owner(core::ptr::addr_of_mut!(",
            "fifo::try_push(word)",
            "fifo::try_pop()",
            "context.waker().wake_by_ref()",
            "if frame.session_id != rx.session_id || frame.lane != rx.lane",
            "store_demux_frame",
            "take_demux_frame(rx.local_role, rx.session_id, rx.lane)",
            "rx.hint_frame_label.take()",
            "pub trait BakerCapsuleFacts",
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
            "static BAKER_WASI_GUEST_ARENA",
            "fn baker_wasi_guest_storage",
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
            "registry.policy::<TIMER_ROUTE_POLICY, 0>",
            "registry.policy::<TIMER_ROUTE_POLICY, 1>",
            "ctx.endpoint().offer().await?",
            "branch.decode::<TimerExpired>().await?",
            "let done = ctx.endpoint().recv::<TimerRouteDone>().await?;",
            "if done != 1",
            "baker_firmware::run::<TimerRoute>()",
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
            "uses an atomic lease on targets with pointer-width RMW",
            "single-owner lease on targets without them",
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
        ],
    );
    assert_present(
        "plan.md",
        plan,
        &[
            "transport `open(local_role, session_id, lane)` receives and preserves",
            "SIO writes the lane into carrier frame metadata",
            "SIO `poll_send` / `poll_recv` are non-blocking carrier polls",
            "partial receive state",
            "physical local-role/core stream parser",
            "ownership first",
            "if physical ownership can express the state, that is the",
            "do not replace ownership with an atomic mailbox",
            "read-modify-write atomics are a second-line primitive",
            "cannot be made single-owner without adding more",
            "true shared concurrent ownership may use read-modify-write atomics",
            "simplest and fastest",
            "RP2040/thumbv6m SIO carrier code must not require pointer-width RMW atomics",
            "uses an atomic lease on targets with",
            "single-owner lease on targets without them",
            "arena is intentionally not `Sync`",
            "separate owner arena for each logical image",
            "`NoWasi` logical images must not lease WASI guest storage",
            "WASI guest storage is supplied by `WasiGuestImage`",
            "`NoWasi` logical images do not implement `WasiGuestImage`",
            "do not expose dummy storage hooks",
            "`recv_frame_hint` is a route-observation hint-drain",
            "static import table is not authority",
            "appkit must not reject a `WasiImage` because static imports exceed",
            "unsupported imports are terminal only when actually called",
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
            "wasi_guest_storage",
            "storage_from_owner(core::ptr::addr_of_mut!(",
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
fn cargo_uses_hibana_release_requirement_and_no_demo_meaning_features() {
    let cargo = include_str!("../Cargo.toml");

    assert_present(
        "Cargo.toml",
        cargo,
        &["hibana = { version = \"0.5.0\", default-features = false }"],
    );
    assert_absent(
        "Cargo.toml",
        cargo,
        &[
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

#[test]
fn embedded_runner_keeps_scheduler_and_role_future_poll_boundary_separate() {
    let appkit = include_str!("../src/appkit.rs");

    assert_present(
        "src/appkit.rs",
        appkit,
        &[
            "#[inline(never)]\nunsafe fn poll_embedded_stored_task",
            "let poll = poll_embedded_stored_task::<F, E>;",
            "let task_context = embedded_task_context(storage);",
            "poll(future_arena, task_context)",
            "future: EmbeddedFutureArena<APPKIT_EMBEDDED_ROLE_FUTURE_BYTES>",
            "embedded_storage: EmbeddedAttachStorageRef<'static>",
            "self.embedded_storage",
            "bare-metal logical images attach exactly one role",
        ],
    );
    assert_absent(
        "src/appkit.rs",
        appkit,
        &[
            "let mut pinned = Pin::new_unchecked(&mut *future_ptr);",
            "pinned.as_mut().poll(&mut task_context)",
            "EMBEDDED_ROLE0_FUTURE_ARENA",
            "EMBEDDED_ROLE1_FUTURE_ARENA",
            "fn embedded_task_waker<const ROLE: u8>",
            "fn embedded_future_arena_for_role<const ROLE: u8>",
            "APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES",
            "APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES",
            "fn poll_localside_once",
        ],
    );
}
