#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;

const LABEL_ENGINE_ABORT_BEGIN: u8 = 129;
const LABEL_ENGINE_ABORT_REASON: u8 = 130;
const LABEL_ENGINE_ABORT_FENCE: u8 = 131;
const LABEL_ENGINE_ABORT_ACK: u8 = 132;

type EngineAbortBegin = g::Msg<LABEL_ENGINE_ABORT_BEGIN, ()>;
type EngineAbortMsg = g::Msg<LABEL_ENGINE_ABORT_REASON, ()>;
type EngineAbortFence = g::Msg<LABEL_ENGINE_ABORT_FENCE, ()>;
type EngineAbortAck = g::Msg<LABEL_ENGINE_ABORT_ACK, ()>;

struct FailSafe;
struct FailSafeLocal;

const RESULT_FAIL_SAFE_OK: u32 = 0x4849_4653;

#[derive(Debug)]
enum FailSafeError {
    Endpoint,
}

impl From<hibana::EndpointError> for FailSafeError {
    fn from(_: hibana::EndpointError) -> Self {
        Self::Endpoint
    }
}

impl appkit::Capsule for FailSafe {
    type Placement = BakerPlacement;
    type Local = FailSafeLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::seq(
            g::send::<1, 0, EngineAbortBegin>(),
            g::seq(
                g::send::<1, 0, EngineAbortMsg>(),
                g::seq(
                    g::send::<0, 1, EngineAbortFence>(),
                    g::send::<1, 0, EngineAbortAck>(),
                ),
            ),
        )
    }
}

impl BakerCapsuleFacts for FailSafe {
    const SUCCESS_RESULT: u32 = RESULT_FAIL_SAFE_OK;

    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<FailSafe> for FailSafeLocal {
    type Error = FailSafeError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                ctx.endpoint().send::<EngineAbortBegin>(&()).await?;

                ctx.endpoint().send::<EngineAbortMsg>(&()).await?;

                ctx.endpoint().recv::<EngineAbortFence>().await?;

                ctx.endpoint().send::<EngineAbortAck>(&()).await?;

                baker_firmware::mark_runtime_ready();
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, FailSafe, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.endpoint().recv::<EngineAbortBegin>().await?;

                ctx.endpoint().recv::<EngineAbortMsg>().await?;

                baker_firmware::mark_safe_state();

                ctx.endpoint().send::<EngineAbortFence>(&()).await?;

                ctx.endpoint().recv::<EngineAbortAck>().await?;

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<FailSafe as BakerCapsuleFacts>::SUCCESS_RESULT);
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, FailSafe, ROLE>,
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
    baker_firmware::run::<FailSafe>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<FailSafe>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<FailSafe>()
}
