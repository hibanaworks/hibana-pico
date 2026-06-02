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

pub struct FailSafe;
pub struct FailSafeLocal;

#[derive(Debug)]
pub enum FailSafeError {
    Endpoint(hibana::EndpointError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for FailSafeError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl appkit::Capsule for FailSafe {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = FailSafeLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable {
        g::seq(
            g::send::<1, 0, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<1, 0, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<0, 1, EngineAbortFenceControl, 0>(),
                    g::send::<1, 0, EngineAbortAckControl, 0>(),
                ),
            ),
        )
    }
}

impl BakerCapsuleFacts for FailSafe {
    const SUCCESS_RESULT: u32 = baker_firmware::RESULT_FAIL_SAFE_OK;
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(20);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(21);
}

impl appkit::Localside<FailSafe> for FailSafeLocal {
    type Error = FailSafeError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                let begin = ctx.endpoint().flow::<EngineAbortBeginControl>()?;
                begin.send(&()).await?;

                let abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 1);
                let abort_flow = ctx.endpoint().flow::<EngineAbortMsg>()?;
                abort_flow.send(&abort).await?;

                ctx.endpoint().recv::<EngineAbortFenceControl>().await?;

                let ack = ctx.endpoint().flow::<EngineAbortAckControl>()?;
                ack.send(&()).await?;

                baker_firmware::mark_runtime_ready();
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.endpoint().recv::<EngineAbortBeginControl>().await?;

                let abort = ctx.endpoint().recv::<EngineAbortMsg>().await?;
                if abort.reason() != EngineAbortReason::FuelExhausted {
                    return Err(FailSafeError::RuntimeViolation);
                }

                baker_firmware::mark_safe_state();

                let fence = ctx.endpoint().flow::<EngineAbortFenceControl>()?;
                fence.send(&()).await?;

                ctx.endpoint().recv::<EngineAbortAckControl>().await?;

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<FailSafe as BakerCapsuleFacts>::SUCCESS_RESULT);
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<FailSafe, I> for BakerArtifacts
where
    I: appkit::LogicalImage<FailSafe, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<FailSafe>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<FailSafe>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<FailSafe>()
}
