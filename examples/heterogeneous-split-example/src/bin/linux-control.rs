use hibana_pico::{appkit, appkit::ArtifactBundle, site};

fn main() {
    let report = appkit::run::<
        site::Local<heterogeneous_split_example::image::LinuxControl>,
        heterogeneous_split_example::Control,
    >(
        heterogeneous_split_example::ARTIFACTS
            .for_image::<site::Local<heterogeneous_split_example::image::LinuxControl>>(),
    );
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(30),
        hibana_pico::appkit::SiteId(300),
        0,
    );
    heterogeneous_split_example::assert_peer_manifests();
}
