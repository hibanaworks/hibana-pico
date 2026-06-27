#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;
use hibana_wasip1_runtime::choreofs;
use hibana_wasip1_runtime::protocol::{
    FdBinding, FdCloseReqMsg, FdCloseRetMsg, FdFdstatGetReqMsg, FdFdstatGetRetMsg, FdPrestat,
    FdPrestatDirNameDone, FdPrestatDirNameReqMsg, FdPrestatDirNameRetMsg, FdPrestatGetReqMsg,
    FdPrestatGetRetMsg, FdStat, FdStatRet, FdWrite, FdWriteReqMsg, FdWriteRetMsg, FdWriteRow,
    MemRights, PathOpen, PathOpenReqMsg, PathOpenRetMsg,
};

const ROOT_PREOPEN_FD: u8 = 3;
const SESSION_MISMATCH_FD: u8 = 4;
const SESSION_MISMATCH_OBJECT: choreofs::ObjectId = choreofs::ObjectId(4);
const ROOT_PREOPEN_NAME: &[u8] = b"/";
const FD_WRITE_RIGHT: u64 = 1 << 6;
const SESSION_MISMATCH_PAYLOAD: &[u8] = b"session mismatch\n";
const ERRNO_BADF: u16 = 8;
const RESULT_SESSION_MISMATCH_OK: u32 = 0x4849_534d;

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_SESSION_MISMATCH: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-session-mismatch-fd-write.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_SESSION_MISMATCH: &[u8] = &[];

const SESSION_MISMATCH_FILE: choreofs::ChoreoFsObject = choreofs::ChoreoFsObject::writable(
    b"device/session-mismatch",
    SESSION_MISMATCH_OBJECT,
    choreofs::FdSpec::new(SESSION_MISMATCH_FD as u32, FD_WRITE_RIGHT, 1),
    FdBinding::write(FdWriteRow::Base),
);
static OBJECT_FACTS: choreofs::ChoreoFsObjectSet<1> =
    choreofs::ChoreoFsObjectSet::new([SESSION_MISMATCH_FILE]);

struct SessionMismatch;
struct SessionMismatchLocal;

impl appkit::Capsule for SessionMismatch {
    type Placement = BakerPlacement;
    type Localside = SessionMismatchLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        let fd_prestat_get = || {
            g::seq(
                g::send::<1, 0, FdPrestatGetReqMsg>(),
                g::send::<0, 1, FdPrestatGetRetMsg>(),
            )
        };
        let fd_prestat_dir_name = || {
            g::seq(
                g::send::<1, 0, FdPrestatDirNameReqMsg>(),
                g::send::<0, 1, FdPrestatDirNameRetMsg>(),
            )
        };
        let fd_fdstat_get = || {
            g::seq(
                g::send::<1, 0, FdFdstatGetReqMsg>(),
                g::send::<0, 1, FdFdstatGetRetMsg>(),
            )
        };
        let path_open = || {
            g::seq(
                g::send::<1, 0, PathOpenReqMsg>(),
                g::send::<0, 1, PathOpenRetMsg>(),
            )
        };
        let fd_write = || {
            g::seq(
                g::send::<1, 0, FdWriteReqMsg>(),
                g::send::<0, 1, FdWriteRetMsg>(),
            )
        };
        let fd_close = || {
            g::seq(
                g::send::<1, 0, FdCloseReqMsg>(),
                g::send::<0, 1, FdCloseRetMsg>(),
            )
        };

        let startup = g::seq(
            fd_prestat_get(),
            g::seq(
                fd_prestat_dir_name(),
                g::seq(fd_prestat_get(), g::seq(fd_fdstat_get(), path_open())),
            ),
        );
        let body = g::seq(fd_write(), fd_close());

        g::seq(startup, body)
    }

    fn observe(tap: &mut hibana::runtime::tap::TapPort<'_>) {
        baker_firmware::poll_epf_diagnostic(tap);
    }
}

impl BakerCapsuleFacts for SessionMismatch {
    const SUCCESS_RESULT: u32 = RESULT_SESSION_MISMATCH_OK;
    const SIO_OPERATIONAL_DEADLINE_TICKS: u32 = 1000;
    const SIO_ROLE0_TX_SESSION_XOR: u32 = 0x1111_0000;

    fn run_engine_image() {
        baker_firmware::run_engine_wasi::<Self>(appkit::WasiImage::from_bytes(
            WASM_SESSION_MISMATCH,
        ));
    }

    fn choreofs() -> choreofs::ChoreoFs<'static> {
        OBJECT_FACTS.choreofs()
    }
}

#[derive(Debug)]
enum SessionMismatchError {
    Endpoint,
    RuntimeViolation,
}

impl From<hibana::EndpointError> for SessionMismatchError {
    fn from(_: hibana::EndpointError) -> Self {
        Self::Endpoint
    }
}

impl appkit::Localside<SessionMismatch> for SessionMismatchLocal {
    type Error = SessionMismatchError;

