use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use std::{env, fs, path::PathBuf};

use hibana::{
    Endpoint,
    g::Msg,
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        policy::ResolverRef,
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
use hibana_pico::{
    choreography::local::wasip1_stdout_uart_roles,
    choreography::protocol::{
        BudgetRun, BudgetRunMsg, EngineAbort, EngineAbortAckControl, EngineAbortBeginControl,
        EngineAbortFenceControl, EngineAbortMsg, EngineAbortReason, EngineAbortRouteControl,
        EngineLabelUniverse, EngineReq, EngineRet, FdWrite, FdWriteDone, GpioSet, LABEL_GPIO_SET,
        LABEL_GPIO_SET_DONE, LABEL_MEM_BORROW_READ, LABEL_MEM_RELEASE, LABEL_UART_WRITE,
        LABEL_UART_WRITE_RET, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASIP1_STDOUT,
        LABEL_WASIP1_STDOUT_RET, MemBorrow, MemReadGrantControl, MemRelease, MemRights, PollOneoff,
        StdoutChunk, TimerSleepDone, UartWrite, UartWriteDone,
    },
    kernel::device::{gpio::GpioStateTable, uart::UartTxLog},
    kernel::fd_object::check_gpio_object_fd_write,
    kernel::features::Wasip1HandlerSet,
    kernel::guest_ledger::GuestLedger,
    kernel::wasi::{MemoryLeaseTable, Wasip1FdWriteModule, Wasip1LedBlinkModule, Wasip1Module},
    machine::rp2040::baker_link::BAKER_LINK_SAFE_GPIO_LEVELS,
    port::exec::run_current_task,
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
    projects::baker_link_led::{
        choreography::{
            BakerTrafficLoopContinueControl, POLICY_BAKER_ENGINE_ABORT_ROUTE,
            POLICY_BAKER_TRAFFIC_LOOP, abort_safe_linear_roles, abort_safe_terminal_roles,
            fd_write_two_cycles_roles, traffic_light_roles,
        },
        ledger::baker_link_pico_min_ledger,
        manifest::{
            BAKER_LINK_LED_ACTIVE_HIGH, BAKER_LINK_LED_FD, BAKER_LINK_LED_PIN,
            BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS, apply_baker_link_led_bank_set,
            baker_link_led_fd_write_route,
        },
        resolver::{BakerAbortRouteResolver, BakerTrafficLoopResolver},
    },
};

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
use hibana_pico::{
    choreography::protocol::{
        LABEL_TIMER_SLEEP_DONE, LABEL_TIMER_SLEEP_UNTIL, LABEL_WASI_POLL_ONEOFF,
        LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, PollReady, ProcExitStatus,
        TimerSleepUntil,
    },
    kernel::resolver::{InterruptEvent, PicoInterruptResolver, ResolvedInterrupt},
    projects::baker_link_led::choreography::BakerTrafficLoopBreakControl,
};

#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
use hibana_pico::choreography::protocol::{
    LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, PathOpen, PathOpened,
};

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
use hibana_pico::kernel::engine::wasm::{Call, Error as WasmError, Event, Guest};

#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
use hibana_pico::{
    kernel::engine::wasm::{Path as WasiPathCall, PathKind, Pending},
    projects::baker_link_led::manifest::{
        BAKER_LINK_LED_RESOURCE_PATHS, BakerLinkLedResourceStore, baker_link_led_resource_store,
    },
    projects::baker_link_led::{
        choreography::choreofs_traffic_light_roles,
        ledger::{
            baker_link_choreofs_ledger, mint_baker_link_choreofs_fd,
            resolve_baker_link_choreofs_path,
        },
    },
};

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
use hibana_pico::projects::baker_link_led::manifest::BAKER_LINK_LED_FDS;

#[cfg(feature = "profile-rp2040-pico-min")]
use hibana_pico::{
    kernel::fd_object::GpioFdWriteError,
    projects::baker_link_led::manifest::{BAKER_LINK_LED_PINS, baker_link_traffic_light_step},
};

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
type TestBakerLedger = GuestLedger<3, 1, 1>;

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
const TEST_WASI_FUEL: u32 = 250_000;

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
fn test_budget() -> BudgetRun {
    BudgetRun::new(0, 1, TEST_WASI_FUEL, 0)
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
trait GuestTestExt<'a> {
    fn next_event(&mut self) -> Result<Event<'_, 'a>, WasmError>;
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
impl<'a> GuestTestExt<'a> for Guest<'a> {
    fn next_event(&mut self) -> Result<Event<'_, 'a>, WasmError> {
        self.resume(test_budget())
    }
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
fn run_large_stack_test(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("hibana-pico-large-stack-test".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .expect("spawn large-stack test")
        .join()
        .expect("large-stack test panicked");
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
#[derive(Clone, Copy)]
struct ExpectedTrafficStep {
    fd: u8,
    payload: u8,
    delay_ticks: u64,
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "wasm-engine-wasip1-std-profile",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full",
))]
impl ExpectedTrafficStep {
    const fn new(fd: u8, payload: u8, delay_ticks: u64) -> Self {
        Self {
            fd,
            payload,
            delay_ticks,
        }
    }
}

#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_expected_traffic_steps() -> [ExpectedTrafficStep; BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS] {
    let mut steps = [ExpectedTrafficStep::new(0, 0, 0); BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS];
    let mut step = 0usize;
    while step < BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS {
        let traffic_step = baker_link_traffic_light_step(step);
        steps[step] = ExpectedTrafficStep::new(
            traffic_step.fd(),
            if traffic_step.high() { b'1' } else { b'0' },
            traffic_step.delay_ticks() as u64,
        );
        step += 1;
    }
    steps
}

fn wasip1_artifact(name: &str) -> Vec<u8> {
    let dir = env::var("HIBANA_WASIP1_GUEST_DIR")
        .unwrap_or_else(|_| "target/wasip1-apps/wasm32-wasip1/release".to_owned());
    let path = PathBuf::from(dir).join(format!("{name}.wasm"));
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

const TEST_MEMORY_LEN: u32 = 4096;
#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
const TEST_CHOREOFS_MEMORY_LEN: u32 = 64 * 1024;
const TEST_MEMORY_EPOCH: u32 = 1;
const TEST_LED_PTR: u32 = 128;
static WASIP1_LED_FD_WRITE_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write hibana baker link led fd 3 1 0\n";

fn register_baker_traffic_loop_resolver<'a>(
    cluster: &TestKit<'a>,
    rv: hibana::substrate::ids::RendezvousId,
    kernel_program: &hibana::substrate::program::RoleProgram<0>,
    engine_program: &hibana::substrate::program::RoleProgram<1>,
    gpio_program: &hibana::substrate::program::RoleProgram<2>,
    timer_program: &hibana::substrate::program::RoleProgram<3>,
    traffic_policy: &'a BakerTrafficLoopResolver,
) {
    let resolver =
        ResolverRef::loop_state(traffic_policy, BakerTrafficLoopResolver::resolve_policy);
    cluster
        .set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 0>(rv, kernel_program, resolver)
        .expect("register baker traffic kernel loop resolver");
    cluster
        .set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 1>(
            rv,
            engine_program,
            ResolverRef::loop_state(traffic_policy, BakerTrafficLoopResolver::resolve_policy),
        )
        .expect("register baker traffic engine loop resolver");
    cluster
        .set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 2>(rv, gpio_program, resolver)
        .expect("register baker traffic gpio loop resolver");
    cluster
        .set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 3>(rv, timer_program, resolver)
        .expect("register baker traffic timer loop resolver");
}

fn register_baker_abort_route_resolver<'a>(
    cluster: &TestKit<'a>,
    rv: hibana::substrate::ids::RendezvousId,
    kernel_program: &hibana::substrate::program::RoleProgram<0>,
    engine_program: &hibana::substrate::program::RoleProgram<1>,
    gpio_program: &hibana::substrate::program::RoleProgram<2>,
    route_policy: &'a BakerAbortRouteResolver,
) {
    let resolver = ResolverRef::route_state(route_policy, BakerAbortRouteResolver::resolve_policy);
    cluster
        .set_resolver::<POLICY_BAKER_ENGINE_ABORT_ROUTE, 0>(rv, kernel_program, resolver)
        .expect("register baker abort kernel route resolver");
    cluster
        .set_resolver::<POLICY_BAKER_ENGINE_ABORT_ROUTE, 1>(
            rv,
            engine_program,
            ResolverRef::route_state(route_policy, BakerAbortRouteResolver::resolve_policy),
        )
        .expect("register baker abort engine route resolver");
    cluster
        .set_resolver::<POLICY_BAKER_ENGINE_ABORT_ROUTE, 2>(rv, gpio_program, resolver)
        .expect("register baker abort gpio route resolver");
}

fn test_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
        test_raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    RawWaker::new(
        core::ptr::null(),
        &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
    )
}

