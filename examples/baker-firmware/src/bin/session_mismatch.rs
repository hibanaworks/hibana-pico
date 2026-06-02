#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{
    BakerArtifacts, BakerCapsuleFacts, BakerPlacement, RESULT_SESSION_MISMATCH_OK,
};
use hibana::g;
use hibana_pico::{appkit, choreography::protocol::EngineAbortFenceControl};

pub struct SessionMismatch;
pub struct SessionMismatchLocal;

impl appkit::Capsule for SessionMismatch {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = SessionMismatchLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable {
        g::send::<0, 1, EngineAbortFenceControl, 0>()
    }
}

impl BakerCapsuleFacts for SessionMismatch {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(56);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(57);
    const SUCCESS_RESULT: u32 = RESULT_SESSION_MISMATCH_OK;
    const SIO_ROLE0_SESSION_XOR: u32 = 0x1111_0000;
}

impl appkit::Localside<SessionMismatch> for SessionMismatchLocal {
    type Error = hibana::EndpointError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                ctx.endpoint().recv::<EngineAbortFenceControl>().await?;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                let flow = ctx.endpoint().flow::<EngineAbortFenceControl>()?;
                flow.send(&()).await?;
                baker_firmware::mark_success(
                    <SessionMismatch as BakerCapsuleFacts>::SUCCESS_RESULT,
                );
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<SessionMismatch, I> for BakerArtifacts
where
    I: appkit::LogicalImage<SessionMismatch, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<SessionMismatch>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<SessionMismatch>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<SessionMismatch>()
}
