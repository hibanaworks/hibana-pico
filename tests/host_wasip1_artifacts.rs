use std::{env, fs, path::PathBuf};

use hibana::{
    Endpoint, g,
    g::{Msg, Role},
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        program::{RoleProgram, project},
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
use hibana_pico::{
    choreography::local::{
        memory_grow_stdout_roles as project_memory_grow_stdout_roles,
        wasip1_clock_stdout_roles as project_clock_stdout_roles,
        wasip1_stderr_roles as project_stderr_roles,
        wasip1_stdin_stdout_roles as project_stdin_stdout_roles,
        wasip1_stdout_roles as project_stdout_roles,
    },
    choreography::protocol::{
        ClockNow, EngineLabelUniverse, EngineReq, EngineRet, LABEL_MEM_BORROW_READ,
        LABEL_MEM_BORROW_WRITE, LABEL_MEM_COMMIT, LABEL_MEM_FENCE, LABEL_MEM_RELEASE,
        LABEL_MGMT_IMAGE_BEGIN, LABEL_WASIP1_CLOCK_NOW, LABEL_WASIP1_CLOCK_NOW_RET,
        LABEL_WASIP1_STDERR, LABEL_WASIP1_STDERR_RET, LABEL_WASIP1_STDIN, LABEL_WASIP1_STDIN_RET,
        LABEL_WASIP1_STDOUT, LABEL_WASIP1_STDOUT_RET, MGMT_IMAGE_CHUNK_CAPACITY, MemBorrow,
        MemCommit, MemFence, MemFenceReason, MemReadGrantControl, MemRelease, MemRights,
        MemWriteGrantControl, MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd,
        StderrChunk, StdinChunk, StdinRequest, StdoutChunk,
    },
    kernel::mgmt::{
        ActivationBoundary, ImageSlotError, ImageSlotTable, ImageTransferPlan, MgmtControl,
    },
    kernel::policy::{NodeRole, RoleMask, SwarmTelemetry, SwarmTelemetryMsg},
    kernel::remote::{
        RemoteActuateAck, RemoteActuateReqMsg, RemoteActuateRequest, RemoteActuateRetMsg,
        RemoteObjectTable, RemoteRights, RemoteRoute, RemoteSample, RemoteSampleReqMsg,
        RemoteSampleRequest, RemoteSampleRetMsg,
    },
    kernel::swarm::{
        HostSwarmMedium, HostSwarmRoleTransport, NodeId, SwarmCredential, SwarmSecurity,
    },
    kernel::wasi::{
        MemoryLeaseError, MemoryLeaseTable, Wasip1ClockModule, Wasip1ImportSummary,
        Wasip1StderrModule, Wasip1StdinModule, Wasip1StdoutModule,
    },
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
};

#[cfg(feature = "profile-host-linux-wasip1-full")]
use hibana_pico::kernel::{
    choreofs::ChoreoFsError,
    wasi::host_runner::{HostRunError, HostRunReport, HostRunner},
};

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
type SwarmArtifactTransport<'a> = HostSwarmRoleTransport<'a, 192, 5>;
type SwarmArtifactKit<'a> =
    SessionKit<'a, SwarmArtifactTransport<'a>, EngineLabelUniverse, CounterClock, 1>;

const COORDINATOR: NodeId = NodeId::new(1);
const SENSOR: NodeId = NodeId::new(2);
const ACTUATOR: NodeId = NodeId::new(3);
const GATEWAY: NodeId = NodeId::new(4);
const SESSION_GENERATION: u16 = 7;
const SWARM_CREDENTIAL: SwarmCredential = SwarmCredential::new(0x4849_4241);
const SECURE: SwarmSecurity = SwarmSecurity::Secure(SWARM_CREDENTIAL);
const TEST_MEMORY_LEN: u32 = 4096;
const TEST_MEMORY_EPOCH: u32 = 1;
const TEST_STDOUT_PTR: u32 = 1024;
const TEST_STDERR_PTR: u32 = 2048;
const TEST_STDIN_PTR: u32 = 3072;
const TEST_STDIN_INPUT: &[u8] = b"hibana stdin\n";
const TEST_STDIN_MAX_LEN: u8 = 24;
const TEST_CLOCK_NANOS: u64 = 123_456_789;

macro_rules! seq_chain {
    ($head:expr, $($tail:expr),+ $(,)?) => {
        g::seq($head, seq_chain!($($tail),+))
    };
    ($last:expr $(,)?) => {
        $last
    };
}

macro_rules! with_pair {
    ($sid:expr, $project:expr, $supervisor:ident, $engine:ident, $body:block) => {{
        let backend = HostQueueBackend::new();
        let clock0 = CounterClock::new();
        let mut tap0 = Box::new([TapEvent::zero(); 128]);
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut *tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = Box::new([TapEvent::zero(); 128]);
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut *tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let (supervisor_program, engine_program) = $project;
        let mut $supervisor = cluster0
            .enter(rv0, SessionId::new($sid), &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut $engine = cluster1
            .enter(rv1, SessionId::new($sid), &engine_program, NoBinding)
            .expect("attach engine endpoint");

        $body
    }};
}

fn project_artifact_global_swarm_roles_5() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
    RoleProgram<4>,
) {
    let program = seq_chain!(
        g::send::<Role<1>, Role<0>, Msg<LABEL_WASIP1_CLOCK_NOW, EngineReq>, 1>(),
        g::send::<Role<0>, Role<1>, Msg<LABEL_WASIP1_CLOCK_NOW_RET, EngineRet>, 1>(),
        g::send::<Role<1>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
        g::send::<Role<0>, Role<1>, MemReadGrantControl, 1>(),
        g::send::<Role<1>, Role<0>, Msg<LABEL_WASIP1_STDOUT, EngineReq>, 1>(),
        g::send::<Role<0>, Role<1>, Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>, 1>(),
        g::send::<Role<1>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        g::send::<Role<0>, Role<2>, RemoteSampleReqMsg, 1>(),
        g::send::<Role<2>, Role<0>, RemoteSampleRetMsg, 1>(),
        g::send::<Role<2>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
        g::send::<Role<0>, Role<2>, MemReadGrantControl, 1>(),
        g::send::<Role<2>, Role<0>, Msg<LABEL_WASIP1_STDOUT, EngineReq>, 1>(),
        g::send::<Role<0>, Role<2>, Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>, 1>(),
        g::send::<Role<2>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        g::send::<Role<0>, Role<3>, RemoteActuateReqMsg, 1>(),
        g::send::<Role<3>, Role<0>, RemoteActuateRetMsg, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>, 1>(),
        g::send::<Role<0>, Role<3>, MemWriteGrantControl, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_WASIP1_STDIN, EngineReq>, 1>(),
        g::send::<Role<0>, Role<3>, Msg<LABEL_WASIP1_STDIN_RET, EngineRet>, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_MEM_COMMIT, MemCommit>, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
        g::send::<Role<0>, Role<3>, MemReadGrantControl, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_WASIP1_STDOUT, EngineReq>, 1>(),
        g::send::<Role<0>, Role<3>, Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>, 1>(),
        g::send::<Role<3>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        g::send::<Role<0>, Role<4>, SwarmTelemetryMsg, 1>(),
        g::send::<Role<4>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
        g::send::<Role<0>, Role<4>, MemReadGrantControl, 1>(),
        g::send::<Role<4>, Role<0>, Msg<LABEL_WASIP1_STDERR, EngineReq>, 1>(),
        g::send::<Role<0>, Role<4>, Msg<LABEL_WASIP1_STDERR_RET, EngineRet>, 1>(),
        g::send::<Role<4>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        g::send::<Role<0>, Role<2>, RemoteActuateReqMsg, 1>(),
        g::send::<Role<2>, Role<0>, RemoteActuateRetMsg, 1>(),
        g::send::<Role<0>, Role<3>, RemoteActuateReqMsg, 1>(),
        g::send::<Role<3>, Role<0>, RemoteActuateRetMsg, 1>(),
    );
    (
        project(&program),
        project(&program),
        project(&program),
        project(&program),
        project(&program),
    )
}

fn artifact(name: &str) -> Vec<u8> {
    let dir = env::var("HIBANA_WASIP1_GUEST_DIR")
        .unwrap_or_else(|_| "target/wasip1-apps/wasm32-wasip1/release".to_owned());
    let path = PathBuf::from(dir).join(format!("{name}.wasm"));
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let summary = Wasip1ImportSummary::parse_strict_preview1(&bytes).unwrap_or_else(|error| {
        panic!("strict Preview 1 import scan {}: {error:?}", path.display())
    });
    assert!(
        summary.import_count() > 0,
        "strict Preview 1 import scan found no imports in {}",
        path.display()
    );
    assert_eq!(
        summary.import_count(),
        summary.preview1_import_count(),
        "strict Preview 1 import scan found a non-P1 import in {}",
        path.display()
    );
    bytes
}

fn bytes_contain(bytes: &[u8], marker: &[u8]) -> bool {
    marker.is_empty() || bytes.windows(marker.len()).any(|window| window == marker)
}

#[test]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_wasip1_smoke_artifacts_cover_timer_trap_and_infinite_loop() {
    hibana_pico::port::exec::run_current_task(async {
        for name in [
            "wasip1-stdout",
            "wasip1-stderr",
            "wasip1-stdin",
            "wasip1-clock",
            "wasip1-random",
            "wasip1-exit",
            "wasip1-timer",
            "wasip1-trap",
            "wasip1-infinite-loop",
            "wasip1-led-fd-write",
            "wasip1-led-blink",
            "wasip1-led-chaser",
            "wasip1-led-bad-order",
            "wasip1-led-ordinary-std-chaser",
            "wasip1-std-core-coverage",
            "wasip1-std-choreofs-read",
            "wasip1-std-choreofs-append",
            "wasip1-std-bad-path",
            "wasip1-std-choreofs-static-write",
            "wasip1-std-sock-send-recv",
            "wasip1-std-sock-accept-send-recv",
            "wasip1-std-sock-accept-bad",
        ] {
            let bytes = artifact(name);
            assert!(
                !bytes_contain(&bytes, b"wasi_snapshot_preview2"),
                "{name} must not import WASI Preview 2"
            );
            assert!(
                !bytes_contain(&bytes, b"component-type"),
                "{name} must not carry Component Model metadata"
            );
        }

        let timer = artifact("wasip1-timer");
        assert!(
            bytes_contain(&timer, b"poll_oneoff"),
            "timer smoke app must use the WASI P1 poll_oneoff path"
        );

        let trap = artifact("wasip1-trap");
        assert!(
            bytes_contain(&trap, b"hibana wasip1 trap"),
            "trap smoke app must carry the panic marker"
        );

        let infinite = artifact("wasip1-infinite-loop");
        assert!(
            bytes_contain(&infinite, b"wasi_snapshot_preview1"),
            "infinite-loop smoke app must still be a WASI P1 artifact"
        );

        let led = artifact("wasip1-led-fd-write");
        assert!(
            bytes_contain(&led, b"fd_write"),
            "LED smoke app must use the WASI P1 fd_write path"
        );

        let led_blink = artifact("wasip1-led-blink");
        assert!(
            bytes_contain(&led_blink, b"fd_write"),
            "LED blink app must use the WASI P1 fd_write path"
        );
        assert!(
            bytes_contain(&led_blink, b"poll_oneoff"),
            "LED blink app must use the WASI P1 poll_oneoff timer path"
        );

        let led_chaser = artifact("wasip1-led-chaser");
        assert!(
            bytes_contain(&led_chaser, b"fd_write"),
            "LED chaser app must use the WASI P1 fd_write path"
        );
        assert!(
            bytes_contain(&led_chaser, b"poll_oneoff"),
            "LED chaser app must use the WASI P1 poll_oneoff timer path"
        );

        let led_bad_order = artifact("wasip1-led-bad-order");
        assert!(
            bytes_contain(&led_bad_order, b"fd_write"),
            "LED bad-order app must still be a real WASI P1 fd_write app"
        );
        assert!(
            bytes_contain(&led_bad_order, b"poll_oneoff"),
            "LED bad-order app must issue the out-of-phase WASI P1 poll_oneoff"
        );

        let led_ordinary_std_chaser = artifact("wasip1-led-ordinary-std-chaser");
        assert!(
            bytes_contain(&led_ordinary_std_chaser, b"fd_write"),
            "LED ordinary std chaser app must use the WASI P1 fd_write path"
        );
        assert!(
            bytes_contain(&led_ordinary_std_chaser, b"poll_oneoff"),
            "LED ordinary std chaser app must use the WASI P1 poll_oneoff timer path"
        );
        assert!(
            bytes_contain(&led_ordinary_std_chaser, b"environ_get")
                || bytes_contain(&led_ordinary_std_chaser, b"args_get")
                || bytes_contain(&led_ordinary_std_chaser, b"proc_exit"),
            "LED ordinary std chaser app must carry Rust std WASI start imports"
        );
        assert!(
            bytes_contain(&led_ordinary_std_chaser, b"_start"),
            "LED ordinary std chaser app must expose Rust std _start"
        );

        let std_core_coverage = artifact("wasip1-std-core-coverage");
        assert!(
            bytes_contain(&std_core_coverage, b"hibana std core coverage"),
            "ordinary std core coverage app must carry the stdout marker"
        );
        assert!(
            bytes_contain(&std_core_coverage, b"memory.grow"),
            "ordinary std core coverage app must exercise memory.grow"
        );
        assert!(
            bytes_contain(&std_core_coverage, b"fd_write"),
            "ordinary std core coverage app must reach stdout through fd_write"
        );

        let std_choreofs_read = artifact("wasip1-std-choreofs-read");
        assert!(
            bytes_contain(&std_choreofs_read, b"path_open"),
            "ordinary std ChoreoFS app must open the resource-store object through WASI P1 path_open"
        );
        assert!(
            bytes_contain(&std_choreofs_read, b"fd_read"),
            "ordinary std ChoreoFS app must read the resource-store object through WASI P1 fd_read"
        );
        assert!(
            bytes_contain(&std_choreofs_read, b"hibana choreofs read"),
            "ordinary std ChoreoFS app must carry the stdout marker"
        );

        let std_choreofs_append = artifact("wasip1-std-choreofs-append");
        assert!(
            bytes_contain(&std_choreofs_append, b"path_open")
                && bytes_contain(&std_choreofs_append, b"fd_write")
                && bytes_contain(&std_choreofs_append, b"fd_read"),
            "ordinary std ChoreoFS append app must open, write, and read through WASI P1"
        );
        assert!(
            bytes_contain(&std_choreofs_append, b"hibana choreofs append"),
            "ordinary std ChoreoFS append app must carry the stdout marker"
        );

        let std_bad_path = artifact("wasip1-std-bad-path");
        assert!(
            bytes_contain(&std_bad_path, b"path_open"),
            "ordinary std bad-path app must exercise a real WASI P1 path_open reject"
        );
        assert!(
            bytes_contain(&std_bad_path, b"forbidden path must reject"),
            "ordinary std bad-path app must carry the typed-reject assertion marker"
        );

        let std_static_write = artifact("wasip1-std-choreofs-static-write");
        assert!(
            bytes_contain(&std_static_write, b"path_open"),
            "ordinary std static-write app must exercise a real WASI P1 path_open reject"
        );
        assert!(
            bytes_contain(&std_static_write, b"readonly static write must reject"),
            "ordinary std static-write app must carry the typed-reject assertion marker"
        );

        let std_sock = artifact("wasip1-std-sock-send-recv");
        assert!(
            bytes_contain(&std_sock, b"path_open")
                && bytes_contain(&std_sock, b"sock_send")
                && bytes_contain(&std_sock, b"sock_recv")
                && bytes_contain(&std_sock, b"sock_shutdown"),
            "ordinary std datagram app must open a ChoreoFS NetworkObject and exercise WASI P1 sock imports"
        );
        assert!(
            bytes_contain(&std_sock, b"hibana network datagram ping pong"),
            "ordinary std datagram app must carry the stdout marker"
        );

        let std_sock_accept = artifact("wasip1-std-sock-accept-send-recv");
        assert!(
            bytes_contain(&std_sock_accept, b"sock_accept")
                && bytes_contain(&std_sock_accept, b"sock_send")
                && bytes_contain(&std_sock_accept, b"sock_recv")
                && bytes_contain(&std_sock_accept, b"sock_shutdown"),
            "ordinary std listener app must accept and then use a NetworkObject through WASI P1 imports"
        );
        assert!(
            bytes_contain(&std_sock_accept, b"hibana listener accept fd ping pong"),
            "ordinary std listener app must carry the stdout marker"
        );

        let std_sock_bad = artifact("wasip1-std-sock-accept-bad");
        assert!(
            bytes_contain(&std_sock_bad, b"sock_accept"),
            "ordinary std bad socket app must exercise WASI P1 sock_accept"
        );
        assert!(
            bytes_contain(&std_sock_bad, b"sock_accept must reject"),
            "ordinary std bad socket app must carry the typed-reject assertion marker"
        );

        let std_stream = artifact("wasip1-std-stream-control");
        assert!(
            bytes_contain(&std_stream, b"path_open")
                && bytes_contain(&std_stream, b"sock_send")
                && bytes_contain(&std_stream, b"sock_recv")
                && bytes_contain(&std_stream, b"sock_shutdown"),
            "ordinary std stream app must open a ChoreoFS NetworkStream and exercise WASI P1 sock imports"
        );
        assert!(
            bytes_contain(&std_stream, b"hibana network stream control ping pong"),
            "ordinary std stream app must carry the stdout marker"
        );

        let memory_grow_ok = artifact("wasip1-memory-grow-ok");
        assert!(
            bytes_contain(&memory_grow_ok, b"hibana wasip1 memory grow ok"),
            "memory-grow success smoke app must carry the success marker"
        );

        let memory_grow_stale = artifact("wasip1-memory-grow-stale-lease");
        assert!(
            bytes_contain(&memory_grow_stale, b"hibana memgrow stale lease"),
            "memory-grow stale-lease smoke app must carry the stale marker"
        );
    });
}

