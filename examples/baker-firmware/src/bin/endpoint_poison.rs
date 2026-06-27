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
    type Localside = EndpointPoisonLocal;

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

    fn engine<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                match ctx.offer().await {
                    Ok(_) => panic!("offer at send-only phase must not produce continuation"),
                    Err(error) => record_endpoint_error(&error),
                }

                match ctx.send::<EngineAbortBegin>(&()).await {
                    Ok(_) => panic!("poisoned generation must not send"),
                    Err(error) => {
                        record_endpoint_error(&error);
                        return Err(error);
                    }
                }
            }
            appkit::pending(ctx).await
        }
    }

    fn driver<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
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
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<EndpointPoison>()
}
