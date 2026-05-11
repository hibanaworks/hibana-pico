use core::cell::Cell;

use hibana::substrate::{
    cap::{
        ResourceKind,
        advanced::{LoopBreakKind, LoopContinueKind},
    },
    policy::{
        LoopResolution, ResolverContext, ResolverError, RouteResolution,
        signals::core as policy_core,
    },
};

pub struct BakerTrafficLoopResolver;

impl BakerTrafficLoopResolver {
    pub const fn new() -> Self {
        Self
    }

    pub fn resolve_policy(&self, ctx: ResolverContext) -> Result<LoopResolution, ResolverError> {
        let Some(tag) = ctx.attr(policy_core::TAG).map(|value| value.as_u8()) else {
            return Err(ResolverError::Reject);
        };
        match tag {
            LoopContinueKind::TAG => Ok(LoopResolution::Continue),
            LoopBreakKind::TAG => Ok(LoopResolution::Break),
            _ => Err(ResolverError::Reject),
        }
    }
}

impl Default for BakerTrafficLoopResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolver for the Baker Engine-owned Abort | Normal boundary.
///
/// The active demo/test selects the abort arm after Engine observes a guest
/// boundary failure. Normal continuation remains a distinct arm and is not
/// represented as LoopBreak.
pub struct BakerAbortRouteResolver {
    arm: Cell<u8>,
}

impl BakerAbortRouteResolver {
    pub const fn new_abort() -> Self {
        Self { arm: Cell::new(0) }
    }

    pub const fn new_normal() -> Self {
        Self { arm: Cell::new(1) }
    }

    pub fn select_abort(&self) {
        self.arm.set(0);
    }

    pub fn select_normal(&self) {
        self.arm.set(1);
    }

    pub fn resolve_policy(&self, _ctx: ResolverContext) -> Result<RouteResolution, ResolverError> {
        match self.arm.get() {
            0 => Ok(RouteResolution::Arm(0)),
            1 => Ok(RouteResolution::Arm(1)),
            _ => Err(ResolverError::Reject),
        }
    }
}

impl Default for BakerAbortRouteResolver {
    fn default() -> Self {
        Self::new_abort()
    }
}