#[test]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_wasip1_memory_grow_artifacts_exercise_fence_and_stale_lease_rejection() {
    hibana_pico::port::exec::run_current_task(async {
        let ok_artifact = artifact("wasip1-memory-grow-ok");
        let stale_artifact = artifact("wasip1-memory-grow-stale-lease");
        let ok_module = Wasip1StdoutModule::parse(&ok_artifact).expect("memory grow ok module");
        Wasip1StdoutModule::parse(&stale_artifact).expect("memory grow stale module");
        assert!(
            bytes_contain(&stale_artifact, b"hibana memgrow stale lease"),
            "stale-lease artifact carries the rejection marker"
        );
        let marker = b"hibana wasip1 memory grow ok";
        let ok_chunk = ok_module
            .stdout_chunk_for(marker)
            .expect("memory grow ok stdout marker");

        with_pair!(
            44,
            project_memory_grow_stdout_roles(),
            supervisor,
            engine,
            {
                let mut leases: MemoryLeaseTable<2> =
                    MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
                let old_grant = leases
                    .grant_read(MemBorrow::new(
                        TEST_STDOUT_PTR,
                        ok_chunk.len() as u8,
                        TEST_MEMORY_EPOCH,
                    ))
                    .expect("old read lease before memory grow");
                let old_chunk = ok_chunk.with_lease(old_grant.lease_id());
                leases
                    .validate_read_chunk(&old_chunk)
                    .expect("old lease is valid before memory grow");

                let fence = MemFence::new(MemFenceReason::MemoryGrow, TEST_MEMORY_EPOCH + 1);
                (engine
                    .flow::<Msg<LABEL_MEM_FENCE, MemFence>>()
                    .expect("engine flow<memory grow fence>")
                    .send(&fence))
                .await
                .expect("engine sends memory grow fence");
                let received_fence = (supervisor.recv::<Msg<LABEL_MEM_FENCE, MemFence>>())
                    .await
                    .expect("supervisor receives memory grow fence");
                assert_eq!(received_fence, fence);
                assert_eq!(received_fence.reason(), MemFenceReason::MemoryGrow);
                leases.fence(received_fence);
                assert_eq!(leases.epoch(), TEST_MEMORY_EPOCH + 1);
                assert!(!leases.has_outstanding_leases());
                assert_eq!(
                    leases.validate_read_chunk(&old_chunk),
                    Err(MemoryLeaseError::UnknownLease)
                );
                assert_eq!(
                    leases.release(MemRelease::new(old_grant.lease_id())),
                    Err(MemoryLeaseError::UnknownLease)
                );
                assert_eq!(
                    leases.grant_read(MemBorrow::new(
                        TEST_STDOUT_PTR,
                        ok_chunk.len() as u8,
                        TEST_MEMORY_EPOCH,
                    )),
                    Err(MemoryLeaseError::EpochMismatch)
                );

                let borrow =
                    MemBorrow::new(TEST_STDOUT_PTR, ok_chunk.len() as u8, TEST_MEMORY_EPOCH + 1);
                (engine
                    .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
                    .expect("engine flow<memory grow stdout borrow>")
                    .send(&borrow))
                .await
                .expect("engine sends post-grow stdout borrow");
                assert_eq!(
                    (supervisor.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
                        .await
                        .expect("supervisor receives post-grow stdout borrow"),
                    borrow
                );

                let grant = leases
                    .grant_read(borrow)
                    .expect("grant stdout lease after memory grow");
                (supervisor
                    .flow::<MemReadGrantControl>()
                    .expect("supervisor flow<post-grow stdout grant>")
                    .send(()))
                .await
                .expect("supervisor sends post-grow stdout grant");
                let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
                    .await
                    .expect("engine receives post-grow stdout grant")
                    .decode_handle()
                    .expect("decode post-grow stdout lease handle");
                assert_eq!(rights, MemRights::Read.tag());
                assert_eq!(lease_id as u8, grant.lease_id());

                let request = EngineReq::Wasip1Stdout(ok_chunk.with_lease(lease_id as u8));
                (engine
                    .flow::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>()
                    .expect("engine flow<post-grow stdout>")
                    .send(&request))
                .await
                .expect("engine sends post-grow stdout");
                let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>())
                    .await
                    .expect("supervisor receives post-grow stdout");
                assert_eq!(received, request);
                let EngineReq::Wasip1Stdout(received_chunk) = received else {
                    panic!("expected post-grow stdout request");
                };
                leases
                    .validate_read_chunk(&received_chunk)
                    .expect("post-grow stdout is lease-authorized");

                let reply = EngineRet::Wasip1StdoutWritten(received_chunk.len() as u8);
                (supervisor
                    .flow::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>()
                    .expect("supervisor flow<post-grow stdout ret>")
                    .send(&reply))
                .await
                .expect("supervisor sends post-grow stdout reply");
                assert_eq!(
                    (engine.recv::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>())
                        .await
                        .expect("engine receives post-grow stdout reply"),
                    reply
                );

                let release = MemRelease::new(lease_id as u8);
                (engine
                    .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
                    .expect("engine flow<post-grow stdout release>")
                    .send(&release))
                .await
                .expect("engine sends post-grow stdout release");
                assert_eq!(
                    (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
                        .await
                        .expect("supervisor receives post-grow stdout release"),
                    release
                );
                leases.release(release).expect("release post-grow lease");
                let telemetry = leases.rejection_telemetry();
                assert_eq!(telemetry.bad_generation(), 1);
                assert_eq!(telemetry.invalid_lease(), 2);
                assert_eq!(telemetry.total(), 3);
            }
        );
    });
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_ordinary_std_core_coverage_runs_on_host_full_profile() {
    let artifact = artifact("wasip1-std-core-coverage");
    let mut runner = HostRunner::new(&artifact).expect("create host/full std runner");
    let report = runner
        .run_until_exit(512)
        .expect("run ordinary std core coverage through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana std core coverage"),
        "ordinary std app must write the coverage marker"
    );
    assert!(
        report.memory_grow_count > 0,
        "ordinary std app must surface memory.grow as a core engine event"
    );
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdWrite(_))),
        "ordinary std app must enter the typed fd_write syscall stream"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_std_choreofs_app_uses_resource_store_through_host_full_runner() {
    let artifact = artifact("wasip1-std-choreofs-read");
    let mut runner = HostRunner::new(&artifact).expect("create host/full ChoreoFS runner");
    runner
        .fs_mut()
        .install_static_blob(b"config.txt", b"ok")
        .expect("install bounded ChoreoFS object");

    let report = runner
        .run_until_exit(1024)
        .expect("run ordinary std ChoreoFS app through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana choreofs read ok"),
        "ordinary std app must read ChoreoFS data and report it through stdout"
    );
    assert!(
        report.choreofs_open_count >= 1,
        "ordinary std app must open a ChoreoFS object through the resource-store path"
    );
    assert!(
        report.choreofs_read_count >= 1,
        "ordinary std app must read a ChoreoFS object through fd_read"
    );
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdRead(_))),
        "ChoreoFS read must enter the typed fd_read syscall stream"
    );
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdWrite(_))),
        "ChoreoFS result must enter the typed fd_write syscall stream"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_std_choreofs_append_app_writes_and_reads_resource_store() {
    let artifact = artifact("wasip1-std-choreofs-append");
    let mut runner = HostRunner::new(&artifact).expect("create host/full ChoreoFS append runner");
    runner
        .fs_mut()
        .install_append_log(b"log.txt")
        .expect("install bounded ChoreoFS append log");

    let report = runner
        .run_until_exit(1024)
        .expect("run ordinary std ChoreoFS append app through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana choreofs append entry"),
        "ordinary std app must append ChoreoFS data and read it back"
    );
    assert!(
        report.choreofs_open_count >= 2,
        "append proof opens once for append and once for readback"
    );
    assert!(
        report.choreofs_write_count >= 1,
        "append proof must write through ChoreoFS fd_write"
    );
    assert!(
        report.choreofs_read_count >= 1,
        "append proof must read through ChoreoFS fd_read"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_bad_std_path_app_rejects_before_hidden_host_fs() {
    let artifact = artifact("wasip1-std-bad-path");
    let mut runner = HostRunner::new(&artifact).expect("create host/full bad-path runner");
    runner.trap_on_path_error(true);

    let error = runner
        .run_until_exit(128)
        .expect_err("bad std path must reject at the typed ChoreoFS boundary");
    assert!(
        matches!(error, HostRunError::PathRejected(_)),
        "bad std path must reject through ChoreoFS, got {error:?}"
    );
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_bad_std_static_write_rejects_at_choreofs_control() {
    let artifact = artifact("wasip1-std-choreofs-static-write");
    let mut runner = HostRunner::new(&artifact).expect("create host/full static-write runner");
    runner
        .fs_mut()
        .install_static_blob(b"readonly.txt", b"fixed")
        .expect("install read-only ChoreoFS object");
    runner.trap_on_path_error(true);

    let error = runner
        .run_until_exit(128)
        .expect_err("static blob write must reject at object control");
    assert!(
        matches!(error, HostRunError::ChoreoFs(ChoreoFsError::ReadOnly)),
        "static blob write must reject through ChoreoFS, got {error:?}"
    );
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_std_sock_app_uses_network_object_without_p2() {
    let artifact = artifact("wasip1-std-sock-send-recv");
    let mut runner = HostRunner::new(&artifact).expect("create host/full sock runner");
    runner
        .fs_mut()
        .install_network_datagram(b"network/datagram/ping-pong")
        .expect("install ping-pong NetworkDatagram");
    runner.enqueue_network_rx(4, b"pong");

    let report = runner
        .run_until_exit(512)
        .expect("run ordinary std datagram app through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana network datagram ping pong"),
        "datagram app must report success through stdout"
    );
    assert_eq!(report.choreofs_open_count, 1);
    assert_eq!(runner.network_tx().len(), 1);
    assert_eq!(runner.network_tx()[0].0, 4);
    assert_eq!(runner.network_tx()[0].1, b"ping");
    assert_eq!(report.network_send_count, 1);
    assert_eq!(report.network_recv_count, 1);
    let sent_write = report
        .engine_trace
        .iter()
        .find_map(|request| match request {
            EngineReq::FdWrite(write) => Some(write),
            _ => None,
        })
        .expect("sock_send must enter the typed fd_write syscall stream");
    assert_eq!(sent_write.fd(), 4);
    assert_eq!(sent_write.as_bytes(), b"ping");
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdRead(_))),
        "sock_recv must enter the typed fd_read syscall stream"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_std_sock_accept_app_mints_network_object_without_socket_authority() {
    let artifact = artifact("wasip1-std-sock-accept-send-recv");
    let mut runner = HostRunner::new(&artifact).expect("create host/full accept runner");
    runner
        .fs_mut()
        .install_network_listener(b"network/listener/control")
        .expect("install control NetworkListener");
    runner.enqueue_stream_accept(4, 5);
    runner.enqueue_network_rx(5, b"pong");

    let report = runner
        .run_until_exit(768)
        .expect("run ordinary std accept app through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana listener accept fd ping pong"),
        "listener app must report success through stdout"
    );
    assert_eq!(report.choreofs_open_count, 1);
    assert_eq!(runner.network_tx().len(), 1);
    assert_eq!(runner.network_tx()[0].0, 5);
    assert_eq!(runner.network_tx()[0].1, b"ping");
    assert_eq!(report.network_accept_count, 1);
    assert!(report.network_send_count >= 1);
    assert!(report.network_recv_count >= 1);
    let accepted_write = report
        .engine_trace
        .iter()
        .find_map(|request| match request {
            EngineReq::FdWrite(write) if write.fd() == 5 => Some(write),
            _ => None,
        })
        .expect("accepted sock_send must enter typed fd_write stream");
    assert_eq!(accepted_write.as_bytes(), b"ping");
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdRead(_))),
        "accepted sock_recv must enter typed fd_read stream"
    );
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdClose(_))),
        "accepted sock_shutdown must enter typed fd_close stream"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_std_stream_control_app_uses_network_object_without_socket_authority() {
    let artifact = artifact("wasip1-std-stream-control");
    let mut runner = HostRunner::new(&artifact).expect("create host/full stream runner");
    runner
        .fs_mut()
        .install_network_stream(b"network/stream/control")
        .expect("install control NetworkStream");
    runner.enqueue_network_rx(4, b"pong");

    let report = runner
        .run_until_exit(512)
        .expect("run ordinary std stream app through host/full typed runner");
    assert_eq!(report.exit_status, Some(0));
    assert!(
        bytes_contain(&report.stdout, b"hibana network stream control ping pong"),
        "stream app must report success through stdout"
    );
    assert_eq!(report.choreofs_open_count, 1);
    assert_eq!(runner.network_tx().len(), 1);
    assert_eq!(runner.network_tx()[0].0, 4);
    assert_eq!(runner.network_tx()[0].1, b"ping");
    assert_eq!(report.network_send_count, 1);
    assert_eq!(report.network_recv_count, 1);
    let sent_write = report
        .engine_trace
        .iter()
        .find_map(|request| match request {
            EngineReq::FdWrite(write) if write.fd() == 4 => Some(write),
            _ => None,
        })
        .expect("stream write_chunk must enter the typed fd_write syscall stream");
    assert_eq!(sent_write.as_bytes(), b"ping");
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdRead(read) if read.fd() == 4)),
        "stream read_chunk must enter the typed fd_read syscall stream"
    );
    assert!(
        report
            .engine_trace
            .iter()
            .any(|request| matches!(request, EngineReq::FdClose(close) if close.fd() == 4)),
        "stream shutdown must enter the typed fd_close syscall stream"
    );
    assert_host_full_runner_drives_projected_localside(&report);
}

