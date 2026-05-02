pub mod baker_link_led;
pub mod local;
pub mod protocol;
#[cfg(any(
    all(target_arch = "arm", target_os = "none"),
    feature = "profile-host-qemu-swarm"
))]
pub mod swarm;
