#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::convert::Infallible;

use baker_firmware::{
    BakerArtifacts, BakerCapsuleFacts, BakerChoreoFsRouteBreak, BakerChoreoFsRouteContinue,
    BakerPlacement, DriverImage, EngineImage, FD_WRITE_RIGHT, WASM_CHOREOFS_TRAFFIC,
};
use hibana::g;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineReq, EngineRet, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
        LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASI_PROC_EXIT,
    },
};

const GREEN_LED: appkit::ObjectSpec = appkit::ObjectSpec::new(
    b"device/led/green",
    appkit::ObjectId(1),
    appkit::FdSpec::new(3, FD_WRITE_RIGHT, 1),
);
const YELLOW_LED: appkit::ObjectSpec = appkit::ObjectSpec::new(
    b"device/led/yellow",
    appkit::ObjectId(2),
    appkit::FdSpec::new(4, FD_WRITE_RIGHT, 1),
);
const RED_LED: appkit::ObjectSpec = appkit::ObjectSpec::new(
    b"device/led/red",
    appkit::ObjectId(3),
    appkit::FdSpec::new(5, FD_WRITE_RIGHT, 1),
);
static OBJECT_FACTS: appkit::ObjectSpecSet<3> =
    appkit::ObjectSpecSet::new([GREEN_LED, YELLOW_LED, RED_LED]);

pub struct ChoreoFsTrafficLoop;
pub struct ChoreoFsTrafficLoopLocal;

impl appkit::Capsule for ChoreoFsTrafficLoop {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = ChoreoFsTrafficLoopLocal;
    type Report = Infallible;

    fn choreography() -> impl hibana::substrate::program::Projectable<Self::Universe> {
        let path_open = || {
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 1>(),
            )
        };
        let open_leds = || g::seq(path_open(), g::seq(path_open(), path_open()));
        let write_wait = || {
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(
                    ),
                    g::seq(
                        g::send::<
                            g::Role<1>,
                            g::Role<0>,
                            g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>,
                            1,
                        >(),
                        g::send::<
                            g::Role<0>,
                            g::Role<1>,
                            g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>,
                            1,
                        >(),
                    ),
                ),
            )
        };
        let admitted_cycle = || {
            g::route(
                g::seq(
                    g::send::<g::Role<1>, g::Role<1>, BakerChoreoFsRouteContinue, 1>(),
                    write_wait(),
                ),
                g::seq(
                    g::send::<g::Role<1>, g::Role<1>, BakerChoreoFsRouteBreak, 1>(),
                    g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
                ),
            )
        };
        g::seq(open_leds(), admitted_cycle())
    }
}

impl BakerCapsuleFacts for ChoreoFsTrafficLoop {
    const CHOREOFS_VISUAL_LOOP: bool = true;
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::WasiImage<'static>;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(12);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(13);

    fn driver_facts() -> appkit::DriverFacts<'static> {
        OBJECT_FACTS.driver_facts()
    }
}

impl appkit::Localside<ChoreoFsTrafficLoop> for ChoreoFsTrafficLoopLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            #[cfg(feature = "wasm-engine-core")]
            {
                if ROLE == 1 && ctx.artifact_len() != 0 {
                    return baker_firmware::baker_drive_wasi_engine(ctx).await;
                }
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        async move {
            if ROLE == 0 && !ctx.choreofs().entries().is_empty() {
                return baker_firmware::baker_choreofs_driver(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = Infallible> {
        ctx.pending()
    }
}

impl appkit::ArtifactForImage<ChoreoFsTrafficLoop, DriverImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::NoWasi {
        appkit::NoWasi
    }
}

impl appkit::ArtifactForImage<ChoreoFsTrafficLoop, EngineImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_CHOREOFS_TRAFFIC)
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
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}