    fn engine<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn driver<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            let choreofs = OBJECT_FACTS.choreofs();
            if ROLE == 0 && !choreofs.facts().entries().is_empty() {
                match drive_session_mismatch_probe(&mut ctx, choreofs).await {
                    Ok(()) | Err(SessionMismatchError::Endpoint) => {}
                    Err(error) => return Err(error),
                }
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

async fn drive_session_mismatch_probe<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), SessionMismatchError> {
    drive_wasi_startup(ctx, choreofs).await?;
    let request = ctx.recv::<FdWriteReqMsg>().await?.0;
    driver_fd_write(ctx, choreofs, request).await?;
    handle_fd_close(ctx, choreofs).await
}

async fn drive_wasi_startup<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), SessionMismatchError> {
    handle_fd_prestat_get(ctx).await?;
    handle_fd_prestat_dir_name(ctx).await?;
    handle_fd_prestat_get(ctx).await?;
    handle_fd_fdstat_get(ctx, choreofs).await?;
    let request = ctx.recv::<PathOpenReqMsg>().await?.0;
    driver_path_open(ctx, choreofs, request).await?;
    Ok(())
}

async fn handle_fd_prestat_get<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<(), SessionMismatchError> {
    let request = ctx.recv::<FdPrestatGetReqMsg>().await?.0;
    let response = if request.fd() == ROOT_PREOPEN_FD {
        FdPrestat::new(request.fd(), ROOT_PREOPEN_NAME.len() as u8)
    } else {
        FdPrestat::new_with_errno(request.fd(), 0, ERRNO_BADF)
    };
    ctx.send::<FdPrestatGetRetMsg>(&hibana_wasip1_runtime::protocol::FdPrestatRet(response))
        .await?;
    Ok(())
}

async fn handle_fd_prestat_dir_name<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<(), SessionMismatchError> {
    let request = ctx.recv::<FdPrestatDirNameReqMsg>().await?.0;
    let response = if request.fd() == ROOT_PREOPEN_FD {
        FdPrestatDirNameDone::new(request.fd(), ROOT_PREOPEN_NAME, 0)
    } else {
        FdPrestatDirNameDone::new(request.fd(), b"", ERRNO_BADF)
    }
    .map_err(|_| SessionMismatchError::RuntimeViolation)?;
    ctx.send::<FdPrestatDirNameRetMsg>(&hibana_wasip1_runtime::protocol::FdPrestatDirNameRet(
        response,
    ))
    .await?;
    Ok(())
}

async fn handle_fd_fdstat_get<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), SessionMismatchError> {
    let request = ctx.recv::<FdFdstatGetReqMsg>().await?.0;
    ctx.send::<FdFdstatGetRetMsg>(&fd_stat_response(choreofs, request)?)
        .await?;
    Ok(())
}

async fn handle_fd_close<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), SessionMismatchError> {
    let request = ctx.recv::<FdCloseReqMsg>().await?.0;
    ctx.send::<FdCloseRetMsg>(&choreofs.fd_close(request))
        .await?;
    Ok(())
}

fn fd_stat_response(
    choreofs: choreofs::ChoreoFs<'static>,
    request: hibana_wasip1_runtime::protocol::FdRequest,
) -> Result<FdStatRet, SessionMismatchError> {
    match request.fd() {
        0 | ROOT_PREOPEN_FD => Ok(FdStatRet(FdStat::new(request.fd(), MemRights::Read))),
        1 | 2 => Ok(FdStatRet(FdStat::new(request.fd(), MemRights::Write))),
        SESSION_MISMATCH_FD => Ok(choreofs.fd_fdstat_get(request)),
        _ => Err(SessionMismatchError::RuntimeViolation),
    }
}

async fn driver_path_open<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
    request: PathOpen,
) -> Result<(), SessionMismatchError> {
    if request.preopen_fd() != ROOT_PREOPEN_FD {
        core::hint::black_box(request);
        return Err(SessionMismatchError::RuntimeViolation);
    }
    let normalized = PathOpen::new(
        request.preopen_fd(),
        request.rights_base(),
        normalize_path(request.path()),
    )
    .map_err(|_| SessionMismatchError::RuntimeViolation)?;
    let open = choreofs.path_open(normalized);
    if open.fd() != Some(SESSION_MISMATCH_FD) || open.object() != Some(SESSION_MISMATCH_OBJECT) {
        core::hint::black_box(open);
        return Err(SessionMismatchError::RuntimeViolation);
    }
    ctx.send::<PathOpenRetMsg>(&open.opened_ret()).await?;
    Ok(())
}

async fn driver_fd_write<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
    request: FdWrite,
) -> Result<(), SessionMismatchError> {
    if request.fd() != SESSION_MISMATCH_FD || request.as_bytes() != SESSION_MISMATCH_PAYLOAD {
        core::hint::black_box(request);
        return Err(SessionMismatchError::RuntimeViolation);
    }
    let write = choreofs.fd_write(request);
    if !write.is_writable() || write.object() != Some(SESSION_MISMATCH_OBJECT) {
        core::hint::black_box(write);
        return Err(SessionMismatchError::RuntimeViolation);
    }
    ctx.send::<FdWriteRetMsg>(&write.written()).await?;
    Ok(())
}

fn normalize_path(path: &[u8]) -> &[u8] {
    match path {
        [b'/', rest @ ..] => rest,
        _ => path,
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
    baker_firmware::run::<SessionMismatch>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<SessionMismatch>()
}
