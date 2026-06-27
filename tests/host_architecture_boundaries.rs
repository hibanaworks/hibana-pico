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
fn epf_plan_keeps_receive_metadata_inside_received_payload_boundary() {
    let plan = include_str!("../plan_epf.md");

    assert_present(
        "plan_epf.md",
        plan,
        &[
            "poll_recv returns ReceivedFrame",
            "ReceivedFrame binds payload bytes and optional FrameHeader in the same value",
            "ReceivedFrame::framed header must describe the exact same staged frame as its payload",
            "missing header remains unknown and must not be synthesized from expected context",
            "post-poll receive metadata peek API does not exist",
            "there is no receive-observation side channel",
        ],
    );
    assert_absent(
        "plan_epf.md",
        plan,
        &[
            "peek_recv_frame",
            "poll_recv returns Payload",
            "poll_recv -> Payload",
            "Payload remains the",
            "Receive Frame Peek",
            "can expose staged FrameHeader through",
        ],
    );
}

#[test]
fn public_root_is_the_capsule_surface_only() {
    let lib = include_str!("../src/lib.rs");

    assert_present("src/lib.rs", lib, &["pub mod appkit;"]);
    assert_absent(
        "src/lib.rs",
        lib,
        &[
            "pub mod site;",
            "pub mod choreography;",
            "pub mod board;",
            "pub mod proof;",
            "pub mod kernel;",
            "mod kernel;",
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
fn projected_role_const_generics_do_not_emit_runtime_consumption_noise() {
    let sources = [
        ("src/appkit", appkit_sources()),
        (
            "examples/baker-firmware/src/lib.rs",
            include_str!("../examples/baker-firmware/src/lib.rs").to_owned(),
        ),
        (
            "examples/rp2w-firmware/src/lib.rs",
            include_str!("../examples/rp2w-firmware/src/lib.rs").to_owned(),
        ),
        (
            "examples/uno-q-heterogeneous/src/lib.rs",
            include_str!("../examples/uno-q-heterogeneous/src/lib.rs").to_owned(),
        ),
    ];

    for (path, source) in sources {
        assert_absent(
            path,
            &source,
            &["core::hint::black_box(ROLE)", "black_box(ROLE)"],
        );
    }
}

#[test]
fn placements_fail_fast_instead_of_falling_back_to_a_role_kind() {
    let placements = [
        (
            "examples/baker-firmware/src/lib.rs",
            include_str!("../examples/baker-firmware/src/lib.rs"),
        ),
        (
            "examples/rp2w-firmware/src/lib.rs",
            include_str!("../examples/rp2w-firmware/src/lib.rs"),
        ),
        (
            "examples/heterogeneous-split-example/src/lib.rs",
            include_str!("../examples/heterogeneous-split-example/src/lib.rs"),
        ),
        (
            "examples/uno-q-heterogeneous/src/lib.rs",
            include_str!("../examples/uno-q-heterogeneous/src/lib.rs"),
        ),
        (
            "tests/host_capsule_api.rs",
            include_str!("host_capsule_api.rs"),
        ),
    ];

    for (path, source) in placements {
        assert_absent(
            path,
            source,
            &[
                "fn role_kind(role: u8)",
                "_ => appkit::RoleKind::",
                "_ => RoleKind::",
                "other => appkit::RoleKind::",
                "other => RoleKind::",
            ],
        );
        assert_present(
            path,
            source,
            &["fn role_kind<const ROLE: u8>() -> appkit::RoleKind"],
        );
    }
}

#[test]
fn sio_transport_role_mapping_is_explicit() {
    let baker = include_str!("../examples/baker-firmware/src/lib.rs");
    let rp2w = include_str!("../examples/rp2w-firmware/src/lib.rs");
    assert_present(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "static mut SIO_LOCAL_ROLE_CORE_MASKS: [u8; BAKER_SIO_ROLE_SLOTS]",
            "static mut SIO_LOCAL_ROLE_LANES: [u8; BAKER_SIO_ROLE_SLOTS * 2]",
            "fn mark_local_role_attached(core_id: u8, role: u8, lane: u8)",
            "fn role_attached_on_core(role: u8, core_id: u8) -> bool",
            "fn role_lane_on_core(role: u8, core_id: u8) -> Option<u8>",
            "if let Some(target_lane) = role_lane_on_core(target_role, tx.core_id)",
            "frame.lane = target_lane;",
            "other => panic!(\"baker SIO transport has no core mask for core {other}\")",
            "other => panic!(\"baker SIO transport has no tx session xor for role {other}\")",
            "const SIO_TRACE_NO_FRAME_LABEL: u8 = 0;",
            "SIO_TRACE_NO_FRAME_LABEL",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "fn baker_role_core(role: u8) -> u8",
            "baker SIO transport has no core assignment for role",
            "frame.lane = outgoing.lane();",
        ],
    );
    assert_present(
        "examples/rp2w-firmware/src/lib.rs",
        rp2w,
        &[
            "0 | 2 => 0",
            "1 => 1",
            "2 => 0",
            "const SIO_TRACE_NO_FRAME_LABEL: u8 = 0;",
            "SIO_TRACE_NO_FRAME_LABEL",
            "other => panic!(\"rp2w SIO transport has no core assignment for role {other}\")",
            "other => panic!(\"rp2w SIO transport has no tx session xor for role {other}\")",
        ],
    );

    for (path, source) in [
        ("examples/baker-firmware/src/lib.rs", baker),
        ("examples/rp2w-firmware/src/lib.rs", rp2w),
    ] {
        assert_absent(
            path,
            source,
            &[
                "_ => 0",
                "other => 0",
                ".unwrap_or(0)",
                "pub struct SioTx",
                "pub struct SioRx",
                "pub fn core_id()",
                "pub fn ready_to_recv()",
                "pub fn ready_to_send()",
                "pub fn status()",
                "pub fn clear_errors()",
                "pub fn try_push(",
                "pub fn try_pop()",
                "#[derive(Debug, Default)]\n    pub struct SioTx",
                "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    pub struct SioTransport",
            ],
        );
    }
}

#[test]
fn gate_scans_current_guest_layout_and_ignores_nested_targets() {
    let ignore = include_str!("../.gitignore");
    let gate = include_str!("../scripts/check_plan_pico_gates.sh");
    let wasip1_gate = include_str!("../scripts/check_wasip1_guest_builds.sh");
    let section_gate = include_str!("../scripts/check_baker_section_budgets.sh");

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
            "bash ./scripts/check_baker_section_budgets.sh",
            "cargo check --workspace --exclude uno-q-heterogeneous --all-targets",
            "--glob '!examples/uno-q-heterogeneous/**'",
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
            "baker-session-mismatch",
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
fn appkit_attach_uses_hibana_rendezvous_without_deadline_knobs() {
    let appkit = include_str!("../src/appkit/internal.rs");

    assert_present(
        "src/appkit/internal.rs",
        appkit,
        &[
            "let rendezvous_slab = attach_slab;",
            ".rendezvous(rendezvous_slab, carrier)",
            "let session = appkit_session(<I::Capsule as Capsule>::SESSION_ID);",
            "let mut tap = rendezvous.tap();",
            "tasks.poll_until_quiescent(|| <I::Capsule as Capsule>::observe(&mut tap));",
            "embedded_tasks.poll_forever(|| <I::Capsule as Capsule>::observe(&mut tap))",
        ],
    );
    assert_absent(
        "src/appkit/internal.rs",
        appkit,
        &[
            "HostMonotonicClock",
            "operational deadline fuse into `hibana::runtime::Config`",
        ],
    );
}

#[test]
fn embedded_wasi_timing_markers_stay_private_and_probe_visible() {
    let appkit_public = appkit_public_source();
    let appkit = include_str!("../src/appkit/internal.rs");
    let baker = include_str!("../examples/baker-firmware/src/lib.rs");
    let baker_hardware_script = include_str!("../scripts/run_baker_link_hardware_pattern.sh");
    let runtime_lib = include_str!("../../hibana-wasip1-runtime/src/lib.rs");
    let runtime_exchange = include_str!("../../hibana-wasip1-runtime/src/exchange.rs");
    let runtime_machine = include_str!("../../hibana-wasip1-runtime/src/engine/wasm/machine.rs");
    let runtime_sources = [runtime_lib, runtime_exchange, runtime_machine].join("\n");

    assert_present(
        "src/appkit/internal.rs",
        appkit,
        &[
            "static mut HIBANA_APPKIT_WASI_METRIC_CLOCK: usize",
            "static mut HIBANA_APPKIT_WASI_METRIC_ENABLED: u32",
            "static mut HIBANA_APPKIT_WASI_RESUME_TOTAL_US: u32",
            "static mut HIBANA_APPKIT_WASI_REQUEST_SEND_TOTAL_US: u32",
            "static mut HIBANA_APPKIT_WASI_COMPLETION_RECV_TOTAL_US: u32",
            "static mut HIBANA_APPKIT_WASI_COMPLETE_TOTAL_US: u32",
            "let resume_start = appkit_wasi_metric_start();",
            "record_appkit_wasi_resume(resume_start);",
            "record_appkit_wasi_request_send(request_send_start);",
            "record_appkit_wasi_completion_recv(completion_recv_start);",
            "record_appkit_wasi_complete(complete_start);",
        ],
    );
    assert_absent("src/appkit/mod.rs", appkit_public, &["HIBANA_APPKIT_WASI"]);
    assert_absent(
        "../hibana-wasip1-runtime/src",
        &runtime_sources,
        &[
            "mod metrics;",
            "pub mod metrics;",
            "pub use metrics",
            "crate::metrics::",
            "HIBANA_WASIP1_RUNTIME",
            "last_instruction_count",
            "last_profile",
            "record_post_",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "static mut HIBANA_APPKIT_WASI_METRIC_CLOCK: usize;",
            "baker_appkit_wasi_metric_clock as *const () as usize",
            "install_appkit_wasi_metric_clock();",
            "reset_appkit_wasi_metrics();",
            "set_appkit_wasi_metrics_enabled(false);",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "HIBANA_WASIP1_RUNTIME",
            "install_runtime_wasi_metric_clock",
            "reset_runtime_wasi_metrics",
            "set_runtime_wasi_metrics_enabled",
        ],
    );
    assert_present(
        "scripts/run_baker_link_hardware_pattern.sh",
        baker_hardware_script,
        &[
            "symbol_addr_or_empty HIBANA_APPKIT_WASI_RESUME_COUNT",
            "read_word_or_zero",
            "appkit_wasi_completion_recv_total_us_addr",
            "choreofs_reply_send_fd_write_object_total_us_addr",
            "choreofs_reply_send_poll_oneoff_total_us_addr",
            "choreofs_reply_send_future_poll_total_us_addr",
            "choreofs_reply_encode_total_us_addr",
            "choreofs_reply_send_endpoint_residual_us",
            "choreofs_sio_tx_wait_role0_total_us_addr",
            "choreofs_sio_tx_poll_role0_total_us_addr",
            "require_appkit_wasi_metrics()",
            "require_nonzero_counter choreofs_reply_send_fd_write_object",
            "require_nonzero_counter choreofs_reply_send_poll_oneoff",
            "require_nonzero_counter choreofs_reply_send_future_poll",
            "require_nonzero_counter choreofs_reply_encode",
            "require_nonzero_counter choreofs_sio_tx_poll",
            "require_nonzero_counter appkit_wasi_complete",
        ],
    );
    assert_absent(
        "scripts/run_baker_link_hardware_pattern.sh",
        baker_hardware_script,
        &[
            "HIBANA_WASIP1_RUNTIME",
            "runtime_vm_",
            "runtime_guest_",
            "runtime_lower_",
            "runtime_complete_",
            "runtime_post_",
            "require_runtime_wasi_metrics",
        ],
    );
}

#[test]
fn uno_q_uart_carrier_uses_hardware_safe_byte_pacing() {
    let uno_q = include_str!("../examples/uno-q-heterogeneous/src/lib.rs");

    assert_present(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[
            "UNO_Q_HIBANA_UART_BYTE_US",
            ".unwrap_or(50_000)",
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
fn uno_q_llm_demo_uses_input_role_cli_and_choreofs_only() {
    let manifest = include_str!("../examples/uno-q-heterogeneous/Cargo.toml");
    let uno_q = include_str!("../examples/uno-q-heterogeneous/src/lib.rs");
    let protocol = include_str!("../examples/uno-q-heterogeneous/src/protocol.rs");
    let plan = include_str!("../examples/uno-q-heterogeneous/plan.md");
    let hardware_cli =
        include_str!("../examples/uno-q-heterogeneous/src/bin/uno_q_hardware_proof.rs");
    let shell_once = include_str!(
        "../examples/uno-q-heterogeneous/wasip1/guest/src/bin/uno-q-llm-face-shell.rs"
    );
    let shell_loop = include_str!(
        "../examples/uno-q-heterogeneous/wasip1/guest/src/bin/uno-q-llm-face-shell-loop.rs"
    );

    assert_present(
        "examples/uno-q-heterogeneous/Cargo.toml",
        manifest,
        &[
            "name = \"host-loopback-proof\"\npath = \"src/bin/host_loopback_proof.rs\"\nrequired-features = [\"runtime-wasip1\"]",
            "name = \"uno-q-hardware-proof\"\npath = \"src/bin/uno_q_hardware_proof.rs\"\nrequired-features = [\"runtime-wasip1\"]",
        ],
    );

    assert_present(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[
            "ROLE_HUMAN_INPUT",
            "HumanInputReqMsg",
            "HumanInputTextMsg",
            "HumanInputAckMsg",
            "struct HumanInputSource",
            "std::sync::mpsc::Receiver<Vec<u8>>",
            "UNO_Q_HUMAN_INPUT_MODE",
            "UNO_Q_HUMAN_INPUT_TEXT",
            "UNO_Q_HUMAN_INPUT_VOICE_CMD",
            "HumanInputText::from_bytes",
            "local_llm_human_face_prompt",
            "Return exactly one shell command",
            "LocalLlmServer",
            "POST /completion",
            "GET /health",
            "DEFAULT_UNO_Q_LOCAL_LLM_SERVER",
            "FACE_FRAME_PATH",
            "pub fn wasi_image() -> appkit::WasiImage<'static>",
            "branch.recv::<FdWriteReqMsg>()",
            "FaceFrame::decode_payload",
            "m33_board_show_face(frame.face())",
        ],
    );
    assert_absent(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[
            "HumanInputPollMsg",
            "LABEL_HUMAN_INPUT_POLL",
            "WasiFdWriteBoundaryRouteControl",
            "WasiFdWriteDriverRouteControl",
            "WasiFdWriteBoundaryPeerRouteMsg",
            "fn artifact()",
            "fn wasi_fd_write_route",
        ],
    );
    assert_present(
        "examples/uno-q-heterogeneous/src/protocol.rs",
        protocol,
        &[
            "LABEL_HUMAN_INPUT_REQ",
            "LABEL_HUMAN_INPUT_TEXT",
            "LABEL_HUMAN_INPUT_ACK",
        ],
    );
    assert_absent(
        "examples/uno-q-heterogeneous/src/protocol.rs",
        protocol,
        &["LABEL_HUMAN_INPUT_POLL"],
    );
    assert_present(
        "examples/uno-q-heterogeneous/plan.md",
        plan,
        &[
            "HumanInput role",
            "prompt shell",
            "voice shell",
            "The prompt-file injection path is not\npart of the demo",
            "it does not classify,\n  rewrite, or convert the text into face commands.",
            "M33 and the local LLM never exchange typed messages directly",
            "HardwarePeerLoopProof",
        ],
    );
    assert_present(
        "examples/uno-q-heterogeneous/src/bin/uno_q_hardware_proof.rs",
        hardware_cli,
        &[
            "enum ProofMode",
            "ProofMode::FaceLoop",
            "HardwarePeerLoopProof",
            "::wasi_image()",
            "--prompt-shell",
            "--voice-shell",
            "--voice-cmd",
            "UNO_Q_HUMAN_INPUT_MODE",
            "UNO_Q_HUMAN_INPUT_VOICE_CMD",
        ],
    );

    let removed_prompt_file_const = ["DEFAULT_UNO_Q_LOCAL_LLM_", "USER_PROMPT_FILE"].concat();
    let removed_prompt_file_env = ["UNO_Q_LOCAL_LLM_", "USER_PROMPT_FILE"].concat();
    let removed_interactive_env = ["UNO_Q_LOCAL_LLM_", "INTERACTIVE"].concat();
    let removed_user_prompt_env = ["UNO_Q_LOCAL_LLM_", "USER_PROMPT"].concat();
    let removed_mood_classifier = ["local_llm_", "mood_key"].concat();
    let removed_mood_words_helper = ["local_llm_", "context_has_any"].concat();
    let removed_prompt_file_script = ["inject_llm_", "prompt.sh"].concat();
    assert_absent(
        "examples/uno-q-heterogeneous/src/lib.rs",
        uno_q,
        &[
            removed_prompt_file_const.as_str(),
            removed_prompt_file_env.as_str(),
            removed_interactive_env.as_str(),
            removed_user_prompt_env.as_str(),
            removed_mood_classifier.as_str(),
            removed_mood_words_helper.as_str(),
            "AtomicU8",
            "std::sync::Mutex<Face",
            "ROLE_LOCAL_LLM, g::Role<ROLE_M33_LED_KERNEL>",
            "ROLE_M33_LED_KERNEL, g::Role<ROLE_LOCAL_LLM>",
            "fn uno_q_hardware_wasi_guest",
            "_ => {}",
        ],
    );
    assert_absent(
        "examples/uno-q-heterogeneous/plan.md",
        plan,
        &[
            "polls the HumanInput role",
            removed_prompt_file_env.as_str(),
            removed_interactive_env.as_str(),
            removed_user_prompt_env.as_str(),
            removed_prompt_file_script.as_str(),
            "swaps in the infinite shell-loop guest",
            "surprised alias",
            "shell also accepts",
        ],
    );
    assert_absent(
        "examples/uno-q-heterogeneous/wasip1/guest/src/bin/uno-q-llm-face-shell*.rs",
        &format!("{shell_once}\n{shell_loop}"),
        &[
            "face[0] == b'v'",
            "surprised_accepts_model_alias_v",
            "model alias",
        ],
    );
    assert!(
        !std::path::Path::new(&format!(
            "examples/uno-q-heterogeneous/scripts/{removed_prompt_file_script}"
        ))
        .exists(),
        "file prompt injection helper must not remain in the baseline live demo"
    );
}

#[test]
fn appkit_has_capsule_shape_without_stale_facades() {
    let appkit_public = appkit_public_source();
    let appkit = appkit_sources();

    assert_eq!(
        appkit_public.matches("pub use ").count(),
        3,
        "src/appkit/mod.rs must expose only the curated attach-storage, WASI, and capsule API re-export groups"
    );
    assert_present(
        "src/appkit/mod.rs",
        appkit_public,
        &[
            "mod internal;",
            "pub use internal::{",
            "Capsule",
            "LogicalImage",
            "Localside",
            "Placement",
            "pending",
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
            "pub use internal::ArtifactInput",
            "#[doc(hidden)]\npub use internal::ArtifactInput;",
            "pub use hibana_wasip1_runtime::choreofs::{",
            "ChoreoFs",
            "ChoreoFsFacts",
            "ChoreoFsObject",
            "ChoreoFsObjectSet",
            "LedgerFacts",
            "LocalCtx, Local, Localside",
            "pub struct Local<Image>",
            "impl<Image> Local<Image>",
            "appkit::Local<",
            "Local::new()",
            "SessionId",
            "RoleSet, ArtifactInput, RunReport",
            "RunReport",
        ],
    );

    assert_present(
        "src/appkit",
        &appkit,
        &[
            "pub trait Capsule",
            "type Localside: Localside<Self>;",
            "fn choreography() -> impl hibana::runtime::program::Projectable;",
            "fn observe(_: &mut hibana::runtime::tap::TapPort<'_>) {}",
            "Localside",
            "pub trait LogicalImage",
            "const REQUESTED_ROLES: RoleSet;",
            "pub struct RoleSet {\n    bits: u16,",
            "pub const fn from_bits(bits: u16) -> Self",
            "assert!(bits != 0);",
            "use core::{",
            "num::NonZeroU32",
            "const APPKIT_DEFAULT_SESSION_ID: NonZeroU32 = nonzero_session_id(1);",
            "const fn nonzero_session_id(raw: u32) -> NonZeroU32",
            "None => panic!(\"appkit session id must be nonzero\")",
            "const SESSION_ID: NonZeroU32 = APPKIT_DEFAULT_SESSION_ID;",
            "hibana::runtime::ids::SessionId::new(session_id.get())",
            "pub trait WasiGuestImage",
            "fn wasi_guest_lease<'guest, const ROLE: u8>() -> WasiGuestLease<'guest>;",
            "fn wasi_budget<const ROLE: u8>() -> BudgetRun",
            "pub const fn from_bytes(bytes: &'a [u8]) -> Self",
            "type Capsule: Capsule;",
            "trait ArtifactInput",
            "#[cfg(feature = \"wasm-engine-core\")]\nimpl<'a, I> ArtifactInput<I> for WasiImage<'a>",
            "I: WasiGuestImage,",
            "#[allow(private_bounds)]",
            "NoWasi` never leases storage",
            "drive_canonical_wasi_engine",
            "self.wasi_guest_bytes.is_some()",
            "struct CanonicalWasiEngine",
            "pub struct WasiGuestError {\n    _private: (),",
            "const fn rejected() -> Self",
            "pub struct WasiGuestArena",
            "pub fn lease<'guest>(&'guest mut self)",
            "pub struct WasiGuestLease<'guest>",
            "HibanaWasiGuestStorage::uninit()",
            "hibana_wasip1_runtime::GuestMemory::new",
            "resume_wasi_boundary(budget)",
            "send_wasi_import_request(self.endpoint(), request).await",
            "recv_wasi_import_completion(self.endpoint(), import).await",
            "type Carrier<'a>: hibana::runtime::transport::Transport + 'a",
            "fn carrier<'a>() -> Self::Carrier<'a>",
            "fn visit_requested_projected_roles<C, V>",
            "pub trait ResolverRegistry<'cfg, C: Capsule, const ROLE: u8>",
            "fn resolver<const POLICY: u16>(",
            "fn register_resolvers<'cfg, R, const ROLE: u8>",
            "R: ResolverRegistry<'cfg, Self, ROLE>",
            "struct AttachProjectedResolvers",
            "C::register_resolvers::<_, ROLE>(&mut resolver_registry);",
            "pub trait Placement",
            "pub enum RoleKind",
            "fn role_kind<const ROLE: u8>() -> RoleKind",
            "pub trait Localside",
            "endpoint: hibana::Endpoint<'endpoint, ROLE>",
            "Static WASI import",
            "pub fn run<I>(artifact: impl ArtifactInput<I>)",
            "fn run_with_artifact<I, A>",
            "pub fn pending<'endpoint, E: 'endpoint, const ROLE: u8>",
            "image.safe_state();",
            "const fn appkit_session(",
            "let program = <I::Capsule as Capsule>::choreography();",
            "let projected_roles = collect_projected_roles::<I>(&program);",
            "projected_roles.roles() == I::REQUESTED_ROLES",
            "expect(\"projected RoleProgram count must not overflow\")",
            "expect(\"attached projected role count must not overflow\")",
            "let rendezvous_slab = attach_slab;",
            "SessionKitStorage::<I::Carrier<'_>>::uninit()",
            ".rendezvous(rendezvous_slab, carrier)",
            "let mut tap = rendezvous.tap();",
            "<I::Capsule as Capsule>::observe(&mut tap)",
        ],
    );
    assert_absent(
        "src/appkit",
        &appkit,
        &[
            "ArtifactEvidence",
            "ArtifactGuestStorage",
            "pub trait ArtifactForImage",
            "RunArtifact",
            "fn use_canonical_wasi_engine() -> bool",
            "use_canonical_wasi_engine",
            "fn requested_roles<I>() -> RoleSet",
            "Placement::requested_roles",
            "UnsupportedGuestEvent",
            "fn artifact_for_image",
            "type Report",
            "C::Report",
            "RunReport<R, I>",
            "RunReport<C::Report, I>",
            "RunReport<I>",
            "RunReport<",
            "RunReport",
            "WasiGuestDrive",
            "drive_wasi_guest_once",
            "drive_wasi_guest_once_blocking",
            "pub struct LocalCtx",
            "pub fn image_mut",
            ".image_mut()",
            "type Exit<R>",
            "I::Exit",
            "FromRunReport",
            "from_run_report",
            "pub struct RoleEndpointCtx",
            "pub struct RoleKindCounts",
            "struct EndpointCarrierFacts",
            "const fn session_id(self) -> u32",
            "pub struct EndpointCarrierFacts",
            "pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts",
            "pub const fn attached_role_kinds(&self) -> RoleKindCounts",
            "pub const fn wasi_imports(&self) -> WasiImports",
            "pub const fn wasi_completion_pair_count(&self) -> u8",
            "pub const fn manifest(&self) -> ImageManifest",
            "pub struct ImageManifest",
            "pub struct ProjectionCaps",
            "pub struct LaneSet",
            "pub struct WasiImports",
            "pub const EMPTY: Self = Self {",
            "pub const EMPTY: Self = Self { words",
            "words: [u64; 4]",
            "pub const fn from_bits(bits: u128) -> Self",
            "pub const fn from_words",
            "pub const fn bits(self)",
            "pub const fn words(self)",
            "pub const fn count(self) -> u8",
            "pub const fn union(self",
            "pub const fn is_subset_of(self",
            "pub const fn contains(self, role",
            "pub const fn ids(self)",
            "pub const fn len(self) -> u8",
            "pub const fn contains(self, image: ImageId)",
            "pub struct PeerImageSet",
            "const PEER_IMAGES",
            "peer_image_count",
            "pub const fn id(self) -> u16",
            "pub const fn bytes(self) -> &'a [u8]",
            "pub const fn image(&self) -> &I",
            "pub const fn tag(self) -> u8",
            "pub struct CarrierKind",
            "const CARRIER: CarrierKind",
            "pub const fn projected_roles(&self) -> RoleSet",
            "pub fn can_attach_peer<PeerImage>",
            "pub trait LogicalImage<C: Capsule>",
            "pub trait WasiGuestImage<C: Capsule>",
            "trait ArtifactInput<C: Capsule, I>",
            "type Local: Localside<Self>;",
            "C::Local as",
            "RoleTaskError::Local(",
            "Local(E)",
            "LocalRoleTask",
            "local_role_task",
            "pub fn run<I, C>(artifact: impl ArtifactInput<C, I>)",
            "fn run_with_artifact<I, C, A>",
            "let projected_roles = collect_projected_roles::<C, I>(&program);",
            "appkit::run::<DriverImage, C>",
            "appkit::run::<EngineImage, C>",
            "struct ProjectionCaps",
            "struct WasiImports",
            "choreography_fingerprint",
            "choreography_session_id",
            "capsule_session_id",
            "type_fingerprint",
            "core::any::type_name",
            "host_metadata_type_fingerprint",
            "derive_projection_caps_from_program",
            "SupervisorCtx",
            "RoleKind::Supervisor",
            "fn supervisor<'a",
            "LinkCtx",
            "RoleKind::Link",
            "fn link<'a",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub struct ImageId",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub struct SiteId",
            "pub struct ImageId",
            "pub struct SiteId",
            "appkit::ImageId",
            "appkit::SiteId",
            "BudgetExpired(BudgetExpired)",
            "pub struct WasiGuestError;",
            "pub enum WasiGuestError",
            "impl From<hibana::runtime::wire::CodecError> for WasiGuestError",
            "WasiGuestErrorKind",
            "no_wasi_artifact",
            "guest_rejected",
            "endpoint_rejected",
            "protocol_rejected",
            "unexpected_reply",
            "fn endpoint(_code",
            "EndpointRejected(u32)",
            "Endpoint {\n        code: u32,\n        source: hibana::EndpointError,\n    }",
            "ProtocolRejected(hibana::runtime::wire::CodecError)",
            "const IMAGE_ID:",
            "const SITE_ID:",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub struct CarrierKind",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub struct RoleSet",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub struct NoWasi",
            "SessionId, SiteId",
            "appkit::SessionId",
            "pub struct SessionId(pub u32)",
            "pub struct SessionId {",
            "pub const fn new(raw: u32) -> Self",
            "if raw == 0 { 1 } else { raw }",
            "struct LaneSet",
            "capacity_overflow",
            "hibana projection metadata exceeded appkit linked metadata capacity",
            "artifact_len",
            "byte_len",
            "pub fn derive_projection_caps",
            "pub fn validate_requested_roles",
            "self.count = self.count.saturating_add(1);",
            "self.count = self.count.saturating_add(",
            "pub const HIBANA_TYPED_ROLE_DOMAIN",
            "pub const HIBANA_TYPED_ROLE_DOMAIN_SIZE",
            "pub trait ArtifactBundle",
            "for_image::<",
            "RunInput",
            "pub trait ArtifactInput",
            "pub const fn from_static",
            "WasiImage::from_static",
            "#[cfg(not(feature = \"wasm-engine-core\"))]\nimpl<'a, I> ArtifactInput<I> for WasiImage<'a>",
            "WASI P1 logical image requires wasm-engine-core",
            "pub(super) trait Sealed",
            "type Artifact:",
            "type Artifact;",
            "pub const fn role(&self) -> u8",
            "pub fn role(&self) -> u8",
            "const fn role(&self) -> u8",
            concat!("fn driver_", "facts("),
            "pub const fn facts(&self)",
            "pub fn facts(&self)",
            "pub const fn diagnostic_code(&self) -> u32",
            "pub fn diagnostic_code(&self) -> u32",
            "pub trait Sealed",
            "Driver ownership for a selected WASI P1 guest image",
            "Role-typed wrapper around a hibana endpoint attached by appkit",
            "choreography wrapper",
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
            "pub async fn drive_wasi_guest(",
            "pub fn drive_wasi_guest(",
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
            "fn resolver<const POLICY: u16, const ROLE: u8>",
            "R: ResolverRegistry<'cfg, Self>,",
            "C::register_resolvers(&mut resolver_registry);",
            "if !self.requested_roles.contains(ROLE)",
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
            "host filesystem fallback",
            "hidden fallback",
            "timeout fallback",
            "direct syscall completion",
            "OPERATIONAL_DEADLINE_TICKS",
            "operational deadline fuse into `hibana::runtime::Config`",
            "Rp2040Sio",
            "SioTransport",
            "ActiveWasiGuestLease",
            "InlineWasiGuestLease",
            "StaticWasiGuestLease",
            "poll_embedded_future_to_completion",
            "Box<dyn Future",
            "Vec<ScheduledTask",
            "Box::pin",
            "std::vec![",
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
fn appkit_does_not_keep_invented_projection_capacity_summaries() {
    let appkit = appkit_sources();

    assert_present(
        "src/appkit",
        &appkit,
        &[
            "let projected_roles = collect_projected_roles::<I>(&program);",
            "projected_roles.roles() == I::REQUESTED_ROLES",
            "let session = appkit_session(<I::Capsule as Capsule>::SESSION_ID);",
        ],
    );
    assert_absent(
        "src/appkit",
        &appkit,
        &[
            "struct ProjectionCaps",
            "struct WasiImports",
            "fn derive_projection_caps_from_program<C>",
            "caps.roles = HIBANA_TYPED_ROLE_DOMAIN;",
            "caps.fingerprint",
            "capsule_session_id",
            "type_fingerprint",
            "core::any::type_name",
            "host_metadata_type_fingerprint",
            "I::REQUESTED_ROLES.is_subset_of(projection.roles)",
            "spec.payload_type == engine_req",
            "spec.payload_type == engine_ret",
        ],
    );
}

#[test]
fn appkit_report_does_not_keep_peer_attach_metadata() {
    let appkit = appkit_sources();

    assert_absent(
        "src/appkit",
        &appkit,
        &[
            "pub struct PeerImageSet",
            "const PEER_IMAGES",
            "peer_image_count",
            "pub fn can_attach_peer<PeerImage>",
            "pub const fn projected_roles(&self) -> RoleSet",
            "choreography_fingerprint",
            "choreography_session_id",
            "self.projected_roles == peer.projected_roles",
            "self.peer_images().contains(peer.image_id)",
            "self.capsule_fingerprint == peer.capsule_fingerprint",
            "self.placement_fingerprint == peer.placement_fingerprint",
            "self.label_universe_fingerprint == peer.label_universe_fingerprint",
        ],
    );
}

#[test]
fn baker_abort_protocol_is_small_and_outside_appkit() {
    let appkit_public = appkit_public_source();
    let baker = include_str!("../examples/baker-firmware/src/lib.rs");

    assert!(
        !std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/choreography")).exists(),
        "src/choreography must not exist as a public or private appkit side language"
    );
    assert_absent(
        "src/appkit/mod.rs",
        appkit_public,
        &[
            "mod abort;",
            "EngineAbortBegin",
            "EngineAbortMsg",
            "EngineAbortFence",
            "EngineAbortAck",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "mod labels;",
            "mod control;",
            "mod route;",
            "mod management;",
            "mod network;",
            "mod remote;",
            "mod swarm;",
            "MgmtImage",
            "RouteKey",
            "Topology",
            "TxCommit",
            "StateSnapshot",
            "crate::kernel",
            "crate::machine",
            "crate::port",
            "crate::projects",
            "appkit runtime state",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "pub type EngineAbortBegin",
            "pub type EngineAbortMsg",
            "pub type EngineAbortFence",
            "pub type EngineAbortAck",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/lib.rs",
        baker,
        &[
            "EngineAbortBeginControl",
            "EngineAbortFenceControl",
            "EngineAbortAckControl",
            "pub enum EngineAbortReason",
            "pub struct EngineAbort",
            "FuelExhausted",
            "GuestTrap",
            "UnsupportedImport",
            "BadImportShape",
            "WireEncode for EngineAbort",
            "WirePayload for EngineAbort",
            "LABEL_ENGINE_ABORT_BEGIN_CONTROL",
            "LABEL_ENGINE_ABORT_FENCE_CONTROL",
            "LABEL_ENGINE_ABORT_ACK_CONTROL",
        ],
    );
}

#[test]
fn docs_do_not_teach_internal_artifact_boundary() {
    let readme = include_str!("../README.md");
    let baker_doc = include_str!("../baker.md");
    let rp2w_doc = include_str!("../examples/rp2w-firmware/README.md");
    let memo = include_str!("../memo.md");

    assert_present(
        "README.md",
        readme,
        &[
            "type Placement: appkit::Placement<Self>;",
            "pub trait Placement<C: appkit::Capsule>",
            "fn role_kind<const ROLE: u8>() -> appkit::RoleKind;",
            "Placement is also selected by the projected const\nrole",
        ],
    );
    assert_absent(
        "README.md",
        readme,
        &[
            "RunArtifact",
            "RunInput",
            "ArtifactInput",
            "appkit::*",
            "appkit::Local<",
            "Local<Image>",
            "Local::new()",
            "generic logical-site marker",
            "Capsule::run_engine_image()",
            "appkit::run::<LogicalImage>()",
            "appkit::run::<LogicalImage, Capsule>",
            "type Local;",
            "type Local: appkit::Localside<Self>;",
            "fn role_kind(role: u8)",
            "pub trait LogicalImage<C",
            "appkit::WasiGuestImage<C>",
            "loop-control",
            "control tag",
            "default resolver",
            "fallback",
            "rescue",
            "wrapper",
            "lane mismatch recovery",
            "WASIP1 runtime",
        ],
    );
    assert_absent(
        "baker.md",
        baker_doc,
        &[
            "default resolver",
            "fallback",
            "rescue",
            "lane-recovery loop",
            "WASIP1 runtime",
        ],
    );
    assert_absent(
        "examples/rp2w-firmware/README.md",
        rp2w_doc,
        &["default resolver", "fallback"],
    );
    assert_absent(
        "memo.md",
        memo,
        &[
            "RunArtifact",
            "RunInput",
            "ArtifactInput",
            "hibana::integration::",
            "type Universe:",
            "type Report;",
            "type Local;",
            "type Local: appkit::Localside<Self>;",
            "fn role_kind(role: u8)",
            "rescue",
            "`hibana-pico` wrapper",
            "wrapper 化",
            "Appkit Local provides site facts only",
            "lane mismatch recovery",
            "pub struct RouteKey",
            "## RouteKey",
            "private `projects/`",
            "control kind",
            "fallback",
        ],
    );
    assert_present(
        "memo.md",
        memo,
        &[
            "type Placement: appkit::Placement<Self>;",
            "pub trait Placement<C: appkit::Capsule>",
            "fn role_kind<const ROLE: u8>() -> appkit::RoleKind;",
            "placement は caller が渡す `u8` ではなく projected const role で決まる",
        ],
    );
}

#[test]
fn rp2w_firmware_has_no_porting_residue() {
    let rp2w_manifest = include_str!("../examples/rp2w-firmware/Cargo.toml");
    let rp2w = include_str!("../examples/rp2w-firmware/src/lib.rs");
    let sensor_panel = include_str!("../examples/rp2w-firmware/src/bin/sensor_panel.rs");
    let epf_policy_timer = include_str!("../examples/rp2w-firmware/src/bin/epf_policy_timer.rs");
    let rp2w_bins = format!("{sensor_panel}\n{epf_policy_timer}");

    assert_present(
        "examples/rp2w-firmware/Cargo.toml",
        rp2w_manifest,
        &[
            "sensor-panel = [\"wasm-engine-core\", \"dep:hibana-wifi\", \"dep:uno-q-heterogeneous\"]",
            "name = \"rp2w-sensor-panel\"\npath = \"src/bin/sensor_panel.rs\"\nrequired-features = [\"sensor-panel\"]",
        ],
    );

    assert_present(
        "examples/rp2w-firmware/src/lib.rs",
        rp2w,
        &[
            "mod rp2350_sio",
            "pub struct Rp2wPlacement;",
            "fn run_engine_image();",
            "pub fn run_engine_no_wasi<C>()",
            "#[cfg(feature = \"wasm-engine-core\")]\npub fn run_engine_wasi<'a, C>(image: appkit::WasiImage<'a>)",
            "pub fn run_engine_wasi<'a, C>(image: appkit::WasiImage<'a>)",
            "struct DriverImage<C>(PhantomData<fn() -> C>);",
            "struct EngineImage<C>(PhantomData<fn() -> C>);",
            "pub(super) struct SioTransport",
            "unhandled_exception_handler",
            "unhandled_irq_handler",
            "SIO_TRACE_NO_FRAME_LABEL",
            "fn record_sio_direction_unmapped(local_role: u8, peer_role: u8, direction: u8)",
            "_ => record_sio_direction_unmapped(local_role, peer_role, 0)",
            "_ => record_sio_direction_unmapped(local_role, sender_role, 1)",
        ],
    );
    assert_absent(
        "examples/rp2w-firmware",
        &format!("{rp2w}\n{rp2w_bins}"),
        &[
            "#[cfg(any())]",
            "if true {",
            "if false",
            "rp2040",
            "RP2040",
            "rp2040_sio",
            "rp2040_boot2",
            "default_handler",
            "default_irq_handler",
            "DriverArtifact",
            "EngineArtifact",
            "struct DriverImage;",
            "struct EngineImage;",
            "pub struct DriverImage;",
            "pub struct EngineImage;",
            "pub struct Rp2wSioTransport",
            "pub fn record_epf_policy_timer_irq_ready",
            "rp2w_firmware::EngineImage",
            "rp2w_firmware::DriverImage",
            "Rp2wArtifacts",
            "Rp2wRunInput",
            "Rp2wArtifactInput",
            "type Artifact: appkit::RunInput<Self, EngineImage>",
            "type Artifact: appkit::ArtifactInput<Self, EngineImage>",
            "type Artifact;",
            "type Artifact =",
            "fn artifact()",
            "ArtifactForImage<C, DriverImage>",
            "ArtifactForImage<SensorPanel, DriverImage>",
            "ArtifactForImage<SensorPanel, EngineImage>",
            "default_timer_irq_resolver",
            "fn check_image",
            "check_image::<",
            "REQUESTED_ROLES.contains",
            ".unwrap_or(0)",
            ".unwrap_or(STAGE_HARD_PANIC)",
            "pub struct SioTx",
            "pub struct SioRx",
            "pub fn core_id()",
            "pub fn ready_to_recv()",
            "pub fn ready_to_send()",
            "pub fn status()",
            "pub fn clear_errors()",
            "pub fn try_push(",
            "pub fn try_pop()",
            "#[derive(Debug, Default)]\n    pub struct SioTx",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    pub struct SioTransport",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    pub struct Rp2wSensorSample",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    pub struct Rp2wCyw43Spi",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    pub struct Rp2wCyw43GspiBitbang",
            "pub struct SensorPanel",
            "pub struct SensorPanelLocal",
            "pub enum SensorPanelError",
            "pub struct Rp2wEpfPolicyTimer",
            "pub struct Rp2wEpfPolicyTimerLocal",
            "pub enum Rp2wEpfPolicyTimerError",
        ],
    );
    assert_present(
        "examples/rp2w-firmware/src/bin",
        &rp2w_bins,
        &[
            "rp2w-firmware examples are RP2350 hardware artifacts; build for thumbv8m.main-none-eabi",
        ],
    );
    assert_absent(
        "examples/rp2w-firmware/src/bin",
        &rp2w_bins,
        &[
            "#[cfg(not(all(target_arch = \"arm\", target_os = \"none\")))]\nfn main() {\n    rp2w_firmware::run::<",
        ],
    );
    assert_present(
        "examples/rp2w-firmware/src/bin/sensor_panel.rs",
        sensor_panel,
        &[
            "    pub(super) use ::rp2w_firmware::",
            "    pub(super) fn rp2w_board_init()",
            "    pub(super) type Rp2wCyw43GspiDriver",
            "    pub static mut RP2W_I2C_DETECT_MASK",
        ],
    );
    assert_absent(
        "examples/rp2w-firmware/src/bin/sensor_panel.rs",
        sensor_panel,
        &[
            "    pub use ::rp2w_firmware",
            "    pub const RP2W_UNO_Q",
            "    pub type Rp2wCyw43",
            "    pub struct Rp2wSensorSample",
            "    pub struct Rp2wUnoQWifiTarget",
            "    pub struct Rp2wCyw43Spi",
            "    pub struct Rp2wCyw43GspiBitbang",
            "    pub enum Rp2wWifiFrameError",
            "    pub enum Rp2wCyw43SpiError",
            "    pub enum Rp2wUnoQWifiSendError",
            "    pub fn rp2w_",
            "        pub const fn new",
        ],
    );
}

#[test]
fn appkit_site_images_are_concrete_types_not_local_marker_wrappers() {
    let appkit = include_str!("../src/appkit/internal.rs");

    assert_absent(
        "src/appkit/internal.rs",
        appkit,
        &[
            "pub struct Local<Image>",
            "impl<Image> Local<Image>",
            "appkit::Local<",
            "Local::new()",
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]",
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
            "GuestLedger",
            "WASI import dispatch",
            "protocol authority",
            "authorize WASI",
            "route mismatch recovery",
            "timeout heuristic",
        ],
    );
}

#[test]
fn wasip1_guest_programs_use_std_without_helper_crates() {
    let root_cargo = include_str!("../Cargo.toml");
    let baker_guest_manifest = include_str!("../examples/baker-firmware/wasip1/guest/Cargo.toml");
    let rp2w_guest_manifest = include_str!("../examples/rp2w-firmware/wasip1/guest/Cargo.toml");
    let uno_q_guest_manifest =
        include_str!("../examples/uno-q-heterogeneous/wasip1/guest/Cargo.toml");
    let wasip1_build_script = include_str!("../scripts/check_wasip1_guest_builds.sh");

    assert_present(
        "examples/baker-firmware/wasip1/guest/Cargo.toml",
        baker_guest_manifest,
        &[
            "name = \"baker-wasip1-guest\"",
            "wasip1-led-choreofs-traffic-cycle",
            "wasip1-led-choreofs-traffic-once",
            "wasip1-session-mismatch-fd-write",
        ],
    );
    assert_present(
        "examples/rp2w-firmware/wasip1/guest/Cargo.toml",
        rp2w_guest_manifest,
        &[
            "name = \"rp2w-wasip1-guest\"",
            "rp2w-epf-policy-timer-guest",
            "rp2w-sensor-panel-guest",
        ],
    );
    assert_present(
        "examples/uno-q-heterogeneous/wasip1/guest/Cargo.toml",
        uno_q_guest_manifest,
        &[
            "name = \"uno-q-heterogeneous-wasip1-guest\"",
            "uno-q-llm-face-shell",
            "uno-q-llm-face-shell-loop",
        ],
    );
    let guest_manifests =
        format!("{baker_guest_manifest}\n{rp2w_guest_manifest}\n{uno_q_guest_manifest}");
    assert_absent(
        "examples/*/wasip1/guest/Cargo.toml",
        &guest_manifests,
        &[
            "[dependencies]",
            concat!("hibana-", "wasip1-", "guest"),
            concat!("hibana_", "wasip1_", "guest"),
            concat!("../../../../guest/", "hibana-", "wasip1-", "guest"),
        ],
    );
    assert_present(
        "scripts/check_wasip1_guest_builds.sh",
        wasip1_build_script,
        &[
            "--manifest-path examples/baker-firmware/wasip1/guest/Cargo.toml",
            "--manifest-path examples/rp2w-firmware/wasip1/guest/Cargo.toml",
            "--manifest-path examples/uno-q-heterogeneous/wasip1/guest/Cargo.toml",
            "wasip1-led-choreofs-traffic-cycle.wasm",
            "wasip1-led-choreofs-traffic-once.wasm",
            "wasip1-session-mismatch-fd-write.wasm",
            "rp2w-epf-policy-timer-guest.wasm",
            "rp2w-sensor-panel-guest.wasm",
            "uno-q-llm-face-shell-loop.wasm",
            "uno-q-llm-face-shell.wasm",
        ],
    );
    assert_absent(
        "Cargo.toml",
        root_cargo,
        &[
            concat!("apps/wasip1/", "hibana-", "wasip1-", "guest"),
            "apps/wasip1/swarm-node-apps",
            "guest/swarm-node-apps",
            "apps/wasip1/wasip1-programs",
            "guest/wasip1-programs",
            "examples/wasip1-guests",
            concat!("guest/", "hibana-", "wasip1-", "guest"),
            concat!("hibana-", "wasip1-", "guest"),
            concat!("hibana_", "wasip1_", "guest"),
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
            "guest/wasip1-programs/Cargo.toml",
            "wasip1-clock.wasm",
            "wasip1-exit.wasm",
            "wasip1-infinite-loop.wasm",
            "wasip1-memory-grow-ok.wasm",
            "wasip1-memory-grow-stale-lease.wasm",
            "wasip1-random.wasm",
            "wasip1-stderr.wasm",
            "wasip1-stdin.wasm",
            "wasip1-stdout.wasm",
            "wasip1-timer.wasm",
            "wasip1-trap.wasm",
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
    let session_mismatch_bin =
        include_str!("../examples/baker-firmware/src/bin/session_mismatch.rs");
    let choreofs_wasi_guest = include_str!(
        "../examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs"
    );
    let choreofs_wasi_once_guest = include_str!(
        "../examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs"
    );
    let fail_safe_bin = include_str!("../examples/baker-firmware/src/bin/fail_safe.rs");
    let recovery_bin = include_str!("../examples/baker-firmware/src/bin/recovery.rs");
    let many_reentry_bin = include_str!("../examples/baker-firmware/src/bin/many_reentry.rs");
    let panic_marker_bin = include_str!("../examples/baker-firmware/src/bin/panic_marker.rs");
    let endpoint_fault_bin = include_str!("../examples/baker-firmware/src/bin/endpoint_fault.rs");
    let endpoint_poison_bin = include_str!("../examples/baker-firmware/src/bin/endpoint_poison.rs");
    let preview_probe_bin = include_str!("../examples/baker-firmware/src/bin/preview_probe.rs");
    let deadline_fault_bin = include_str!("../examples/baker-firmware/src/bin/deadline_fault.rs");
    let timer_route_bin = include_str!("../examples/baker-firmware/src/bin/timer_route.rs");
    let capacity_fault_bin = include_str!("../examples/baker-firmware/src/bin/capacity_fault.rs");
    let epf_policy_timer_bin =
        include_str!("../examples/baker-firmware/src/bin/epf_policy_timer.rs");
    let baker_hardware_script = include_str!("../scripts/run_baker_link_hardware_pattern.sh");
    let readme = include_str!("../README.md");
    let baker_bins = format!(
        "{traffic_bin}\n{choreofs_bin}\n{choreofs_loop_bin}\n{session_mismatch_bin}\n{fail_safe_bin}\n{recovery_bin}\n{many_reentry_bin}\n{panic_marker_bin}\n{endpoint_fault_bin}\n{endpoint_poison_bin}\n{preview_probe_bin}\n{deadline_fault_bin}\n{timer_route_bin}\n{capacity_fault_bin}\n{epf_policy_timer_bin}"
    );

    assert_present(
        "examples/baker-firmware/Cargo.toml",
        baker_manifest,
        &[
            "name = \"baker-choreofs-traffic\"\npath = \"src/bin/choreofs_traffic.rs\"\nrequired-features = [\"wasm-engine-core\"]",
            "name = \"baker-choreofs-traffic-loop\"\npath = \"src/bin/choreofs_traffic_loop.rs\"\nrequired-features = [\"wasm-engine-core\"]",
            "name = \"baker-session-mismatch\"\npath = \"src/bin/session_mismatch.rs\"\nrequired-features = [\"wasm-engine-core\"]",
        ],
    );

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
            "unhandled_exception_handler",
            "unhandled_irq_handler",
            "pub(super) struct SioTransport",
            "fn open<'a>(",
            "lane: u8",
            "operational_deadline_ticks,",
            "pending: Option<PendingMailboxFrame>",
            "struct SioMailboxSlot",
            "static mut SIO_MAILBOX_CORE0_TO_CORE1",
            "static mut SIO_MAILBOX_CORE1_TO_CORE0",
            "HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE0_TOTAL_US",
            "HIBANA_CHOREOFS_SIO_RX_WAIT_ROLE1_TOTAL_US",
            "HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE0_TOTAL_US",
            "HIBANA_CHOREOFS_SIO_TX_WAIT_ROLE1_TOTAL_US",
            "HIBANA_CHOREOFS_SIO_TX_POLL_ROLE0_TOTAL_US",
            "HIBANA_CHOREOFS_SIO_TX_POLL_ROLE1_TOTAL_US",
            "fn outbound_mailbox(core_id: u8) -> *mut SioMailboxSlot",
            "fn inbound_mailbox(core_id: u8) -> *mut SioMailboxSlot",
            "compiler_fence(Ordering::Release)",
            "compiler_fence(Ordering::Acquire)",
            "core::arch::asm!(\"dsb sy\", \"sev\", options(nostack, preserves_flags));",
            "core::arch::asm!(\"wfe\", options(nomem, nostack, preserves_flags));",
            "mailbox_frame_publish_and_take_preserves_header_and_payload",
            "frame_session_id",
            "frame_lane",
            "take_mailbox_until_deadline",
            "BAKER_ENGINE_WASI_GUEST_ARENA",
            "baker_engine_wasi_guest_lease",
            "addr_of_mut!(BAKER_ENGINE_WASI_GUEST_ARENA)",
            "arena.lease()",
            "if target_role == tx.local_role",
            "hibana::runtime::transport::TransportError::Failed",
            "publish_mailbox_frame(tx.core_id, tx.session_id, tx.local_role, pending)",
            "take_mailbox_frame(rx.core_id)",
            "context.waker().wake_by_ref()",
            "frame_targets_rx_lane",
            "same_lane_session_mismatch_reaches_endpoint_evidence",
            "let header = rx.frame_header();",
            ".take()",
            "rx.delivered = true;",
            "fn record_choreofs_sio_tx_wait(local_role: u8, us: u32)",
            "fn record_choreofs_sio_tx_poll(local_role: u8, us: u32)",
            "HIBANA_CHOREOFS_REPLY_SEND_FD_WRITE_OBJECT_TOTAL_US",
            "HIBANA_CHOREOFS_REPLY_SEND_POLL_ONEOFF_TOTAL_US",
            "pub fn record_choreofs_reply_send_elapsed",
            "record_choreofs_reply_send_hot_label(label, elapsed)",
            "finish_tx_poll(tx, poll_start, core::task::Poll::Ready(Ok(())))",
            "hibana::runtime::transport::ReceivedFrame::framed(header, rx.payload())",
            "fn record_sio_direction_unmapped(local_role: u8, peer_role: u8, direction: u8)",
            "_ => record_sio_direction_unmapped(local_role, peer_role, 0)",
            "_ => record_sio_direction_unmapped(local_role, sender_role, 1)",
            "poll_epf_diagnostic",
            "epf_spool_tap_event",
            "requeued_sio_payload_returns_same_frame_observation",
            "staged_sio_payload_returns_payload_and_frame_observation",
            "pub trait BakerCapsuleFacts",
            "fn run_engine_image();",
            "pub fn run_engine_no_wasi<C>()",
            "#[cfg(feature = \"wasm-engine-core\")]\npub fn run_engine_wasi<'a, C>(image: appkit::WasiImage<'a>)",
            "pub fn run_engine_wasi<'a, C>(image: appkit::WasiImage<'a>)",
            "pub fn baker_timer_route_resolver_ready(timeout_ms: u64) -> bool",
            "struct DriverImage<C>(PhantomData<fn() -> C>);",
            "struct EngineImage<C>(PhantomData<fn() -> C>);",
            "type Capsule = C;",
            "appkit::run::<DriverImage<C>>(appkit::NoWasi)",
            "appkit::run::<EngineImage<C>>(image)",
            "C::run_engine_image()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/Cargo.toml",
        baker_manifest,
        &[
            "rp2040-boot2",
            "rp2350",
            "RP2350",
            "rp2w",
            "RP2W",
            "pico2w",
            "uart",
            "UART",
        ],
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
            "default_handler",
            "default_irq_handler",
            ".unwrap_or(STAGE_HARD_PANIC)",
            "rp2350",
            "RP2350",
            "rp2w",
            "RP2W",
            "pico2w",
            "uart",
            "UART",
            "fn main()",
            "DriverArtifact",
            "EngineArtifact",
            "struct DriverImage;",
            "struct EngineImage;",
            "pub struct DriverImage;",
            "pub struct EngineImage;",
            "pub struct BakerSioTransport",
            "tx.sent_frames = tx.sent_frames.saturating_add(1);\n                return core::task::Poll::Ready(Ok(()));\n            }\n            if bytes.len() > SIO_FRAME_BYTES",
            "pub fn record_stack_high_water",
            "pub fn record_epf_policy_timer_irq_ready",
            "baker_firmware::EngineImage",
            "baker_firmware::DriverImage",
            "BakerArtifacts",
            "BakerRunInput",
            "BakerArtifactInput",
            "fn check_image",
            "check_image::<",
            "REQUESTED_ROLES.contains",
            "SIO_MAILBOX_DOORBELL",
            "try_pop_until_deadline",
            "fifo::try_push",
            "fifo::try_pop()",
            "type Artifact: appkit::RunInput<Self, EngineImage>",
            "type Artifact: appkit::ArtifactInput<Self, EngineImage>",
            "type Artifact;",
            "type Artifact =",
            "fn artifact()",
            "ArtifactForImage<C, DriverImage>",
            "ArtifactForImage<ChoreoFsTraffic, DriverImage>",
            "ArtifactForImage<ChoreoFsTrafficLoop, DriverImage>",
            "ArtifactForImage<SessionMismatch, DriverImage>",
            " _lane: u8",
            "impl appkit::Capsule for",
            "const GREEN_LED: choreofs::ChoreoFsObject",
            "const YELLOW_LED: choreofs::ChoreoFsObject",
            "const RED_LED: choreofs::ChoreoFsObject",
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
            "const TRAFFIC_STATE: choreofs::ChoreoFsObject",
            "FdBinding::write(FdWriteRow::Object)",
            "impl appkit::Capsule for ChoreoFsTraffic",
            "fn choreography()",
            "impl appkit::Localside<ChoreoFsTraffic> for ChoreoFsTrafficLocal",
            "wasip1-led-choreofs-traffic-once.wasm",
            "fn enter_import(label: u8)",
            "async fn drive_wasi_startup",
            "async fn drive_traffic_cycle",
            "async fn handle_next_state",
            "handle_path_open(",
            "handle_fd_write(ctx, choreofs, request, state).await?",
            "handle_poll_oneoff(ctx, request, timeout_ms).await?",
            "baker_firmware::run::<ChoreoFsTraffic>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/choreofs_traffic.rs",
        choreofs_bin,
        &[
            "WasiImportLoopContinue",
            "WasiImportLoopBreak",
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
            "const TRAFFIC_STATE: choreofs::ChoreoFsObject",
            "FdBinding::write(FdWriteRow::Object)",
            "impl appkit::Capsule for ChoreoFsTrafficLoop",
            "impl appkit::Localside<ChoreoFsTrafficLoop> for ChoreoFsTrafficLoopLocal",
            "fn enter_import(label: u8)",
            "async fn drive_wasi_startup",
            "async fn drive_traffic_cycle",
            "async fn handle_next_state",
            "g::send::<1, 0, FdWriteObjectReqMsg>()",
            ".roll()",
            "loop {",
            "completed_cycles = completed_cycles.saturating_add(1);",
            "wasip1-led-choreofs-traffic-cycle.wasm",
            "baker_firmware::run::<ChoreoFsTrafficLoop>()",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin",
        &baker_bins,
        &[
            "#[cfg(not(all(target_arch = \"arm\", target_os = \"none\")))]\nfn main() {\n    baker_firmware::run::<",
            "BakerArtifacts",
            "ArtifactForImage<ChoreoFsTraffic, EngineImage>",
            "ArtifactForImage<ChoreoFsTrafficLoop, EngineImage>",
            "ArtifactForImage<SessionMismatch, EngineImage>",
            "pub struct Traffic",
            "pub struct TrafficLocal",
            "pub enum TrafficError",
            "pub struct ChoreoFsTraffic",
            "pub struct ChoreoFsTrafficLocal",
            "pub enum ChoreoFsTrafficError",
            "pub struct ChoreoFsTrafficLoop",
            "pub struct ChoreoFsTrafficLoopLocal",
            "pub enum ChoreoFsTrafficLoopError",
            "pub struct FailSafe",
            "pub struct FailSafeLocal",
            "pub enum FailSafeError",
            "pub struct Recovery",
            "pub struct RecoveryLocal",
            "pub enum RecoveryError",
            "pub struct ManyReentry",
            "pub struct ManyReentryLocal",
            "pub enum ManyReentryError",
            "pub struct EndpointFault",
            "pub struct EndpointFaultLocal",
            "pub struct EndpointPoison",
            "pub struct EndpointPoisonLocal",
            "pub struct DeadlineFault",
            "pub struct DeadlineFaultLocal",
            "pub struct SessionMismatch",
            "pub struct SessionMismatchLocal",
            "pub enum SessionMismatchError",
            "pub struct TimerRoute",
            "pub struct TimerRouteLocal",
            "pub enum TimerRouteError",
            "pub struct EpfPolicyTimer",
            "pub struct EpfPolicyTimerLocal",
            "pub enum EpfPolicyTimerError",
            "default_timer_irq_resolver",
            "pub struct CapacityFault",
            "pub struct CapacityFaultLocal",
            "pub struct PreviewProbe",
            "pub struct PreviewProbeLocal",
            "pub enum PreviewProbeError",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin",
        &baker_bins,
        &["baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi"],
    );
    assert_present(
        "examples/baker-firmware/src/bin/session_mismatch.rs",
        session_mismatch_bin,
        &[
            "RuntimeViolation",
            "return Err(SessionMismatchError::RuntimeViolation);",
        ],
    );
    assert_absent(
        "examples/baker-firmware/src/bin/session_mismatch.rs",
        session_mismatch_bin,
        &["return Ok(());"],
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
            "WasiImportLoopContinue",
            "WasiImportLoopBreak",
            "let write_wait = ||",
            "let traffic_cycle",
            "g::seq(write_wait",
            "const DRIVER_REQUESTED_ROLES",
            "const ENGINE_REQUESTED_ROLES",
            "RoleSet::single(2)",
            "RoleSet::from_bits(0b0011)",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs",
        choreofs_wasi_guest,
        &[
            "fs::OpenOptions",
            "io::{self, Write}",
            "thread",
            "time::Duration",
            "fn main()",
            "const TRAFFIC_STATE_PATH: &str = \"/device/traffic/state\";",
            "open_traffic_state()",
            "const TRAFFIC_RED: &[u8] = b\"R\";",
            "const COLOR_STEP: Duration = Duration::from_millis(40);",
            "const YELLOW_BLINK_STEP: Duration = Duration::from_millis(20);",
            "set_state(",
            "TRAFFIC_YELLOW",
            "thread::sleep(delay);",
        ],
    );
    assert_present(
        "examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs",
        choreofs_wasi_once_guest,
        &[
            "fs::OpenOptions",
            "io::{self, Write}",
            "thread",
            "time::Duration",
            "fn main()",
            "const TRAFFIC_STATE_PATH: &str = \"/device/traffic/state\";",
            "open_traffic_state()",
            "const TRAFFIC_RED: &[u8] = b\"R\";",
            "const COLOR_STEP: Duration = Duration::from_millis(40);",
            "const YELLOW_BLINK_STEP: Duration = Duration::from_millis(20);",
            "set_state(",
            "TRAFFIC_YELLOW",
            "thread::sleep(delay);",
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
            concat!("hibana_", "wasip1_", "guest"),
            concat!("baker_", "wasip1_", "guest"),
            "choreofs::",
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
            "const SUCCESS_RESULT: u32 = RESULT_FAIL_SAFE_OK",
            "impl appkit::Localside<FailSafe> for FailSafeLocal",
            "baker_firmware::run::<FailSafe>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/recovery.rs",
        recovery_bin,
        &[
            "impl appkit::Capsule for Recovery",
            "const SUCCESS_RESULT: u32 = RESULT_RECOVERY_OK",
            "impl appkit::Localside<Recovery> for RecoveryLocal",
            "baker_firmware::run::<Recovery>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/many_reentry.rs",
        many_reentry_bin,
        &[
            "impl appkit::Capsule for ManyReentry",
            "EngineAbortBegin",
            "EngineAbortFence",
            "EngineAbortAck",
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
            "record_choreofs_engine_error_code(",
            "offer at send-only phase must not produce continuation",
            "poisoned generation must not send",
            "baker_firmware::run::<EndpointPoison>()",
        ],
    );
    assert_present(
        "examples/baker-firmware/src/bin/timer_route.rs",
        timer_route_bin,
        &[
            "impl appkit::Capsule for TimerRoute",
            "fn timer_route_resolver",
            "DecisionArm",
            "baker_firmware::baker_timer_route_resolver_ready(100)",
            "if baker_firmware::baker_timer_route_irq_observed()",
            "Ok(DecisionArm::Right)",
            "Ok(DecisionArm::Left)",
            "ResolverRef::decision_state",
            ".resolve::<TIMER_ROUTE_POLICY>()",
            "fn register_resolvers<'cfg, R, const ROLE: u8>",
            "R: appkit::ResolverRegistry<'cfg, Self, ROLE>",
            "registry.resolver::<TIMER_ROUTE_POLICY>(resolver);",
            "ctx.send::<TimerExpired>(&1).await?;",
            "let expired = ctx.recv::<TimerExpired>().await?;",
            "baker_firmware::baker_wait_timer_route_irq_observed(500)",
            "let done = ctx.recv::<TimerRouteDone>().await?;",
            "let ack = ctx.recv::<TimerRouteAck>().await?;",
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
            "branch.recv::<TimerExpired>().await?",
            "branch.send::<TimerExpired>(&1).await?",
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
            "capacity-fault)",
            "bin_name=\"baker-capacity-fault\"",
            "require_choreofs_sio_cross_core()",
            "require_nonzero_counter choreofs_sio_core0_to_core1_tx",
            "require_nonzero_counter choreofs_sio_core0_to_core1_rx",
            "require_nonzero_counter choreofs_sio_core1_to_core0_tx",
            "require_nonzero_counter choreofs_sio_core1_to_core0_rx",
        ],
    );
    assert_present(
        "README.md",
        readme,
        &[
            "Transport::open(PortOpen)",
            "logical lane carried by Hibana `Transport::open(PortOpen)`",
            "frame metadata, demultiplexes before yielding payload bytes",
            "bytes plus the staged `FrameHeader` inside the same `ReceivedFrame`",
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
            "There is no separate receive-observation hook",
            "staged frame header crosses the transport boundary only with the",
            "`ReceivedFrame` that carries the payload bytes",
            "Static WASI import",
            "not admission authority",
            "materialized for that logical image",
            "a WASI import becomes meaningful only when the guest actually calls it",
            "`appkit` itself is also a curated facade",
            "Implementation modules under `src/appkit/` stay",
            "bash ./scripts/check_baker_section_budgets.sh",
            "gates `.text`, `.rodata`, `.data`, `.bss`, and flash-size totals",
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
            "pub struct LinuxControl;",
            "pub struct M33Realtime;",
            "pub struct Rp2040Io;",
            "struct ExampleEdgeSlots",
            "static mut EXAMPLE_FRAME_0_TO_1",
            "static mut ROLE2_TO_ROLE0_RECV",
            "fn edge_slots_for_send(local_role: u8, peer: u8)",
            "fn edge_slots_for_recv(local_role: u8)",
            "bump_counter(sent_counter);",
            "bump_counter(recv_counter);",
            "lane: u8",
            "outgoing.lane() != tx.lane",
            "edge.slot_mut(rx.lane)",
            "edge_slots_are_lane_scoped",
            "impl appkit::Capsule for Control",
            "impl appkit::LogicalImage for image::LinuxControl",
            "impl appkit::LogicalImage for image::M33Realtime",
            "impl appkit::LogicalImage for image::Rp2040Io",
            "type Capsule = Control;",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);",
            "const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(2);",
            "appkit::run::<image::LinuxControl>(appkit::NoWasi);",
            "appkit::run::<image::M33Realtime>(appkit::NoWasi);",
            "appkit::run::<image::Rp2040Io>(appkit::NoWasi);",
        ],
    );
    assert_absent(
        "examples/heterogeneous-split-example/src/lib.rs",
        hetero,
        &[
            "WASI_GUEST_ARENA",
            "wasi_guest_lease",
            "storage_from_owner(",
            "can_attach_peer",
            "PeerImageSet",
            "PEER_IMAGES",
            "peer_image_count",
            "_ => {}",
            "appkit::run::<image::LinuxControl, Control>",
            "appkit::run::<image::M33Realtime, Control>",
            "appkit::run::<image::Rp2040Io, Control>",
        ],
    );
    assert_present(
        "examples/heterogeneous-split-example/src/bin/linux-control.rs",
        linux,
        &[
            "appkit::run::<",
            "heterogeneous_split_example::image::LinuxControl",
            "<Image as appkit::LogicalImage>::REQUESTED_ROLES",
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
            "heterogeneous_split_example::image::M33Realtime",
            "<Image as appkit::LogicalImage>::REQUESTED_ROLES",
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
            "heterogeneous_split_example::image::Rp2040Io",
            "<Image as appkit::LogicalImage>::REQUESTED_ROLES",
        ],
    );
    assert_absent(
        "examples/heterogeneous-split-example/src/lib.rs",
        hetero,
        &[
            "macro_rules!",
            " _lane: u8",
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
        &[
            "hibana = { version = \"0.9.4\", default-features = false }",
            "hibana-wasip1-runtime = { path = \"../hibana-wasip1-runtime\", default-features = false }",
        ],
    );
    assert_absent(
        "Cargo.toml",
        cargo,
        &[
            "hibana = { version = \"0.9.1\", default-features = false }",
            "hibana = { version = \"0.9.0\", default-features = false }",
            "hibana = { git",
            "hibana-wasip1-runtime = { git = \"https://github.com/hibanaworks/hibana-wasip1-runtime\"",
            "[patch.crates-io]",
            "hibana = { path = \"../hibana\" }",
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
        .find("C::register_resolvers::<_, ROLE>(&mut resolver_registry);")
        .expect("appkit attach registers capsule resolvers");
    let role_future_start = appkit
        .find("let mut visitor = AttachProjectedRoles {")
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
            "fn embedded_task_waker(woke: &mut bool) -> Waker",
            "let task_waker = embedded_task_waker(&mut woke);",
            "let mut task_context = Context::from_waker(&task_waker);",
            "if !woke {\n                    embedded_wait_for_event();\n                }",
            "struct EmbeddedScheduledTasks<'task, E>",
            "polls: [Option<ScheduledTaskPoll<E>>; APPKIT_EMBEDDED_ROLE_SLOTS]",
            "poll_embedded_stored_task(poll, self.slot_ptr(task_idx), task_context)",
            "core::arch::asm!(\"wfe\", options(nomem, nostack, preserves_flags));",
            "future: EmbeddedFutureArena<APPKIT_EMBEDDED_FUTURE_BYTES>",
            "embedded_tasks:",
            "self.embedded_tasks.push(localside_role_task(",
            "run_canonical_wasi_engine_forever::<C, ImageTy, ArtifactTy, ROLE>(\n                                engine,\n                                self.embedded_tasks,",
            "fn blocking_engine_state<T>(&self) -> *mut T",
            "self.len < APPKIT_EMBEDDED_ROLE_SLOTS",
            "blocking_engine_state::<CanonicalWasiEngine",
            "engine_ptr.write(engine)",
            "tasks.poll_once(&mut task_context);",
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
            "embedded_storage: EmbeddedAttachStorageRef<'static>",
            "Context<'static>",
            "waker: EmbeddedWakerSlot",
            "embedded_task_context(",
            "fn embedded_future_arena_for_role<const ROLE: u8>",
            "run_canonical_wasi_engine_forever::<C, ImageTy, ArtifactTy, ROLE>(\n                                self.embedded_storage,",
            "run_canonical_wasi_engine_forever::<C, ImageTy, ROLE>(ctx)",
            "bare-metal WASI logical images attach exactly one role",
            "APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES",
            "APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES",
            "fn poll_localside_once",
            "TestTimerFiredFact",
            "TEST_TIMER_FIRED_FACT_LABEL",
            "APPKIT_EMBEDDED_WASI_COOPERATIVE_POLLS",
            "cooperative_polls",
        ],
    );
}
