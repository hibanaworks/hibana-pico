//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose engine internals,
//! or complete WASI P1 imports outside projected endpoint/carrier progress.
//!
//! The public path is deliberately flat: application, site, and firmware code
//! import curated `appkit::*` items only. Implementation layout stays private.

mod internal;

#[cfg(all(not(test), target_os = "none"))]
pub use internal::{EmbeddedAttachStorage, EmbeddedAttachStorageRef};
#[cfg(feature = "wasm-engine-core")]
pub use internal::{
    WasiGuestArena, WasiGuestDrive, WasiGuestError, WasiGuestImage, WasiGuestLease, WasiGuestStatus,
};

pub use hibana_wasip1_runtime::choreofs::{
    ChoreoFsFacts, ChoreoFsObject, ChoreoFsObjectSet, DriverFacts, FdSpec, LedgerFacts,
    LedgerFdFact, ObjectId,
};

pub use internal::{
    BoundaryCtx, Capsule, DriverCtx, EngineCtx, Local, Localside, LogicalImage, NoWasi, Placement,
    ResolverRegistry, RoleKind, RoleResult, RoleSet, WasiImage, run,
};