fn poll_once<F: Future>(future: &mut F) -> Poll<F::Output> {
    let waker = unsafe { Waker::from_raw(test_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(future) };
    future.as_mut().poll(&mut cx)
}

async fn gpio_decode_set(gpio: &mut Endpoint<'_, 2>, route_depth: u8) -> GpioSet {
    if route_depth == 0 {
        return gpio
            .recv::<Msg<LABEL_GPIO_SET, GpioSet>>()
            .await
            .expect("gpio receives fd_write gpio set");
    }
    let branch = (gpio.offer()).await.expect("gpio offers fd_write gpio set");
    assert_eq!(branch.label(), LABEL_GPIO_SET);
    branch
        .decode::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .await
        .expect("gpio decodes fd_write gpio set")
}

async fn exchange_linear_led_write<const ROLE: u8>(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ROLE>,
    gpio: &mut Endpoint<'_, 2>,
    ledger: &mut TestBakerLedger,
    pins: &mut GpioStateTable<32>,
    fd: u8,
    payload: &[u8],
) {
    let borrow = MemBorrow::new(TEST_LED_PTR, payload.len() as u8, TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<led borrow>")
        .send(&borrow))
    .await
    .expect("engine sends led borrow");

    let borrow = (kernel.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
        .await
        .expect("kernel receives led borrow");
    let grant = ledger
        .grant_read_lease(borrow)
        .expect("ledger grants led read lease");
    (kernel
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<led grant>")
        .send(()))
    .await
    .expect("kernel sends led grant");

    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives led grant")
        .decode_handle()
        .expect("decode led lease");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let write = FdWrite::new_with_lease(fd, lease_id as u8, payload).expect("make led fd_write");
    let request = EngineReq::FdWrite(write);
    (engine
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .expect("engine flow<led fd_write>")
        .send(&request))
    .await
    .expect("engine sends led fd_write");

    let request = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .await
        .expect("kernel receives led fd_write");
    let EngineReq::FdWrite(write) = request else {
        panic!("expected fd_write request");
    };
    ledger
        .validate_fd_write_lease(&write, grant)
        .expect("ledger validates fd_write lease");
    let set = check_gpio_object_fd_write(ledger.fd_view(), &write, baker_link_led_fd_write_route())
        .expect("route led fd");
    (kernel
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<gpio set>")
        .send(&set))
    .await
    .expect("kernel sends gpio set");

    let received_set = gpio_decode_set(gpio, 0).await;
    assert_eq!(received_set, set);
    apply_baker_link_led_bank_set(
        |pin, high| {
            pins.apply(GpioSet::new(pin, high))
                .expect("apply baker led gpio");
        },
        received_set,
    );
    (gpio
        .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .expect("gpio flow<set done>")
        .send(&received_set))
    .await
    .expect("gpio sends set done");

    let received_set_done = (kernel.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>())
        .await
        .expect("kernel receives gpio set done");
    assert_eq!(received_set_done, set);

    let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
    (kernel
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("kernel flow<led fd_write ret>")
        .send(&reply))
    .await
    .expect("kernel sends led fd_write ret");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
            .await
            .expect("engine receives led fd_write ret"),
        reply
    );

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<led release>")
        .send(&release))
    .await
    .expect("engine sends led release");
    ledger
        .release_lease(
            (kernel.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
                .await
                .expect("kernel receives led release"),
        )
        .expect("ledger releases led lease");
}

async fn exchange_policy_entry_led_write<const ROLE: u8, const FDS: usize>(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ROLE>,
    gpio: &mut Endpoint<'_, 2>,
    ledger: &mut GuestLedger<FDS, 1, 1>,
    pins: &mut GpioStateTable<32>,
    fd: u8,
    payload: &[u8],
) {
    let borrow = MemBorrow::new(TEST_LED_PTR, payload.len() as u8, TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<led borrow>")
        .send(&borrow))
    .await
    .expect("engine sends led borrow");

    let branch = (kernel.offer())
        .await
        .expect("kernel offers loop body mem borrow");
    assert_eq!(branch.label(), LABEL_MEM_BORROW_READ);
    let borrow = branch
        .decode::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .await
        .expect("kernel decodes loop body mem borrow");
    assert_eq!(borrow.ptr(), TEST_LED_PTR);
    assert_eq!(borrow.len(), payload.len() as u8);
    assert_eq!(borrow.epoch(), TEST_MEMORY_EPOCH);

    let grant = ledger
        .grant_read_lease(borrow)
        .expect("ledger grants led read lease");
    (kernel
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<led grant>")
        .send(()))
    .await
    .expect("kernel sends led grant");

    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives led grant")
        .decode_handle()
        .expect("decode led lease");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let write = FdWrite::new_with_lease(fd, lease_id as u8, payload).expect("make led fd_write");
    let request = EngineReq::FdWrite(write);
    (engine
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .expect("engine flow<led fd_write>")
        .send(&request))
    .await
    .expect("engine sends led fd_write");

    let request = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .await
        .expect("kernel receives led fd_write");
    let EngineReq::FdWrite(write) = request else {
        panic!("expected fd_write request");
    };
    assert_eq!(write.lease_id(), grant.lease_id());
    assert_eq!(write.as_bytes(), payload);
    ledger
        .validate_fd_write_lease(&write, grant)
        .expect("ledger validates fd_write lease");

    let set = check_gpio_object_fd_write(ledger.fd_view(), &write, baker_link_led_fd_write_route())
        .expect("route led fd");
    (kernel
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<gpio set>")
        .send(&set))
    .await
    .expect("kernel sends gpio set");

    let received_set = gpio_decode_set(gpio, 1).await;
    assert_eq!(received_set, set);
    apply_baker_link_led_bank_set(
        |pin, high| {
            pins.apply(GpioSet::new(pin, high))
                .expect("apply baker led gpio");
        },
        received_set,
    );
    (gpio
        .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .expect("gpio flow<set done>")
        .send(&received_set))
    .await
    .expect("gpio sends set done");

    let received_set_done = (kernel.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>())
        .await
        .expect("kernel receives gpio set done");
    assert_eq!(received_set_done, set);

    let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
    (kernel
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("kernel flow<led fd_write ret>")
        .send(&reply))
    .await
    .expect("kernel sends led fd_write ret");

    let received_reply = (engine.recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
        .await
        .expect("engine receives led fd_write ret");
    assert_eq!(received_reply, reply);

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<led release>")
        .send(&release))
    .await
    .expect("engine sends led release");

    let release = (kernel.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
        .await
        .expect("kernel receives led release");
    assert_eq!(release.lease_id(), grant.lease_id());
    ledger
        .release_lease(release)
        .expect("ledger releases led lease");
}

async fn exchange_baker_led_write<const ROLE: u8, const FDS: usize>(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ROLE>,
    gpio: &mut Endpoint<'_, 2>,
    ledger: &mut GuestLedger<FDS, 1, 1>,
    pins: &mut GpioStateTable<32>,
    fd: u8,
    payload: &[u8],
) {
    exchange_policy_entry_led_write(kernel, engine, gpio, ledger, pins, fd, payload).await;
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
async fn exchange_policy_entry_poll_oneoff<const ROLE: u8, const FDS: usize>(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ROLE>,
    gpio: &mut Endpoint<'_, 2>,
    timer: &mut Endpoint<'_, 3>,
    ledger: &mut GuestLedger<FDS, 1, 1>,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    timeout_tick: u64,
) {
    let _ = gpio;
    let request = EngineReq::PollOneoff(PollOneoff::new(timeout_tick));
    (engine
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .expect("engine flow<poll_oneoff>")
        .send(&request))
    .await
    .expect("engine sends poll_oneoff");

    let received = (kernel.recv::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>())
        .await
        .expect("kernel receives poll_oneoff");
    assert_eq!(received, request);
    let EngineReq::PollOneoff(poll) = received else {
        panic!("expected poll_oneoff request");
    };
    let pending_poll = ledger.begin_poll_oneoff(poll).expect("ledger begins poll");

    let sleep = TimerSleepUntil::new(poll.timeout_tick());
    (kernel
        .flow::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .expect("kernel flow<timer sleep>")
        .send(&sleep))
    .await
    .expect("kernel sends timer sleep");

    let branch = (timer.offer())
        .await
        .expect("timer offers policy-entry sleep");
    assert_eq!(branch.label(), LABEL_TIMER_SLEEP_UNTIL);
    let received_sleep = branch
        .decode::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .await
        .expect("timer decodes policy-entry sleep");
    assert_eq!(received_sleep, sleep);
    resolver
        .request_timer_sleep(received_sleep)
        .expect("register poll timeout");
    resolver
        .push_irq(InterruptEvent::TimerTick {
            tick: poll.timeout_tick().saturating_sub(1),
        })
        .expect("record early timer tick");
    assert_eq!(resolver.resolve_next(), Ok(None));
    resolver
        .push_irq(InterruptEvent::TimerTick {
            tick: poll.timeout_tick(),
        })
        .expect("record due timer tick");
    let Some(ResolvedInterrupt::TimerSleepDone(done)) =
        resolver.resolve_next().expect("resolve timer tick")
    else {
        panic!("expected timer sleep done");
    };
    assert_eq!(done, TimerSleepDone::new(poll.timeout_tick()));
    (timer
        .flow::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>()
        .expect("timer flow<sleep done>")
        .send(&done))
    .await
    .expect("timer sends sleep done");

    let received_done = (kernel.recv::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>())
        .await
        .expect("kernel receives timer sleep done");
    assert_eq!(received_done, done);
    ledger
        .complete_poll_oneoff(pending_poll, done)
        .expect("ledger completes poll");

    let reply = EngineRet::PollReady(PollReady::new(1));
    (kernel
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
        .expect("kernel flow<poll_oneoff ret>")
        .send(&reply))
    .await
    .expect("kernel sends poll_oneoff ret");

    let received_reply = (engine.recv::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>())
        .await
        .expect("engine receives poll_oneoff ret");
    assert_eq!(received_reply, reply);
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
async fn exchange_baker_poll_oneoff<const ROLE: u8, const FDS: usize>(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, ROLE>,
    gpio: &mut Endpoint<'_, 2>,
    timer: &mut Endpoint<'_, 3>,
    ledger: &mut GuestLedger<FDS, 1, 1>,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    timeout_tick: u64,
) {
    exchange_policy_entry_poll_oneoff(kernel, engine, gpio, timer, ledger, resolver, timeout_tick)
        .await;
}

async fn exchange_app_activation_start(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, 1>,
    activation_id: u16,
    tick: u64,
) {
    let run = BudgetRun::new(
        activation_id,
        1,
        BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS as u32,
        tick,
    );
    (kernel
        .flow::<BudgetRunMsg>()
        .expect("kernel flow<traffic run>")
        .send(&run))
    .await
    .expect("kernel sends traffic run");

    let received = (engine.recv::<BudgetRunMsg>())
        .await
        .expect("engine receives traffic run");
    assert_eq!(received, run);
}

#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
async fn exchange_choreofs_path_open(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, 1>,
    store: &BakerLinkLedResourceStore,
    ledger: &mut GuestLedger<4, 1, 1>,
    call: Pending<'_, '_, WasiPathCall>,
    expected_path: &[u8],
    expected_fd: u8,
) {
    assert_eq!(call.kind(), PathKind::PathOpen);
    let ptr = call.arg_i32(2).expect("path ptr");
    let len = call.arg_i32(3).expect("path len");
    let preopen_fd = call.fd().expect("path preopen fd");
    let rights_base = call.arg_i64(5).expect("path rights_base");
    let path = call.path_bytes().expect("path bytes");
    assert_eq!(path.as_bytes(), expected_path);

    let borrow = MemBorrow::new(ptr, len as u8, TEST_MEMORY_EPOCH);
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<choreofs path borrow>")
        .send(&borrow))
    .await
    .expect("engine sends choreofs path borrow");
    let received_borrow = (kernel.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
        .await
        .expect("kernel receives choreofs path borrow");
    assert_eq!(received_borrow, borrow);
    let grant = ledger
        .grant_read_lease(received_borrow)
        .expect("ledger grants choreofs path lease");
    (kernel
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<choreofs path grant>")
        .send(()))
    .await
    .expect("kernel sends choreofs path grant");
    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives choreofs path grant")
        .decode_handle()
        .expect("decode choreofs path lease");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let request = EngineReq::PathOpen(
        PathOpen::new(preopen_fd, lease_id as u8, rights_base, path.as_bytes())
            .expect("make path_open request"),
    );
    (engine
        .flow::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
        .expect("engine flow<path_open>")
        .send(&request))
    .await
    .expect("engine sends path_open");
    let received_request = (kernel.recv::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>())
        .await
        .expect("kernel receives path_open");
    let EngineReq::PathOpen(open) = received_request else {
        panic!("expected path_open request");
    };
    assert_eq!(open.preopen_fd(), preopen_fd);
    assert_eq!(open.lease_id(), grant.lease_id());
    assert_eq!(open.path(), path.as_bytes());
    let opened = resolve_baker_link_choreofs_path(store, ledger, open.path(), open.rights_base())
        .expect("Baker ChoreoFS opens LED object path");
    assert_eq!(opened.fd(), expected_fd);
    (kernel
        .flow::<hibana_pico::choreography::protocol::ChoreoFsOpenAdmitRouteMsg>()
        .expect("kernel flow<choreofs open admit route>")
        .send(&hibana_pico::choreography::protocol::ChoreoFsOpenAdmitRoute))
    .await
    .expect("kernel selects ChoreoFS open admit route");
    (engine.recv::<hibana_pico::choreography::protocol::ChoreoFsOpenAdmitRouteMsg>())
        .await
        .expect("engine receives ChoreoFS open admit route");
    let opened_fd =
        mint_baker_link_choreofs_fd(ledger, opened).expect("route admit materializes LED fd");
    let reply = EngineRet::PathOpened(PathOpened::new(opened_fd.fd(), 0));
    (kernel
        .flow::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
        .expect("kernel flow<path_open ret>")
        .send(&reply))
    .await
    .expect("kernel sends path_open ret");
    assert_eq!(
        (engine.recv::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>())
            .await
            .expect("engine receives path_open ret"),
        reply
    );

    let release = MemRelease::new(lease_id as u8);
    (engine
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<choreofs path release>")
        .send(&release))
    .await
    .expect("engine sends choreofs path release");
    let received_release = (kernel.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
        .await
        .expect("kernel receives choreofs path release");
    assert_eq!(received_release, release);
    ledger
        .release_lease(received_release)
        .expect("ledger releases choreofs path lease");

    call.complete_path_open(opened.fd() as u32, 0)
        .expect("complete guest path_open");
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
async fn exchange_wasip1_proc_exit(
    kernel: &mut Endpoint<'_, 0>,
    engine: &mut Endpoint<'_, 1>,
    code: u8,
) {
    let request = EngineReq::ProcExit(ProcExitStatus::new(code));
    (engine
        .flow::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
        .expect("engine flow<proc_exit>")
        .send(&request))
    .await
    .expect("engine sends proc_exit");

    let branch = (kernel.offer()).await.expect("kernel offers proc_exit");
    assert_eq!(branch.label(), LABEL_WASI_PROC_EXIT);
    assert_eq!(
        branch
            .decode::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
            .await
            .expect("kernel decodes proc_exit"),
        request
    );
}

async fn exchange_traffic_loop_continue(engine: &mut Endpoint<'_, 1>) {
    (engine
        .flow::<BakerTrafficLoopContinueControl>()
        .expect("engine flow<traffic loop continue>")
        .send(()))
    .await
    .expect("engine sends traffic loop continue");
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    all(
        feature = "wasm-engine-wasip1-std-profile",
        feature = "wasip1-sys-path-minimal",
    ),
))]
async fn exchange_traffic_loop_break(engine: &mut Endpoint<'_, 1>) {
    (engine
        .flow::<BakerTrafficLoopBreakControl>()
        .expect("engine flow<traffic loop break>")
        .send(()))
    .await
    .expect("engine sends traffic loop break");
}

fn baker_ledger() -> TestBakerLedger {
    baker_link_pico_min_ledger(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
        .expect("create Baker Pico-Min guest ledger")
}

fn register_rendezvous<'a>(
    cluster: &TestKit<'a>,
    tap: &'a mut [TapEvent; 128],
    slab: &'a mut [u8],
    backend: &'a HostQueueBackend,
) -> hibana::substrate::ids::RendezvousId {
    cluster
        .add_rendezvous_from_config(
            Config::new(tap, slab).with_universe(EngineLabelUniverse),
            SioTransport::new(backend),
        )
        .expect("register rendezvous")
}

#[cfg(any(
    feature = "profile-rp2040-pico-min",
    feature = "baker-ordinary-std-demo",
    feature = "profile-host-linux-wasip1-full"
))]
fn baker_expected_chaser_steps() -> [ExpectedTrafficStep; BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS] {
    [
        ExpectedTrafficStep::new(3, b'1', 250),
        ExpectedTrafficStep::new(4, b'1', 50),
        ExpectedTrafficStep::new(5, b'1', 50),
        ExpectedTrafficStep::new(4, b'1', 50),
        ExpectedTrafficStep::new(3, b'1', 50),
        ExpectedTrafficStep::new(4, b'1', 50),
        ExpectedTrafficStep::new(5, b'1', 250),
    ]
}

#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
fn baker_expected_choreofs_steps() -> [ExpectedTrafficStep; BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS]
{
    [
        ExpectedTrafficStep::new(3, b'1', 180),
        ExpectedTrafficStep::new(4, b'1', 40),
        ExpectedTrafficStep::new(4, b'0', 40),
        ExpectedTrafficStep::new(4, b'1', 40),
        ExpectedTrafficStep::new(4, b'0', 40),
        ExpectedTrafficStep::new(4, b'1', 40),
        ExpectedTrafficStep::new(5, b'1', 180),
    ]
}

#[cfg(feature = "profile-rp2040-pico-min")]
async fn run_baker_wasip1_pattern(
    artifact_name: &str,
    expected_steps: [ExpectedTrafficStep; BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS],
    sid: u32,
) {
    let backend = HostQueueBackend::new();

    let clock = CounterClock::new();
    let mut tap = [TapEvent::zero(); 128];
    let mut slab = vec![0u8; 262_144];
    let traffic_policy = BakerTrafficLoopResolver::new();
    let cluster = TestKit::new(&clock);
    let rv = cluster
        .add_rendezvous_from_config(
            Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
            SioTransport::new(&backend),
        )
        .expect("register baker rendezvous");

    let (kernel_program, engine_program, gpio_program, timer_program) = traffic_light_roles();
    register_baker_traffic_loop_resolver(
        &cluster,
        rv,
        &kernel_program,
        &engine_program,
        &gpio_program,
        &timer_program,
        &traffic_policy,
    );
    let mut kernel = cluster
        .enter(rv, SessionId::new(sid), &kernel_program, NoBinding)
        .expect("attach kernel endpoint");
    let mut engine = cluster
        .enter(rv, SessionId::new(sid), &engine_program, NoBinding)
        .expect("attach engine endpoint");
    let mut gpio = cluster
        .enter(rv, SessionId::new(sid), &gpio_program, NoBinding)
        .expect("attach gpio endpoint");
    let mut timer = cluster
        .enter(rv, SessionId::new(sid), &timer_program, NoBinding)
        .expect("attach timer endpoint");

    let mut ledger = baker_ledger();
    let mut pins: GpioStateTable<32> = GpioStateTable::new();
    let mut resolver: PicoInterruptResolver<2, 4, 1> = PicoInterruptResolver::new();

    let mut expected_levels = [!BAKER_LINK_LED_ACTIVE_HIGH; 32];
    let mut tick = 0u64;
    let wasip1_guest = wasip1_artifact(artifact_name);
    exchange_app_activation_start(&mut kernel, &mut engine, 0, tick).await;
    let mut guest = Guest::new(&wasip1_guest).unwrap_or_else(|error| {
        panic!("instantiate {artifact_name} through core wasip1: {error:?}")
    });
    for (step, expected_step) in expected_steps.iter().copied().enumerate() {
        let Event::Call(Call::FdWrite(write)) =
            guest.next_event().expect("guest resumes to fd_write")
        else {
            panic!("expected fd_write import trap");
        };
        exchange_traffic_loop_continue(&mut engine).await;
        let payload = write.payload().expect("fd_write iovec");
        let fd = write.fd();
        let payload_bytes = payload.as_bytes();
        assert_eq!(fd, expected_step.fd);
        assert_eq!(payload_bytes, &[expected_step.payload]);
        let selected_pin = BAKER_LINK_LED_FDS
            .iter()
            .position(|candidate| *candidate == fd)
            .map(|index| BAKER_LINK_LED_PINS[index])
            .expect("fd maps to Baker LED pin");
        exchange_baker_led_write(
            &mut kernel,
            &mut engine,
            &mut gpio,
            &mut ledger,
            &mut pins,
            fd,
            payload_bytes,
        )
        .await;
        write.complete(0).expect("complete fd_write");

        let high = expected_step.payload == b'1';
        if high == BAKER_LINK_LED_ACTIVE_HIGH {
            for pin in BAKER_LINK_LED_PINS {
                expected_levels[pin as usize] = !BAKER_LINK_LED_ACTIVE_HIGH;
            }
        }
        expected_levels[selected_pin as usize] = high;
        for pin in BAKER_LINK_LED_PINS {
            assert_eq!(
                pins.level(pin),
                Ok(expected_levels[pin as usize]),
                "fd {fd} should update GPIO {selected_pin} during traffic step {step}"
            );
        }

        let Event::Call(Call::PollOneoff(poll)) =
            guest.next_event().expect("guest resumes to poll_oneoff")
        else {
            panic!("expected poll_oneoff import trap");
        };
        let delay = poll.delay_ticks().expect("poll delay");
        assert_eq!(delay, expected_step.delay_ticks);
        tick = tick.saturating_add(delay);
        exchange_baker_poll_oneoff(
            &mut kernel,
            &mut engine,
            &mut gpio,
            &mut timer,
            &mut ledger,
            &mut resolver,
            tick,
        )
        .await;
        poll.complete(1, 0).expect("complete poll_oneoff");
    }
    assert!(matches!(
        guest.next_event().expect("guest reaches done"),
        Event::Done
    ));
    exchange_traffic_loop_break(&mut engine).await;
    exchange_wasip1_proc_exit(&mut kernel, &mut engine, 0).await;
}

#[cfg(feature = "profile-rp2040-pico-min")]
async fn run_baker_wasip1_fd_object_reject(
    artifact_name: &str,
    expected_fd: u8,
    expected_payload: &[u8],
    expected_error: GpioFdWriteError,
    sid: u32,
) {
    let backend = HostQueueBackend::new();
    let clock = CounterClock::new();
    let mut tap = [TapEvent::zero(); 128];
    let mut slab = vec![0u8; 192 * 1024];
    let traffic_policy = BakerTrafficLoopResolver::new();
    let cluster = TestKit::new(&clock);
    let rv = cluster
        .add_rendezvous_from_config(
            Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
            SioTransport::new(&backend),
        )
        .expect("register firmware-sized rendezvous");

    let (kernel_program, engine_program, _gpio_program, _timer_program) = traffic_light_roles();
    register_baker_traffic_loop_resolver(
        &cluster,
        rv,
        &kernel_program,
        &engine_program,
        &_gpio_program,
        &_timer_program,
        &traffic_policy,
    );
    let mut kernel: Endpoint<'_, 0> = cluster
        .enter(rv, SessionId::new(sid), &kernel_program, NoBinding)
        .expect("attach kernel endpoint");
    let mut engine: Endpoint<'_, 1> = cluster
        .enter(rv, SessionId::new(sid), &engine_program, NoBinding)
        .expect("attach engine endpoint");
    let mut gpio: Endpoint<'_, 2> = cluster
        .enter(rv, SessionId::new(sid), &_gpio_program, NoBinding)
        .expect("attach gpio endpoint");
    let mut ledger = baker_ledger();

    exchange_app_activation_start(&mut kernel, &mut engine, 0, 0).await;
    let artifact = wasip1_artifact(artifact_name);
    let mut guest = Guest::new(&artifact)
        .unwrap_or_else(|error| panic!("instantiate {artifact_name}: {error:?}"));

    let Event::Call(Call::FdWrite(call)) =
        guest.next_event().expect("guest reaches first fd_write")
    else {
        panic!("expected first fd_write import trap");
    };
    exchange_traffic_loop_continue(&mut engine).await;
    let payload = call.payload().expect("fd_write payload");
    assert_eq!(call.fd(), expected_fd);
    assert_eq!(payload.as_bytes(), expected_payload);

    let borrow = MemBorrow::new(
        TEST_LED_PTR,
        payload.as_bytes().len() as u8,
        TEST_MEMORY_EPOCH,
    );
    (engine
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<bad led borrow>")
        .send(&borrow))
    .await
    .expect("engine sends bad led borrow");

    let branch = (kernel.offer())
        .await
        .expect("kernel offers bad led borrow");
    assert_eq!(branch.label(), LABEL_MEM_BORROW_READ);
    let borrow = branch
        .decode::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .await
        .expect("kernel decodes bad led borrow");
    let grant = ledger
        .grant_read_lease(borrow)
        .expect("ledger grants bad led read lease");
    (kernel
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<bad led grant>")
        .send(()))
    .await
    .expect("kernel sends bad led grant");

    let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
        .await
        .expect("engine receives bad led grant")
        .decode_handle()
        .expect("decode bad led lease");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let write = FdWrite::new_with_lease(call.fd(), lease_id as u8, payload.as_bytes())
        .expect("make bad led fd_write");
    let request = EngineReq::FdWrite(write);
    (engine
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .expect("engine flow<bad led fd_write>")
        .send(&request))
    .await
    .expect("engine sends bad led fd_write");

    let request = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .await
        .expect("kernel receives bad led fd_write");
    let EngineReq::FdWrite(write) = request else {
        panic!("expected fd_write request");
    };
    ledger
        .validate_fd_write_lease(&write, grant)
        .expect("bad led write still has a valid lease");
    assert_eq!(
        check_gpio_object_fd_write(ledger.fd_view(), &write, baker_link_led_fd_write_route()),
        Err(expected_error),
        "{artifact_name} must fail at fd object fact check, not at choreography order or memory lease"
    );

    assert!(
        kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .is_err(),
        "Kernel cannot skip the explicit GPIO route and synthesize a fd_write return"
    );
    let mut pending_gpio = gpio.offer();
    assert!(
        matches!(poll_once(&mut pending_gpio), Poll::Pending),
        "bad fd_write must not advance GPIO without a valid fd/object route"
    );
    let _ = gpio;
}

#[test]
fn baker_link_led_fd_write_is_wasip1_preview1_artifact() {
    let module =
        Wasip1FdWriteModule::parse(WASIP1_LED_FD_WRITE_GUEST).expect("parse fd_write module");
    assert!(
        module
            .bytes()
            .windows(b"fd_write".len())
            .any(|w| w == b"fd_write")
    );
    assert!(
        !module
            .bytes()
            .windows(b"wasi_snapshot_preview2".len())
            .any(|w| w == b"wasi_snapshot_preview2")
    );
}

#[test]
fn baker_link_led_blink_is_wasip1_preview1_fd_write_plus_poll_oneoff_artifact() {
    let artifact = wasip1_artifact("wasip1-led-blink");
    let module = Wasip1LedBlinkModule::parse(&artifact).expect("parse blink module");
    assert!(
        module
            .bytes()
            .windows(b"fd_write".len())
            .any(|w| w == b"fd_write")
    );
    assert!(
        module
            .bytes()
            .windows(b"poll_oneoff".len())
            .any(|w| w == b"poll_oneoff")
    );
    assert!(
        !module
            .bytes()
            .windows(b"wasi_snapshot_preview2".len())
            .any(|w| w == b"wasi_snapshot_preview2")
    );
}

#[test]
fn wasip1_stdout_reaches_uart_device_role_through_choreography() {
    run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = register_rendezvous(&cluster0, &mut tap0, slab0.as_mut_slice(), &backend);

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = register_rendezvous(&cluster1, &mut tap1, slab1.as_mut_slice(), &backend);

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = TestKit::new(&clock2);
        let rv2 = register_rendezvous(&cluster2, &mut tap2, slab2.as_mut_slice(), &backend);

        let (kernel_program, engine_program, uart_program) = wasip1_stdout_uart_roles();
        let sid = SessionId::new(135);
        let mut kernel = cluster0
            .enter(rv0, sid, &kernel_program, NoBinding)
            .expect("attach kernel endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");
        let mut uart = cluster2
            .enter(rv2, sid, &uart_program, NoBinding)
            .expect("attach uart endpoint");

        let mut leases: MemoryLeaseTable<1> =
            MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
        let mut uart_log: UartTxLog<256> = UartTxLog::new();
        let stdout = b"hello choreographed uart\n";

        let borrow = MemBorrow::new(TEST_LED_PTR, stdout.len() as u8, TEST_MEMORY_EPOCH);
        (engine
            .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
            .expect("engine flow<stdout borrow>")
            .send(&borrow))
        .await
        .expect("engine sends stdout borrow");

        let borrow = (kernel.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("kernel receives stdout borrow");
        let grant = leases.grant_read(borrow).expect("grant stdout lease");
        (kernel
            .flow::<MemReadGrantControl>()
            .expect("kernel flow<stdout grant>")
            .send(()))
        .await
        .expect("kernel sends stdout grant");

        let (rights, lease_id) = (engine.recv::<MemReadGrantControl>())
            .await
            .expect("engine receives stdout grant")
            .decode_handle()
            .expect("decode stdout lease");
        assert_eq!(rights, MemRights::Read.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let request = EngineReq::Wasip1Stdout(
            StdoutChunk::new_with_lease(lease_id as u8, stdout).expect("stdout chunk"),
        );
        (engine
            .flow::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>()
            .expect("engine flow<stdout>")
            .send(&request))
        .await
        .expect("engine sends stdout");

        let received = (kernel.recv::<Msg<LABEL_WASIP1_STDOUT, EngineReq>>())
            .await
            .expect("kernel receives stdout");
        let EngineReq::Wasip1Stdout(chunk) = received else {
            panic!("expected stdout request");
        };
        assert_eq!(chunk.lease_id(), grant.lease_id());
        assert_eq!(chunk.as_bytes(), stdout);

        let uart_write = UartWrite::new(chunk.as_bytes()).expect("make uart write");
        (kernel
            .flow::<Msg<LABEL_UART_WRITE, UartWrite>>()
            .expect("kernel flow<uart write>")
            .send(&uart_write))
        .await
        .expect("kernel sends uart write");

        let received_uart = (uart.recv::<Msg<LABEL_UART_WRITE, UartWrite>>())
            .await
            .expect("uart receives stdout write");
        assert_eq!(received_uart, uart_write);
        let uart_done = uart_log.write(received_uart).expect("uart writes stdout");
        (uart
            .flow::<Msg<LABEL_UART_WRITE_RET, UartWriteDone>>()
            .expect("uart flow<write ret>")
            .send(&uart_done))
        .await
        .expect("uart sends write ret");

        let received_uart_done = (kernel.recv::<Msg<LABEL_UART_WRITE_RET, UartWriteDone>>())
            .await
            .expect("kernel receives uart ret");
        assert_eq!(received_uart_done.written(), stdout.len() as u8);

        let reply = EngineRet::Wasip1StdoutWritten(stdout.len() as u8);
        (kernel
            .flow::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>()
            .expect("kernel flow<stdout ret>")
            .send(&reply))
        .await
        .expect("kernel sends stdout ret");

        assert_eq!(
            (engine.recv::<Msg<LABEL_WASIP1_STDOUT_RET, EngineRet>>())
                .await
                .expect("engine receives stdout ret"),
            reply
        );

        let release = MemRelease::new(lease_id as u8);
        (engine
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("engine flow<stdout release>")
            .send(&release))
        .await
        .expect("engine sends stdout release");
        let release = (kernel.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("kernel receives stdout release");
        leases.release(release).expect("release stdout lease");
        assert_eq!(uart_log.as_bytes(), stdout);
    });
}

#[test]
fn baker_link_led_fd_write_digits_drive_first_visible_led_through_choreography() {
    run_current_task(async {
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
            .expect("register kernel rendezvous");

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

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = TestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register gpio rendezvous");

        let (kernel_program, engine_program, gpio_program) = fd_write_two_cycles_roles();
        let sid = SessionId::new(133);
        let mut kernel = cluster0
            .enter(rv0, sid, &kernel_program, NoBinding)
            .expect("attach kernel endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");
        let mut gpio = cluster2
            .enter(rv2, sid, &gpio_program, NoBinding)
            .expect("attach gpio endpoint");

        let mut ledger = baker_ledger();
        let mut pins: GpioStateTable<32> = GpioStateTable::new();

        exchange_linear_led_write(
            &mut kernel,
            &mut engine,
            &mut gpio,
            &mut ledger,
            &mut pins,
            BAKER_LINK_LED_FD,
            b"1",
        )
        .await;
        assert_eq!(
            pins.level(BAKER_LINK_LED_PIN),
            Ok(BAKER_LINK_LED_ACTIVE_HIGH)
        );

        exchange_linear_led_write(
            &mut kernel,
            &mut engine,
            &mut gpio,
            &mut ledger,
            &mut pins,
            BAKER_LINK_LED_FD,
            b"0",
        )
        .await;
        assert_eq!(
            pins.level(BAKER_LINK_LED_PIN),
            Ok(!BAKER_LINK_LED_ACTIVE_HIGH)
        );
    });
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_led_blink_uses_timer_resolver_between_fd_writes() {
    run_large_stack_test(|| {
        run_current_task(async {
            run_baker_wasip1_pattern("wasip1-led-blink", baker_expected_traffic_steps(), 134).await;
        });
    });
}

#[test]
#[cfg(all(
    feature = "wasm-engine-wasip1-std-profile",
    feature = "wasip1-sys-path-minimal",
))]
fn baker_link_choreofs_opened_led_fds_drive_gpio_and_timer_like_firmware() {
    run_large_stack_test(|| {
        run_current_task(async {
            let backend = HostQueueBackend::new();
            let clock = CounterClock::new();
            let mut tap = [TapEvent::zero(); 128];
            let mut slab = vec![0u8; 262_144];
            let traffic_policy = BakerTrafficLoopResolver::new();
            let cluster = TestKit::new(&clock);
            let rv = cluster
                .add_rendezvous_from_config(
                    Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                    SioTransport::new(&backend),
                )
                .expect("register Baker ChoreoFS rendezvous");
            let (kernel_program, engine_program, gpio_program, timer_program) =
                choreofs_traffic_light_roles();
            register_baker_traffic_loop_resolver(
                &cluster,
                rv,
                &kernel_program,
                &engine_program,
                &gpio_program,
                &timer_program,
                &traffic_policy,
            );
            let sid = SessionId::new(172);
            let mut kernel = cluster
                .enter(rv, sid, &kernel_program, NoBinding)
                .expect("attach ChoreoFS kernel endpoint");
            let mut engine = cluster
                .enter(rv, sid, &engine_program, NoBinding)
                .expect("attach ChoreoFS engine endpoint");
            let mut gpio = cluster
                .enter(rv, sid, &gpio_program, NoBinding)
                .expect("attach ChoreoFS gpio endpoint");
            let mut timer = cluster
                .enter(rv, sid, &timer_program, NoBinding)
                .expect("attach ChoreoFS timer endpoint");

            let store = baker_link_led_resource_store().expect("create Baker ChoreoFS store");
            let mut ledger = baker_link_choreofs_ledger::<4, 1, 1>(
                &store,
                TEST_CHOREOFS_MEMORY_LEN,
                TEST_MEMORY_EPOCH,
            )
            .expect("create Baker ChoreoFS ledger");
            let mut pins: GpioStateTable<32> = GpioStateTable::new();
            let mut resolver: PicoInterruptResolver<2, 4, 1> = PicoInterruptResolver::new();
            let artifact = wasip1_artifact("wasip1-led-choreofs-open");
            let mut guest =
                Guest::new(&artifact).expect("instantiate Baker ChoreoFS WASI P1 guest");

            exchange_app_activation_start(&mut kernel, &mut engine, 0, 0).await;
            for (path, fd) in BAKER_LINK_LED_RESOURCE_PATHS
                .into_iter()
                .zip(BAKER_LINK_LED_FDS)
            {
                let Event::Call(Call::Path(call)) =
                    guest.next_event().expect("guest reaches path_open")
                else {
                    panic!("expected ChoreoFS path_open trap");
                };
                assert!(!call.is_full(), "Baker ChoreoFS path_open is minimal");
                exchange_choreofs_path_open(
                    &mut kernel,
                    &mut engine,
                    &store,
                    &mut ledger,
                    call,
                    path,
                    fd,
                )
                .await;
            }

            let mut tick = 0u64;
            for expected_step in baker_expected_choreofs_steps() {
                let Event::Call(Call::FdWrite(write)) =
                    guest.next_event().expect("guest reaches ChoreoFS fd_write")
                else {
                    panic!("expected fd_write after ChoreoFS opens");
                };
                exchange_traffic_loop_continue(&mut engine).await;
                let payload = write.payload().expect("fd_write payload");
                let fd = write.fd();
                let payload_bytes = payload.as_bytes();
                assert_eq!(fd, expected_step.fd);
                assert_eq!(payload_bytes, &[expected_step.payload]);
                exchange_baker_led_write(
                    &mut kernel,
                    &mut engine,
                    &mut gpio,
                    &mut ledger,
                    &mut pins,
                    fd,
                    payload_bytes,
                )
                .await;
                write.complete(0).expect("complete ChoreoFS fd_write");

                let Event::Call(Call::PollOneoff(poll)) = guest
                    .next_event()
                    .expect("guest reaches ChoreoFS poll_oneoff")
                else {
                    panic!("expected poll_oneoff after ChoreoFS fd_write");
                };
                let delay = poll.delay_ticks().expect("poll delay");
                assert_eq!(delay, expected_step.delay_ticks);
                tick = tick.saturating_add(delay);
                exchange_baker_poll_oneoff(
                    &mut kernel,
                    &mut engine,
                    &mut gpio,
                    &mut timer,
                    &mut ledger,
                    &mut resolver,
                    tick,
                )
                .await;
                poll.complete(1, 0).expect("complete ChoreoFS poll_oneoff");
            }

            let final_status = match guest.next_event().expect("guest finishes ChoreoFS traffic") {
                Event::Done => 0,
                Event::Exit(status) => {
                    assert_eq!(status.status(), 0);
                    status.status() as u8
                }
                _ => panic!("unexpected ChoreoFS final event"),
            };
            exchange_traffic_loop_break(&mut engine).await;
            exchange_wasip1_proc_exit(&mut kernel, &mut engine, final_status).await;
        });
    });
}

