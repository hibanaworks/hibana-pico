#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;

const LABEL_ENGINE_ABORT_BEGIN: u8 = 129;
const LABEL_ENGINE_ABORT_FENCE: u8 = 131;

type EngineAbortBegin = g::Msg<LABEL_ENGINE_ABORT_BEGIN, ()>;
type EngineAbortFence = g::Msg<LABEL_ENGINE_ABORT_FENCE, ()>;

struct EndpointFault;
struct EndpointFaultLocal;

impl appkit::Capsule for EndpointFault {
    type Placement = BakerPlacement;
    type Localside = EndpointFaultLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::send::<1, 0, EngineAbortBegin>()
    }
}

impl BakerCapsuleFacts for EndpointFault {
    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<EndpointFault> for EndpointFaultLocal {
    type Error = hibana::EndpointError;

    fn engine<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                ctx.recv::<EngineAbortFence>().await?;
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
    baker_firmware::run::<EndpointFault>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<EndpointFault>()
}
