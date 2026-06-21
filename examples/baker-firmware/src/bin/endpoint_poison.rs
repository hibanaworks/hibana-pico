#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;

const LABEL_ENGINE_ABORT_BEGIN: u8 = 129;
type EngineAbortBegin = g::Msg<LABEL_ENGINE_ABORT_BEGIN, ()>;

struct EndpointPoison;
struct EndpointPoisonLocal;

fn record_endpoint_error(error: &hibana::EndpointError) {
    core::hint::black_box(error);
    baker_firmware::record_choreofs_engine_error_code(0x5745_0f00);
}

impl appkit::Capsule for EndpointPoison {
    type Placement = BakerPlacement;
    type Local = EndpointPoisonLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::send::<1, 0, EngineAbortBegin>()
    }
}

impl BakerCapsuleFacts for EndpointPoison {
    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
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
                    Err(error) => record_endpoint_error(&error),
                }

                match ctx.endpoint().send::<EngineAbortBegin>(&()).await {
                    Ok(_) => panic!("poisoned generation must not send"),
                    Err(error) => {
                        record_endpoint_error(&error);
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
