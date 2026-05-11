use hibana::{
    g::Msg,
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
use hibana_pico::{
    choreography::local::{
        wasi_clock_res_get_roles as project_wasi_clock_res_get_roles,
        wasi_fd_read_stat_close_roles as project_wasi_fd_read_stat_close_roles,
        wasi_poll_oneoff_roles as project_wasi_poll_oneoff_roles,
        wasip1_clock_now_roles as project_wasip1_clock_roles,
        wasip1_exit_roles as project_wasip1_exit_roles,
        wasip1_random_seed_roles as project_wasip1_random_roles,
        wasip1_sched_yield_roles as project_wasip1_sched_yield_roles,
        wasip1_stderr_roles as project_wasip1_stderr_roles,
        wasip1_stdin_roles as project_wasip1_stdin_roles,
        wasip1_stdout_roles as project_wasip1_stdout_roles,
    },
    choreography::protocol::{
        ClockResGet, ClockResolution, EngineLabelUniverse, EngineReq, EngineRet, FdClosed, FdRead,
        FdReadDone, FdRequest, FdStat, LABEL_MEM_BORROW_READ, LABEL_MEM_BORROW_WRITE,
        LABEL_MEM_COMMIT, LABEL_MEM_RELEASE, LABEL_WASI_CLOCK_RES_GET,
        LABEL_WASI_CLOCK_RES_GET_RET, LABEL_WASI_FD_CLOSE, LABEL_WASI_FD_CLOSE_RET,
        LABEL_WASI_FD_FDSTAT_GET, LABEL_WASI_FD_FDSTAT_GET_RET, LABEL_WASI_FD_READ,
        LABEL_WASI_FD_READ_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASIP1_CLOCK_NOW, LABEL_WASIP1_CLOCK_NOW_RET, LABEL_WASIP1_EXIT,
        LABEL_WASIP1_RANDOM_SEED, LABEL_WASIP1_RANDOM_SEED_RET, LABEL_WASIP1_STDERR,
        LABEL_WASIP1_STDERR_RET, LABEL_WASIP1_STDIN, LABEL_WASIP1_STDIN_RET, LABEL_WASIP1_STDOUT,
        LABEL_WASIP1_STDOUT_RET, LABEL_YIELD_REQ, LABEL_YIELD_RET, MemBorrow, MemCommit,
        MemReadGrantControl, MemRelease, MemRights, MemWriteGrantControl, PollOneoff, PollReady,
        StdinRequest,
    },
    kernel::wasi::{
        ChoreoResourceKind, MemoryLeaseTable, PicoFdError, PicoFdRights, PicoFdView,
        Wasip1ClockModule, Wasip1ExitModule, Wasip1RandomModule, Wasip1StderrModule,
        Wasip1StdinModule, Wasip1StdoutModule,
    },
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
};

static WASIP1_STDOUT_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stdout\n";
static WASIP1_STDERR_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stderr\n";
static WASIP1_STDIN_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_read environ_get hibana stdin\n";
static WASIP1_CLOCK_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 clock_time_get environ_get";
static WASIP1_RANDOM_GUEST: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1 random_get environ_get";
static WASIP1_EXIT_GUEST: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1 proc_exit environ_get";

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
const TEST_MEMORY_EPOCH: u32 = 1;
const TEST_STDOUT_PTR: u32 = 1024;
const TEST_STDERR_PTR: u32 = 2048;
const TEST_STDIN_PTR: u32 = 3072;
const TEST_STDOUT_TEXT: &[u8] = b"hibana wasip1 stdout\n";
const TEST_STDERR_TEXT: &[u8] = b"hibana wasip1 stderr\n";
const TEST_STDIN_INPUT: &[u8] = b"hibana stdin\n";
const TEST_STDIN_MAX_LEN: u8 = 24;
const TEST_CLOCK_NANOS: u64 = 123_456_789;
const TEST_RANDOM_SEED_LO: u64 = 0x4849_4241_5241_4e44;
const TEST_RANDOM_SEED_HI: u64 = 0x5345_4544_0000_0001;
const TEST_EXIT_CODE: u8 = 7;

#[test]
fn rust_wasm32_wasip1_stdout_module_reaches_supervisor() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse module");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(51);
        let (supervisor_program, engine_program) = project_wasip1_stdout_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let mut leases: MemoryLeaseTable<4> = MemoryLeaseTable::new(4096, TEST_MEMORY_EPOCH);
        let borrow = MemBorrow::new(TEST_STDOUT_PTR, chunk.len() as u8, TEST_MEMORY_EPOCH);
        (engine
            .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
            .expect("engine flow<mem borrow read>")
            .send(&borrow))
        .await
        .expect("engine send mem borrow read");

        let received_borrow = (supervisor.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("supervisor recv mem borrow read");
        assert_eq!(received_borrow, borrow);
        let grant = leases
            .grant_read(received_borrow)
            .expect("grant read lease");
        (supervisor
            .flow::<MemReadGrantControl>()
            .expect("supervisor flow<mem read grant control>")
            .send(()))
        .await
        .expect("supervisor send mem read grant control");

        let received_grant = (engine.recv::<MemReadGrantControl>())
            .await
            .expect("engine recv mem read grant control");
        let (rights, lease_id) = received_grant
            .decode_handle()
            .expect("decode read lease token");
        assert_eq!(rights, MemRights::Read.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let request = EngineReq::Wasip1Stdout(chunk.with_lease(lease_id as u8));
        (engine
            .flow::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>()
            .expect("engine flow<stdout>")
            .send(&request))
        .await
        .expect("engine send stdout");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>())
            .await
            .expect("supervisor recv stdout");
        assert_eq!(received, request);
        let EngineReq::Wasip1Stdout(received_chunk) = received else {
            panic!("expected stdout request");
        };
        leases
            .validate_read_chunk(&received_chunk)
            .expect("stdout read lease");

        let reply = EngineRet::Wasip1StdoutWritten(received_chunk.len() as u8);
        (supervisor
            .flow::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>()
            .expect("supervisor flow<stdout ret>")
            .send(&reply))
        .await
        .expect("supervisor send stdout ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>())
            .await
            .expect("engine recv stdout ret");
        assert_eq!(received_reply, reply);

        let release = MemRelease::new(lease_id as u8);
        (engine
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("engine flow<mem release>")
            .send(&release))
        .await
        .expect("engine send mem release");
        let received_release = (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor recv mem release");
        assert_eq!(received_release, release);
        leases
            .release(received_release)
            .expect("release read lease");
    });
}

#[test]
fn rust_wasm32_wasip1_stderr_module_reaches_supervisor() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StderrModule::parse(WASIP1_STDERR_GUEST).expect("parse module");
        let chunk = module
            .stderr_chunk_for(TEST_STDERR_TEXT)
            .expect("stderr chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDERR_TEXT);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(52);
        let (supervisor_program, engine_program) = project_wasip1_stderr_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let mut leases: MemoryLeaseTable<4> = MemoryLeaseTable::new(4096, TEST_MEMORY_EPOCH);
        let borrow = MemBorrow::new(TEST_STDERR_PTR, chunk.len() as u8, TEST_MEMORY_EPOCH);
        (engine
            .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
            .expect("engine flow<mem borrow read>")
            .send(&borrow))
        .await
        .expect("engine send mem borrow read");

        let received_borrow = (supervisor.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("supervisor recv mem borrow read");
        assert_eq!(received_borrow, borrow);
        let grant = leases
            .grant_read(received_borrow)
            .expect("grant read lease");
        (supervisor
            .flow::<MemReadGrantControl>()
            .expect("supervisor flow<mem read grant control>")
            .send(()))
        .await
        .expect("supervisor send mem read grant control");

        let received_grant = (engine.recv::<MemReadGrantControl>())
            .await
            .expect("engine recv mem read grant control");
        let (rights, lease_id) = received_grant
            .decode_handle()
            .expect("decode read lease token");
        assert_eq!(rights, MemRights::Read.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let request = EngineReq::Wasip1Stderr(chunk.with_lease(lease_id as u8));
        (engine
            .flow::<Msg<LABEL_WASIP1_STDERR, EngineReq>>()
            .expect("engine flow<stderr>")
            .send(&request))
        .await
        .expect("engine send stderr");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDERR, EngineReq>>())
            .await
            .expect("supervisor recv stderr");
        assert_eq!(received, request);
        let EngineReq::Wasip1Stderr(received_chunk) = received else {
            panic!("expected stderr request");
        };
        leases
            .validate_read_chunk(&received_chunk)
            .expect("stderr read lease");

        let reply = EngineRet::Wasip1StderrWritten(received_chunk.len() as u8);
        (supervisor
            .flow::<Msg<LABEL_WASIP1_STDERR_RET, EngineRet>>()
            .expect("supervisor flow<stderr ret>")
            .send(&reply))
        .await
        .expect("supervisor send stderr ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASIP1_STDERR_RET, EngineRet>>())
            .await
            .expect("engine recv stderr ret");
        assert_eq!(received_reply, reply);

        let release = MemRelease::new(lease_id as u8);
        (engine
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("engine flow<mem release>")
            .send(&release))
        .await
        .expect("engine send mem release");
        let received_release = (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor recv mem release");
        assert_eq!(received_release, release);
        leases
            .release(received_release)
            .expect("release read lease");
    });
}

#[test]
fn rust_wasm32_wasip1_stdin_module_reaches_engine() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StdinModule::parse(WASIP1_STDIN_GUEST).expect("parse module");
        let request = module
            .stdin_request_for(TEST_STDIN_MAX_LEN)
            .expect("stdin request");
        let chunk = module
            .stdin_chunk_for(TEST_STDIN_INPUT)
            .expect("stdin chunk");
        assert_eq!(request.max_len(), TEST_STDIN_MAX_LEN);
        assert_eq!(chunk.as_bytes(), TEST_STDIN_INPUT);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(53);
        let (supervisor_program, engine_program) = project_wasip1_stdin_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let mut leases: MemoryLeaseTable<4> = MemoryLeaseTable::new(4096, TEST_MEMORY_EPOCH);
        let borrow = MemBorrow::new(TEST_STDIN_PTR, request.max_len(), TEST_MEMORY_EPOCH);
        (engine
            .flow::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>()
            .expect("engine flow<mem borrow write>")
            .send(&borrow))
        .await
        .expect("engine send mem borrow write");

        let received_borrow = (supervisor.recv::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>())
            .await
            .expect("supervisor recv mem borrow write");
        assert_eq!(received_borrow, borrow);
        let grant = leases
            .grant_write(received_borrow)
            .expect("grant write lease");
        (supervisor
            .flow::<MemWriteGrantControl>()
            .expect("supervisor flow<mem write grant control>")
            .send(()))
        .await
        .expect("supervisor send mem write grant control");

        let received_grant = (engine.recv::<MemWriteGrantControl>())
            .await
            .expect("engine recv mem write grant control");
        let (rights, lease_id) = received_grant
            .decode_handle()
            .expect("decode write lease token");
        assert_eq!(rights, MemRights::Write.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let request = StdinRequest::new_with_lease(lease_id as u8, request.max_len())
            .expect("leased stdin request");
        let request = EngineReq::Wasip1Stdin(request);
        (engine
            .flow::<Msg<LABEL_WASIP1_STDIN, EngineReq>>()
            .expect("engine flow<stdin>")
            .send(&request))
        .await
        .expect("engine send stdin");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_STDIN, EngineReq>>())
            .await
            .expect("supervisor recv stdin");
        assert_eq!(received, request);
        let EngineReq::Wasip1Stdin(received_request) = received else {
            panic!("expected stdin request");
        };
        leases
            .validate_write_request(&received_request)
            .expect("stdin write request lease");
        assert!(chunk.len() <= received_request.max_len() as usize);

        let chunk = chunk.with_lease(received_request.lease_id());
        leases
            .validate_write_chunk(&chunk)
            .expect("stdin write chunk lease");
        let reply = EngineRet::Wasip1StdinRead(chunk);
        (supervisor
            .flow::<Msg<LABEL_WASIP1_STDIN_RET, EngineRet>>()
            .expect("supervisor flow<stdin ret>")
            .send(&reply))
        .await
        .expect("supervisor send stdin ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASIP1_STDIN_RET, EngineRet>>())
            .await
            .expect("engine recv stdin ret");
        assert_eq!(received_reply, reply);

        let commit = MemCommit::new(lease_id as u8, chunk.len() as u8);
        (engine
            .flow::<Msg<LABEL_MEM_COMMIT, MemCommit>>()
            .expect("engine flow<mem commit>")
            .send(&commit))
        .await
        .expect("engine send mem commit");
        let received_commit = (supervisor.recv::<Msg<LABEL_MEM_COMMIT, MemCommit>>())
            .await
            .expect("supervisor recv mem commit");
        assert_eq!(received_commit, commit);
        leases.commit(received_commit).expect("commit write lease");

        let release = MemRelease::new(lease_id as u8);
        (engine
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("engine flow<mem release>")
            .send(&release))
        .await
        .expect("engine send mem release");
        let received_release = (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor recv mem release");
        assert_eq!(received_release, release);
        leases
            .release(received_release)
            .expect("release write lease");
    });
}

#[test]
fn rust_wasm32_wasip1_clock_module_reaches_engine() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1ClockModule::parse(WASIP1_CLOCK_GUEST).expect("parse module");
        let now = module.clock_now(TEST_CLOCK_NANOS);
        assert_eq!(now.nanos(), TEST_CLOCK_NANOS);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(54);
        let (supervisor_program, engine_program) = project_wasip1_clock_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let request = EngineReq::Wasip1ClockNow;
        (engine
            .flow::<Msg<LABEL_WASIP1_CLOCK_NOW, EngineReq>>()
            .expect("engine flow<clock now>")
            .send(&request))
        .await
        .expect("engine send clock now");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_CLOCK_NOW, EngineReq>>())
            .await
            .expect("supervisor recv clock now");
        assert_eq!(received, request);

        let reply = EngineRet::Wasip1ClockNow(now);
        (supervisor
            .flow::<Msg<LABEL_WASIP1_CLOCK_NOW_RET, EngineRet>>()
            .expect("supervisor flow<clock now ret>")
            .send(&reply))
        .await
        .expect("supervisor send clock now ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASIP1_CLOCK_NOW_RET, EngineRet>>())
            .await
            .expect("engine recv clock now ret");
        assert_eq!(received_reply, reply);
    });
}

