#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::ptr::read_volatile;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_BASE: usize = 0xD000_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_CPUID: *const u32 = SIO_BASE as *const u32;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn core_id() -> u32 {
    unsafe { read_volatile(SIO_CPUID) }
}