#[cfg(feature = "profile-host-linux-wasip1-full")]
fn assert_host_full_runner_drives_projected_localside(report: &HostRunReport) {
    assert!(
        report.localside_drive_count > 0,
        "host/full runner must drive projected localside, not just record EngineReq/EngineRet"
    );
    assert_eq!(
        report.localside_drive_count as usize,
        report.engine_trace.len(),
        "every recorded EngineReq must pass through a projected localside send/recv path"
    );
}

#[test]
fn wasip1_network_smoke_sources_use_guest_facade_not_raw_sock_imports() {
    let datagram =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-sock-send-recv.rs");
    let accept = include_str!(
        "../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-sock-accept-send-recv.rs"
    );
    let bad_accept =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-sock-accept-bad.rs");
    let stream =
        include_str!("../apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-std-stream-control.rs");

    assert!(
        datagram.contains("hibana_wasi_guest::net::Datagram"),
        "datagram smoke app must use the safe Datagram facade"
    );
    assert!(
        accept.contains("hibana_wasi_guest::net::{Listener, Stream}")
            || accept.contains("hibana_wasi_guest::net::{Stream, Listener}"),
        "accept smoke app must use the safe Listener/Stream facade"
    );
    assert!(
        bad_accept.contains("hibana_wasi_guest::net::Listener"),
        "bad accept smoke app must use the safe Listener facade"
    );
    assert!(
        stream.contains("hibana_wasi_guest::net::Stream"),
        "stream smoke app must use the safe Stream facade"
    );

    for (name, source) in [
        ("datagram", datagram),
        ("accept", accept),
        ("bad_accept", bad_accept),
        ("stream", stream),
    ] {
        assert!(
            !source.contains("unsafe extern \"C\""),
            "{name} network smoke app must not declare raw WASI imports"
        );
        assert!(
            !source.contains("wasi_sock_"),
            "{name} network smoke app must not call raw sock_* imports"
        );
    }
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_bad_std_sock_accept_rejects_without_listener_route() {
    let artifact = artifact("wasip1-std-sock-accept-bad");
    let mut runner = HostRunner::new(&artifact).expect("create host/full bad sock runner");
    runner
        .fs_mut()
        .install_network_listener(b"network/listener/control")
        .expect("install control NetworkListener");
    runner.trap_on_network_error(true);

    let error = runner
        .run_until_exit(128)
        .expect_err("sock_accept must reject without explicit accept route");
    assert!(
        matches!(error, HostRunError::NetworkRejected(_)),
        "bad sock_accept must reject through NetworkObject, got {error:?}"
    );
}

async fn exchange_clock<const ENGINE: u8>(
    supervisor: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ENGINE>,
    now: ClockNow,
) {
    let request = EngineReq::Wasip1ClockNow;
    (engine
        .flow::<Msg<LABEL_WASIP1_CLOCK_NOW, EngineReq>>()
        .expect("engine flow<clock>")
        .send(&request))
    .await
    .expect("engine sends artifact clock request");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_WASIP1_CLOCK_NOW, EngineReq>>())
            .await
            .expect("supervisor receives artifact clock request"),
        request
    );

    let reply = EngineRet::Wasip1ClockNow(now);
    (supervisor
        .flow::<Msg<LABEL_WASIP1_CLOCK_NOW_RET, EngineRet>>()
        .expect("supervisor flow<clock ret>")
        .send(&reply))
    .await
    .expect("supervisor sends artifact clock reply");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASIP1_CLOCK_NOW_RET, EngineRet>>())
            .await
            .expect("engine receives artifact clock reply"),
        reply
    );
}

