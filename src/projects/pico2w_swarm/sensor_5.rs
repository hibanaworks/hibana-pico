#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod runtime;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SWARM_KERNEL_ROLE: runtime::SwarmKernelRole = runtime::SwarmKernelRole {
    node_role: Some(1),
    fixed_node_count: Some(6),
    fixed_sensor_hibana_role: Some(4),
};

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {}
