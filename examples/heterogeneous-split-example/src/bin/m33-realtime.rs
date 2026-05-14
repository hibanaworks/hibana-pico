#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
    core::hint::black_box(info);
    loop {
        core::hint::spin_loop();
    }
}

fn run_image() {
    let report = heterogeneous_split_example::run_m33_realtime();
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(31),
        hibana_pico::appkit::SiteId(331),
        1,
    );
}

#[cfg(not(target_os = "none"))]
fn main() {
    run_image();
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    run_image();
    loop {
        core::hint::spin_loop();
    }
}
