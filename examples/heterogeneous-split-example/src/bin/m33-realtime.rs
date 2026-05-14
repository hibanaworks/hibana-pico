#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

use hibana_pico::{appkit, appkit::ArtifactBundle, site};

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
    let report = appkit::run::<
        site::Local<heterogeneous_split_example::image::M33Realtime>,
        heterogeneous_split_example::Control,
    >(
        heterogeneous_split_example::ARTIFACTS
            .for_image::<site::Local<heterogeneous_split_example::image::M33Realtime>>(),
    );
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(31),
        hibana_pico::appkit::SiteId(331),
        1,
    );
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    let report = appkit::run::<
        site::Local<heterogeneous_split_example::image::M33Realtime>,
        heterogeneous_split_example::Control,
    >(
        heterogeneous_split_example::ARTIFACTS
            .for_image::<site::Local<heterogeneous_split_example::image::M33Realtime>>(),
    );
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(31),
        hibana_pico::appkit::SiteId(331),
        1,
    );
    loop {
        core::hint::spin_loop();
    }
}