#[test]
fn rust_wasm32_wasip1_random_module_reaches_engine() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1RandomModule::parse(WASIP1_RANDOM_GUEST).expect("parse module");
        let seed = module.random_seed(TEST_RANDOM_SEED_LO, TEST_RANDOM_SEED_HI);
        assert_eq!(seed.lo(), TEST_RANDOM_SEED_LO);
        assert_eq!(seed.hi(), TEST_RANDOM_SEED_HI);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(55);
        let (supervisor_program, engine_program) = project_wasip1_random_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let request = EngineReq::Wasip1RandomSeed;
        (engine
            .flow::<Msg<LABEL_WASIP1_RANDOM_SEED, EngineReq>>()
            .expect("engine flow<random seed>")
            .send(&request))
        .await
        .expect("engine send random seed");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_RANDOM_SEED, EngineReq>>())
            .await
            .expect("supervisor recv random seed");
        assert_eq!(received, request);

        let reply = EngineRet::Wasip1RandomSeed(seed);
        (supervisor
            .flow::<Msg<LABEL_WASIP1_RANDOM_SEED_RET, EngineRet>>()
            .expect("supervisor flow<random seed ret>")
            .send(&reply))
        .await
        .expect("supervisor send random seed ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASIP1_RANDOM_SEED_RET, EngineRet>>())
            .await
            .expect("engine recv random seed ret");
        assert_eq!(received_reply, reply);
    });
}

