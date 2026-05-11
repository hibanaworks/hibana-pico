#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{arch::naked_asm, cell::UnsafeCell, mem::MaybeUninit, ptr::read_volatile};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::{
    Endpoint, g,
    g::{Msg, Role},
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        program::{RoleProgram, project},
        runtime::{Config, CounterClock, LabelUniverse},
        tap::TapEvent,
    },
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::{
    machine::rp2040::{
        bringup,
        sio::{Rp2040SioBackend, core_id, launch_core1},
        uart,
    },
    port::exec::{park, run_current_task, signal, wait_until},
    port::transport::SioTransport,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".boot2")]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
const LABEL_PING: u8 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const LABEL_PONG: u8 = 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PING_VALUE: u8 = 0x2a;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PONG_VALUE: u8 = 0x55;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SLAB_BYTES: usize = 40 * 1024;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[derive(Clone, Copy, Debug, Default)]
struct PingPongLabelUniverse;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl LabelUniverse for PingPongLabelUniverse {
    const MAX_LABEL: u8 = LABEL_PONG;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
macro_rules! ping_pong_program {
    () => {
        g::seq(
            g::send::<Role<1>, Role<0>, Msg<LABEL_PING, u8>, 0>(),
            g::send::<Role<0>, Role<1>, Msg<LABEL_PONG, u8>, 0>(),
        )
    };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static CORE0_PROGRAM: RoleProgram<0> = project(&ping_pong_program!());
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CORE1_PROGRAM: RoleProgram<1> = project(&ping_pong_program!());

#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoTransport = SioTransport<Rp2040SioBackend>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoKit = SessionKit<'static, DemoTransport, PingPongLabelUniverse, CounterClock, 1>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore0Endpoint = Endpoint<'static, 0>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore1Endpoint = Endpoint<'static, 1>;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_RESULT: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RUNTIME_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut CORE1_STARTED: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct VectorTable {
    initial_stack_pointer: *const u32,
    reset: unsafe extern "C" fn() -> !,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".vector_table.reset_vector")]