async fn exchange_stdout<const ENGINE: u8>(
    supervisor: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ENGINE>,
    chunk: StdoutChunk,
) {
    let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
    let borrow = MemBorrow::new(TEST_STDOUT_PTR, chunk.len() as u8, TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<stdout borrow>")
        .send(&borrow))
    .await
    .expect("engine sends stdout borrow");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("supervisor receives stdout borrow"),
        borrow
    );

    let grant = leases.grant_read(borrow).expect("grant stdout read lease");
    (supervisor
        .flow::<MemReadGrantControl>()
        .expect("supervisor flow<stdout grant>")
        .send(()))
    .await
    .expect("supervisor sends stdout grant");
    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives stdout grant")
        .decode_handle()
        .expect("decode stdout lease handle");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let request = EngineReq::Wasip1Stdout(chunk.with_lease(lease_id as u8));
    (engine
        .flow::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>()
        .expect("engine flow<stdout>")
        .send(&request))
    .await
    .expect("engine sends artifact stdout");
    let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>())
        .await
        .expect("supervisor receives artifact stdout");
    assert_eq!(received, request);
    let EngineReq::Wasip1Stdout(received_chunk) = received else {
        panic!("expected stdout request");
    };
    leases
        .validate_read_chunk(&received_chunk)
        .expect("artifact stdout is lease-authorized");

    let reply = EngineRet::Wasip1StdoutWritten(received_chunk.len() as u8);
    (supervisor
        .flow::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>()
        .expect("supervisor flow<stdout ret>")
        .send(&reply))
    .await
    .expect("supervisor sends artifact stdout reply");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>())
            .await
            .expect("engine receives artifact stdout reply"),
        reply
    );

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<stdout release>")
        .send(&release))
    .await
    .expect("engine sends stdout release");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor receives stdout release"),
        release
    );
    leases.release(release).expect("release stdout lease");
}