#[test]
fn rust_wasm32_wasip1_exit_module_reaches_supervisor() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1ExitModule::parse(WASIP1_EXIT_GUEST).expect("parse module");
        let status = module.exit_status(TEST_EXIT_CODE);
        assert_eq!(status.code(), TEST_EXIT_CODE);

        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(56);
        let (supervisor_program, engine_program) = project_wasip1_exit_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let request = EngineReq::Wasip1Exit(status);
        (engine
            .flow::<Msg<LABEL_WASIP1_EXIT, EngineReq>>()
            .expect("engine flow<exit>")
            .send(&request))
        .await
        .expect("engine send exit");

        let received = (supervisor.recv::<Msg<LABEL_WASIP1_EXIT, EngineReq>>())
            .await
            .expect("supervisor recv exit");
        assert_eq!(received, request);
    });
}

#[test]
fn generic_wasi_sched_yield_is_choreography_wired() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(57);
        let (supervisor_program, engine_program) = project_wasip1_sched_yield_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        (engine
            .flow::<Msg<LABEL_YIELD_REQ, EngineReq>>()
            .expect("engine flow<sched_yield>")
            .send(&EngineReq::Yield))
        .await
        .expect("engine send sched_yield");

        let received = (supervisor.recv::<Msg<LABEL_YIELD_REQ, EngineReq>>())
            .await
            .expect("supervisor recv sched_yield");
        assert_eq!(received, EngineReq::Yield);

        (supervisor
            .flow::<Msg<LABEL_YIELD_RET, EngineRet>>()
            .expect("supervisor flow<sched_yield ret>")
            .send(&EngineRet::Yielded))
        .await
        .expect("supervisor send sched_yield ret");

        let received_reply = (engine.recv::<Msg<LABEL_YIELD_RET, EngineRet>>())
            .await
            .expect("engine recv sched_yield ret");
        assert_eq!(received_reply, EngineRet::Yielded);
    });
}

