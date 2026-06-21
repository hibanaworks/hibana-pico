#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
extern crate std;

#[cfg(all(not(test), not(target_os = "none")))]
extern crate std;

pub mod appkit;
