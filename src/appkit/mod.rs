//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose kernel internals,
//! or complete WASI P1 imports outside projected endpoint/carrier progress.
//!
//! The public path is deliberately flat: application, site, and firmware code
//! import curated `appkit::*` items only. Implementation layout stays private.

mod internal;

pub use crate::choreography::protocol::BuiltInLabelUniverse as BuiltInUniverse;

#[cfg(all(not(test), target_os = "none"))]
pub use internal::{EmbeddedAttachStorage, EmbeddedAttachStorageRef};
#[cfg(feature = "wasm-engine-core")]
pub use internal::{
    WasiGuestArena, WasiGuestError, WasiGuestImage, WasiGuestLease, WasiGuestStatus,
};

pub use internal::{
    ArtifactBundle, ArtifactEvidence, ArtifactForImage, ArtifactGuestStorage, BoundaryCtx, Capsule,
    CarrierKind, ChoreoFsFact, ChoreoFsFacts, ChoreoFsObject, ChoreoFsObjectSet, DriverCtx,
    DriverFacts, EndpointCarrierFacts, EngineCtx, FdSpec, FromRunReport, HIBANA_TYPED_ROLE_DOMAIN,
    HIBANA_TYPED_ROLE_DOMAIN_SIZE, ImageId, ImageManifest, LaneSet, LedgerFacts, LedgerFdFact,
    LinkCtx, Localside, LogicalImage, NoWasi, ObjectId, PeerImageSet, Placement, ProjectionCaps,
    ResolverRegistry, RoleEndpointCtx, RoleKind, RoleKindCounts, RoleResult, RoleSet, RunReport,
    SiteId, SupervisorCtx, WasiImage, WasiImports, derive_projection_caps, run,
    validate_requested_roles,
};
