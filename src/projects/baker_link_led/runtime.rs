#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::hardware::*;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::stages::*;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::status;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::status::{
    clear_core1_started, clear_runtime_ready, core1_started, mark_core1_started,
    mark_runtime_ready, mark_stage, record_failure_stage, runtime_ready,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{arch::naked_asm, cell::UnsafeCell, mem::MaybeUninit};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana::substrate::{
    cap::{
        ResourceKind,
        advanced::{LoopBreakKind, LoopContinueKind},
    },
    policy::{
        LoopResolution, ResolverContext, ResolverError, ResolverRef, signals::core as policy_core,
    },
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::{
    Endpoint,
    g::Msg,
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-bad-order-demo")
))]
use hibana_pico::choreography::protocol::PollOneoff;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::choreography::protocol::{
    ChoreoFsOpenAdmitRoute, ChoreoFsOpenAdmitRouteMsg, ChoreoFsOpenRejectRoute,
    ChoreoFsOpenRejectRouteMsg, LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, PathOpen,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::projects::baker_link_led::choreography::abort_safe_linear_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-choreofs-bad-path-demo"
))]
use hibana_pico::projects::baker_link_led::choreography::choreofs_bad_path_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::projects::baker_link_led::choreography::choreofs_traffic_light_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    not(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))
))]
use hibana_pico::projects::baker_link_led::choreography::traffic_light_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::projects::baker_link_led::choreography::{
    BakerTrafficLoopBreakControl, BakerTrafficLoopContinueControl, POLICY_BAKER_TRAFFIC_LOOP,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::{
    choreography::protocol::{
        BudgetRun, BudgetRunMsg, EngineReq, EngineRet, FdWrite, FdWriteDone, LABEL_MEM_BORROW_READ,
        LABEL_MEM_RELEASE, LABEL_TIMER_SLEEP_DONE, LABEL_TIMER_SLEEP_UNTIL, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASI_PROC_EXIT, MemReadGrantControl, MemRights, PollReady, ProcExitStatus,
        TimerSleepUntil, WASIP1_STREAM_CHUNK_CAPACITY,
    },
    kernel::{
        fd_object::check_gpio_object_fd_write,
        guest_ledger::{GuestLedger, WASI_ERRNO_SUCCESS},
        resolver::{InterruptEvent, PicoInterruptResolver, ResolvedInterrupt},
    },
    projects::baker_link_led::manifest::{
        BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS, baker_link_led_fd_write_route,
    },
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::{
    choreography::protocol::{
        EngineLabelUniverse, GpioSet, LABEL_GPIO_SET, LABEL_GPIO_SET_DONE, MemBorrow, MemRelease,
        TimerSleepDone,
    },
    machine::rp2040::sio::{Rp2040SioBackend, core_id, fifo_drain, launch_core1},
    machine::rp2040::{clock, timer, uart},
    port::exec::{park, run_current_task, signal, wait_until},
    port::transport::SioTransport,
    projects::baker_link_led::manifest::{BAKER_LINK_LED_ACTIVE_HIGH, BAKER_LINK_LED_PINS},
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::{
    kernel::engine::wasm::{Call, Event, Guest},
    projects::baker_link_led::guest::write_selected_guest_in_place,
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::kernel::engine::wasm::PathKind;

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::{
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason,
    },
    machine::rp2040::baker_link::BAKER_LINK_SAFE_GPIO_LEVELS,
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::{
    choreography::protocol::PathOpened,
    projects::baker_link_led::ledger::{
        baker_link_choreofs_ledger, mint_baker_link_choreofs_fd, resolve_baker_link_choreofs_path,
    },
    projects::baker_link_led::manifest::{
        BAKER_LINK_CHOREOFS_PREOPEN_FD, BakerLinkLedResourceStore, baker_link_led_resource_store,
    },
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".boot2")]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
    static __data_load_start: u8;
    static mut __data_start: u8;
    static mut __data_end: u8;
    static mut __bss_start: u8;
    static mut __bss_end: u8;
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
const SLAB_BYTES: usize = 200 * 1024;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
const SLAB_BYTES: usize = 124 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TEST_MEMORY_LEN: u32 = 64 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TEST_MEMORY_EPOCH: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TEST_LED_PTR: u32 = 128;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
const BAKER_LINK_WASM_FUEL_PER_ACTIVATION: u32 = 250_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoTransport = SioTransport<Rp2040SioBackend>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoKit = SessionKit<'static, DemoTransport, EngineLabelUniverse, CounterClock, 4>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type KernelEndpoint = Endpoint<'static, 0>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type EngineEndpoint = Endpoint<'static, 1>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type GpioEndpoint = Endpoint<'static, 2>;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
type TimerEndpoint = Endpoint<'static, 3>;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
#[cfg(any(
    feature = "baker-choreofs-demo",
    feature = "baker-choreofs-bad-path-demo",
    feature = "baker-choreofs-bad-payload-demo",
    feature = "baker-choreofs-wrong-object-demo"
))]
type BakerLedger = GuestLedger<4, 1, 1>;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    not(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))
))]
type BakerLedger = GuestLedger<3, 1, 1>;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct VectorTable {
    initial_stack_pointer: *const u32,
    reset: unsafe extern "C" fn() -> !,
    exceptions: [timer::IrqHandler; 14],
    timer_irq0: timer::IrqHandler,
    external_irqs: [timer::IrqHandler; 31],
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".vector_table.reset_vector")]
#[used]
static VECTOR_TABLE: VectorTable = VectorTable {
    initial_stack_pointer: core::ptr::addr_of!(__stack_top) as *const u32,
    reset,
    exceptions: [timer::default_irq_handler; 14],
    timer_irq0: timer::timer0_irq_handler,
    external_irqs: [timer::default_irq_handler; 31],
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntime {
    clock: CounterClock,
    tap: [TapEvent; 128],
    slab: [u8; SLAB_BYTES],
    session: MaybeUninit<DemoKit>,
    core0_endpoint: MaybeUninit<KernelEndpoint>,
    core1_endpoint: MaybeUninit<EngineEndpoint>,
    core2_endpoint: MaybeUninit<GpioEndpoint>,
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    core3_endpoint: MaybeUninit<TimerEndpoint>,
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    core1_guest: MaybeUninit<Guest<'static>>,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl SharedRuntime {
    const fn new() -> Self {
        Self {
            clock: CounterClock::new(),
            tap: [TapEvent::zero(); 128],
            slab: [0; SLAB_BYTES],
            session: MaybeUninit::uninit(),
            core0_endpoint: MaybeUninit::uninit(),
            core1_endpoint: MaybeUninit::uninit(),
            core2_endpoint: MaybeUninit::uninit(),
            #[cfg(not(feature = "baker-abort-safe-demo"))]
            core3_endpoint: MaybeUninit::uninit(),
            #[cfg(not(feature = "baker-abort-safe-demo"))]
            core1_guest: MaybeUninit::uninit(),
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntimeCell(UnsafeCell<SharedRuntime>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for SharedRuntimeCell {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static SHARED_RUNTIME: SharedRuntimeCell = SharedRuntimeCell(UnsafeCell::new(SharedRuntime::new()));

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
fn baker_traffic_loop_resolver(ctx: ResolverContext) -> Result<LoopResolution, ResolverError> {
    let Some(tag) = ctx.attr(policy_core::TAG).map(|value| value.as_u8()) else {
        return Err(ResolverError::Reject);
    };
    match tag {
        LoopContinueKind::TAG => Ok(LoopResolution::Continue),
        LoopBreakKind::TAG => Ok(LoopResolution::Break),
        _ => Err(ResolverError::Reject),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(naked)]
#[unsafe(export_name = "Reset")]
pub unsafe extern "C" fn reset() -> ! {
    naked_asm!(
        "ldr r0, =0xD0000000",
        "ldr r0, [r0]",
        "ldr r2, ={entry}",
        "cmp r0, #0",
        "beq 1f",
        "ldr r1, ={core1_stack_top}",
        "mov sp, r1",
        "bx r2",
        "1:",
        "bx r2",
        core1_stack_top = sym __core1_stack_top,
        entry = sym reset_entry,
    )
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn reset_entry() -> ! {
    match core_id() {
        0 => {
            init_ram();
            core0_main()
        }
        _ => core1_main(),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn core1_launch_entry() -> ! {
    core1_main()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_ram() {
    unsafe {
        let data_src = core::ptr::addr_of!(__data_load_start);
        let data_start = core::ptr::addr_of_mut!(__data_start);
        let data_end = core::ptr::addr_of_mut!(__data_end);
        let data_len = data_end as usize - data_start as usize;
        core::ptr::copy_nonoverlapping(data_src, data_start, data_len);

        let bss_start = core::ptr::addr_of_mut!(__bss_start);
        let bss_end = core::ptr::addr_of_mut!(__bss_end);
        let bss_len = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, bss_len);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn shared_runtime_ptr() -> *mut SharedRuntime {
    SHARED_RUNTIME.0.get()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_kernel_endpoint() -> &'static mut KernelEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core0_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_engine_endpoint() -> &'static mut EngineEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core1_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_gpio_endpoint() -> &'static mut GpioEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core2_endpoint.as_mut_ptr() }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
unsafe fn shared_timer_endpoint() -> &'static mut TimerEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core3_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn ensure_core1_launched() {
    clear_core1_started();
    clear_runtime_ready();
    let launched = launch_core1(
        core::ptr::addr_of!(VECTOR_TABLE) as u32,
        core::ptr::addr_of!(__core1_stack_top) as u32,
        core1_launch_entry as *const () as usize as u32,
    );
    if !launched {
        record_failure_stage(STAGE_CORE1_LAUNCH_ERR);
        panic!();
    }
    for _ in 0..100_000 {
        if core1_started() {
            return;
        }
        core::hint::spin_loop();
    }
    record_failure_stage(STAGE_CORE1_START_TIMEOUT);
    panic!();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_runtime_once() {
    mark_stage(STAGE_RUNTIME_BEGIN);
    let runtime = shared_runtime_ptr();
    unsafe {
        let session_ptr = (*runtime).session.as_mut_ptr();
        session_ptr.write(SessionKit::new(&(*runtime).clock));
        let kit = &*session_ptr;
        let rv = match kit.add_rendezvous_from_config(
            Config::new(&mut (*runtime).tap, &mut (*runtime).slab)
                .with_universe(EngineLabelUniverse),
            SioTransport::new(Rp2040SioBackend::new()),
        ) {
            Ok(rv) => rv,
            Err(_) => panic!(),
        };
        mark_stage(STAGE_RENDEZVOUS_READY);
        let sid = SessionId::new(10);
        #[cfg(feature = "baker-abort-safe-demo")]
        {
            let (core0_program, core1_program, core2_program) = abort_safe_linear_roles();
            mark_stage(STAGE_PROGRAM_READY);
            (*runtime).core0_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core0_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_KERNEL_ATTACHED);
            (*runtime).core1_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core1_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_ENGINE_ATTACHED);
            (*runtime).core2_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core2_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_GPIO_ATTACHED);
            mark_runtime_ready();
        }
        #[cfg(not(feature = "baker-abort-safe-demo"))]
        {
            #[cfg(any(
                feature = "baker-choreofs-demo",
                feature = "baker-choreofs-bad-payload-demo",
                feature = "baker-choreofs-wrong-object-demo"
            ))]
            let (core0_program, core1_program, core2_program, core3_program) =
                choreofs_traffic_light_roles();
            #[cfg(feature = "baker-choreofs-bad-path-demo")]
            let (core0_program, core1_program, core2_program, core3_program) =
                choreofs_bad_path_roles();
            #[cfg(not(any(
                feature = "baker-choreofs-demo",
                feature = "baker-choreofs-bad-path-demo",
                feature = "baker-choreofs-bad-payload-demo",
                feature = "baker-choreofs-wrong-object-demo"
            )))]
            let (core0_program, core1_program, core2_program, core3_program) =
                traffic_light_roles();
            mark_stage(STAGE_PROGRAM_READY);
            let traffic_loop_resolver = ResolverRef::loop_fn(baker_traffic_loop_resolver);
            kit.set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 0>(
                rv,
                &core0_program,
                traffic_loop_resolver,
            )
            .unwrap_or_else(|_| panic!());
            kit.set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 1>(
                rv,
                &core1_program,
                traffic_loop_resolver,
            )
            .unwrap_or_else(|_| panic!());
            kit.set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 2>(
                rv,
                &core2_program,
                traffic_loop_resolver,
            )
            .unwrap_or_else(|_| panic!());
            kit.set_resolver::<POLICY_BAKER_TRAFFIC_LOOP, 3>(
                rv,
                &core3_program,
                traffic_loop_resolver,
            )
            .unwrap_or_else(|_| panic!());
            (*runtime).core0_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core0_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_KERNEL_ATTACHED);
            (*runtime).core1_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core1_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_ENGINE_ATTACHED);
            (*runtime).core2_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core2_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_stage(STAGE_GPIO_ATTACHED);
            (*runtime).core3_endpoint.as_mut_ptr().write(
                kit.enter(rv, sid, &core3_program, NoBinding)
                    .unwrap_or_else(|_| panic!()),
            );
            mark_runtime_ready();
        }
    }
    signal();
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn gpio_device_recv_set_payload_once(endpoint: &mut GpioEndpoint) -> GpioSet {
    let branch = endpoint.offer().await.unwrap_or_else(|_| {
        record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
        panic!()
    });
    if branch.label() != LABEL_GPIO_SET {
        record_failure_stage(STAGE_GPIO_SET_LABEL_ERR);
        panic!();
    }
    let set = match branch.decode::<Msg<LABEL_GPIO_SET, GpioSet>>().await {
        Ok(set) => set,
        Err(_) => {
            record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
            panic!()
        }
    };
    rp2040_gpio_apply_baker_led_set(set);
    set
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn gpio_device_send_set_done_once(endpoint: &mut GpioEndpoint, set: GpioSet) {
    let flow = endpoint
        .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_GPIO_SET_DONE_SEND_ERR);
            panic!()
        });
    match flow.send(&set).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_GPIO_SET_DONE_SEND_ERR);
            panic!()
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn gpio_device_recv_set_once(endpoint: &mut GpioEndpoint) {
    let set = gpio_device_recv_set_payload_once(endpoint).await;
    gpio_device_send_set_done_once(endpoint, set).await;
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
async fn gpio_device_recv_linear_set_once(endpoint: &mut GpioEndpoint) {
    let set = endpoint
        .recv::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .await
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
            panic!()
        });
    rp2040_gpio_apply_baker_led_set(set);
    gpio_device_send_set_done_once(endpoint, set).await;
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn timer_device_recv_sleep_once(
    endpoint: &mut TimerEndpoint,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    delay_ticks: u32,
) {
    let branch = match endpoint.offer().await {
        Ok(branch) => branch,
        Err(_) => {
            record_failure_stage(STAGE_TIMER_SLEEP_RECV);
            panic!()
        }
    };
    if branch.label() != LABEL_TIMER_SLEEP_UNTIL {
        record_failure_stage(STAGE_TIMER_SLEEP_RECV);
        panic!();
    }
    let sleep = match branch
        .decode::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .await
    {
        Ok(sleep) => sleep,
        Err(_) => {
            record_failure_stage(STAGE_TIMER_SLEEP_RECV);
            panic!()
        }
    };
    mark_stage(STAGE_TIMER_SLEEP_RECV);

    resolver
        .request_timer_sleep(sleep)
        .unwrap_or_else(|_| panic!());
    resolver
        .push_irq(InterruptEvent::TimerTick {
            tick: sleep.tick().saturating_sub(1),
        })
        .unwrap_or_else(|_| panic!());
    if resolver
        .resolve_next()
        .unwrap_or_else(|_| panic!())
        .is_some()
    {
        panic!();
    }

    timer::arm_alarm0_after_ticks(delay_ticks);
    mark_stage(STAGE_TIMER_ALARM_ARMED);
    wait_until(timer::alarm0_ready);
    mark_stage(STAGE_TIMER_RAW_READY);
    let Some(_ready) = timer::take_alarm0_ready() else {
        panic!();
    };
    resolver
        .push_irq(InterruptEvent::TimerTick { tick: sleep.tick() })
        .unwrap_or_else(|_| panic!());
    let Some(ResolvedInterrupt::TimerSleepDone(done)) =
        resolver.resolve_next().unwrap_or_else(|_| panic!())
    else {
        panic!();
    };
    if done.tick() != sleep.tick() {
        panic!();
    }

    match endpoint
        .flow::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>()
        .expect("timer flow<sleep done>")
        .send(&done)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_TIMER_DONE_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_send_gpio_set(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    set: GpioSet,
) {
    match endpoint
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<gpio set>")
        .send(&set)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    gpio_device_recv_set_once(gpio_endpoint).await;
    let done = match endpoint.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>().await {
        Ok(done) => done,
        Err(_) => panic!(),
    };
    if done != set {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
async fn kernel_send_gpio_set_remote(endpoint: &mut KernelEndpoint, set: GpioSet) {
    endpoint
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<abort safe gpio set>")
        .send(&set)
        .await
        .unwrap_or_else(|_| panic!());
    let done = endpoint
        .recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .await
        .unwrap_or_else(|_| panic!());
    if done != set {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_fd_write(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    ledger: &mut BakerLedger,
    borrow: MemBorrow,
) {
    mark_stage(STAGE_KERNEL_FD_WRITE_BEGIN);
    mark_stage(STAGE_KERNEL_FD_WRITE_BORROW_RECV);
    if borrow.ptr() != TEST_LED_PTR
        || borrow.len() == 0
        || borrow.len() as usize > WASIP1_STREAM_CHUNK_CAPACITY
        || borrow.epoch() != TEST_MEMORY_EPOCH
    {
        panic!();
    }
    let grant = ledger.grant_read_lease(borrow).unwrap_or_else(|_| panic!());
    match endpoint
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<led grant>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_FD_WRITE_GRANT_SENT);

    let request = match endpoint.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>().await {
        Ok(request) => request,
        Err(_) => {
            record_failure_stage(STAGE_KERNEL_FD_WRITE_REQ_RECV_ERR);
            panic!()
        }
    };
    mark_stage(STAGE_KERNEL_FD_WRITE_REQ_RECV);
    let EngineReq::FdWrite(write) = request else {
        record_failure_stage(STAGE_KERNEL_FD_WRITE_REQ_MISMATCH);
        panic!();
    };
    if ledger.validate_fd_write_lease(&write, grant).is_err() {
        record_failure_stage(STAGE_KERNEL_FD_WRITE_LEASE_ERR);
        panic!();
    }
    let (written, errno) =
        match check_gpio_object_fd_write(ledger.fd_view(), &write, baker_link_led_fd_write_route())
        {
            Ok(set) => {
                kernel_send_gpio_set(endpoint, gpio_endpoint, set).await;
                mark_stage(STAGE_KERNEL_FD_WRITE_GPIO_DONE);
                (write.len() as u8, WASI_ERRNO_SUCCESS)
            }
            Err(error) => {
                let _ = gpio_endpoint;
                let errno = status::gpio_fd_write_errno(error);
                if status::mark_expected_reject_if_recorded() {
                    park();
                }
                (0, errno)
            }
        };

    let reply = EngineRet::FdWriteDone(FdWriteDone::new_with_errno(write.fd(), written, errno));
    match endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("kernel flow<led fd_write ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }

    let release = match endpoint.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>().await {
        Ok(release) => release,
        Err(_) => panic!(),
    };
    if release.lease_id() != grant.lease_id() {
        panic!();
    }
    ledger.release_lease(release).unwrap_or_else(|_| panic!());
    if status::mark_expected_reject_if_recorded() {
        park();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
async fn kernel_path_open(
    endpoint: &mut KernelEndpoint,
    ledger: &mut BakerLedger,
    store: &BakerLinkLedResourceStore,
    borrow: MemBorrow,
) {
    mark_stage(STAGE_KERNEL_PATH_OPEN_BORROW_RECV);
    if borrow.len() == 0 || borrow.epoch() != TEST_MEMORY_EPOCH {
        panic!();
    }
    let grant = ledger.grant_read_lease(borrow).unwrap_or_else(|_| panic!());
    match endpoint
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<path grant>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_PATH_OPEN_GRANT_SENT);

    let request = match endpoint
        .recv::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
        .await
    {
        Ok(request) => request,
        Err(_) => panic!(),
    };
    let EngineReq::PathOpen(open) = request else {
        panic!();
    };
    mark_stage(STAGE_KERNEL_PATH_OPEN_REQ_RECV);
    if open.preopen_fd() != BAKER_LINK_CHOREOFS_PREOPEN_FD
        || open.lease_id() != grant.lease_id()
        || open.len() > borrow.len() as usize
    {
        panic!();
    }

    let (opened, errno) =
        match resolve_baker_link_choreofs_path(store, ledger, open.path(), open.rights_base()) {
            Ok(opened) => {
                mark_stage(STAGE_KERNEL_PATH_OPEN_OBJECT_OPENED);
                (Some(opened), WASI_ERRNO_SUCCESS)
            }
            Err(error) => {
                status::record_choreofs_open_reject(error);
                (None, error.wasi_errno())
            }
        };
    let opened_fd = if let Some(opened) = opened {
        match endpoint
            .flow::<ChoreoFsOpenAdmitRouteMsg>()
            .expect("kernel flow<choreofs open admit route>")
            .send(&ChoreoFsOpenAdmitRoute)
            .await
        {
            Ok(_) => {}
            Err(_) => panic!(),
        }
        match mint_baker_link_choreofs_fd(ledger, opened) {
            Ok(fd) => fd.fd(),
            Err(_) => panic!(),
        }
    } else {
        match endpoint
            .flow::<ChoreoFsOpenRejectRouteMsg>()
            .expect("kernel flow<choreofs open reject route>")
            .send(&ChoreoFsOpenRejectRoute)
            .await
        {
            Ok(_) => {}
            Err(_) => panic!(),
        }
        0
    };
    let rejected = errno != WASI_ERRNO_SUCCESS;
    #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
    let _ = rejected;
    let reply = EngineRet::PathOpened(PathOpened::new(opened_fd, errno));
    match endpoint
        .flow::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
        .expect("kernel flow<path_open ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_PATH_OPEN_RET_SENT);

    let release = match endpoint.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>().await {
        Ok(release) => release,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_KERNEL_PATH_OPEN_RELEASE_RECV);
    if release.lease_id() != grant.lease_id() {
        panic!();
    }
    ledger.release_lease(release).unwrap_or_else(|_| panic!());
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    if rejected {
        record_failure_stage(STAGE_BAD_PATH_REJECTED);
        mark_stage(RESULT_EXPECTED_REJECT);
        park();
    }
    if status::mark_expected_reject_if_recorded() {
        park();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_poll_oneoff(
    endpoint: &mut KernelEndpoint,
    timer_endpoint: &mut TimerEndpoint,
    ledger: &mut BakerLedger,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    last_tick: &mut u64,
) {
    let request = match endpoint
        .recv::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .await
    {
        Ok(request) => request,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_KERNEL_POLL_RECV);
    let EngineReq::PollOneoff(poll) = request else {
        panic!();
    };
    if poll.timeout_tick() < *last_tick {
        panic!();
    }
    let pending_poll = ledger.begin_poll_oneoff(poll).unwrap_or_else(|_| panic!());
    let delta = poll.timeout_tick() - *last_tick;
    if delta > u32::MAX as u64 {
        panic!();
    }
    let delay_ticks = delta as u32;
    *last_tick = poll.timeout_tick();

    let sleep = TimerSleepUntil::new(poll.timeout_tick());
    match endpoint
        .flow::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .expect("kernel flow<timer sleep>")
        .send(&sleep)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_TIMER_SLEEP_SENT);
    timer_device_recv_sleep_once(timer_endpoint, resolver, delay_ticks).await;
    let done = match endpoint
        .recv::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>()
        .await
    {
        Ok(done) => done,
        Err(_) => panic!(),
    };
    if done.tick() != poll.timeout_tick() {
        panic!();
    }
    ledger
        .complete_poll_oneoff(pending_poll, done)
        .unwrap_or_else(|_| panic!());

    let reply = EngineRet::PollReady(PollReady::new(1));
    match endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
        .expect("kernel flow<led poll_oneoff ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_recv_proc_exit(endpoint: &mut KernelEndpoint) {
    let branch = endpoint.offer().await.unwrap_or_else(|_| panic!());
    if branch.label() != LABEL_WASI_PROC_EXIT {
        panic!();
    }
    let request = branch
        .decode::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
        .await
        .unwrap_or_else(|_| panic!());
    let EngineReq::ProcExit(status) = request else {
        panic!();
    };
    if status.code() != 0 {
        panic!();
    }
    mark_stage(STAGE_KERNEL_PROC_EXIT_RECV);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_process_traffic_fd_write(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    ledger: &mut BakerLedger,
) {
    let branch = endpoint.offer().await.unwrap_or_else(|_| {
        record_failure_stage(STAGE_KERNEL_TRAFFIC_OFFER_ERR);
        panic!()
    });
    match branch.label() {
        LABEL_MEM_BORROW_READ => {
            let borrow = {
                let decoded = branch
                    .decode::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
                    .await
                    .unwrap_or_else(|_| {
                        record_failure_stage(STAGE_KERNEL_TRAFFIC_MEM_RECV_ERR);
                        panic!()
                    });
                MemBorrow::new(decoded.ptr(), decoded.len(), decoded.epoch())
            };
            if borrow.ptr() != TEST_LED_PTR
                || borrow.len() == 0
                || borrow.len() as usize > WASIP1_STREAM_CHUNK_CAPACITY
                || borrow.epoch() != TEST_MEMORY_EPOCH
            {
                record_failure_stage(STAGE_KERNEL_TRAFFIC_MEM_MISMATCH);
                panic!();
            }
            kernel_fd_write(endpoint, gpio_endpoint, ledger, borrow).await;
        }
        _ => {
            record_failure_stage(STAGE_KERNEL_TRAFFIC_OFFER_ERR);
            panic!()
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_session(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    timer_endpoint: &mut TimerEndpoint,
) {
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    let store = baker_link_led_resource_store().unwrap_or_else(|_| panic!());
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    let mut ledger =
        baker_link_choreofs_ledger::<4, 1, 1>(&store, TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
            .unwrap_or_else(|_| panic!());
    #[cfg(not(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )))]
    let mut ledger = hibana_pico::projects::baker_link_led::ledger::baker_link_pico_min_ledger::<
        1,
        1,
    >(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
    .unwrap_or_else(|_| panic!());
    let mut resolver: PicoInterruptResolver<2, 4, 1> = PicoInterruptResolver::new();

    let activation_id = 0u16;
    kernel_start_app_activation(endpoint, activation_id, 0).await;
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    {
        #[cfg(feature = "baker-choreofs-bad-path-demo")]
        let path_open_count = 1usize;
        #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
        let path_open_count = 3usize;
        for _ in 0..path_open_count {
            let borrow = endpoint
                .recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
                .await
                .unwrap_or_else(|_| panic!());
            kernel_path_open(endpoint, &mut ledger, &store, borrow).await;
        }
    }
    let mut tick = 0u64;
    for step in 0..BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS {
        kernel_process_traffic_fd_write(endpoint, gpio_endpoint, &mut ledger).await;
        if step == 0 {
            mark_stage(STAGE_FIRST_LED_WRITE_DONE);
        }
        kernel_poll_oneoff(
            endpoint,
            timer_endpoint,
            &mut ledger,
            &mut resolver,
            &mut tick,
        )
        .await;
        if step == 0 {
            mark_stage(STAGE_POLL_ON_DONE);
        }
        mark_stage(STAGE_FINAL_LED_WRITE_DONE);
    }
    kernel_recv_proc_exit(endpoint).await;
    mark_stage(RESULT_SUCCESS);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
async fn kernel_abort_safe_session(endpoint: &mut KernelEndpoint) {
    let mut ledger = hibana_pico::projects::baker_link_led::ledger::baker_link_pico_min_ledger::<
        1,
        1,
    >(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
    .unwrap_or_else(|_| panic!());
    let grant = ledger
        .grant_read_lease(MemBorrow::new(TEST_LED_PTR, 1, TEST_MEMORY_EPOCH))
        .unwrap_or_else(|_| panic!());
    let pending = ledger
        .begin_poll_oneoff(PollOneoff::new(50))
        .unwrap_or_else(|_| panic!());
    if ledger.fd_view().active_count() != 3
        || ledger.lease_table().outstanding_lease_count() != 1
        || ledger.pending_table().pending_count() != 1
    {
        panic!();
    }

    let abort = endpoint
        .recv::<EngineAbortMsg>()
        .await
        .unwrap_or_else(|_| panic!());
    if abort.reason() != EngineAbortReason::GuestTrap || abort.code() != 1 {
        panic!();
    }

    endpoint
        .recv::<EngineAbortBeginControl>()
        .await
        .unwrap_or_else(|_| panic!());

    ledger.apply_abort_fence(TEST_MEMORY_EPOCH + 1);
    if ledger.fd_view().active_count() != 0
        || ledger.lease_table().outstanding_lease_count() != 0
        || ledger.pending_table().pending_count() != 0
        || ledger
            .release_lease(MemRelease::new(grant.lease_id()))
            .is_ok()
        || ledger
            .complete_poll_oneoff(pending, TimerSleepDone::new(50))
            .is_ok()
    {
        panic!();
    }
    mark_stage(STAGE_KERNEL_ABORT_FENCE_APPLIED);

    endpoint
        .flow::<EngineAbortFenceControl>()
        .expect("kernel flow<abort fence>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_KERNEL_ABORT_FENCE_SENT);

    for safe in BAKER_LINK_SAFE_GPIO_LEVELS {
        mark_stage(STAGE_KERNEL_ABORT_SAFE_GPIO_BEGIN);
        kernel_send_gpio_set_remote(endpoint, GpioSet::new(safe.pin(), safe.high())).await;
    }
    mark_stage(STAGE_KERNEL_ABORT_SAFE_GPIO_DONE);

    endpoint
        .flow::<EngineAbortAckControl>()
        .expect("kernel flow<abort ack>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_start_app_activation(endpoint: &mut KernelEndpoint, activation_id: u16, tick: u64) {
    mark_stage(STAGE_KERNEL_RUN_SEND_BEGIN);
    let run = BudgetRun::new(activation_id, 1, BAKER_LINK_WASM_FUEL_PER_ACTIVATION, tick);
    let flow = endpoint.flow::<BudgetRunMsg>().unwrap_or_else(|_| {
        record_failure_stage(STAGE_KERNEL_RUN_FLOW_ERR);
        panic!()
    });
    match flow.send(&run).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_KERNEL_RUN_SEND_ERR);
            panic!()
        }
    }
    mark_stage(STAGE_KERNEL_RUN_SEND_DONE);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_fd_write(endpoint: &mut EngineEndpoint, fd: u8, payload: &[u8]) -> u16 {
    mark_stage(STAGE_ENGINE_FD_WRITE_BEGIN);
    let borrow = MemBorrow::new(TEST_LED_PTR, payload.len() as u8, TEST_MEMORY_EPOCH);
    let flow = endpoint
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_ENGINE_BORROW_FLOW_ERR);
            panic!()
        });
    match flow.send(&borrow).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_BORROW_SEND_ERR);
            panic!();
        }
    }
    mark_stage(STAGE_ENGINE_FD_WRITE_BORROW_SENT);

    let grant = match endpoint.recv::<MemReadGrantControl>().await {
        Ok(grant) => grant,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_GRANT_RECV_ERR);
            panic!()
        }
    };
    let (rights, lease_id) = grant.decode_handle().unwrap_or_else(|_| {
        record_failure_stage(STAGE_ENGINE_GRANT_DECODE_ERR);
        panic!()
    });
    if rights != MemRights::Read.tag() || lease_id > u8::MAX as u64 {
        record_failure_stage(STAGE_ENGINE_GRANT_MISMATCH);
        panic!();
    }

    let write = FdWrite::new_with_lease(fd, lease_id as u8, payload).unwrap_or_else(|_| panic!());
    let request = EngineReq::FdWrite(write);
    let flow = endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_REQ_FLOW_ERR);
            panic!()
        });
    match flow.send(&request).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_REQ_SEND_ERR);
            panic!()
        }
    }

    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_RECV_ERR);
            panic!()
        }
    };
    let EngineRet::FdWriteDone(done) = reply else {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    };
    if done.fd() != fd {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }
    if done.errno() == WASI_ERRNO_SUCCESS && done.written() != payload.len() as u8 {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }
    if done.errno() != WASI_ERRNO_SUCCESS && done.written() != 0 {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }

    let release = MemRelease::new(lease_id as u8);
    match endpoint
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<led release>")
        .send(&release)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    done.errno()
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    not(feature = "baker-bad-order-demo")
))]
async fn engine_poll_oneoff(endpoint: &mut EngineEndpoint, tick: u64) {
    let request = EngineReq::PollOneoff(PollOneoff::new(tick));
    match endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .expect("engine flow<led poll_oneoff>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }

    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => panic!(),
    };
    let EngineRet::PollReady(ready) = reply else {
        panic!();
    };
    if ready.ready() != 1 {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    feature = "baker-bad-order-demo"
))]
async fn engine_expect_poll_oneoff_rejected(endpoint: &mut EngineEndpoint) {
    if endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .is_ok()
    {
        panic!();
    }
    record_failure_stage(STAGE_BAD_ORDER_POLL_REJECTED);
    mark_stage(RESULT_EXPECTED_REJECT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
async fn engine_path_open(
    endpoint: &mut EngineEndpoint,
    call: hibana_pico::kernel::engine::wasm::Pending<
        '_,
        '_,
        hibana_pico::kernel::engine::wasm::Path,
    >,
) {
    mark_stage(STAGE_ENGINE_PATH_OPEN_BEGIN);
    if call.kind() != PathKind::PathOpen {
        panic!();
    }
    let ptr = call.arg_i32(2).unwrap_or_else(|_| panic!());
    let len = call.arg_i32(3).unwrap_or_else(|_| panic!());
    if len > u8::MAX as u32 {
        panic!();
    }
    let preopen_fd = call.fd().unwrap_or_else(|_| panic!());
    let rights_base = call.arg_i64(5).unwrap_or_else(|_| panic!());

    let borrow = MemBorrow::new(ptr, len as u8, TEST_MEMORY_EPOCH);
    match endpoint
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<path borrow>")
        .send(&borrow)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_BORROW_SENT);
    let grant = match endpoint.recv::<MemReadGrantControl>().await {
        Ok(grant) => grant,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_ENGINE_PATH_OPEN_GRANT_RECV);
    let (rights, lease_id) = grant.decode_handle().unwrap_or_else(|_| panic!());
    if rights != MemRights::Read.tag() || lease_id > u8::MAX as u64 {
        panic!();
    }

    let path = call.path_bytes().unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_PATH_OPEN_PATH_DECODED);
    let request = EngineReq::PathOpen(
        PathOpen::new(preopen_fd, lease_id as u8, rights_base, path.as_bytes())
            .unwrap_or_else(|_| panic!()),
    );
    match endpoint
        .flow::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
        .expect("engine flow<path_open>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_REQ_SENT);
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    {
        if endpoint.recv::<ChoreoFsOpenRejectRouteMsg>().await.is_err() {
            panic!();
        }
    }
    #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
    {
        if endpoint.recv::<ChoreoFsOpenAdmitRouteMsg>().await.is_err() {
            panic!();
        }
    }
    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => panic!(),
    };
    let EngineRet::PathOpened(opened) = reply else {
        panic!();
    };
    mark_stage(STAGE_ENGINE_PATH_OPEN_RET_RECV);

    let release = MemRelease::new(lease_id as u8);
    match endpoint
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<path release>")
        .send(&release)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_RELEASE_SENT);

    call.complete_path_open(opened.fd() as u32, opened.errno() as u32)
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_PATH_OPEN_COMPLETED);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_session(endpoint: &mut EngineEndpoint, guest: &mut Guest<'static>) {
    #[cfg(not(feature = "baker-bad-order-demo"))]
    let mut tick = 0u64;
    let run = engine_recv_traffic_run(endpoint, 0).await;
    loop {
        let event = match guest.resume(run) {
            Ok(event) => event,
            Err(error) => {
                let _ = error;
                if status::mark_expected_reject_if_recorded() {
                    break;
                }
                record_failure_stage(STAGE_ENGINE_RESUME_ERR_TRAP);
                panic!();
            }
        };
        match event {
            Event::Call(Call::FdWrite(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_FD_WRITE);
                engine_continue_traffic_loop(endpoint).await;
                let payload = call.payload().unwrap_or_else(|_| panic!());
                let errno = engine_fd_write(endpoint, call.fd(), payload.as_bytes()).await;
                if call.complete(errno as u32).is_err() {
                    if status::mark_expected_reject_if_recorded() {
                        break;
                    }
                    panic!();
                }
            }
            Event::Call(Call::PollOneoff(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_POLL_ONEOFF);
                let delay_ticks = call.delay_ticks().unwrap_or_else(|_| panic!());
                #[cfg(feature = "baker-bad-order-demo")]
                {
                    if delay_ticks != 50 {
                        panic!();
                    }
                    engine_expect_poll_oneoff_rejected(endpoint).await;
                    break;
                }
                #[cfg(not(feature = "baker-bad-order-demo"))]
                {
                    tick = tick.saturating_add(delay_ticks);
                    engine_poll_oneoff(endpoint, tick).await;
                    call.complete(1, 0).unwrap_or_else(|_| panic!());
                }
            }
            Event::Call(
                Call::FdRead(_)
                | Call::FdFdstatGet(_)
                | Call::FdClose(_)
                | Call::ClockResGet(_)
                | Call::ClockTimeGet(_)
                | Call::RandomGet(_)
                | Call::SchedYield(_)
                | Call::Socket(_)
                | Call::ProcRaise(_),
            ) => {
                mark_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                record_failure_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                panic!();
            }
            Event::Call(Call::Path(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_PATH_OPEN);
                #[cfg(any(
                    feature = "baker-choreofs-demo",
                    feature = "baker-choreofs-bad-path-demo",
                    feature = "baker-choreofs-bad-payload-demo",
                    feature = "baker-choreofs-wrong-object-demo"
                ))]
                {
                    engine_path_open(endpoint, call).await;
                }
                #[cfg(not(any(
                    feature = "baker-choreofs-demo",
                    feature = "baker-choreofs-bad-path-demo",
                    feature = "baker-choreofs-bad-payload-demo",
                    feature = "baker-choreofs-wrong-object-demo"
                )))]
                {
                    let _ = call;
                    record_failure_stage(STAGE_ENGINE_TRAP_PATH_OPEN);
                    panic!();
                }
            }
            Event::Call(Call::ArgsSizesGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ARGS_SIZES);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ARGS_SIZES);
                panic!();
            }
            Event::Call(Call::ArgsGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ARGS_GET);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ARGS_GET);
                panic!();
            }
            Event::Call(Call::EnvironSizesGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ENVIRON_SIZES);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ENVIRON_SIZES);
                panic!();
            }
            Event::Call(Call::EnvironGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ENVIRON_GET);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ENVIRON_GET);
                panic!();
            }
            Event::Exit(status) => {
                if status.status() > u8::MAX as u32 {
                    panic!();
                }
                engine_break_traffic_loop(endpoint).await;
                engine_proc_exit(endpoint, status.status() as u8).await;
                break;
            }
            Event::Call(Call::MemoryGrow(_)) => {
                mark_stage(STAGE_ENGINE_TRAP_MEMORY_GROW);
                record_failure_stage(STAGE_ENGINE_TRAP_MEMORY_GROW);
                panic!();
            }
            Event::BudgetExpired(_) => {
                mark_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                record_failure_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                panic!();
            }
            Event::Done => {
                if status::mark_expected_reject_if_recorded() {
                    break;
                }
                engine_break_traffic_loop(endpoint).await;
                engine_proc_exit(endpoint, 0).await;
                break;
            }
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
async fn engine_abort_safe_session(
    endpoint: &mut EngineEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
) {
    mark_stage(STAGE_ENGINE_ABORT_ROUTE_SENT);

    let abort = EngineAbort::new(EngineAbortReason::GuestTrap, 1);
    endpoint
        .flow::<EngineAbortMsg>()
        .expect("engine flow<abort reason>")
        .send(&abort)
        .await
        .unwrap_or_else(|_| panic!());
    endpoint
        .flow::<EngineAbortBeginControl>()
        .expect("engine flow<abort begin>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());

    endpoint
        .recv::<EngineAbortFenceControl>()
        .await
        .unwrap_or_else(|_| panic!());
    for _ in BAKER_LINK_SAFE_GPIO_LEVELS {
        gpio_device_recv_linear_set_once(gpio_endpoint).await;
    }
    endpoint
        .recv::<EngineAbortAckControl>()
        .await
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_ABORT_ACK_RECV);
    mark_stage(RESULT_ABORT_SAFE_OK);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_continue_traffic_loop(endpoint: &mut EngineEndpoint) {
    match endpoint
        .flow::<BakerTrafficLoopContinueControl>()
        .expect("engine flow<traffic loop continue>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_LOOP_CONTINUE_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_break_traffic_loop(endpoint: &mut EngineEndpoint) {
    match endpoint
        .flow::<BakerTrafficLoopBreakControl>()
        .expect("engine flow<traffic loop break>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_LOOP_BREAK_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_proc_exit(endpoint: &mut EngineEndpoint, code: u8) {
    let request = EngineReq::ProcExit(ProcExitStatus::new(code));
    match endpoint
        .flow::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
        .expect("engine flow<proc_exit>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PROC_EXIT_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_recv_traffic_run(endpoint: &mut EngineEndpoint, expected_cycle: u16) -> BudgetRun {
    mark_stage(STAGE_ENGINE_RUN_RECV_BEGIN);
    let run = match endpoint.recv::<BudgetRunMsg>().await {
        Ok(run) => run,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_RUN_RECV_ERR);
            panic!()
        }
    };
    if run.run_id() != expected_cycle
        || run.generation() != 1
        || run.fuel() != BAKER_LINK_WASM_FUEL_PER_ACTIVATION
    {
        {
            record_failure_stage(STAGE_ENGINE_RUN_MISMATCH);
            panic!()
        };
    }
    mark_stage(STAGE_ENGINE_RUN_RECV_DONE);
    run
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core0_main() -> ! {
    mark_stage(STAGE_CORE0_START);
    let _ = clock::init_125mhz();
    uart::init();
    ensure_core1_launched();
    fifo_drain();
    mark_stage(STAGE_CORE1_LAUNCHED);
    rp2040_gpio_bank_init();
    for pin in BAKER_LINK_LED_PINS {
        rp2040_gpio_init_output(pin, !BAKER_LINK_LED_ACTIVE_HIGH);
    }
    mark_stage(STAGE_GPIO_READY);
    init_runtime_once();
    mark_stage(STAGE_RUNTIME_READY);
    let endpoint = unsafe { shared_kernel_endpoint() };
    #[cfg(feature = "baker-abort-safe-demo")]
    {
        run_current_task(kernel_abort_safe_session(endpoint));
        park();
    }
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    {
        let gpio_endpoint = unsafe { shared_gpio_endpoint() };
        let timer_endpoint = unsafe { shared_timer_endpoint() };
        run_current_task(kernel_session(endpoint, gpio_endpoint, timer_endpoint));
        park();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core1_main() -> ! {
    fifo_drain();
    mark_core1_started();
    signal();
    wait_until(uart::ready);
    wait_until(runtime_ready);
    mark_stage(STAGE_ENGINE_RUNTIME_READY_SEEN);
    let endpoint = unsafe { shared_engine_endpoint() };
    mark_stage(STAGE_ENGINE_ENDPOINT_READY);
    mark_stage(STAGE_ENGINE_BEGIN);
    #[cfg(feature = "baker-abort-safe-demo")]
    {
        let gpio_endpoint = unsafe { shared_gpio_endpoint() };
        run_current_task(engine_abort_safe_session(endpoint, gpio_endpoint));
        park();
    }
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    {
        let guest_slot = unsafe { &mut (*shared_runtime_ptr()).core1_guest };
        let guest = write_selected_guest_in_place(guest_slot).unwrap_or_else(|_| panic!());
        mark_stage(STAGE_ENGINE_PARSE_DONE);
        run_current_task(engine_session(endpoint, guest));
        park();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_link_leds_off_direct();
    status::hard_panic(info)
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {}
