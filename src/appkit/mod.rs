//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose engine internals,
//! or complete WASI P1 imports outside projected endpoint/carrier progress.
//!
//! The public path is deliberately flat for capsule assembly. Hibana choreography
//! and WASI/ChoreoFS facts stay owned by their own crates; implementation
//! layout under this module stays private.

mod internal;

#[cfg(all(not(test), target_os = "none"))]
pub use internal::{EmbeddedAttachStorage, EmbeddedAttachStorageRef};
#[cfg(feature = "wasm-engine-core")]
pub use internal::{
    WasiGuestArena, WasiGuestError, WasiGuestImage, WasiGuestLease, WasiGuestStatus,
};

pub use internal::{
    Capsule, Localside, LogicalImage, NoWasi, Placement, ResolverRegistry, RoleKind, RoleResult,
    RoleSet, WasiImage, pending, run,
};
