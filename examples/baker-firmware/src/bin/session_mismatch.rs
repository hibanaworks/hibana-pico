#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;
#[cfg(feature = "wasm-engine-core")]
use hibana_wasip1_runtime::protocol::BudgetRun;
use hibana_wasip1_runtime::protocol::{
    EngineReq, EngineRet, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
    LABEL_WASI_PATH_OPEN_RET, PathOpen, PathOpened,
};

const DEVICE_PREOPEN_FD: u8 = 9;
const SESSION_MISMATCH_FD: u8 = 3;
const SESSION_MISMATCH_OBJECT: appkit::ObjectId = appkit::ObjectId(4);
const FD_WRITE_RIGHT: u64 = 1 << 6;
const RESULT_SESSION_MISMATCH_OK: u32 = 0x4849_534d;

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_SESSION_MISMATCH: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-session-mismatch-fd-write.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_SESSION_MISMATCH: &[u8] = &[];

const SESSION_MISMATCH_FILE: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/session-mismatch",
    SESSION_MISMATCH_OBJECT,
    appkit::FdSpec::new(SESSION_MISMATCH_FD as u32, FD_WRITE_RIGHT, 1),
);
static OBJECT_FACTS: appkit::ChoreoFsObjectSet<1> =
    appkit::ChoreoFsObjectSet::new([SESSION_MISMATCH_FILE]);

struct SessionMismatch;
struct SessionMismatchLocal;

impl appkit::Capsule for SessionMismatch {
    type Placement = BakerPlacement;
    type Local = SessionMismatchLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::seq(
            g::send::<1, 0, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>(),
            g::seq(
                g::send::<0, 1, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>(),
                g::seq(
                    g::send::<1, 0, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>(),
                    g::send::<0, 1, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>(),
                ),
            ),
        )
    }

    fn observe(tap: &mut hibana::runtime::tap::TapPort<'_>) {
        baker_firmware::poll_epf_diagnostic(tap);
    }

    #[cfg(feature = "wasm-engine-core")]
    const WASI_GUEST_DRIVE: appkit::WasiGuestDrive = appkit::WasiGuestDrive::Localside;
}

impl BakerCapsuleFacts for SessionMismatch {
    const SUCCESS_RESULT: u32 = RESULT_SESSION_MISMATCH_OK;
    const SIO_OPERATIONAL_DEADLINE_TICKS: u32 = 1000;
    const SIO_ROLE0_TX_SESSION_XOR: u32 = 0x1111_0000;

    fn run_engine_image() {
        baker_firmware::run_engine_wasi::<Self>(appkit::WasiImage::from_static(
            WASM_SESSION_MISMATCH,
        ));
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        OBJECT_FACTS.driver_facts()
    }
}

#[derive(Debug)]
enum SessionMismatchError {
    Endpoint,
    #[cfg(feature = "wasm-engine-core")]
    Wasi,
}

impl From<hibana::EndpointError> for SessionMismatchError {
    fn from(_: hibana::EndpointError) -> Self {
        Self::Endpoint
    }
}

#[cfg(feature = "wasm-engine-core")]
impl From<appkit::WasiGuestError> for SessionMismatchError {
    fn from(_: appkit::WasiGuestError) -> Self {
        Self::Wasi
    }
}

impl appkit::Localside<SessionMismatch> for SessionMismatchLocal {
    type Error = SessionMismatchError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            #[cfg(feature = "wasm-engine-core")]
            let mut ctx = ctx;
            if ROLE == 1 {
                #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
                {
                    let mut attempts = 0usize;
                    while attempts < 8 {
                        let status = ctx
                            .drive_wasi_guest_once(BudgetRun::new(1, 0, 2048))
                            .await?;
                        if !matches!(status, appkit::WasiGuestStatus::BudgetExpired(_)) {
                            break;
                        }
                        attempts += 1;
                    }
                }
                #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
                {
                    let mut attempts = 0usize;
                    while attempts < 8 {
                        let status =
                            match ctx.drive_wasi_guest_once_blocking(BudgetRun::new(1, 0, 2048)) {
                                Ok(status) => status,
                                Err(error) => {
                                    baker_firmware::record_choreofs_engine_error_code(0x4550_0001);
                                    core::hint::black_box(&error);
                                    break;
                                }
                            };
                        if !matches!(status, appkit::WasiGuestStatus::BudgetExpired(_)) {
                            break;
                        }
                        attempts += 1;
                    }
                }
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                let open = ctx
                    .endpoint()
                    .recv::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
                    .await?;
                let EngineReq::PathOpen(open) = open else {
                    core::hint::black_box(open);
                    return ctx.pending().await;
                };
                driver_path_open(&mut ctx, open).await?;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, SessionMismatch, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

async fn driver_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SessionMismatch, ROLE>,
    request: PathOpen,
) -> Result<(), SessionMismatchError> {
    if request.preopen_fd() != DEVICE_PREOPEN_FD || request.rights_base() != FD_WRITE_RIGHT {
        core::hint::black_box(request);
        return Ok(());
    }
    let object = match ctx.choreofs().resolve(request.path()) {
        Some(object) => object,
        None => {
            core::hint::black_box(request);
            return Ok(());
        }
    };
    if object != SESSION_MISMATCH_OBJECT {
        core::hint::black_box(object);
        return Ok(());
    }
    let fact = match ctx.ledger().fd(SESSION_MISMATCH_FD as u32) {
        Some(fact) if fact.object() == object && fact.rights() == FD_WRITE_RIGHT => fact,
        Some(fact) => {
            core::hint::black_box(fact);
            return Ok(());
        }
        None => {
            core::hint::black_box(request);
            return Ok(());
        }
    };
    let reply = EngineRet::PathOpened(PathOpened::new(fact.fd() as u8, 0));
    ctx.endpoint()
        .send::<g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>(&reply)
        .await?;
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn baker_selected_run() -> ! {
    baker_firmware::run::<SessionMismatch>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    baker_firmware::run::<SessionMismatch>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<SessionMismatch>()
}
