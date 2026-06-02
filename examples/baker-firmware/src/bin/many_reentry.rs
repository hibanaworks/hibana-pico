#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason,
    },
};

pub struct ManyReentry;
pub struct ManyReentryLocal;

#[derive(Debug)]
pub enum ManyReentryError {
    Endpoint(hibana::EndpointError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for ManyReentryError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl appkit::Capsule for ManyReentry {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = ManyReentryLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable {
        g::seq(
            g::send::<1, 0, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<1, 0, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<0, 1, EngineAbortFenceControl, 0>(),
                    g::seq(
                        g::send::<1, 0, EngineAbortAckControl, 0>(),
                        g::seq(
                            g::send::<1, 0, EngineAbortBeginControl, 1>(),
                            g::seq(
                                g::send::<1, 0, EngineAbortMsg, 1>(),
                                g::seq(
                                    g::send::<0, 1, EngineAbortFenceControl, 1>(),
                                    g::send::<1, 0, EngineAbortAckControl, 1>(),
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
    type Error = ManyReentryError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                let begin = ctx.endpoint().flow::<EngineAbortBeginControl>()?;
                begin.send(&()).await?;

                let first_abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 1);
                let first_abort_flow = ctx.endpoint().flow::<EngineAbortMsg>()?;
                first_abort_flow.send(&first_abort).await?;

                ctx.endpoint().recv::<EngineAbortFenceControl>().await?;

                let first_ack = ctx.endpoint().flow::<EngineAbortAckControl>()?;
                first_ack.send(&()).await?;

                let second_begin = ctx.endpoint().flow::<EngineAbortBeginControl>()?;
                second_begin.send(&()).await?;

                let second_abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 2);
                let second_abort_flow = ctx.endpoint().flow::<EngineAbortMsg>()?;
                second_abort_flow.send(&second_abort).await?;

                ctx.endpoint().recv::<EngineAbortFenceControl>().await?;

                let second_ack = ctx.endpoint().flow::<EngineAbortAckControl>()?;
                second_ack.send(&()).await?;

                baker_firmware::mark_runtime_ready();
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.endpoint().recv::<EngineAbortBeginControl>().await?;

                let abort = ctx.endpoint().recv::<EngineAbortMsg>().await?;
                if abort.reason() != EngineAbortReason::FuelExhausted {
                    return Err(ManyReentryError::RuntimeViolation);
                }

                baker_firmware::mark_safe_state();

                let abort_fence = ctx.endpoint().flow::<EngineAbortFenceControl>()?;
                abort_fence.send(&()).await?;

                ctx.endpoint().recv::<EngineAbortAckControl>().await?;

                ctx.endpoint().recv::<EngineAbortBeginControl>().await?;

                let abort = ctx.endpoint().recv::<EngineAbortMsg>().await?;
                if abort.reason() != EngineAbortReason::FuelExhausted {
                    return Err(ManyReentryError::RuntimeViolation);
                }

                baker_firmware::mark_safe_state();

                let second_abort_fence = ctx.endpoint().flow::<EngineAbortFenceControl>()?;
                second_abort_fence.send(&()).await?;

                ctx.endpoint().recv::<EngineAbortAckControl>().await?;

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<ManyReentry as BakerCapsuleFacts>::SUCCESS_RESULT);
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
