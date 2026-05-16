#![no_std]

//! Guest-side helpers for hibana-pico WASI proof apps.
//!
//! The public API is a small generic ChoreoFS and timer layer. Board, device,
//! network, and proof-specific meaning belongs in the example or user capsule
//! that owns that meaning. Raw WASI Preview 1 imports stay private in `sys`.

pub mod choreofs;
mod error;
mod sys;
pub mod time;

pub use error::{Error, Result, Syscall};
