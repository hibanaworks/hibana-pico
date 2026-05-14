#![no_std]

//! Guest-side helpers for hibana-pico WASI smoke apps.
//!
//! The public API is split between a small generic ChoreoFS layer and
//! proof-specific helpers. Raw WASI Preview 1 imports stay private in `sys`.

pub mod baker;
pub mod choreofs;
mod error;
pub mod net;
mod sys;

pub use error::{Error, Result, Syscall};
