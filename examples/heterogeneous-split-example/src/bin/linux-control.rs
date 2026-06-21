use hibana_pico::appkit;

fn main() {
    type Image = appkit::Local<heterogeneous_split_example::image::LinuxControl>;

    appkit::run::<
        appkit::Local<heterogeneous_split_example::image::LinuxControl>,
        heterogeneous_split_example::Control,
    >(appkit::NoWasi);
    assert_eq!(
        <Image as appkit::LogicalImage<heterogeneous_split_example::Control>>::REQUESTED_ROLES,
        appkit::RoleSet::single(0)
    );
    heterogeneous_split_example::assert_projected_role_progress();
}
