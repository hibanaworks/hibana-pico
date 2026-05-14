fn main() {
    let report = heterogeneous_split_example::run_rp2040_io();
    heterogeneous_split_example::assert_single_role_image(
        &report,
        hibana_pico::appkit::ImageId(32),
        hibana_pico::appkit::SiteId(2040),
        2,
    );
}
