#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::ptr::write_volatile;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::{machine::rp2350::uart, port::exec::park};

pub const RESULT_SUCCESS: u32 = 0x4849_4f4b;
pub const RESULT_FAILURE: u32 = 0x4849_4641;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn mark_result(result: *mut u32, value: u32) {
    unsafe {
        write_volatile(result, value);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn hard_stop(result: *mut u32, stage: &str) -> ! {
    mark_result(result, RESULT_FAILURE);
    uart::write_bytes(stage.as_bytes());
    uart::line(" fail");
    park();
}