#[test]
fn baker_link_abort_terminal_fences_ledger_and_uses_gpio_choreography_for_safe_state() {
    run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock = CounterClock::new();
        let mut tap = [TapEvent::zero(); 128];
        let mut slab = vec![0u8; 262_144];
        let abort_policy = BakerAbortRouteResolver::new_abort();
        let cluster = TestKit::new(&clock);
        let rv = cluster
            .add_rendezvous_from_config(
                Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register abort terminal rendezvous");
        let (kernel_program, engine_program, gpio_program) = abort_safe_terminal_roles();
        register_baker_abort_route_resolver(
            &cluster,
            rv,
            &kernel_program,
            &engine_program,
            &gpio_program,
            &abort_policy,
        );
        let sid = SessionId::new(171);
        let mut kernel: Endpoint<'_, 0> = cluster
            .enter(rv, sid, &kernel_program, NoBinding)
            .expect("attach abort kernel endpoint");
        let mut engine: Endpoint<'_, 1> = cluster
            .enter(rv, sid, &engine_program, NoBinding)
            .expect("attach abort engine endpoint");
        let mut gpio: Endpoint<'_, 2> = cluster
            .enter(rv, sid, &gpio_program, NoBinding)
            .expect("attach abort gpio endpoint");

        let mut ledger = baker_ledger();
        let grant = ledger
            .grant_read_lease(MemBorrow::new(TEST_LED_PTR, 1, TEST_MEMORY_EPOCH))
            .expect("grant pre-abort lease");
        let pending = ledger
            .begin_poll_oneoff(PollOneoff::new(50))
            .expect("create pre-abort pending syscall");
        assert_eq!(ledger.fd_view().active_count(), 3);
        assert_eq!(ledger.lease_table().outstanding_lease_count(), 1);
        assert_eq!(ledger.pending_table().pending_count(), 1);

        (engine
            .flow::<EngineAbortRouteControl>()
            .expect("engine flow<abort route>")
            .send(()))
        .await
        .expect("engine selects abort route");

        let abort = EngineAbort::new(EngineAbortReason::GuestTrap, 1);
        (engine
            .flow::<EngineAbortMsg>()
            .expect("engine flow<abort reason>")
            .send(&abort))
        .await
        .expect("engine sends abort reason");
        let branch = (kernel.offer()).await.expect("kernel offers abort reason");
        assert_eq!(
            branch.label(),
            hibana_pico::choreography::protocol::LABEL_ENGINE_ABORT_REASON
        );
        assert_eq!(
            branch
                .decode::<EngineAbortMsg>()
                .await
                .expect("kernel decodes abort reason"),
            abort
        );

        (engine
            .flow::<EngineAbortBeginControl>()
            .expect("engine flow<abort begin>")
            .send(()))
        .await
        .expect("engine sends abort begin");
        (kernel.recv::<EngineAbortBeginControl>())
            .await
            .expect("kernel receives abort begin");

        ledger.apply_abort_fence(TEST_MEMORY_EPOCH + 1);
        assert_eq!(ledger.fd_view().active_count(), 0);
        assert_eq!(ledger.lease_table().outstanding_lease_count(), 0);
        assert_eq!(ledger.pending_table().pending_count(), 0);
        assert!(
            ledger
                .release_lease(MemRelease::new(grant.lease_id()))
                .is_err(),
            "fenced lease must not be reusable after Abort/Fence"
        );
        assert!(
            ledger
                .complete_poll_oneoff(pending, TimerSleepDone::new(50))
                .is_err(),
            "fenced pending token must not complete after Abort/Fence"
        );

        (kernel
            .flow::<EngineAbortFenceControl>()
            .expect("kernel flow<abort fence>")
            .send(()))
        .await
        .expect("kernel sends abort fence");
        (engine.recv::<EngineAbortFenceControl>())
            .await
            .expect("engine receives abort fence");

        let mut pins: GpioStateTable<32> = GpioStateTable::new();
        for (idx, safe) in BAKER_LINK_SAFE_GPIO_LEVELS.iter().enumerate() {
            let set = GpioSet::new(safe.pin(), safe.high());
            (kernel
                .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
                .expect("kernel flow<safe gpio set>")
                .send(&set))
            .await
            .expect("kernel sends safe gpio set");
            let received = gpio_decode_set(&mut gpio, if idx == 0 { 1 } else { 0 }).await;
            assert_eq!(received, set);
            pins.apply(received).expect("apply safe-state gpio set");
            (gpio
                .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
                .expect("gpio flow<safe done>")
                .send(&received))
            .await
            .expect("gpio sends safe-state done");
            assert_eq!(
                (kernel.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>())
                    .await
                    .expect("kernel receives safe-state done"),
                received
            );
        }
        for safe in BAKER_LINK_SAFE_GPIO_LEVELS {
            assert_eq!(pins.level(safe.pin()), Ok(safe.high()));
        }

        (kernel
            .flow::<EngineAbortAckControl>()
            .expect("kernel flow<abort ack>")
            .send(()))
        .await
        .expect("kernel sends abort ack");
        (engine.recv::<EngineAbortAckControl>())
            .await
            .expect("engine receives abort ack");
    });
}

