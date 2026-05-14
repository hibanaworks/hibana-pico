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
            "fn choreography() -> impl hibana::substrate::program::Projectable<Self::Universe>;",
            "pub trait LogicalImage",
            "const REQUESTED_ROLES: RoleSet;",
            "fn wasi_guest_storage<'guest, const ROLE: u8>() -> WasiGuestStorage<'guest>;",
            "pub struct WasiGuestArena",
            "pub struct WasiGuestStorage<'guest>",
            "Guest::init_in_place(ptr, module)?;",
            "pub struct CarrierKind",
            "pub struct PeerImageSet",
            "type Carrier<'a>: hibana::substrate::Transport + 'a",
            "fn carrier<'a>() -> Self::Carrier<'a>;",
            "fn visit_requested_projected_roles<C, V>",
            "pub trait Placement",
            "pub enum RoleKind",
            "pub struct RoleKindCounts",
            "fn role_kind(role: u8) -> RoleKind",
            "pub trait ArtifactBundle",
            "pub trait Localside",
            "pub struct EngineCtx<'endpoint, 'guest, C: Capsule, const ROLE: u8>",
            "pub struct GuestArtifact<'a>",
            "pub const fn guest_artifact(&self) -> GuestArtifact<'guest>",
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
            "pub enum ArtifactError",
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
            "AttachedImage",
            "fn run(attached",
            "BoardRuntime",
            "BoardRun",
            "appkit::Program",
            "fragment::Fragment",
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
fn choreography_is_protocol_vocabulary_and_optional_raw_helpers() {
    let choreography_mod = include_str!("../src/choreography/mod.rs");
    let fragment = include_str!("../src/choreography/fragment.rs");
    let protocol_mod = include_str!("../src/choreography/protocol/mod.rs");

    assert_present(
        "src/choreography/mod.rs",
        choreography_mod,
        &["pub mod fragment;", "pub mod protocol;"],
    );
    assert_absent(
        "src/choreography/mod.rs",
        choreography_mod,
        &["pub mod local;", "pub mod proof;"],
    );
    assert_absent(
        "src/choreography/fragment.rs",
        fragment,
        &[
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
fn site_exposes_substrate_facts_not_protocol_authority() {
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
fn private_baker_artifact_contains_two_logical_images_without_runtime_escape() {
    let baker = include_str!("../examples/baker-firmware/src/main.rs");

    assert_present(
        "examples/baker-firmware/src/main.rs",
        baker,
        &[
            "rp2040_sio::core_id()",
            "type DriverImage = site::Local<image::Driver>",
            "type EngineImage = site::Local<image::Engine>",
            "pub struct SioTransport",
            "appkit::run::<DriverImage, BakerTraffic>",
            "appkit::run::<EngineImage, BakerTraffic>",
            "appkit::run::<DriverImage, BakerChoreoFsTraffic>",
            "appkit::run::<EngineImage, BakerChoreoFsTraffic>",
            "appkit::run::<DriverImage, BakerFailSafe>",
            "appkit::run::<DriverImage, BakerRecovery>",
            "appkit::run::<DriverImage, BakerManyReentry>",
            "baker_control_driver_one_cycle",
            "baker_many_reentry_driver",
            "g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/main.rs",
        baker,
        &[
            "RunCtx",
            "project_role",
            "g::Role<2>",
            "RoleSet::from_bits(0b101)",
            "direct syscall completion",
            "site::rp2040",
            "core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), stage)",
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
        &["heterogeneous_split_example::run_linux_control()"],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/m33-realtime.rs",
        m33,
        &["heterogeneous_split_example::run_m33_realtime()"],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/rp2040-io.rs",
        rp2040,
        &["heterogeneous_split_example::run_rp2040_io()"],
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
        ],
    );
}

#[test]
fn cargo_uses_crates_io_hibana_and_no_demo_meaning_features() {
    let cargo = include_str!("../Cargo.toml");

    assert_present(
        "Cargo.toml",
        cargo,
        &["hibana = { version = \"0.3.0\", default-features = false }"],
    );
    assert_absent("Cargo.toml", cargo, &["hibana = { path ="]);
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
