#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::convert::Infallible;

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason, LABEL_MEM_FENCE, MemFence, MemFenceReason,
    },
};

pub struct ManyReentry;
pub struct ManyReentryLocal;

impl appkit::Capsule for ManyReentry {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = ManyReentryLocal;
    type Report = Infallible;

    fn choreography() -> impl hibana::substrate::program::Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::seq(
                        g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                        g::seq(
                            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 1>(),
                            g::seq(
                                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 1>(),
                                g::seq(
                                    g::send::<
                                        g::Role<0>,
                                        g::Role<1>,
                                        g::Msg<LABEL_MEM_FENCE, MemFence>,
                                        1,
                                    >(),
                                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 1>(),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
    }
}

impl BakerCapsuleFacts for ManyReentry {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(40);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(41);
    const SUCCESS_RESULT: u32 = baker_firmware::RESULT_MANY_REENTRY_OK;
}

impl appkit::Localside<ManyReentry> for ManyReentryLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, ManyReentry, ROLE>,
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

                let first_abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 1);
                let first_abort_flow = match ctx.endpoint().flow::<EngineAbortMsg>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if first_abort_flow.send(&first_abort).await.is_err() {
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

                let first_ack = match ctx.endpoint().flow::<EngineAbortAckControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if first_ack.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                let second_begin = match ctx.endpoint().flow::<EngineAbortBeginControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if second_begin.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                let second_abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 2);
                let second_abort_flow = match ctx.endpoint().flow::<EngineAbortMsg>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if second_abort_flow.send(&second_abort).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                if ctx
                    .endpoint()
                    .recv::<g::Msg<LABEL_MEM_FENCE, MemFence>>()
                    .await
                    .is_err()
                {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                let second_ack = match ctx.endpoint().flow::<EngineAbortAckControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if second_ack.send(()).await.is_err() {
                    baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR);
                }

                baker_firmware::mark_runtime_ready();
                return core::future::pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, ManyReentry, ROLE>,
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

                let abort_fence = match ctx.endpoint().flow::<EngineAbortFenceControl>() {
                    Ok(flow) => flow,
                    Err(_) => {
                        baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                    }
                };
                if abort_fence.send(()).await.is_err() {
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

                let mem_fence = MemFence::new(MemFenceReason::HotSwap, 2);
                let mem_fence_flow =
                    match ctx.endpoint().flow::<g::Msg<LABEL_MEM_FENCE, MemFence>>() {
                        Ok(flow) => flow,
                        Err(_) => {
                            baker_firmware::runtime_fail(baker_firmware::STAGE_CONTROL_FLOW_ERROR)
                        }
                    };
                if mem_fence_flow.send(&mem_fence).await.is_err() {
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
                baker_firmware::mark_success(<ManyReentry as BakerCapsuleFacts>::SUCCESS_RESULT);
                return core::future::pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<ManyReentry, I> for BakerArtifacts
where
    I: appkit::LogicalImage<ManyReentry, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<ManyReentry>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<ManyReentry>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<ManyReentry>()
}
