#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{appkit, choreography::protocol::EngineAbortBeginControl};

pub struct DeadlineFault;
pub struct DeadlineFaultLocal;

impl appkit::Capsule for DeadlineFault {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = DeadlineFaultLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
        g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>()
    }
}

impl BakerCapsuleFacts for DeadlineFault {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(54);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(55);
    const OPERATIONAL_DEADLINE_TICKS: u32 = 2;
}

impl appkit::Localside<DeadlineFault> for DeadlineFaultLocal {
    type Error = hibana::EndpointError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, DeadlineFault, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, DeadlineFault, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.endpoint().recv::<EngineAbortBeginControl>().await?;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, DeadlineFault, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, DeadlineFault, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, DeadlineFault, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<DeadlineFault, I> for BakerArtifacts
where
    I: appkit::LogicalImage<DeadlineFault, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<DeadlineFault>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<DeadlineFault>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<DeadlineFault>()
}
