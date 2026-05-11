use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

#[cfg(not(feature = "baker-abort-safe-demo"))]
use hibana_pico::kernel::{
    fd_object::GpioFdWriteError,
    guest_ledger::{WASI_ERRNO_BADF, WASI_ERRNO_INVAL, WasiErrnoMap},
};
use hibana_pico::{
    machine::rp2040::{sio::core_id, uart},
    port::exec::park,
};

use super::stages::*;

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::kernel::choreofs::ChoreoFsError;

unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
    static __stack_limit: u32;
}

#[unsafe(no_mangle)]
static mut HIBANA_DEMO_RESULT: u32 = 0;
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_FAILURE_STAGE: u32 = 0;
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES: u32 = 0;
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES: u32 = 0;
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STAGE: u32 = 0;
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STAGE: u32 = 0;

static mut CORE1_STARTED: u32 = 0;
static mut RUNTIME_READY: u32 = 0;

pub fn mark_stage(value: u32) {
    record_stack_high_water();
    unsafe {
        if core_id() == 0 {
            write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STAGE), value);
        } else {
            write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STAGE), value);
        }
        write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), value);
    }
}

fn record_stack_high_water() {
    let sp: u32;
    unsafe {
        asm!("mov {0}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
    }
    let core = core_id();
    let (top, limit, slot) = if core == 0 {
        (
            core::ptr::addr_of!(__stack_top) as u32,
            core::ptr::addr_of!(__core1_stack_top) as u32,
            core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES),
        )
    } else {
        (
            core::ptr::addr_of!(__core1_stack_top) as u32,
            core::ptr::addr_of!(__stack_limit) as u32,
            core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES),
        )
    };
    if sp < limit || sp > top {
        return;
    }
    let used = top.saturating_sub(sp);
    unsafe {
        let current = read_volatile(slot);
        if used > current {
            write_volatile(slot, used);
        }
    }
}

pub fn record_failure_stage(stage: u32) {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_FAILURE_STAGE), stage);
    }
}

#[cfg(any(
    feature = "baker-bad-order-demo",
    feature = "baker-invalid-fd-demo",
    feature = "baker-bad-payload-demo",
    feature = "baker-choreofs-bad-path-demo",
    feature = "baker-choreofs-bad-payload-demo",
    feature = "baker-choreofs-wrong-object-demo"
))]
pub fn failure_stage() -> u32 {
    unsafe { read_volatile(core::ptr::addr_of!(HIBANA_DEMO_FAILURE_STAGE)) }
}

fn record_hard_panic_if_unset() {
    unsafe {
        let failure_slot = core::ptr::addr_of_mut!(HIBANA_DEMO_FAILURE_STAGE);
        if read_volatile(failure_slot) == 0 {
            let last_stage = read_volatile(core::ptr::addr_of!(HIBANA_DEMO_RESULT));
            if last_stage == 0 {
                write_volatile(failure_slot, STAGE_HARD_PANIC);
            } else {
                write_volatile(failure_slot, last_stage);
            }
        }
    }
}

pub fn hard_panic(_info: &PanicInfo<'_>) -> ! {
    super::hardware::baker_link_leds_off_direct();
    record_hard_panic_if_unset();
    mark_stage(RESULT_FAILURE);
    uart::write_bytes(b"[panic]\n");
    park();
}

pub fn mark_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 1);
    }
}

pub fn clear_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 0);
    }
}

pub fn core1_started() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(CORE1_STARTED)) != 0 }
}

pub fn mark_runtime_ready() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RUNTIME_READY), 1);
    }
}

pub fn clear_runtime_ready() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RUNTIME_READY), 0);
    }
}

pub fn runtime_ready() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(RUNTIME_READY)) != 0 }
}

#[cfg(not(feature = "baker-abort-safe-demo"))]
pub fn gpio_fd_write_errno(error: GpioFdWriteError) -> u16 {
    #[cfg(feature = "baker-invalid-fd-demo")]
    {
        if error == GpioFdWriteError::BadFd {
            record_failure_stage(STAGE_INVALID_FD_REJECTED);
        }
    }
    #[cfg(feature = "baker-bad-payload-demo")]
    {
        if error == GpioFdWriteError::BadPayload {
            record_failure_stage(STAGE_BAD_PAYLOAD_REJECTED);
        }
    }
    #[cfg(feature = "baker-choreofs-bad-payload-demo")]
    {
        if error == GpioFdWriteError::BadPayload {
            record_failure_stage(STAGE_CHOREOFS_BAD_PAYLOAD_REJECTED);
        }
    }
    #[cfg(feature = "baker-choreofs-wrong-object-demo")]
    {
        if error == GpioFdWriteError::Fd(hibana_pico::kernel::wasi::PicoFdError::WrongResource) {
            record_failure_stage(STAGE_WRONG_OBJECT_REJECTED);
        }
    }
    match error {
        GpioFdWriteError::BadFd => WASI_ERRNO_BADF,
        GpioFdWriteError::BadPayload => WASI_ERRNO_INVAL,
        GpioFdWriteError::Fd(error) => WasiErrnoMap::new().map_fd_error(error),
    }
}

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-bad-order-demo",
        feature = "baker-invalid-fd-demo",
        feature = "baker-bad-payload-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
pub fn mark_expected_reject_if_recorded() -> bool {
    if Some(failure_stage()) != expected_reject_stage() {
        return false;
    }
    mark_stage(RESULT_EXPECTED_REJECT);
    true
}

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-bad-order-demo",
        feature = "baker-invalid-fd-demo",
        feature = "baker-bad-payload-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
fn expected_reject_stage() -> Option<u32> {
    let mut stage = None;
    #[cfg(feature = "baker-bad-order-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_BAD_ORDER_POLL_REJECTED);
        }
    }
    #[cfg(feature = "baker-invalid-fd-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_INVALID_FD_REJECTED);
        }
    }
    #[cfg(feature = "baker-bad-payload-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_BAD_PAYLOAD_REJECTED);
        }
    }
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_BAD_PATH_REJECTED);
        }
    }
    #[cfg(feature = "baker-choreofs-bad-payload-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_CHOREOFS_BAD_PAYLOAD_REJECTED);
        }
    }
    #[cfg(feature = "baker-choreofs-wrong-object-demo")]
    {
        if stage.is_none() {
            stage = Some(STAGE_WRONG_OBJECT_REJECTED);
        }
    }
    stage
}

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    not(any(
        feature = "baker-bad-order-demo",
        feature = "baker-invalid-fd-demo",
        feature = "baker-bad-payload-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))
))]
pub fn mark_expected_reject_if_recorded() -> bool {
    false
}

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
pub fn record_choreofs_open_reject(error: ChoreoFsError) {
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    {
        if matches!(error, ChoreoFsError::NotFound | ChoreoFsError::AbsolutePath) {
            record_failure_stage(STAGE_BAD_PATH_REJECTED);
        }
    }
    let _ = error;
}