#[test]
fn generic_wasi_clock_res_get_is_choreography_wired() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(59);
        let (supervisor_program, engine_program) = project_wasi_clock_res_get_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let request = EngineReq::ClockResGet(ClockResGet::new(0));
        (engine
            .flow::<Msg<LABEL_WASI_CLOCK_RES_GET, EngineReq>>()
            .expect("engine flow<clock_res_get>")
            .send(&request))
        .await
        .expect("engine send clock_res_get");

        let received = (supervisor.recv::<Msg<LABEL_WASI_CLOCK_RES_GET, EngineReq>>())
            .await
            .expect("supervisor recv clock_res_get");
        assert_eq!(received, request);
        let EngineReq::ClockResGet(clock) = received else {
            panic!("expected clock_res_get request");
        };
        assert_eq!(clock.clock_id(), 0);

        let reply = EngineRet::ClockResolution(ClockResolution::new(1_000_000));
        (supervisor
            .flow::<Msg<LABEL_WASI_CLOCK_RES_GET_RET, EngineRet>>()
            .expect("supervisor flow<clock_res_get ret>")
            .send(&reply))
        .await
        .expect("supervisor send clock_res_get ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASI_CLOCK_RES_GET_RET, EngineRet>>())
            .await
            .expect("engine recv clock_res_get ret");
        assert_eq!(received_reply, reply);
    });
}

