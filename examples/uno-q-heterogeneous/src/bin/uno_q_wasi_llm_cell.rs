use hibana_pico::{appkit, appkit::ArtifactBundle, site};
use uno_q_heterogeneous::{UnoQCapsule, image, protocol};

fn main() {
    type Image = site::Local<image::WasiLlmCellProcess>;

    let report =
        appkit::run::<Image, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Image>());

    assert_eq!(report.image_id(), appkit::ImageId(711));
    assert_eq!(report.site_id(), appkit::SiteId(7101));
    assert_eq!(
        report.requested_roles(),
        appkit::RoleSet::single(protocol::ROLE_WASI_LLM_CELL)
    );
}
