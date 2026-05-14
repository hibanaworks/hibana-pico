#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::convert::Infallible;

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl, EngineAbortMsg,
    },
};

pub struct Recovery;
pub struct RecoveryLocal;

impl appkit::Capsule for Recovery {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = RecoveryLocal;
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

impl BakerCapsuleFacts for Recovery {
    const SUCCESS_RESULT: u32 = baker_firmware::RESULT_RECOVERY_OK;
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::NoWasi;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(30);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(31);
}

impl appkit::Localside<Recovery> for RecoveryLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, Recovery, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 1 {
                return baker_firmware::baker_control_engine_one_cycle(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, Recovery, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 0 {
                return baker_firmware::baker_control_driver_one_cycle(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, Recovery, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, Recovery, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, Recovery, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<Recovery, I> for BakerArtifacts
where
    I: appkit::LogicalImage<Recovery, Artifact = appkit::NoWasi>,
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
    baker_firmware::run::<Recovery>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<Recovery>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<Recovery>()
}
