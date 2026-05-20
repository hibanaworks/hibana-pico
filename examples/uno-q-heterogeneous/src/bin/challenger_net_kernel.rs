#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

use hibana_pico::{appkit, appkit::ArtifactBundle, site};
use uno_q_heterogeneous::{UnoQCapsule, image, protocol};

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
    run();
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    run();
    loop {
        core::hint::spin_loop();
    }
}

fn run() {
    type Image = site::Local<image::ChallengerNetKernelImage>;

    let report =
        appkit::run::<Image, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Image>());

    assert_eq!(report.image_id(), appkit::ImageId(716));
    assert_eq!(report.site_id(), appkit::SiteId(7106));
    assert_eq!(
        report.requested_roles(),
        appkit::RoleSet::single(protocol::ROLE_CHALLENGER_KERNEL)
    );
}
