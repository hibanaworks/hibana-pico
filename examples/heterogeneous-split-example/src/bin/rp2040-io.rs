#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

use hibana_pico::appkit;

#[cfg(target_os = "none")]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
    core::hint::black_box(info);
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    type Image = heterogeneous_split_example::image::Rp2040Io;

    appkit::run::<Image>(appkit::NoWasi);
    assert_eq!(
        <Image as appkit::LogicalImage>::REQUESTED_ROLES,
        appkit::RoleSet::single(2)
    );
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    type Image = heterogeneous_split_example::image::Rp2040Io;

    appkit::run::<Image>(appkit::NoWasi);
    assert_eq!(
        <Image as appkit::LogicalImage>::REQUESTED_ROLES,
        appkit::RoleSet::single(2)
    );
    loop {
        core::hint::spin_loop();
    }
}
