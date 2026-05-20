use hibana_pico::{appkit, appkit::ArtifactBundle, site};
use uno_q_heterogeneous::{UnoQCapsule, image, protocol};

fn main() {
    type Image = site::Local<image::LinuxKernelProcess>;

    let report =
        appkit::run::<Image, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Image>());

    assert_eq!(report.image_id(), appkit::ImageId(712));
    assert_eq!(report.site_id(), appkit::SiteId(7102));
    assert_eq!(
        report.requested_roles(),
        appkit::RoleSet::single(protocol::ROLE_LINUX_KERNEL)
    );
}
