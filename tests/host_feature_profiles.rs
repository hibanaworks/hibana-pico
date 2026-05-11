use hibana_pico::kernel::features::{
    FeatureMatrix, WASIP1_PREVIEW1_IMPORT_COVERAGE, WASIP1_PREVIEW1_IMPORTS, Wasip1ControlCapacity,
    Wasip1HandlerSet, Wasip1ImportDisposition, Wasip1ImportEffectiveDisposition, Wasip1ImportName,
    Wasip1Syscall, WasmEngineProfile,
};

fn cargo_toml() -> &'static str {
    include_str!("../Cargo.toml")
}

#[test]
fn cargo_features_define_small_pico_and_full_host_profiles() {
    let cargo = cargo_toml();

    for feature in [
        "profile-rp2040-pico-min",
        "profile-rp2040-pico-control-min",
        "profile-rp2040-picow-swarm-min",
        "profile-rp2350-pico2w-swarm-min",
        "profile-host-qemu-swarm",
        "profile-host-linux-wasip1-full",
        "wasm-engine-core",
        "wasm-engine-wasip1-full",
        "wasip1-sys-full",
        "wasip1-ctrl-common",
        "wasip1-ledger-pico-min",
        "wasip1-ledger-embedded-std",
        "wasip1-ledger-host-full",
    ] {
        assert!(cargo.contains(feature), "Cargo.toml is missing {feature}");
    }

    assert!(cargo.contains("\"wasm-engine-core\""));
    assert!(cargo.contains("\"wasip1-sys-fd-write\""));
    assert!(cargo.contains("\"wasip1-sys-poll-oneoff\""));
    assert!(cargo.contains("\"wasip1-sys-proc-exit\""));
    assert!(cargo.contains("\"wasip1-sys-proc-raise\""));
    assert!(cargo.contains("\"wasm-engine-wasip1-full\""));
    assert!(cargo.contains("\"wasip1-sys-full\""));
    assert!(cargo.contains("\"wasip1-ledger-pico-min\""));
}

#[test]
fn feature_control_matrix_keeps_pico_small_and_host_full_as_separate_axes() {
    let pico = FeatureMatrix {
        profiles: Default::default(),
        engine: WasmEngineProfile::Core,
        wasip1_handlers: Wasip1HandlerSet::PICO_MIN,
        wasip1_control: Wasip1ControlCapacity::FULL,
    };
    let host = FeatureMatrix {
        profiles: Default::default(),
        engine: WasmEngineProfile::Wasip1Full,
        wasip1_handlers: Wasip1HandlerSet::FULL,
        wasip1_control: Wasip1ControlCapacity::FULL,
    };

    assert!(pico.can_claim_wasip1_profile());
    assert!(!pico.can_claim_full_ordinary_std());
    assert!(pico.wasip1_handlers.supports(Wasip1Syscall::FdWrite));
    assert!(pico.wasip1_handlers.supports(Wasip1Syscall::PollOneoff));
    assert!(!pico.wasip1_handlers.supports(Wasip1Syscall::FdRead));

    assert!(host.can_claim_wasip1_profile());
    assert!(host.can_claim_full_ordinary_std());
    assert!(host.wasip1_handlers.supports(Wasip1Syscall::FdRead));
    assert!(host.wasip1_handlers.supports(Wasip1Syscall::RandomGet));
    assert!(host.wasip1_handlers.supports(Wasip1Syscall::ProcRaise));
}

