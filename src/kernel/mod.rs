//! Private appkit kernel-service namespace.
//!
//! Public callers reach kernel services only through sealed `appkit` contexts.

#[cfg(feature = "wasm-engine-core")]
pub(crate) mod engine;
#[cfg(feature = "wasm-engine-core")]
pub(crate) mod features;