async fn exchange_stderr<const ENGINE: u8>(
    supervisor: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ENGINE>,
    chunk: StderrChunk,
) {
    let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
    let borrow = MemBorrow::new(TEST_STDERR_PTR, chunk.len() as u8, TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<stderr borrow>")
        .send(&borrow))
    .await
    .expect("engine sends stderr borrow");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("supervisor receives stderr borrow"),
        borrow
    );

    let grant = leases.grant_read(borrow).expect("grant stderr read lease");
    (supervisor
        .flow::<MemReadGrantControl>()
        .expect("supervisor flow<stderr grant>")
        .send(()))
    .await
    .expect("supervisor sends stderr grant");
    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives stderr grant")
        .decode_handle()
        .expect("decode stderr lease handle");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let request = EngineReq::Wasip1Stderr(chunk.with_lease(lease_id as u8));
    (engine
        .flow::<Msg<LABEL_WASIP1_STDERR, EngineReq>>()
        .expect("engine flow<stderr>")
        .send(&request))
    .await
    .expect("engine sends artifact stderr");
    let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDERR, EngineReq>>())
        .await
        .expect("supervisor receives artifact stderr");
    assert_eq!(received, request);
    let EngineReq::Wasip1Stderr(received_chunk) = received else {
        panic!("expected stderr request");
    };
    leases
        .validate_read_chunk(&received_chunk)
        .expect("artifact stderr is lease-authorized");

    let reply = EngineRet::Wasip1StderrWritten(received_chunk.len() as u8);
    (supervisor
        .flow::<Msg<LABEL_WASIP1_STDERR_RET, EngineRet>>()
        .expect("supervisor flow<stderr ret>")
        .send(&reply))
    .await
    .expect("supervisor sends artifact stderr reply");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASIP1_STDERR_RET, EngineRet>>())
            .await
            .expect("engine receives artifact stderr reply"),
        reply
    );

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<stderr release>")
        .send(&release))
    .await
    .expect("engine sends stderr release");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor receives stderr release"),
        release
    );
    leases.release(release).expect("release stderr lease");
}

