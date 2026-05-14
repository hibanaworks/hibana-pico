#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::convert::Infallible;

use baker_firmware::{BakerArtifacts, BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl, EngineAbortMsg,
        LABEL_MEM_FENCE, MemFence,
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
        ctx: appkit::EngineCtx<'endpoint, 'guest, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 1 {
                return baker_firmware::baker_many_reentry_engine(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, ManyReentry, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 0 {
                return baker_firmware::baker_many_reentry_driver(ctx).await;
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
