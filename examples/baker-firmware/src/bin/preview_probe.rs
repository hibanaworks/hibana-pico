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

pub struct PreviewProbe;
pub struct PreviewProbeLocal;

#[derive(Debug)]
pub enum PreviewProbeError {
    Endpoint(hibana::EndpointError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for PreviewProbeError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl appkit::Capsule for PreviewProbe {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = PreviewProbeLocal;
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

impl BakerCapsuleFacts for PreviewProbe {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(52);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(53);
    const SUCCESS_RESULT: u32 = baker_firmware::RESULT_PREVIEW_PROBE_OK;
}

impl appkit::Localside<PreviewProbe> for PreviewProbeLocal {
    type Error = PreviewProbeError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, PreviewProbe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                let preview = ctx.endpoint().flow::<EngineAbortBeginControl>()?;
                core::mem::drop(preview);

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
        mut ctx: appkit::DriverCtx<'a, PreviewProbe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.endpoint().recv::<EngineAbortBeginControl>().await?;

                let abort = ctx.endpoint().recv::<EngineAbortMsg>().await?;
                if abort.reason() != EngineAbortReason::FuelExhausted {
                    return Err(PreviewProbeError::RuntimeViolation);
                }

                baker_firmware::mark_safe_state();

                let fence = ctx.endpoint().flow::<EngineAbortFenceControl>()?;
                fence.send(&()).await?;

                ctx.endpoint().recv::<EngineAbortAckControl>().await?;

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<PreviewProbe as BakerCapsuleFacts>::SUCCESS_RESULT);
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, PreviewProbe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, PreviewProbe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, PreviewProbe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<PreviewProbe, I> for BakerArtifacts
where
    I: appkit::LogicalImage<PreviewProbe, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<PreviewProbe>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<PreviewProbe>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<PreviewProbe>()
}