#[used]
static VECTOR_TABLE: VectorTable = VectorTable {
    initial_stack_pointer: core::ptr::addr_of!(__stack_top) as *const u32,
    reset,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntime {
    clock: CounterClock,
    tap: [TapEvent; 128],
    slab: [u8; SLAB_BYTES],
    session: MaybeUninit<DemoKit>,
    core0_endpoint: MaybeUninit<DemoCore0Endpoint>,
    core1_endpoint: MaybeUninit<DemoCore1Endpoint>,
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
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntimeCell(UnsafeCell<SharedRuntime>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for SharedRuntimeCell {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static SHARED_RUNTIME: SharedRuntimeCell = SharedRuntimeCell(UnsafeCell::new(SharedRuntime::new()));

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
        0 => core0_main(),
        _ => core1_main(),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn core1_launch_entry() -> ! {
    core1_main()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn shared_runtime_ptr() -> *mut SharedRuntime {
    SHARED_RUNTIME.0.get()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core0_endpoint() -> &'static mut DemoCore0Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core0_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core1_endpoint() -> &'static mut DemoCore1Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core1_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn result_ptr() -> *mut u32 {
    core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn hard_stop(stage: &str) -> ! {
    bringup::hard_stop(result_ptr(), stage)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn must_attach<T>(result: Result<T, hibana::substrate::AttachError>, stage: &str) -> T {
    bringup::attach_or_stop(result, result_ptr(), stage)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_runtime_once() {
    let runtime = shared_runtime_ptr();
    unsafe {
        let session_ptr = (*runtime).session.as_mut_ptr();
        session_ptr.write(SessionKit::new(&(*runtime).clock));
        let kit = &*session_ptr;
        let rv = match kit.add_rendezvous_from_config(
            Config::new(&mut (*runtime).tap, &mut (*runtime).slab)
                .with_universe(PingPongLabelUniverse),
            SioTransport::new(Rp2040SioBackend::new()),
        ) {
            Ok(rv) => rv,
            Err(_) => hard_stop("[core0] add rendezvous"),
        };
        let sid = SessionId::new(1);
        (*runtime).core0_endpoint.as_mut_ptr().write(must_attach(
            kit.enter(rv, sid, &CORE0_PROGRAM, NoBinding),
            "[core0] attach endpoint",
        ));
        (*runtime).core1_endpoint.as_mut_ptr().write(must_attach(
            kit.enter(rv, sid, &CORE1_PROGRAM, NoBinding),
            "[core1] attach endpoint",
        ));
        RUNTIME_READY = 1;
    }
    signal();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn ensure_core1_launched() {
    unsafe {
        CORE1_STARTED = 0;
        RUNTIME_READY = 0;
    }
    if !launch_core1(
        core::ptr::addr_of!(VECTOR_TABLE) as u32,
        core::ptr::addr_of!(__core1_stack_top) as u32,
        core1_launch_entry as *const () as usize as u32,
    ) {
        hard_stop("[core0] launch core1");
    }
    for _ in 0..100_000 {
        if unsafe { read_volatile(core::ptr::addr_of!(CORE1_STARTED)) } != 0 {
            return;
        }
        core::hint::spin_loop();
    }
    hard_stop("[core0] core1 start");
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_session(endpoint: &mut DemoCore0Endpoint) {
    uart::line("[core0] wait ping");
    let ping = match endpoint.recv::<Msg<LABEL_PING, u8>>().await {
        Ok(ping) => ping,
        Err(_) => hard_stop("[core0] recv ping"),
    };
    uart::hex_line("[core0] recv ping 0x", ping as u32);
    if ping != PING_VALUE {
        hard_stop("[core0] ping mismatch");
    }

    match endpoint
        .flow::<Msg<LABEL_PONG, u8>>()
        .expect("core0 flow<pong>")
        .send(&PONG_VALUE)
        .await
    {
        Ok(_) => {}
        Err(_) => hard_stop("[core0] send pong"),
    }
    uart::hex_line("[core0] sent pong 0x", PONG_VALUE as u32);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_session(endpoint: &mut DemoCore1Endpoint) {
    uart::hex_line("[core1] send ping 0x", PING_VALUE as u32);
    match endpoint
        .flow::<Msg<LABEL_PING, u8>>()
        .expect("core1 flow<ping>")
        .send(&PING_VALUE)
        .await
    {
        Ok(_) => {}
        Err(_) => hard_stop("[core1] send ping"),
    }

    let pong = match endpoint.recv::<Msg<LABEL_PONG, u8>>().await {
        Ok(pong) => pong,
        Err(_) => hard_stop("[core1] recv pong"),
    };
    uart::hex_line("[core1] recv pong 0x", pong as u32);

    let result = if pong == PONG_VALUE {
        bringup::RESULT_SUCCESS
    } else {
        bringup::RESULT_FAILURE
    };
    bringup::mark_result(result_ptr(), result);

    if pong == PONG_VALUE {
        uart::line("[core1] hibana sio ping-pong ok");
    } else {
        uart::line("[core1] hibana sio ping-pong fail");
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core0_main() -> ! {
    uart::init();
    uart::line("[core0] hibana sio ping-pong");
    ensure_core1_launched();
    uart::line("[core0] init runtime");
    init_runtime_once();
    let endpoint = unsafe { shared_core0_endpoint() };
    run_current_task(core0_session(endpoint));
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core1_main() -> ! {
    unsafe {
        CORE1_STARTED = 1;
    }
    signal();
    wait_until(uart::ready);
    wait_until(|| unsafe { read_volatile(core::ptr::addr_of!(RUNTIME_READY)) } != 0);
    let endpoint = unsafe { shared_core1_endpoint() };
    run_current_task(core1_session(endpoint));
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    hard_stop("[panic]")
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {}