async fn exchange_stdin<const ENGINE: u8>(
    supervisor: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ENGINE>,
    request: StdinRequest,
    chunk: StdinChunk,
) {
    let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
    let borrow = MemBorrow::new(TEST_STDIN_PTR, request.max_len(), TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>()
        .expect("engine flow<stdin borrow>")
        .send(&borrow))
    .await
    .expect("engine sends stdin borrow");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>())
            .await
            .expect("supervisor receives stdin borrow"),
        borrow
    );

    let grant = leases.grant_write(borrow).expect("grant stdin write lease");
    (supervisor
        .flow::<MemWriteGrantControl>()
        .expect("supervisor flow<stdin grant>")
        .send(()))
    .await
    .expect("supervisor sends stdin grant");
    let (rights, lease_id) = (engine.recv::<MemWriteGrantControl>())
        .await
        .expect("engine receives stdin grant")
        .decode_handle()
        .expect("decode stdin lease handle");
    assert_eq!(rights, MemRights::Write.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let request =
        StdinRequest::new_with_lease(lease_id as u8, request.max_len()).expect("stdin request");
    let request = EngineReq::Wasip1Stdin(request);
    (engine
        .flow::<Msg<LABEL_WASIP1_STDIN, EngineReq>>()
        .expect("engine flow<stdin>")
        .send(&request))
    .await
    .expect("engine sends artifact stdin request");
    let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDIN, EngineReq>>())
        .await
        .expect("supervisor receives artifact stdin request");
    assert_eq!(received, request);
    let EngineReq::Wasip1Stdin(received_request) = received else {
        panic!("expected stdin request");
    };
    leases
        .validate_write_request(&received_request)
        .expect("artifact stdin request is lease-authorized");

    let chunk = chunk.with_lease(received_request.lease_id());
    leases
        .validate_write_chunk(&chunk)
        .expect("artifact stdin reply is lease-authorized");
    let reply = EngineRet::Wasip1StdinRead(chunk);
    (supervisor
        .flow::<Msg<LABEL_WASIP1_STDIN_RET, EngineRet>>()
        .expect("supervisor flow<stdin ret>")
        .send(&reply))
    .await
    .expect("supervisor sends artifact stdin reply");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASIP1_STDIN_RET, EngineRet>>())
            .await
            .expect("engine receives artifact stdin reply"),
        reply
    );

    let commit = MemCommit::new(lease_id as u8, chunk.len() as u8);
    (engine
        .flow::<Msg<LABEL_MEM_COMMIT, MemCommit>>()
        .expect("engine flow<stdin commit>")
        .send(&commit))
    .await
    .expect("engine sends stdin commit");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_COMMIT, MemCommit>>())
            .await
            .expect("supervisor receives stdin commit"),
        commit
    );
    leases.commit(commit).expect("commit stdin lease");

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<stdin release>")
        .send(&release))
    .await
    .expect("engine sends stdin release");
    assert_eq!(
        (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor receives stdin release"),
        release
    );
    leases.release(release).expect("release stdin lease");
}

