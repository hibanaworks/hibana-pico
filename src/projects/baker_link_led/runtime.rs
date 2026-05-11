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
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::storage::shared_timer_endpoint;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::storage::{
    shared_engine_endpoint, shared_gpio_endpoint, shared_kernel_endpoint, shared_runtime_ptr,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use super::{engine_session::engine_abort_safe_session, kernel_session::kernel_abort_safe_session};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::{engine_session::engine_session, kernel_session::kernel_session};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::arch::naked_asm;

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana::substrate::policy::ResolverRef;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::substrate::{SessionKit, binding::NoBinding, ids::SessionId, runtime::Config};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::proof::baker_link::choreography::POLICY_BAKER_TRAFFIC_LOOP;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-recoverable-abort-demo"),
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::proof::baker_link::choreography::abort_safe_terminal_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-choreofs-bad-path-demo"
))]
use hibana_pico::proof::baker_link::choreography::choreofs_bad_path_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::proof::baker_link::choreography::choreofs_traffic_light_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-recoverable-abort-demo"
))]
use hibana_pico::proof::baker_link::choreography::recoverable_abort_roles;
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
use hibana_pico::proof::baker_link::choreography::traffic_light_roles;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::proof::baker_link::guest::write_selected_guest_in_place;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::proof::baker_link::resolver::baker_traffic_loop_policy;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::{
    choreography::protocol::EngineLabelUniverse,
    machine::rp2040::sio::{Rp2040SioBackend, core_id, fifo_drain, launch_core1},
    machine::rp2040::{clock, timer, uart},
    port::exec::{park, run_current_task, signal, wait_until},
    port::transport::SioTransport,
    proof::baker_link::manifest::{BAKER_LINK_LED_ACTIVE_HIGH, BAKER_LINK_LED_PINS},
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
            #[cfg(feature = "baker-recoverable-abort-demo")]
            let (core0_program, core1_program, core2_program) = recoverable_abort_roles();
            #[cfg(not(feature = "baker-recoverable-abort-demo"))]
            let (core0_program, core1_program, core2_program) = abort_safe_terminal_roles();
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
            let traffic_loop_resolver = ResolverRef::loop_fn(baker_traffic_loop_policy);
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