#[test]
fn wasi_p1_import_coverage_table_is_the_source_of_truth_for_profiles() {
    assert_eq!(WASIP1_PREVIEW1_IMPORT_COVERAGE.len(), 46);
    assert_eq!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE.len(),
        WASIP1_PREVIEW1_IMPORTS.len()
    );
    for (coverage, import) in WASIP1_PREVIEW1_IMPORT_COVERAGE
        .iter()
        .zip(WASIP1_PREVIEW1_IMPORTS.iter())
    {
        assert_eq!(coverage.kind, *import);
        assert_eq!(coverage.import, import.name());
        assert_eq!(coverage.syscall, import.syscall());
        assert_eq!(coverage.disposition, import.disposition());
    }
    assert_eq!(
        Wasip1ImportName::from_bytes(b"fd_write"),
        Some(Wasip1ImportName::FdWrite)
    );

    for required in [
        "fd_write",
        "fd_read",
        "path_open",
        "fd_pwrite",
        "sock_send",
        "sock_recv",
        "sock_accept",
        "proc_raise",
    ] {
        assert!(
            WASIP1_PREVIEW1_IMPORT_COVERAGE
                .iter()
                .any(|entry| entry.import == required),
            "coverage table missing {required}"
        );
    }

    let full = Wasip1HandlerSet::FULL;
    assert!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .all(|entry| entry.effective(full)
                != Wasip1ImportEffectiveDisposition::UnsupportedByProfile),
        "full profile must classify every Preview 1 import"
    );
    assert!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .any(|entry| entry.disposition == Wasip1ImportDisposition::TypedEnosys),
        "coverage table must make ENOSYS imports explicit"
    );
    assert!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .any(|entry| entry.disposition == Wasip1ImportDisposition::TypedReject),
        "coverage table must make typed-reject reject imports explicit"
    );

    let pico = Wasip1HandlerSet::PICO_MIN;
    assert_eq!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .find(|entry| entry.import == "fd_write")
            .expect("fd_write coverage")
            .effective(pico),
        Wasip1ImportEffectiveDisposition::Supported
    );
    assert_eq!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .find(|entry| entry.import == "path_open")
            .expect("path_open coverage")
            .effective(pico),
        Wasip1ImportEffectiveDisposition::UnsupportedByProfile
    );
    assert_eq!(
        WASIP1_PREVIEW1_IMPORT_COVERAGE
            .iter()
            .find(|entry| entry.import == "sock_send")
            .expect("sock_send coverage")
            .effective(pico),
        Wasip1ImportEffectiveDisposition::UnsupportedByProfile
    );
}

#[test]
fn wasi_p1_import_names_are_not_redeclared_as_manual_byte_tables() {
    for (path, source) in [
        (
            "src/kernel/wasi/mod.rs",
            include_str!("../src/kernel/wasi/mod.rs"),
        ),
        (
            "src/kernel/engine/wasm/mod.rs",
            include_str!("../src/kernel/engine/wasm/mod.rs"),
        ),
    ] {
        for import in WASIP1_PREVIEW1_IMPORTS {
            let forbidden = format!("= b\"{}\"", import.name());
            assert!(
                !source.contains(&forbidden),
                "{path} redeclares WASI P1 import name {forbidden}; use Wasip1ImportName"
            );
        }
    }
}

#[test]
fn choreography_sources_do_not_use_feature_cfg_as_protocol_authority() {
    const CHOREOGRAPHY_SOURCES: &[(&str, &str)] = &[
        (
            "src/choreography/protocol/mod.rs",
            include_str!("../src/choreography/protocol/mod.rs"),
        ),
        (
            "src/choreography/local.rs",
            include_str!("../src/choreography/local.rs"),
        ),
        (
            "src/choreography/swarm.rs",
            include_str!("../src/choreography/swarm.rs"),
        ),
    ];

    for (path, source) in CHOREOGRAPHY_SOURCES {
        assert!(
            !source.contains("feature ="),
            "{path} must not gate protocol shape on Cargo features"
        );
    }
}

#[test]
fn ordinary_std_corpus_is_full_profile_engine_coverage_not_choreography_policy() {
    let cargo = cargo_toml();
    assert!(cargo.contains("wasm-engine-wasip1-full"));
    assert!(cargo.contains("wasip1-sys-full"));

    let smoke_manifest = include_str!("../apps/wasip1/wasip1-smoke-apps/Cargo.toml");
    assert!(smoke_manifest.contains("wasip1-std-core-coverage"));

    let source =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-core-coverage.rs");
    for needle in [
        "Vec::",
        "String::",
        "match ",
        "memory_grow",
        "sqrt",
        "File::from_raw_fd",
    ] {
        assert!(source.contains(needle), "coverage app is missing {needle}");
    }
    assert!(
        !source.contains("#![no_main]") && !source.contains("__main_void"),
        "coverage app must remain an ordinary Rust std fn main artifact"
    );
}