#[test]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_swarm_wasip1_artifacts_exercise_localside_choreography() {
    hibana_pico::port::exec::run_current_task(async {
        let coordinator = artifact("swarm-coordinator");
        let clock =
            Wasip1ClockModule::parse(&coordinator).expect("coordinator imports clock_time_get");
        let stdout = Wasip1StdoutModule::parse(&coordinator).expect("coordinator imports fd_write");
        let coordinator_marker = stdout
            .stdout_chunk_for(b"hibana swarm coordinator")
            .expect("coordinator marker chunk");
        with_pair!(310, project_clock_stdout_roles(), supervisor, engine, {
            exchange_clock(
                &mut supervisor,
                &mut engine,
                clock.clock_now(TEST_CLOCK_NANOS),
            )
            .await;
            exchange_stdout(&mut supervisor, &mut engine, coordinator_marker).await;
        });

        let sensor = artifact("swarm-sensor");
        let stdout = Wasip1StdoutModule::parse(&sensor).expect("sensor imports fd_write");
        let sensor_marker = stdout
            .stdout_chunk_for(b"hibana swarm sensor")
            .expect("sensor marker chunk");
        with_pair!(311, project_stdout_roles(), supervisor, engine, {
            exchange_stdout(&mut supervisor, &mut engine, sensor_marker).await;
        });

        let actuator = artifact("swarm-actuator");
        let stdin = Wasip1StdinModule::parse(&actuator).expect("actuator imports fd_read");
        let stdout = Wasip1StdoutModule::parse(&actuator).expect("actuator imports fd_write");
        let stdin_request = stdin
            .stdin_request_for(TEST_STDIN_MAX_LEN)
            .expect("actuator stdin request");
        let stdin_chunk = stdin
            .stdin_chunk_for(TEST_STDIN_INPUT)
            .expect("actuator stdin chunk");
        let actuator_marker = stdout
            .stdout_chunk_for(b"hibana swarm actuator")
            .expect("actuator marker chunk");
        with_pair!(312, project_stdin_stdout_roles(), supervisor, engine, {
            exchange_stdin(&mut supervisor, &mut engine, stdin_request, stdin_chunk).await;
            exchange_stdout(&mut supervisor, &mut engine, actuator_marker).await;
        });

        let gateway = artifact("swarm-gateway");
        let stderr = Wasip1StderrModule::parse(&gateway).expect("gateway imports fd_write");
        let gateway_marker = stderr
            .stderr_chunk_for(b"hibana swarm gateway")
            .expect("gateway marker chunk");
        with_pair!(313, project_stderr_roles(), supervisor, engine, {
            exchange_stderr(&mut supervisor, &mut engine, gateway_marker).await;
        });
    });
}

