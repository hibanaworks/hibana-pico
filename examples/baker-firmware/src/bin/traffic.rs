#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::convert::Infallible;

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason,
    },
};

pub struct Traffic;
pub struct TrafficLocal;

impl appkit::Capsule for Traffic {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = TrafficLocal;
    type Report = Infallible;

    fn choreography() -> impl hibana::substrate::program::Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                ),
            ),
        )
    }
}

impl BakerCapsuleFacts for Traffic {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(0);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(1);
}

impl appkit::Localside<Traffic> for TrafficLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, Traffic, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 1 {
                let begin = match ctx.endpoint().flow::<EngineAbortBeginControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if begin.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                let abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 1);
                let abort_flow = match ctx.endpoint().flow::<EngineAbortMsg>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if abort_flow.send(&abort).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                if ctx
                    .endpoint()
                    .recv::<EngineAbortFenceControl>()
                    .await
                    .is_err()
                {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                let ack = match ctx.endpoint().flow::<EngineAbortAckControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if ack.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                baker_firmware::mark_runtime_ready();
                return core::future::pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, Traffic, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 0 {
                if ctx
                    .endpoint()
                    .recv::<EngineAbortBeginControl>()
                    .await
                    .is_err()
                {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                match ctx.endpoint().recv::<EngineAbortMsg>().await {
                    Ok(abort) if abort.reason() == EngineAbortReason::FuelExhausted => {}
                    Ok(_) | Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                }

                baker_firmware::mark_safe_state();

                let fence = match ctx.endpoint().flow::<EngineAbortFenceControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if fence.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                if ctx
                    .endpoint()
                    .recv::<EngineAbortAckControl>()
                    .await
                    .is_err()
                {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<Traffic as BakerCapsuleFacts>::SUCCESS_RESULT);
                return core::future::pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, Traffic, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, Traffic, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, Traffic, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<Traffic, I> for BakerArtifacts
where
    I: appkit::LogicalImage<Traffic, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<Traffic>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<Traffic>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<Traffic>()
}
