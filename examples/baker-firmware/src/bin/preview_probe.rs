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

struct PreviewProbe;
struct PreviewProbeLocal;

const RESULT_PREVIEW_PROBE_OK: u32 = 0x4849_5050;

#[derive(Debug)]
enum PreviewProbeError {
    Endpoint,
}

impl From<hibana::EndpointError> for PreviewProbeError {
    fn from(_: hibana::EndpointError) -> Self {
        Self::Endpoint
    }
}

impl appkit::Capsule for PreviewProbe {
    type Placement = BakerPlacement;
    type Localside = PreviewProbeLocal;

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

impl BakerCapsuleFacts for PreviewProbe {
    const SUCCESS_RESULT: u32 = RESULT_PREVIEW_PROBE_OK;

    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<PreviewProbe> for PreviewProbeLocal {
    type Error = PreviewProbeError;

    fn engine<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                let preview = ctx.send::<EngineAbortBegin>(&());
                core::mem::drop(preview);

                ctx.send::<EngineAbortBegin>(&()).await?;

                ctx.send::<EngineAbortMsg>(&()).await?;

                ctx.recv::<EngineAbortFence>().await?;

                ctx.send::<EngineAbortAck>(&()).await?;

                baker_firmware::mark_runtime_ready();
                return appkit::pending(ctx).await;
            }
            appkit::pending(ctx).await
        }
    }

    fn driver<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                ctx.recv::<EngineAbortBegin>().await?;

                ctx.recv::<EngineAbortMsg>().await?;

                baker_firmware::mark_safe_state();

                ctx.send::<EngineAbortFence>(&()).await?;

                ctx.recv::<EngineAbortAck>().await?;

                baker_firmware::mark_runtime_ready();
                baker_firmware::mark_success(<PreviewProbe as BakerCapsuleFacts>::SUCCESS_RESULT);
                return appkit::pending(ctx).await;
            }
            appkit::pending(ctx).await
        }
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
    baker_firmware::run::<PreviewProbe>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<PreviewProbe>()
}
