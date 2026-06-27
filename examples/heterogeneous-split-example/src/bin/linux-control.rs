use hibana_pico::appkit;

fn main() {
    type Image = heterogeneous_split_example::image::LinuxControl;

    appkit::run::<Image>(appkit::NoWasi);
    assert_eq!(
        <Image as appkit::LogicalImage>::REQUESTED_ROLES,
        appkit::RoleSet::single(0)
    );
    heterogeneous_split_example::assert_projected_role_progress();
}
