#[cfg(all(target_arch = "arm", target_os = "none"))]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SwarmKernelRole {
    pub node_role: Option<u8>,
    pub fixed_node_count: Option<u8>,
    pub fixed_sensor_hibana_role: Option<u8>,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub(super) fn configured_node_role(qemu_role: u8) -> u8 {
    crate::SWARM_KERNEL_ROLE.node_role.unwrap_or(qemu_role)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub(super) fn fixed_node_count() -> Option<u8> {
    crate::SWARM_KERNEL_ROLE.fixed_node_count
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub(super) fn fixed_sensor_hibana_role() -> Option<u8> {
    crate::SWARM_KERNEL_ROLE.fixed_sensor_hibana_role
}