#[test]
fn generic_wasi_fd_read_stat_close_are_lease_and_choreography_wired() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(57);
        let (supervisor_program, engine_program) = project_wasi_fd_read_stat_close_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let mut leases: MemoryLeaseTable<4> = MemoryLeaseTable::new(4096, TEST_MEMORY_EPOCH);
        let mut fds: PicoFdView<2> = PicoFdView::new();
        let stdin_fd = fds
            .apply_local_cap_grant(0, PicoFdRights::Read, ChoreoResourceKind::Stdin, 1, 0, 0)
            .expect("grant stdin fd");
        let borrow = MemBorrow::new(TEST_STDIN_PTR + 128, 12, TEST_MEMORY_EPOCH);
        (engine
            .flow::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>()
            .expect("engine flow<mem borrow write>")
            .send(&borrow))
        .await
        .expect("engine send mem borrow write");

        let received_borrow = (supervisor.recv::<Msg<LABEL_MEM_BORROW_WRITE, MemBorrow>>())
            .await
            .expect("supervisor recv mem borrow write");
        assert_eq!(received_borrow, borrow);
        let grant = leases
            .grant_write(received_borrow)
            .expect("grant fd_read destination lease");
        (supervisor
            .flow::<MemWriteGrantControl>()
            .expect("supervisor flow<mem write grant control>")
            .send(()))
        .await
        .expect("supervisor send mem write grant control");

        let received_grant = (engine.recv::<MemWriteGrantControl>())
            .await
            .expect("engine recv mem write grant control");
        let (rights, lease_id) = received_grant
            .decode_handle()
            .expect("decode write lease token");
        assert_eq!(rights, MemRights::Write.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let read = FdRead::new_with_lease(0, lease_id as u8, 8).expect("leased fd_read");
        let request = EngineReq::FdRead(read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<fd_read>")
            .send(&request))
        .await
        .expect("engine send fd_read");

        let received = (supervisor.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("supervisor recv fd_read");
        assert_eq!(received, request);
        let EngineReq::FdRead(received_read) = received else {
            panic!("expected fd_read request");
        };
        let resolved_read = fds
            .resolve_current(
                received_read.fd(),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin,
            )
            .expect("resolve fd_read through local fd view");
        assert_eq!(resolved_read, stdin_fd);
        assert_eq!(received_read.lease_id(), grant.lease_id());
        assert!(received_read.max_len() <= grant.len());

        let fd_read_bytes = b"pico-fd";
        let done =
            FdReadDone::new_with_lease(0, received_read.lease_id(), fd_read_bytes).expect("done");
        assert_eq!(done.as_bytes(), fd_read_bytes);
        assert!(done.len() <= received_read.max_len() as usize);
        let reply = EngineRet::FdReadDone(done);
        (supervisor
            .flow::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>()
            .expect("supervisor flow<fd_read ret>")
            .send(&reply))
        .await
        .expect("supervisor send fd_read ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>())
            .await
            .expect("engine recv fd_read ret");
        assert_eq!(received_reply, reply);

        let commit = MemCommit::new(lease_id as u8, done.len() as u8);
        (engine
            .flow::<Msg<LABEL_MEM_COMMIT, MemCommit>>()
            .expect("engine flow<mem commit>")
            .send(&commit))
        .await
        .expect("engine send mem commit");
        let received_commit = (supervisor.recv::<Msg<LABEL_MEM_COMMIT, MemCommit>>())
            .await
            .expect("supervisor recv mem commit");
        assert_eq!(received_commit, commit);
        leases
            .commit(received_commit)
            .expect("commit fd_read destination lease");

        let release = MemRelease::new(lease_id as u8);
        (engine
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("engine flow<mem release>")
            .send(&release))
        .await
        .expect("engine send mem release");
        let received_release = (supervisor.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("supervisor recv mem release");
        assert_eq!(received_release, release);
        leases
            .release(received_release)
            .expect("release fd_read destination lease");

        let fdstat_request = EngineReq::FdFdstatGet(FdRequest::new(0));
        (engine
            .flow::<Msg<LABEL_WASI_FD_FDSTAT_GET, EngineReq>>()
            .expect("engine flow<fd_fdstat_get>")
            .send(&fdstat_request))
        .await
        .expect("engine send fd_fdstat_get");
        let received_fdstat = (supervisor.recv::<Msg<LABEL_WASI_FD_FDSTAT_GET, EngineReq>>())
            .await
            .expect("supervisor recv fd_fdstat_get");
        assert_eq!(received_fdstat, fdstat_request);
        let EngineReq::FdFdstatGet(fdstat) = received_fdstat else {
            panic!("expected fd_fdstat_get request");
        };
        let resolved_stat = fds
            .resolve_current(fdstat.fd(), PicoFdRights::Read, ChoreoResourceKind::Stdin)
            .expect("resolve fd_fdstat_get through local fd view");
        assert_eq!(resolved_stat, stdin_fd);

        let fdstat_reply = EngineRet::FdStat(FdStat::new(fdstat.fd(), MemRights::Read));
        (supervisor
            .flow::<Msg<LABEL_WASI_FD_FDSTAT_GET_RET, EngineRet>>()
            .expect("supervisor flow<fd_fdstat_get ret>")
            .send(&fdstat_reply))
        .await
        .expect("supervisor send fd_fdstat_get ret");
        let received_fdstat_reply = (engine.recv::<Msg<LABEL_WASI_FD_FDSTAT_GET_RET, EngineRet>>())
            .await
            .expect("engine recv fd_fdstat_get ret");
        assert_eq!(received_fdstat_reply, fdstat_reply);

        let close_request = EngineReq::FdClose(FdRequest::new(0));
        (engine
            .flow::<Msg<LABEL_WASI_FD_CLOSE, EngineReq>>()
            .expect("engine flow<fd_close>")
            .send(&close_request))
        .await
        .expect("engine send fd_close");
        let received_close = (supervisor.recv::<Msg<LABEL_WASI_FD_CLOSE, EngineReq>>())
            .await
            .expect("supervisor recv fd_close");
        assert_eq!(received_close, close_request);
        let EngineReq::FdClose(close) = received_close else {
            panic!("expected fd_close request");
        };
        let closed = fds
            .close_current(close.fd())
            .expect("close current stdin fd");
        assert_eq!(closed.fd(), stdin_fd.fd());
        assert!(closed.is_revoked());
        assert_eq!(
            fds.resolve_current(close.fd(), PicoFdRights::Read, ChoreoResourceKind::Stdin),
            Err(PicoFdError::Revoked)
        );
        let reopened = fds
            .apply_local_cap_grant(0, PicoFdRights::Read, ChoreoResourceKind::Stdin, 1, 0, 0)
            .expect("reopen stdin with a new generation");
        assert_eq!(
            fds.resolve(
                0,
                stdin_fd.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin
            ),
            Err(PicoFdError::BadGeneration)
        );
        assert_eq!(
            fds.resolve(
                0,
                reopened.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin
            ),
            Ok(reopened)
        );

        let close_reply = EngineRet::FdClosed(FdClosed::new(close.fd()));
        (supervisor
            .flow::<Msg<LABEL_WASI_FD_CLOSE_RET, EngineRet>>()
            .expect("supervisor flow<fd_close ret>")
            .send(&close_reply))
        .await
        .expect("supervisor send fd_close ret");
        let received_close_reply = (engine.recv::<Msg<LABEL_WASI_FD_CLOSE_RET, EngineRet>>())
            .await
            .expect("engine recv fd_close ret");
        assert_eq!(received_close_reply, close_reply);
    });
}

#[test]
fn generic_wasi_poll_oneoff_is_choreography_wired() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register engine rendezvous");

        let sid = SessionId::new(58);
        let (supervisor_program, engine_program) = project_wasi_poll_oneoff_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let request = EngineReq::PollOneoff(PollOneoff::new(44));
        (engine
            .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
            .expect("engine flow<poll_oneoff>")
            .send(&request))
        .await
        .expect("engine send poll_oneoff");

        let received = (supervisor.recv::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>())
            .await
            .expect("supervisor recv poll_oneoff");
        assert_eq!(received, request);
        let EngineReq::PollOneoff(poll) = received else {
            panic!("expected poll_oneoff request");
        };
        assert_eq!(poll.timeout_tick(), 44);

        let reply = EngineRet::PollReady(PollReady::new(1));
        (supervisor
            .flow::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
            .expect("supervisor flow<poll_oneoff ret>")
            .send(&reply))
        .await
        .expect("supervisor send poll_oneoff ret");

        let received_reply = (engine.recv::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>())
            .await
            .expect("engine recv poll_oneoff ret");
        assert_eq!(received_reply, reply);
    });
}
