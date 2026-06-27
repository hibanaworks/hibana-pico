#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::{
    g,
    runtime::resolver::{DecisionArm, ResolverError, ResolverRef},
};
use hibana_pico::appkit;

const LABEL_RESPONSE_MESSAGE: u8 = 133;
const LABEL_TIMER_EXPIRED_MESSAGE: u8 = 134;
const LABEL_TIMER_ROUTE_DONE: u8 = 135;
const LABEL_TIMER_ROUTE_ACK: u8 = 137;
const TIMER_ROUTE_POLICY: u16 = 56;
const RESULT_TIMER_ROUTE_OK: u32 = 0x4849_5452;
const ENDPOINT_ERROR_CODE: u32 = 0x5745_0f00;
static TIMER_ROUTE_RESOLVER_STATE: () = ();

type ResponseReady = g::Msg<LABEL_RESPONSE_MESSAGE, u8>;
type TimerExpired = g::Msg<LABEL_TIMER_EXPIRED_MESSAGE, u8>;
type TimerRouteDone = g::Msg<LABEL_TIMER_ROUTE_DONE, u8>;
type TimerRouteAck = g::Msg<LABEL_TIMER_ROUTE_ACK, u8>;

struct TimerRoute;
struct TimerRouteLocal;

#[derive(Clone, Copy, Debug)]
enum TimerRouteError {
    Endpoint,
    Resolver,
    RuntimeViolation,
}

impl From<hibana::EndpointError> for TimerRouteError {
    fn from(_: hibana::EndpointError) -> Self {
        baker_firmware::record_choreofs_engine_status(0x5452_e000 | ENDPOINT_ERROR_CODE);
        Self::Endpoint
    }
}

impl From<hibana::runtime::resolver::ResolverError> for TimerRouteError {
    fn from(_: hibana::runtime::resolver::ResolverError) -> Self {
        baker_firmware::record_choreofs_engine_status(0x5452_f000);
        Self::Resolver
    }
}

fn timer_route_resolver(_: &()) -> Result<DecisionArm, ResolverError> {
    if baker_firmware::baker_timer_route_irq_observed() {
        Ok(DecisionArm::Right)
    } else {
        Ok(DecisionArm::Left)
    }
}

impl appkit::Capsule for TimerRoute {
    type Placement = BakerPlacement;
    type Localside = TimerRouteLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::seq(
            g::route(
                g::send::<1, 0, ResponseReady>(),
                g::send::<1, 0, TimerExpired>(),
            )
            .resolve::<TIMER_ROUTE_POLICY>(),
            g::seq(
                g::send::<0, 1, TimerRouteDone>(),
                g::send::<1, 0, TimerRouteAck>(),
            ),
        )
    }

    fn register_resolvers<'cfg, R, const ROLE: u8>(registry: &mut R)
    where
        R: appkit::ResolverRegistry<'cfg, Self, ROLE>,
    {
        baker_firmware::record_choreofs_engine_status(0x5452_0200);
        let resolver =
            ResolverRef::decision_state(&TIMER_ROUTE_RESOLVER_STATE, timer_route_resolver);
        registry.resolver::<TIMER_ROUTE_POLICY>(resolver);
        baker_firmware::record_choreofs_engine_status(0x5452_0201);
    }
}

impl BakerCapsuleFacts for TimerRoute {
    const SUCCESS_RESULT: u32 = RESULT_TIMER_ROUTE_OK;

    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<TimerRoute> for TimerRouteLocal {
    type Error = TimerRouteError;

    fn engine<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                baker_firmware::record_choreofs_engine_status(0x5452_010f);

                while !baker_firmware::baker_timer_route_resolver_ready(100) {
                    core::hint::spin_loop();
                }
                if !baker_firmware::baker_timer_route_irq_observed() {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_engine_status(0x5452_0111);

                ctx.send::<TimerExpired>(&1).await?;
                baker_firmware::record_choreofs_engine_status(0x5452_0112);

                let done = ctx.recv::<TimerRouteDone>().await?;
                if done != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_engine_status(0x5452_0113);

                ctx.send::<TimerRouteAck>(&1).await?;
                baker_firmware::record_choreofs_engine_status(0x5452_0114);

                baker_firmware::mark_runtime_ready();
                return appkit::pending(ctx).await;
            }
            appkit::pending(ctx).await
        }
    }

    fn driver<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                baker_firmware::record_choreofs_driver_trace(0x5452_000f);
                baker_firmware::record_choreofs_driver_trace(0x5452_0010);
                if !baker_firmware::baker_wait_timer_route_irq_observed(500) {
                    return Err(TimerRouteError::RuntimeViolation);
                }

                let expired = ctx.recv::<TimerExpired>().await?;
                if expired != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_driver_trace(0x5452_0012);

                ctx.send::<TimerRouteDone>(&1).await?;
                baker_firmware::record_choreofs_driver_trace(0x5452_0013);

                let ack = ctx.recv::<TimerRouteAck>().await?;
                if ack != 1 {
                    return Err(TimerRouteError::RuntimeViolation);
                }
                baker_firmware::record_choreofs_driver_trace(0x5452_0014);

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<TimerRoute as BakerCapsuleFacts>::SUCCESS_RESULT);
                return appkit::pending(ctx).await;
            }
            appkit::pending(ctx).await
        }
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
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
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<TimerRoute>()
}