#[test]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_swarm_wasip1_artifacts_exercise_one_global_swarm_choreography() {
    hibana_pico::port::exec::run_current_task(async {
        let coordinator = artifact("swarm-coordinator");
        let coordinator_clock =
            Wasip1ClockModule::parse(&coordinator).expect("coordinator imports clock_time_get");
        let coordinator_stdout =
            Wasip1StdoutModule::parse(&coordinator).expect("coordinator imports fd_write");
        let coordinator_marker = coordinator_stdout
            .stdout_chunk_for(b"hibana swarm coordinator")
            .expect("coordinator marker chunk");

        let sensor = artifact("swarm-sensor");
        let sensor_stdout = Wasip1StdoutModule::parse(&sensor).expect("sensor imports fd_write");
        let sensor_marker = sensor_stdout
            .stdout_chunk_for(b"hibana swarm sensor")
            .expect("sensor marker chunk");

        let actuator = artifact("swarm-actuator");
        let actuator_stdin = Wasip1StdinModule::parse(&actuator).expect("actuator imports fd_read");
        let actuator_stdout =
            Wasip1StdoutModule::parse(&actuator).expect("actuator imports fd_write");
        let actuator_stdin_request = actuator_stdin
            .stdin_request_for(TEST_STDIN_MAX_LEN)
            .expect("actuator stdin request");
        let actuator_stdin_chunk = actuator_stdin
            .stdin_chunk_for(TEST_STDIN_INPUT)
            .expect("actuator stdin chunk");
        let actuator_marker = actuator_stdout
            .stdout_chunk_for(b"hibana swarm actuator")
            .expect("actuator marker chunk");

        let gateway = artifact("swarm-gateway");
        let gateway_stderr = Wasip1StderrModule::parse(&gateway).expect("gateway imports fd_write");
        let gateway_marker = gateway_stderr
            .stderr_chunk_for(b"hibana swarm gateway")
            .expect("gateway marker chunk");

        let medium: HostSwarmMedium<192> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, COORDINATOR, SENSOR, ACTUATOR, GATEWAY];

        let clock0 = CounterClock::new();
        let mut tap0 = Box::new([TapEvent::zero(); 128]);
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmArtifactKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut *tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    5,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register coordinator supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = Box::new([TapEvent::zero(); 128]);
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmArtifactKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut *tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    5,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register coordinator app rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = Box::new([TapEvent::zero(); 128]);
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmArtifactKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut *tap2, slab2.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    5,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register sensor app rendezvous");

        let clock3 = CounterClock::new();
        let mut tap3 = Box::new([TapEvent::zero(); 128]);
        let mut slab3 = vec![0u8; 262_144];
        let cluster3 = SwarmArtifactKit::new(&clock3);
        let rv3 = cluster3
            .add_rendezvous_from_config(
                Config::new(&mut *tap3, slab3.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    ACTUATOR,
                    role_nodes,
                    5,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register actuator app rendezvous");

        let clock4 = CounterClock::new();
        let mut tap4 = Box::new([TapEvent::zero(); 128]);
        let mut slab4 = vec![0u8; 262_144];
        let cluster4 = SwarmArtifactKit::new(&clock4);
        let rv4 = cluster4
            .add_rendezvous_from_config(
                Config::new(&mut *tap4, slab4.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    5,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register gateway app rendezvous");

        let (program0, program1, program2, program3, program4) =
            project_artifact_global_swarm_roles_5();
        let mut supervisor = cluster0
            .enter(rv0, SessionId::new(314), &program0, NoBinding)
            .expect("attach coordinator supervisor");
        let mut coordinator_app = cluster1
            .enter(rv1, SessionId::new(314), &program1, NoBinding)
            .expect("attach coordinator app");
        let mut sensor_app = cluster2
            .enter(rv2, SessionId::new(314), &program2, NoBinding)
            .expect("attach sensor app");
        let mut actuator_app = cluster3
            .enter(rv3, SessionId::new(314), &program3, NoBinding)
            .expect("attach actuator app");
        let mut gateway_app = cluster4
            .enter(rv4, SessionId::new(314), &program4, NoBinding)
            .expect("attach gateway app");

        exchange_clock(
            &mut supervisor,
            &mut coordinator_app,
            coordinator_clock.clock_now(TEST_CLOCK_NANOS),
        )
        .await;
        exchange_stdout(&mut supervisor, &mut coordinator_app, coordinator_marker).await;

        let sensor_value = sensor_marker
            .as_bytes()
            .iter()
            .fold(0u32, |sum, byte| sum.wrapping_add(*byte as u32));
        let sample_request = RemoteSampleRequest::new(1, 1, SENSOR.raw() as u8);
        (supervisor
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sensor sample>")
            .send(&sample_request))
        .await
        .expect("coordinator sends artifact-backed sample request");
        assert_eq!(
            (sensor_app.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor app receives artifact-backed sample request"),
            sample_request
        );
        let sample = RemoteSample::new(SENSOR.raw() as u8, 0, sensor_value, 3140);
        (sensor_app
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor flow<sample ret>")
            .send(&sample))
        .await
        .expect("sensor app sends artifact-derived sample");
        assert_eq!(
            (supervisor.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator receives artifact-derived sample"),
            sample
        );
        exchange_stdout(&mut supervisor, &mut sensor_app, sensor_marker).await;

        let actuator_command = RemoteActuateRequest::new(
            2,
            1,
            ACTUATOR.raw() as u8,
            sample
                .value()
                .wrapping_add(actuator_marker.len() as u32)
                .wrapping_add(coordinator_marker.len() as u32),
        );
        (supervisor
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<actuator command>")
            .send(&actuator_command))
        .await
        .expect("coordinator sends artifact-backed actuator command");
        assert_eq!(
            (actuator_app.recv::<RemoteActuateReqMsg>())
                .await
                .expect("actuator app receives artifact-backed command"),
            actuator_command
        );
        let actuator_ack = RemoteActuateAck::new(ACTUATOR.raw() as u8, 0);
        (actuator_app
            .flow::<RemoteActuateRetMsg>()
            .expect("actuator flow<command ack>")
            .send(&actuator_ack))
        .await
        .expect("actuator app acks command");
        assert_eq!(
            (supervisor.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator receives command ack"),
            actuator_ack
        );
        exchange_stdin(
            &mut supervisor,
            &mut actuator_app,
            actuator_stdin_request,
            actuator_stdin_chunk,
        )
        .await;
        exchange_stdout(&mut supervisor, &mut actuator_app, actuator_marker).await;

        let telemetry = SwarmTelemetry::new(
            COORDINATOR,
            RoleMask::single(NodeRole::Coordinator).with(NodeRole::Sensor),
            (sensor_marker.len() % 8) as u8,
            0,
            128u16.saturating_sub(gateway_marker.len() as u16),
            2_600,
            SESSION_GENERATION,
        );
        assert!(!telemetry.blocks_runtime_authority());
        (supervisor
            .flow::<SwarmTelemetryMsg>()
            .expect("coordinator flow<gateway telemetry>")
            .send(&telemetry))
        .await
        .expect("coordinator sends artifact-backed telemetry");
        assert_eq!(
            (gateway_app.recv::<SwarmTelemetryMsg>())
                .await
                .expect("gateway app receives telemetry"),
            telemetry
        );
        exchange_stderr(&mut supervisor, &mut gateway_app, gateway_marker).await;

        let aggregate = sample
            .value()
            .wrapping_add(actuator_command.value())
            .wrapping_add(gateway_marker.len() as u32);
        let sensor_aggregate = RemoteActuateRequest::new(3, 1, SENSOR.raw() as u8, aggregate);
        (supervisor
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<sensor aggregate>")
            .send(&sensor_aggregate))
        .await
        .expect("coordinator sends aggregate to sensor");
        assert_eq!(
            (sensor_app.recv::<RemoteActuateReqMsg>())
                .await
                .expect("sensor receives aggregate"),
            sensor_aggregate
        );
        let sensor_ack = RemoteActuateAck::new(SENSOR.raw() as u8, 0);
        (sensor_app
            .flow::<RemoteActuateRetMsg>()
            .expect("sensor flow<aggregate ack>")
            .send(&sensor_ack))
        .await
        .expect("sensor acks aggregate");
        assert_eq!(
            (supervisor.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator receives sensor ack"),
            sensor_ack
        );

        let actuator_aggregate = RemoteActuateRequest::new(3, 1, ACTUATOR.raw() as u8, aggregate);
        (supervisor
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<actuator aggregate>")
            .send(&actuator_aggregate))
        .await
        .expect("coordinator sends aggregate to actuator");
        assert_eq!(
            (actuator_app.recv::<RemoteActuateReqMsg>())
                .await
                .expect("actuator receives aggregate"),
            actuator_aggregate
        );
        let aggregate_ack = RemoteActuateAck::new(ACTUATOR.raw() as u8, 0);
        (actuator_app
            .flow::<RemoteActuateRetMsg>()
            .expect("actuator flow<aggregate ack>")
            .send(&aggregate_ack))
        .await
        .expect("actuator acks aggregate");
        assert_eq!(
            (supervisor.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator receives actuator aggregate ack"),
            aggregate_ack
        );
    });
}

#[test]
#[ignore = "requires scripts/check_wasip1_guest_builds.sh to build wasm32-wasip1 artifacts first"]
fn rust_built_wasip1_artifact_installs_as_hotswap_image_and_requires_fence() {
    hibana_pico::port::exec::run_current_task(async {
        let image = artifact("swarm-sensor");
        assert!(image.len() > MGMT_IMAGE_CHUNK_CAPACITY);

        let mut images: Box<ImageSlotTable<2, 131_072>> = Box::new(ImageSlotTable::new());
        let mut leases: MemoryLeaseTable<2> =
            MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
        leases
            .grant_read(MemBorrow::new(TEST_STDOUT_PTR, 8, TEST_MEMORY_EPOCH))
            .expect("seed outstanding lease before hotswap");

        let mut remote_objects: RemoteObjectTable<2> = RemoteObjectTable::new();
        let management_cap = remote_objects
            .apply_cap_grant_management(
                SENSOR,
                SWARM_CREDENTIAL,
                RemoteRoute::new(
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    1,
                    LABEL_MGMT_IMAGE_BEGIN,
                    SESSION_GENERATION,
                ),
                RemoteRights::Write,
            )
            .expect("install authenticated remote management cap");
        assert_eq!(
            remote_objects.resolve(
                management_cap.fd(),
                management_cap.generation(),
                RemoteRights::Write,
                SESSION_GENERATION,
            ),
            Ok(management_cap)
        );

        let plan = ImageTransferPlan::new(image.len()).expect("plan artifact image transfer");
        assert!(plan.chunk_count() > 1);
        let mgmt_grant =
            MgmtControl::install_grant(SENSOR, SWARM_CREDENTIAL, SESSION_GENERATION, 0, 314);

        let begin = MgmtImageBegin::new(0, plan.total_len(), 314);
        images
            .begin_with_control(
                mgmt_grant,
                SENSOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                begin,
            )
            .expect("begin artifact image");
        for index in 0..plan.chunk_count() {
            let (offset, end) = plan.chunk_range(index).expect("artifact image chunk range");
            let chunk = MgmtImageChunk::new(0, offset as u32, &image[offset..end])
                .expect("artifact image chunk");
            images
                .chunk_with_control(
                    mgmt_grant,
                    SENSOR,
                    SWARM_CREDENTIAL,
                    SESSION_GENERATION,
                    chunk,
                )
                .expect("append artifact image chunk");
        }
        images
            .end_with_control(
                mgmt_grant,
                SENSOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                MgmtImageEnd::new(0, plan.total_len()),
            )
            .expect("finish artifact image");
        assert_eq!(images.slot(0).expect("artifact slot").as_bytes(), image);

        let activate = MgmtImageActivate::new(0, TEST_MEMORY_EPOCH + 1);
        assert_eq!(
            images.activate_with_control(
                mgmt_grant,
                SENSOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                activate,
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    true,
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            ),
            Err(ImageSlotError::NeedFence)
        );

        leases.fence(MemFence::new(
            MemFenceReason::HotSwap,
            TEST_MEMORY_EPOCH + 1,
        ));
        assert_eq!(remote_objects.quiesce_all(), 1);
        assert!(!leases.has_outstanding_leases());
        assert!(!remote_objects.has_active());
        images
            .activate_with_control(
                mgmt_grant,
                SENSOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                activate,
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    true,
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            )
            .expect("activate artifact image after safe boundary");
        assert_eq!(images.active_slot(), Some(0));
    });
}
