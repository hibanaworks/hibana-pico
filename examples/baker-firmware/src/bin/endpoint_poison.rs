#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{appkit, choreography::protocol::EngineAbortBeginControl};

pub struct EndpointPoison;
pub struct EndpointPoisonLocal;

impl appkit::Capsule for EndpointPoison {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = EndpointPoisonLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
        g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>()
    }
}

impl BakerCapsuleFacts for EndpointPoison {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(54);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(55);
}

impl appkit::Localside<EndpointPoison> for EndpointPoisonLocal {
    type Error = hibana::EndpointError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, EndpointPoison, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                match ctx.endpoint().offer().await {
                    Ok(_) => panic!("offer at send-only phase must not produce continuation"),
                    Err(error) => baker_firmware::record_endpoint_error(&error),
                }

                match ctx.endpoint().flow::<EngineAbortBeginControl>() {
                    Ok(_) => panic!("poisoned generation must not produce a flow continuation"),
                    Err(error) => {
                        baker_firmware::record_endpoint_error(&error);
                        return Err(error);
                    }
                }
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, EndpointPoison, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, EndpointPoison, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, EndpointPoison, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, EndpointPoison, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<EndpointPoison, I> for BakerArtifacts
where
    I: appkit::LogicalImage<EndpointPoison, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<EndpointPoison>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<EndpointPoison>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<EndpointPoison>()
}
