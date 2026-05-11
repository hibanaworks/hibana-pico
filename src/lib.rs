#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
extern crate std;

#[cfg(all(
    not(test),
    any(feature = "platform-host-linux", feature = "wasm-engine-wasip1-full")
))]
extern crate std;

pub mod choreography;
pub mod kernel;
pub mod machine;
pub mod port;
pub mod projects;