#[test]
fn baker_link_abort_linear_fragment_attaches_and_runs_safe_state() {
    run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock = CounterClock::new();
        let mut tap = [TapEvent::zero(); 128];
        let mut slab = vec![0u8; 262_144];
        let cluster = TestKit::new(&clock);
        let rv = cluster
            .add_rendezvous_from_config(
                Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register abort linear rendezvous");
        let (kernel_program, engine_program, gpio_program) = abort_safe_linear_roles();
        let sid = SessionId::new(172);
        let mut kernel: Endpoint<'_, 0> = cluster
            .enter(rv, sid, &kernel_program, NoBinding)
            .expect("attach abort linear kernel endpoint");
        let mut engine: Endpoint<'_, 1> = cluster
            .enter(rv, sid, &engine_program, NoBinding)
            .expect("attach abort linear engine endpoint");
        let mut gpio: Endpoint<'_, 2> = cluster
            .enter(rv, sid, &gpio_program, NoBinding)
            .expect("attach abort linear gpio endpoint");

        let abort = EngineAbort::new(EngineAbortReason::GuestTrap, 1);
        (engine
            .flow::<EngineAbortMsg>()
            .expect("engine flow<linear abort reason>")
            .send(&abort))
        .await
        .expect("engine sends linear abort reason");
        assert_eq!(
            (kernel.recv::<EngineAbortMsg>())
                .await
                .expect("kernel receives linear abort reason"),
            abort
        );
        (engine
            .flow::<EngineAbortBeginControl>()
            .expect("engine flow<linear abort begin>")
            .send(()))
        .await
        .expect("engine sends linear abort begin");
        (kernel.recv::<EngineAbortBeginControl>())
            .await
            .expect("kernel receives linear abort begin");
        (kernel
            .flow::<EngineAbortFenceControl>()
            .expect("kernel flow<linear abort fence>")
            .send(()))
        .await
        .expect("kernel sends linear abort fence");
        (engine.recv::<EngineAbortFenceControl>())
            .await
            .expect("engine receives linear abort fence");

        for safe in BAKER_LINK_SAFE_GPIO_LEVELS {
            let set = GpioSet::new(safe.pin(), safe.high());
            (kernel
                .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
                .expect("kernel flow<linear safe gpio>")
                .send(&set))
            .await
            .expect("kernel sends linear safe gpio");
            let received = (gpio.recv::<Msg<LABEL_GPIO_SET, GpioSet>>())
                .await
                .expect("gpio receives linear safe gpio");
            assert_eq!(received, set);
            (gpio
                .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
                .expect("gpio flow<linear safe done>")
                .send(&received))
            .await
            .expect("gpio sends linear safe done");
            assert_eq!(
                (kernel.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>())
                    .await
                    .expect("kernel receives linear safe done"),
                received
            );
        }

        (kernel
            .flow::<EngineAbortAckControl>()
            .expect("kernel flow<linear abort ack>")
            .send(()))
        .await
        .expect("kernel sends linear abort ack");
        (engine.recv::<EngineAbortAckControl>())
            .await
            .expect("engine receives linear abort ack");
    });
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_no_main_wasip1_app_runs_on_core_wasip1_trampoline() {
    let artifact = wasip1_artifact("wasip1-led-blink");
    let mut guest =
        Guest::new(&artifact).expect("instantiate no_main app through core wasip1 trampoline");
    let Event::Call(Call::FdWrite(write)) = guest.next_event().expect("first core fd_write") else {
        panic!("expected fd_write import trap from no_main artifact");
    };
    assert_eq!(write.fd(), 3);
    let payload = write.payload().expect("core fd_write payload");
    assert_eq!(payload.as_bytes(), b"1");
    write.complete(0).expect("complete core fd_write");
    let Event::Call(Call::PollOneoff(poll)) = guest.next_event().expect("first core poll_oneoff")
    else {
        panic!("expected poll_oneoff import trap from no_main artifact");
    };
    assert_eq!(poll.delay_ticks(), Ok(250));
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_chaser_wasip1_app_changes_fd_order_without_choreography_changes() {
    run_large_stack_test(|| {
        run_current_task(async {
            run_baker_wasip1_pattern("wasip1-led-chaser", baker_expected_chaser_steps(), 148).await;
        });
    });
}

#[test]
fn baker_link_ordinary_std_wasip1_app_is_rust_std_artifact_and_not_pico_min() {
    let artifact = wasip1_artifact("wasip1-led-ordinary-std-chaser");
    assert!(
        artifact
            .windows(b"_start".len())
            .any(|window| window == b"_start"),
        "ordinary std artifact must use Rust std _start"
    );
    assert!(
        !artifact
            .windows(b"hibana ordinary std wasip1 chaser".len())
            .any(|window| window == b"hibana ordinary std wasip1 chaser"),
        "ordinary std execution must not depend on a traffic marker"
    );
    assert!(
        Wasip1Module::install_with_handlers(&artifact, Wasip1HandlerSet::PICO_MIN).is_err(),
        "ordinary std artifact must not be accepted by explicit pico-min capacity"
    );
}

#[test]
#[cfg(feature = "baker-ordinary-std-demo")]
fn baker_link_ordinary_std_wasip1_app_fits_embedded_std_start_profile_when_sized() {
    let artifact = wasip1_artifact("wasip1-led-ordinary-std-chaser");
    for needle in [
        b"fd_write".as_slice(),
        b"poll_oneoff".as_slice(),
        b"proc_exit".as_slice(),
        b"_start".as_slice(),
    ] {
        assert!(
            artifact
                .windows(needle.len())
                .any(|window| window == needle),
            "ordinary std artifact must keep full-profile WASI import {:?}",
            core::str::from_utf8(needle).unwrap_or("<non-utf8>")
        );
    }
    assert!(
        [b"args_get".as_slice(), b"environ_get".as_slice()]
            .iter()
            .any(|needle| artifact
                .windows(needle.len())
                .any(|window| window == *needle)),
        "ordinary std artifact must keep at least one Rust std startup environment import"
    );
    let mut guest = Guest::new(&artifact)
        .expect("64KiB ordinary Rust std WASI P1 artifact fits the embedded std-start profile");
    let expected = baker_expected_chaser_steps();
    let mut write_index = 0usize;
    let mut poll_index = 0usize;
    for _ in 0..64 {
        match guest
            .next_event()
            .expect("ordinary std embedded profile reaches typed import or done")
        {
            Event::Call(Call::EnvironSizesGet(call)) => call
                .complete(0, 0, 0)
                .expect("complete empty environ sizes"),
            Event::Call(Call::EnvironGet(call)) => {
                call.complete(&[], 0).expect("complete empty environ")
            }
            Event::Call(Call::ArgsSizesGet(call)) => {
                call.complete(0, 0, 0).expect("complete empty args sizes")
            }
            Event::Call(Call::ArgsGet(call)) => call.complete(&[], 0).expect("complete empty args"),
            Event::Call(Call::FdWrite(call)) => {
                let payload = call.payload().expect("ordinary std fd_write");
                let expected_step = expected
                    .get(write_index)
                    .copied()
                    .expect("ordinary std emitted too many fd_write calls");
                assert_eq!(call.fd(), expected_step.fd);
                assert_eq!(payload.as_bytes(), &[expected_step.payload]);
                call.complete(0).expect("complete ordinary std fd_write");
                write_index += 1;
            }
            Event::Call(Call::PollOneoff(call)) => {
                let delay = call.delay_ticks().expect("ordinary std poll_oneoff delay");
                let expected_step = expected
                    .get(poll_index)
                    .copied()
                    .expect("ordinary std emitted too many poll_oneoff calls");
                assert_eq!(delay, expected_step.delay_ticks);
                call.complete(1, 0)
                    .expect("complete ordinary std poll_oneoff");
                poll_index += 1;
            }
            Event::Exit(status) => {
                assert_eq!(status.status(), 0);
                break;
            }
            Event::Done => break,
            _ => panic!("unexpected embedded ordinary std event"),
        }
    }
    assert_eq!(write_index, expected.len());
    assert_eq!(poll_index, expected.len());
}

#[test]
fn baker_link_firmware_demo_uses_core_wasip1_engine() {
    let source = include_str!("../src/projects/baker_link_led/runtime.rs");
    assert!(source.contains("write_selected_guest_in_place"));
    assert!(
        !source.contains("Guest::write_new_in_place"),
        "Baker firmware runtime must not call engine placement internals directly"
    );
    assert!(
        !source.contains("TestWasip1TrafficLight"),
        "Baker firmware demo must stay on the core WASI P1 engine path"
    );
    assert!(
        !source.contains("complete_environ_get") && !source.contains("complete_environ_sizes_get"),
        "Baker firmware must not complete unused std-start imports in an adapter side channel"
    );
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
fn baker_link_ordinary_std_wasip1_app_instantiates_on_host_full_profile() {
    let artifact = wasip1_artifact("wasip1-led-ordinary-std-chaser");
    Guest::new(&artifact)
        .expect("host/full profile instantiates ordinary Rust std WASI P1 artifact");
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
fn baker_link_ordinary_std_wasip1_app_reaches_first_host_import_on_host_full_profile() {
    let artifact = wasip1_artifact("wasip1-led-ordinary-std-chaser");
    let mut guest = Guest::new(&artifact)
        .expect("host/full profile instantiates ordinary Rust std WASI P1 artifact");
    let first = guest.next_event();
    assert!(
        matches!(
            first,
            Ok(Event::Call(Call::FdWrite(_)))
                | Ok(Event::Call(Call::PollOneoff(_)))
                | Ok(Event::Call(Call::EnvironSizesGet(_)))
                | Ok(Event::Call(Call::EnvironGet(_)))
                | Ok(Event::Call(Call::ArgsSizesGet(_)))
                | Ok(Event::Call(Call::ArgsGet(_)))
                | Ok(Event::Exit(_))
        ),
        "ordinary std guest must reach a typed WASI P1 host import"
    );
}

#[test]
#[cfg(feature = "profile-host-linux-wasip1-full")]
fn baker_link_ordinary_std_wasip1_app_runs_import_stream_on_host_full_profile() {
    let artifact = wasip1_artifact("wasip1-led-ordinary-std-chaser");
    let mut guest = Guest::new(&artifact)
        .expect("host/full profile instantiates ordinary Rust std WASI P1 artifact");
    let expected = baker_expected_chaser_steps();
    let mut write_index = 0usize;
    let mut poll_index = 0usize;

    for _ in 0..128 {
        match guest
            .next_event()
            .expect("ordinary std guest reaches typed import or done")
        {
            Event::Call(Call::EnvironSizesGet(call)) => call
                .complete(0, 0, 0)
                .expect("complete empty environ sizes"),
            Event::Call(Call::EnvironGet(call)) => {
                call.complete(&[], 0).expect("complete empty environ")
            }
            Event::Call(Call::ArgsSizesGet(call)) => {
                call.complete(0, 0, 0).expect("complete empty args sizes")
            }
            Event::Call(Call::ArgsGet(call)) => call.complete(&[], 0).expect("complete empty args"),
            Event::Call(Call::FdWrite(call)) => {
                let payload = call.payload().expect("ordinary std fd_write");
                let expected_step = expected
                    .get(write_index)
                    .copied()
                    .expect("ordinary std emitted too many fd_write calls");
                assert_eq!(call.fd(), expected_step.fd);
                assert_eq!(payload.as_bytes(), &[expected_step.payload]);
                call.complete(0).expect("complete ordinary std fd_write");
                write_index += 1;
            }
            Event::Call(Call::PollOneoff(call)) => {
                let delay = call.delay_ticks().expect("ordinary std poll_oneoff delay");
                let expected_step = expected
                    .get(poll_index)
                    .copied()
                    .expect("ordinary std emitted too many poll_oneoff calls");
                assert_eq!(delay, expected_step.delay_ticks);
                call.complete(1, 0)
                    .expect("complete ordinary std poll_oneoff");
                poll_index += 1;
            }
            Event::Exit(status) => {
                assert_eq!(status.status(), 0);
                break;
            }
            Event::Call(Call::MemoryGrow(event)) => {
                assert!(
                    event.event().new_pages.is_some(),
                    "ordinary std memory.grow must stay within host/full profile"
                );
                event
                    .complete()
                    .expect("complete ordinary std memory.grow event");
            }
            Event::Done => break,
            _ => panic!("unexpected ordinary std host import"),
        }
    }

    assert_eq!(write_index, expected.len());
    assert_eq!(poll_index, expected.len());
}

#[test]
fn baker_link_static_projection_attaches_with_firmware_sized_slab() {
    let backend = HostQueueBackend::new();
    let clock = CounterClock::new();
    let mut tap = [TapEvent::zero(); 128];
    let mut slab = vec![0u8; 192 * 1024];
    let traffic_policy = BakerTrafficLoopResolver::new();
    let cluster = TestKit::new(&clock);
    let rv = cluster
        .add_rendezvous_from_config(
            Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
            SioTransport::new(&backend),
        )
        .expect("register firmware-sized rendezvous");

    let (kernel_program, engine_program, gpio_program, timer_program) = traffic_light_roles();
    register_baker_traffic_loop_resolver(
        &cluster,
        rv,
        &kernel_program,
        &engine_program,
        &gpio_program,
        &timer_program,
        &traffic_policy,
    );
    let sid = SessionId::new(144);
    let _kernel: Endpoint<'_, 0> = cluster
        .enter(rv, sid, &kernel_program, NoBinding)
        .expect("attach kernel endpoint");
    let _engine: Endpoint<'_, 1> = cluster
        .enter(rv, sid, &engine_program, NoBinding)
        .expect("attach engine endpoint");
    let _gpio: Endpoint<'_, 2> = cluster
        .enter(rv, sid, &gpio_program, NoBinding)
        .expect("attach gpio endpoint");
    let _timer: Endpoint<'_, 3> = cluster
        .enter(rv, sid, &timer_program, NoBinding)
        .expect("attach timer endpoint");
}

#[test]
fn baker_link_single_runtime_drives_first_fd_write_like_firmware() {
    run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock = CounterClock::new();
        let mut tap = [TapEvent::zero(); 128];
        let mut slab = vec![0u8; 192 * 1024];
        let traffic_policy = BakerTrafficLoopResolver::new();
        let cluster = TestKit::new(&clock);
        let rv = cluster
            .add_rendezvous_from_config(
                Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register firmware-sized rendezvous");

        let (kernel_program, engine_program, gpio_program, _timer_program) = traffic_light_roles();
        register_baker_traffic_loop_resolver(
            &cluster,
            rv,
            &kernel_program,
            &engine_program,
            &gpio_program,
            &_timer_program,
            &traffic_policy,
        );
        let sid = SessionId::new(145);
        let mut kernel: Endpoint<'_, 0> = cluster
            .enter(rv, sid, &kernel_program, NoBinding)
            .expect("attach kernel endpoint");
        let mut engine: Endpoint<'_, 1> = cluster
            .enter(rv, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");
        let mut gpio: Endpoint<'_, 2> = cluster
            .enter(rv, sid, &gpio_program, NoBinding)
            .expect("attach gpio endpoint");

        let mut ledger = baker_ledger();
        let mut pins: GpioStateTable<32> = GpioStateTable::new();

        exchange_app_activation_start(&mut kernel, &mut engine, 0, 0).await;
        exchange_traffic_loop_continue(&mut engine).await;
        exchange_baker_led_write(
            &mut kernel,
            &mut engine,
            &mut gpio,
            &mut ledger,
            &mut pins,
            BAKER_LINK_LED_FD,
            b"1",
        )
        .await;
        assert_eq!(
            pins.level(BAKER_LINK_LED_PIN),
            Ok(BAKER_LINK_LED_ACTIVE_HIGH)
        );
    });
}

#[test]
fn baker_link_kernel_recv_can_wait_before_engine_sends() {
    run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock = CounterClock::new();
        let mut tap = [TapEvent::zero(); 128];
        let mut slab = vec![0u8; 192 * 1024];
        let traffic_policy = BakerTrafficLoopResolver::new();
        let cluster = TestKit::new(&clock);
        let rv = cluster
            .add_rendezvous_from_config(
                Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register firmware-sized rendezvous");

        let (kernel_program, engine_program, _gpio_program, _timer_program) = traffic_light_roles();
        register_baker_traffic_loop_resolver(
            &cluster,
            rv,
            &kernel_program,
            &engine_program,
            &_gpio_program,
            &_timer_program,
            &traffic_policy,
        );
        let sid = SessionId::new(146);
        let mut kernel: Endpoint<'_, 0> = cluster
            .enter(rv, sid, &kernel_program, NoBinding)
            .expect("attach kernel endpoint");
        let mut engine: Endpoint<'_, 1> = cluster
            .enter(rv, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        exchange_app_activation_start(&mut kernel, &mut engine, 0, 0).await;
        let mut pending_recv = kernel.offer();
        assert!(
            matches!(poll_once(&mut pending_recv), Poll::Pending),
            "kernel offer must wait for Engine loop control instead of inventing body progress"
        );
    });
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_bad_order_wasip1_poll_oneoff_is_rejected_before_fd_write_phase() {
    run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock = CounterClock::new();
        let mut tap = [TapEvent::zero(); 128];
        let mut slab = vec![0u8; 192 * 1024];
        let traffic_policy = BakerTrafficLoopResolver::new();
        let cluster = TestKit::new(&clock);
        let rv = cluster
            .add_rendezvous_from_config(
                Config::new(&mut tap, slab.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register firmware-sized rendezvous");

        let (kernel_program, engine_program, _gpio_program, _timer_program) = traffic_light_roles();
        register_baker_traffic_loop_resolver(
            &cluster,
            rv,
            &kernel_program,
            &engine_program,
            &_gpio_program,
            &_timer_program,
            &traffic_policy,
        );
        let sid = SessionId::new(147);
        let mut kernel: Endpoint<'_, 0> = cluster
            .enter(rv, sid, &kernel_program, NoBinding)
            .expect("attach kernel endpoint");
        let mut engine: Endpoint<'_, 1> = cluster
            .enter(rv, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        exchange_app_activation_start(&mut kernel, &mut engine, 0, 0).await;
        let bad_guest = wasip1_artifact("wasip1-led-bad-order");
        let mut guest =
            Guest::new(&bad_guest).expect("instantiate bad-order core wasip1 traffic guest");

        let Event::Call(Call::PollOneoff(poll)) =
            guest.next_event().expect("bad guest reaches first syscall")
        else {
            panic!("bad-order guest must issue poll_oneoff before fd_write");
        };
        assert_eq!(poll.delay_ticks(), Ok(50));
        assert!(
            engine
                .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
                .is_err(),
            "Engine localside must reject poll_oneoff because choreography is still at fd_write/memory-borrow phase"
        );

        let mut pending_recv = kernel.offer();
        assert!(
            matches!(poll_once(&mut pending_recv), Poll::Pending),
            "bad poll_oneoff must not create hidden progress on the Kernel fd_write phase"
        );
    });
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_invalid_fd_wasip1_app_is_rejected_by_fd_object_without_gpio_progress() {
    run_large_stack_test(|| {
        run_current_task(async {
            run_baker_wasip1_fd_object_reject(
                "wasip1-led-invalid-fd",
                6,
                b"1",
                GpioFdWriteError::BadFd,
                149,
            )
            .await;
        });
    });
}

#[test]
#[cfg(feature = "profile-rp2040-pico-min")]
fn baker_link_bad_payload_wasip1_app_is_rejected_by_fd_object_without_gpio_progress() {
    run_large_stack_test(|| {
        run_current_task(async {
            run_baker_wasip1_fd_object_reject(
                "wasip1-led-bad-payload",
                3,
                b"2",
                GpioFdWriteError::BadPayload,
                150,
            )
            .await;
        });
    });
}
