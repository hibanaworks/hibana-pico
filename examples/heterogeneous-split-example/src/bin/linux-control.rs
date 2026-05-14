fn main() {
    let report = heterogeneous_split_example::run_linux_control();
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(30),
        hibana_pico::appkit::SiteId(300),
        0,
    );
    heterogeneous_split_example::assert_peer_manifests();
}
