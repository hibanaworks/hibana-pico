#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

const PANIC_MARKER_MESSAGE: &str = "baker panic marker proof";

#[inline(never)]
fn trigger_panic_marker() -> ! {
    panic!("{PANIC_MARKER_MESSAGE}")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn baker_selected_run() -> ! {
    trigger_panic_marker()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    trigger_panic_marker()
}
