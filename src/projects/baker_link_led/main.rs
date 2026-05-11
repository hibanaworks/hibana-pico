#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware;
#[cfg(all(target_arch = "arm", target_os = "none"))]
mod runtime;
#[cfg(all(target_arch = "arm", target_os = "none"))]
mod stages;
#[cfg(all(target_arch = "arm", target_os = "none"))]
mod status;

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {}
