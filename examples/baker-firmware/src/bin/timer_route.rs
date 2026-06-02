#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::{
    g,
    integration::{
        cap::control::RouteDecisionKind,
        policy::{DecisionArm, DecisionResolution, ResolverError, ResolverRef},
    },
};
use hibana_pico::appkit;

const LABEL_RESPONSE_READY: u8 = 120;
const LABEL_TIMER_EXPIRED: u8 = 121;
const LABEL_RESPONSE_MESSAGE: u8 = 133;
const LABEL_TIMER_EXPIRED_MESSAGE: u8 = 134;
const LABEL_TIMER_ROUTE_DONE: u8 = 135;
const LABEL_TIMER_ROUTE_ACK: u8 = 137;
const TIMER_ROUTE_POLICY: u16 = 56;

type ResponseRoute = g::Msg<LABEL_RESPONSE_READY, (), RouteDecisionKind>;
type TimerExpiredRoute = g::Msg<LABEL_TIMER_EXPIRED, (), RouteDecisionKind>;
type ResponseReady = g::Msg<LABEL_RESPONSE_MESSAGE, u8>;
type TimerExpired = g::Msg<LABEL_TIMER_EXPIRED_MESSAGE, u8>;
type TimerRouteDone = g::Msg<LABEL_TIMER_ROUTE_DONE, u8>;
type TimerRouteAck = g::Msg<LABEL_TIMER_ROUTE_ACK, u8>;

pub struct TimerRoute;
pub struct TimerRouteLocal;

#[derive(Clone, Copy, Debug)]
pub enum TimerRouteError {
    Endpoint(hibana::EndpointError),
    Resolver(hibana::integration::policy::ResolverError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for TimerRouteError {
    fn from(error: hibana::EndpointError) -> Self {
        baker_firmware::record_choreofs_engine_status(
            0x5452_e000 | baker_firmware::choreofs_endpoint_error_code(&error),
        );
        Self::Endpoint(error)
    }
}

impl From<hibana::integration::policy::ResolverError> for TimerRouteError {
    fn from(error: hibana::integration::policy::ResolverError) -> Self {
        baker_firmware::record_choreofs_engine_status(0x5452_f000);
        Self::Resolver(error)
    }
}

fn timer_route_resolver() -> Result<DecisionResolution, ResolverError> {
    Ok(DecisionResolution::Arm(DecisionArm::Right))
}

impl appkit::Capsule for TimerRoute {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = TimerRouteLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable {
        g::seq(
            g::route(
                g::seq(
                    g::send::<1, 1, ResponseRoute, 1>().policy::<TIMER_ROUTE_POLICY>(),
                    g::send::<1, 0, ResponseReady, 1>(),
                ),
                g::seq(
                    g::send::<1, 1, TimerExpiredRoute, 1>().policy::<TIMER_ROUTE_POLICY>(),
                    g::send::<1, 0, TimerExpired, 1>(),
                ),
            ),
            g::seq(
                g::send::<0, 1, TimerRouteDone, 1>(),
                g::send::<1, 0, TimerRouteAck, 1>(),
            ),
        )
    }

    fn register_resolvers<'cfg, R>(registry: &mut R)
    where
        R: appkit::ResolverRegistry<'cfg, Self>,
    {
        baker_firmware::record_choreofs_engine_status(0x5452_0200);
        registry.policy::<TIMER_ROUTE_POLICY, 0>(ResolverRef::decision_fn(timer_route_resolver));
        registry.policy::<TIMER_ROUTE_POLICY, 1>(ResolverRef::decision_fn(timer_route_resolver));
        baker_firmware::record_choreofs_engine_status(0x5452_0201);
    }
}

impl BakerCapsuleFacts for TimerRoute {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(56);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(57);
    const SUCCESS_RESULT: u32 = baker_firmware::RESULT_TIMER_ROUTE_OK;
}

impl appkit::Localside<TimerRoute> for TimerRouteLocal {
    type Error = TimerRouteError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, TimerRoute, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                baker_firmware::record_choreofs_engine_status(0x5452_010f);

                while !baker_firmware::baker_timer_route_resolver_ready(100) {
                    baker_firmware::baker_poll_delay(1);
                }

                let route = ctx.endpoint().flow::<TimerExpiredRoute>()?;
                route.send(&()).await?;
                baker_firmware::record_choreofs_engine_status(0x5452_0111);

                let expired = ctx.endpoint().flow::<TimerExpired>()?;
                expired.send(&1).await?;
                baker_firmware::record_choreofs_engine_status(0x5452_0112);

                let done = ctx.endpoint().recv::<TimerRouteDone>().await?;
                if done != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_engine_status(0x5452_0113);

                let ack = ctx.endpoint().flow::<TimerRouteAck>()?;
                ack.send(&1).await?;
                baker_firmware::record_choreofs_engine_status(0x5452_0114);

                baker_firmware::mark_runtime_ready();
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, TimerRoute, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                baker_firmware::record_choreofs_driver_trace(0x5452_000f);
                baker_firmware::record_choreofs_driver_trace(0x5452_0010);

                let branch = ctx.endpoint().offer().await?;
                baker_firmware::record_choreofs_driver_trace(
                    0x5452_1000 | u32::from(branch.label()),
                );
                let expired = branch.decode::<TimerExpired>().await?;
                if expired != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_driver_trace(0x5452_0012);

                let done = ctx.endpoint().flow::<TimerRouteDone>()?;
                done.send(&1).await?;
                baker_firmware::record_choreofs_driver_trace(0x5452_0013);

                let ack = ctx.endpoint().recv::<TimerRouteAck>().await?;
                if ack != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_driver_trace(0x5452_0014);

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<TimerRoute as BakerCapsuleFacts>::SUCCESS_RESULT);
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, TimerRoute, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, TimerRoute, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, TimerRoute, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<TimerRoute, I> for BakerArtifacts
where
    I: appkit::LogicalImage<TimerRoute, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn baker_selected_run() -> ! {
    baker_firmware::run::<TimerRoute>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<TimerRoute>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<TimerRoute>()
}
