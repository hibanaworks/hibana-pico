#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::ptr::write_volatile;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::substrate::{AttachError, CpError};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::{machine::rp2040::uart, port::exec::park};

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

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn attach_or_stop<T>(result: Result<T, AttachError>, result_cell: *mut u32, stage: &str) -> T {
    match result {
        Ok(value) => value,
        Err(AttachError::Control(CpError::ResourceExhausted)) => {
            uart::write_bytes(stage.as_bytes());
            uart::line(" control resource exhausted");
            hard_stop(result_cell, stage)
        }
        Err(AttachError::Control(_)) => {
            uart::write_bytes(stage.as_bytes());
            uart::line(" control error");
            hard_stop(result_cell, stage)
        }
        Err(AttachError::Rendezvous(_)) => {
            uart::write_bytes(stage.as_bytes());
            uart::line(" rendezvous error");
            hard_stop(result_cell, stage)
        }
    }
}