#[test]
fn choreofs_led_smoke_app_uses_safe_guest_wrapper_for_normal_device_access() {
    let smoke_manifest = include_str!("../apps/wasip1/wasip1-smoke-apps/Cargo.toml");
    assert!(
        smoke_manifest.contains("hibana-wasi-guest"),
        "WASI smoke apps must depend on the safe guest wrapper crate"
    );

    let source =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-led-choreofs-open.rs");
    for needle in ["hibana_wasi_guest", "baker", "Led", "sleep_ms"] {
        assert!(
            source.contains(needle),
            "normal ChoreoFS LED app should use the Baker-scoped safe guest wrapper API ({needle})"
        );
    }
    assert!(
        source.contains("fn main()") && !source.contains("#![no_main]"),
        "normal ChoreoFS LED app must remain an ordinary Rust WASI P1 main"
    );
    for needle in [
        "Led::open(\"/device/led/green\")",
        "Led::open(\"/device/led/orange\")",
        "Led::open(\"/device/led/red\")",
        ".set(true)",
        ".set(false)",
    ] {
        assert!(
            source.contains(needle),
            "normal ChoreoFS LED app should use the preferred Baker LED command {needle}"
        );
    }
    for forbidden in [
        "Led::green(",
        "Led::orange(",
        "Led::red(",
        ".on()",
        ".off()",
    ] {
        assert!(
            !source.contains(forbidden),
            "normal ChoreoFS LED app should not use alternate LED helper {forbidden}"
        );
    }
    assert!(
        !source.contains("unsafe extern \"C\"") && !source.contains("unsafe {"),
        "normal ChoreoFS LED app must not declare or call WASI imports directly"
    );

    let lib_source = include_str!("../apps/wasip1/hibana-wasi-guest/src/lib.rs");
    assert!(
        lib_source.contains("pub mod baker")
            && lib_source.contains("pub mod choreofs")
            && lib_source.contains("pub mod net"),
        "hibana-wasi-guest must expose Baker-specific helpers, generic ChoreoFS helpers, and NetworkObject helpers"
    );
    assert!(
        lib_source.contains("mod sys") && !lib_source.contains("pub mod sys"),
        "raw WASI ABI sys module must stay crate-private"
    );
    assert!(
        !lib_source.contains("pub mod device") && !lib_source.contains("pub mod time"),
        "Baker-specific device/time helpers should not be public root modules"
    );

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_paths = [
        "apps/wasip1/hibana-wasi-guest/src/baker.rs",
        "apps/wasip1/hibana-wasi-guest/src/baker/device.rs",
        "apps/wasip1/hibana-wasi-guest/src/baker/time.rs",
        "apps/wasip1/hibana-wasi-guest/src/choreofs.rs",
        "apps/wasip1/hibana-wasi-guest/src/error.rs",
        "apps/wasip1/hibana-wasi-guest/src/lib.rs",
        "apps/wasip1/hibana-wasi-guest/src/net.rs",
        "apps/wasip1/hibana-wasi-guest/src/sys.rs",
    ];

    for path in wrapper_paths {
        let source = std::fs::read_to_string(manifest_dir.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
        let is_sys = path.ends_with("/sys.rs");
        for needle in [
            "#[repr(C)]",
            "unsafe extern \"C\"",
            "unsafe {",
            "wasi_snapshot_preview1",
            "fn path_open(",
            "fn fd_write(",
            "fn poll_oneoff(",
            "fn sock_send(",
            "fn sock_recv(",
            "fn sock_shutdown(",
        ] {
            assert!(
                is_sys || !source.contains(needle),
                "{path} must not contain raw WASI ABI marker {needle:?}; keep it in sys.rs"
            );
        }
    }

    let choreofs_source =
        std::fs::read_to_string(manifest_dir.join("apps/wasip1/hibana-wasi-guest/src/choreofs.rs"))
            .expect("generic ChoreoFS helper module");
    assert!(
        choreofs_source.contains("pub fn open_write")
            && choreofs_source.contains("pub struct WriteFile")
            && choreofs_source.contains("write_once_exact"),
        "generic ChoreoFS module should expose path_open/write helpers without raw ABI"
    );

    let device_source = std::fs::read_to_string(
        manifest_dir.join("apps/wasip1/hibana-wasi-guest/src/baker/device.rs"),
    )
    .expect("Baker LED helper module");
    assert!(
        !device_source.contains("pub fn fd("),
        "Led must not expose the raw fd from the safe wrapper API"
    );
    assert!(
        device_source.contains("DEVICE_PREOPEN_FD: u32 = 9")
            && device_source.contains("device/led/")
            && device_source.contains("choreofs::"),
        "Baker LED helper should hold Baker-specific constants and build on generic ChoreoFS"
    );

    let net_source =
        std::fs::read_to_string(manifest_dir.join("apps/wasip1/hibana-wasi-guest/src/net.rs"))
            .expect("NetworkObject helper module");
    assert!(
        net_source.contains("pub struct Datagram")
            && net_source.contains("pub fn ping_pong")
            && net_source.contains("pub fn gateway")
            && net_source.contains("pub fn send")
            && net_source.contains("pub fn recv")
            && net_source.contains("pub fn shutdown"),
        "NetworkObject helper should expose a bounded Datagram facade"
    );
    assert!(
        net_source.contains("network/datagram/ping-pong")
            && net_source.contains("network/datagram/gateway"),
        "Datagram facade should keep ChoreoFS selectors private to the helper"
    );
    for forbidden in [
        "pub fn fd",
        "SocketAddr",
        "IpAddr",
        "DnsName",
        "connect(",
        "bind(",
        "send_to",
        "recv_from",
        "reconnect",
    ] {
        assert!(
            !net_source.contains(forbidden),
            "NetworkObject helper must not expose socket/network-stack authority marker {forbidden}"
        );
    }

    let std_sock_source =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-sock-send-recv.rs");
    assert!(
        std_sock_source.contains("hibana_wasi_guest::net::Datagram")
            && std_sock_source.contains("Datagram::ping_pong()"),
        "network smoke app should use the safe Datagram capability facade"
    );
    assert!(
        !std_sock_source.contains("unsafe extern \"C\"") && !std_sock_source.contains("unsafe {"),
        "network smoke app must not declare or call WASI imports directly"
    );
}

#[test]
fn pico_plan_gate_runs_wasi_guest_wrapper_and_baker_choreofs_builds() {
    let script = include_str!("../scripts/check_plan_pico_gates.sh");
    assert!(
        script.contains("cargo test --manifest-path apps/wasip1/hibana-wasi-guest/Cargo.toml"),
        "Pico plan gate must run hibana-wasi-guest wrapper unit tests"
    );
    assert!(
        script.contains("--target thumbv6m-none-eabi") && script.contains("baker-choreofs-demo"),
        "Pico plan gate must keep the RP2040 Baker ChoreoFS firmware build gate"
    );
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
fn active_host_linux_full_profile_claims_full_ordinary_std_capacity() {
    use hibana_pico::kernel::features::ACTIVE_FEATURE_MATRIX;

    assert!(ACTIVE_FEATURE_MATRIX.profiles.host_linux_wasip1_full);
    assert!(ACTIVE_FEATURE_MATRIX.can_claim_full_ordinary_std());
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn active_rp2040_pico_profile_is_small_not_full_std() {
    use hibana_pico::kernel::features::ACTIVE_FEATURE_MATRIX;

    assert!(ACTIVE_FEATURE_MATRIX.profiles.rp2040_pico_min);
    assert!(ACTIVE_FEATURE_MATRIX.can_claim_wasip1_profile());
    assert!(!ACTIVE_FEATURE_MATRIX.can_claim_full_ordinary_std());
}

#[test]
#[cfg(feature = "profile-rp2040-picow-swarm-min")]
fn active_rp2040_picow_profile_is_wireless_capacity_not_full_std() {
    use hibana_pico::kernel::features::ACTIVE_FEATURE_MATRIX;

    assert!(ACTIVE_FEATURE_MATRIX.profiles.rp2040_picow_swarm_min);
    assert!(ACTIVE_FEATURE_MATRIX.can_claim_wasip1_profile());
    assert!(!ACTIVE_FEATURE_MATRIX.can_claim_full_ordinary_std());
}

#[test]
#[cfg(feature = "profile-rp2350-pico2w-swarm-min")]
fn active_rp2350_pico2w_profile_is_wireless_capacity_not_full_std() {
    use hibana_pico::kernel::features::ACTIVE_FEATURE_MATRIX;

    assert!(ACTIVE_FEATURE_MATRIX.profiles.rp2350_pico2w_swarm_min);
    assert!(ACTIVE_FEATURE_MATRIX.can_claim_wasip1_profile());
    assert!(!ACTIVE_FEATURE_MATRIX.can_claim_full_ordinary_std());
}
