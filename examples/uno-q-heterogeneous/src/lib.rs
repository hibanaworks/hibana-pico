#![cfg_attr(all(target_os = "none", not(test)), no_std)]

pub mod protocol;

use core::cell::{Cell, UnsafeCell};
use core::convert::Infallible;
use core::task::Poll;

use hibana::g;
use hibana::integration::{
    program::Projectable,
    runtime::LabelUniverse,
    wire::{CodecError, Payload, WirePayload},
};
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineReq, EngineRet, FdReadDone, FdWriteDone, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET,
        LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_IMPORT_LOOP_BREAK_CONTROL,
        LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_PROC_EXIT, PathOpened,
        WasiImportLoopBreak, WasiImportLoopContinue,
    },
    site,
};
use protocol::{
    FACE_ANGRY, FACE_HAPPY, FACE_MOUTH_CLOSED, FACE_MOUTH_ROUND, FACE_MOUTH_SMALL, FACE_MOUTH_WIDE,
    FACE_SAD, FACE_SURPRISED, FaceFrame, ROLE_M33_LED_KERNEL, ROLE_PSEUDO_LLM, ROLE_WASI_LLM_CELL,
};

pub struct UnoQCapsule;
pub struct UnoQPlacement;
pub struct UnoQLocal;
pub struct UnoQArtifacts;

#[derive(Clone, Copy, Debug, Default)]
pub struct UnoQLabelUniverse;

impl LabelUniverse for UnoQLabelUniverse {
    const MAX_LABEL: u8 = LABEL_WASI_IMPORT_LOOP_BREAK_CONTROL;
}

pub mod image {
    pub struct HostLoopbackProof;
    pub struct HardwarePeerProof;
    pub struct PseudoLlmProcess;
    pub struct WasiLlmCellProcess;
    pub struct M33LedKernelImage;
}

pub const UNO_Q_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(0x7101);
pub const PREOPEN_FD: u8 = 9;
pub const LLM_FRAME_FD: u8 = 12;
pub const FACE_FRAME_FD: u8 = 15;

const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const PROOF_CARRIER_ROLES: usize = 3;
const PROOF_CARRIER_QUEUE_DEPTH: usize = 24;
const PROOF_CARRIER_FRAME_BYTES: usize = 128;
const UART_CARRIER_MAGIC: [u8; 4] = *b"HBU1";
const UART_CARRIER_CHECK: u8 = 0xa7;
const UART_CARRIER_HEADER_BYTES: usize = 13;
const UART_CARRIER_FRAME_BYTES: usize = UART_CARRIER_HEADER_BYTES + PROOF_CARRIER_FRAME_BYTES + 1;
#[cfg(any(test, target_os = "none"))]
const UNO_Q_M33_HINT_DRAIN_TICKS: u32 = 2_000_000;
const HARDWARE_PEER_ROLE_BITS: u128 = (1u128 << ROLE_WASI_LLM_CELL) | (1u128 << ROLE_PSEUDO_LLM);
#[cfg(any(test, not(target_os = "none")))]
const UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS: u32 = 50_000;
#[cfg(any(test, target_os = "none"))]
const UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS: u32 = 1_000_000_000;
#[cfg_attr(target_os = "none", allow(dead_code))]
const UNO_Q_FACE_EMOTION_HOLD_US: u64 = 500_000;
#[cfg_attr(target_os = "none", allow(dead_code))]
const UNO_Q_FACE_MOUTH_HOLD_US: u64 = 250_000;
const UNO_Q_FACE_EMOTION_FRAMES: [u8; 12] = [
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
];
const UNO_Q_FACE_MOUTH_FRAMES: [u8; 8] = [
    FACE_MOUTH_CLOSED,
    FACE_MOUTH_SMALL,
    FACE_MOUTH_WIDE,
    FACE_MOUTH_ROUND,
    FACE_MOUTH_CLOSED,
    FACE_MOUTH_SMALL,
    FACE_MOUTH_WIDE,
    FACE_MOUTH_ROUND,
];
const UNO_Q_FACE_CYCLE_FRAME_COUNT: usize =
    UNO_Q_FACE_EMOTION_FRAMES.len() + UNO_Q_FACE_MOUTH_FRAMES.len();

const LLM_FRAME_PATH: &[u8] = b"llm/frame";
const FACE_FRAME_PATH: &[u8] = b"face/frame";

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
pub fn configure_uno_q_uart_modem_ready(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn ioctl(fd: i32, request: u64, argp: *const u32) -> i32;
    }

    const TIOCMBIS: u64 = 0x5416;
    const TIOCM_DTR: u32 = 0x002;
    const TIOCM_RTS: u32 = 0x004;

    let bits = TIOCM_DTR | TIOCM_RTS;
    let rc = unsafe { ioctl(file.as_raw_fd(), TIOCMBIS, &bits) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
fn drain_uno_q_uart_byte(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn tcdrain(fd: i32) -> i32;
    }

    let rc = unsafe { tcdrain(file.as_raw_fd()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
pub fn configure_uno_q_uart_modem_ready(_file: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
fn drain_uno_q_uart_byte(_file: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

pub const LLM_FRAME_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    LLM_FRAME_PATH,
    appkit::ObjectId(71_002),
    appkit::FdSpec::new(LLM_FRAME_FD as u32, FD_READ_RIGHT, 1),
);
pub const FACE_FRAME_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    FACE_FRAME_PATH,
    appkit::ObjectId(71_005),
    appkit::FdSpec::new(FACE_FRAME_FD as u32, FD_WRITE_RIGHT, 1),
);

static UNO_Q_DRIVER_FACTS: appkit::ChoreoFsObjectSet<1> =
    appkit::ChoreoFsObjectSet::new([FACE_FRAME_OBJECT]);

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_UNO_Q_LLM_FACE_CELL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-cell.wasm"
));
#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_UNO_Q_LLM_FACE_ROUTER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-router.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_UNO_Q_LLM_FACE_CELL: &[u8] = &[];
#[cfg(not(feature = "embed-wasip1-artifacts"))]
#[cfg_attr(target_os = "none", allow(dead_code))]
const WASM_UNO_Q_LLM_FACE_ROUTER: &[u8] = &[];

#[cfg(feature = "runtime-wasip1")]
static mut UNO_Q_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(feature = "runtime-wasip1")]
fn uno_q_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
    core::hint::black_box(ROLE);
    unsafe { (&mut *core::ptr::addr_of_mut!(UNO_Q_WASI_GUEST_ARENA)).lease() }
}

#[cfg(all(not(test), target_os = "none"))]
const UNO_Q_ATTACH_SLAB_BYTES: usize = 64 * 1024;
#[cfg(all(not(test), target_os = "none"))]
static HOST_PROOF_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static HARDWARE_PEER_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static PSEUDO_LLM_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static WASI_CELL_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static M33_LED_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

type WasiPathOpenReqMsg = g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>;
type WasiPathOpenRetMsg = g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>;
type WasiFdReadReqMsg = g::Msg<LABEL_WASI_FD_READ, EngineReq>;
type WasiFdReadRetMsg = g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>;
type WasiFdWriteReqMsg = g::Msg<LABEL_WASI_FD_WRITE, EngineReq>;
type WasiFdWriteRetMsg = g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>;
type WasiProcExitReqMsg = g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>;

#[derive(Debug)]
pub enum UnoQRuntimeError {
    Endpoint(hibana::EndpointError),
    Protocol(CodecError),
    LocalProtocol(protocol::ProtocolError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for UnoQRuntimeError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<CodecError> for UnoQRuntimeError {
    fn from(error: CodecError) -> Self {
        Self::Protocol(error)
    }
}

impl From<protocol::ProtocolError> for UnoQRuntimeError {
    fn from(error: protocol::ProtocolError) -> Self {
        Self::LocalProtocol(error)
    }
}

impl appkit::Capsule for UnoQCapsule {
    type Universe = UnoQLabelUniverse;
    type Placement = UnoQPlacement;
    type Local = UnoQLocal;
    type Report = Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        let frame_cycle = g::seq(
            g::send::<
                g::Role<ROLE_WASI_LLM_CELL>,
                g::Role<ROLE_WASI_LLM_CELL>,
                WasiImportLoopContinue,
                0,
            >(),
            g::seq(
                g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_PSEUDO_LLM>, WasiFdReadReqMsg, 0>(
                ),
                g::seq(
                    g::send::<
                        g::Role<ROLE_PSEUDO_LLM>,
                        g::Role<ROLE_WASI_LLM_CELL>,
                        WasiFdReadRetMsg,
                        0,
                    >(),
                    g::seq(
                        g::send::<
                            g::Role<ROLE_WASI_LLM_CELL>,
                            g::Role<ROLE_M33_LED_KERNEL>,
                            WasiFdWriteReqMsg,
                            1,
                        >(),
                        g::send::<
                            g::Role<ROLE_M33_LED_KERNEL>,
                            g::Role<ROLE_WASI_LLM_CELL>,
                            WasiFdWriteRetMsg,
                            1,
                        >(),
                    ),
                ),
            ),
        );
        let face_frame_loop = g::route(
            frame_cycle,
            g::seq(
                g::send::<
                    g::Role<ROLE_WASI_LLM_CELL>,
                    g::Role<ROLE_WASI_LLM_CELL>,
                    WasiImportLoopBreak,
                    0,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_WASI_LLM_CELL>,
                        g::Role<ROLE_PSEUDO_LLM>,
                        WasiProcExitReqMsg,
                        0,
                    >(),
                    g::send::<
                        g::Role<ROLE_WASI_LLM_CELL>,
                        g::Role<ROLE_M33_LED_KERNEL>,
                        WasiProcExitReqMsg,
                        1,
                    >(),
                ),
            ),
        );
        g::seq(
            g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_PSEUDO_LLM>, WasiPathOpenReqMsg, 0>(
            ),
            g::seq(
                g::send::<
                    g::Role<ROLE_PSEUDO_LLM>,
                    g::Role<ROLE_WASI_LLM_CELL>,
                    WasiPathOpenRetMsg,
                    0,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_WASI_LLM_CELL>,
                        g::Role<ROLE_M33_LED_KERNEL>,
                        WasiPathOpenReqMsg,
                        1,
                    >(),
                    g::seq(
                        g::send::<
                            g::Role<ROLE_M33_LED_KERNEL>,
                            g::Role<ROLE_WASI_LLM_CELL>,
                            WasiPathOpenRetMsg,
                            1,
                        >(),
                        face_frame_loop,
                    ),
                ),
            ),
        )
    }
}

impl appkit::Placement<UnoQCapsule> for UnoQPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            ROLE_WASI_LLM_CELL => appkit::RoleKind::Engine,
            ROLE_M33_LED_KERNEL => appkit::RoleKind::Driver,
            ROLE_PSEUDO_LLM => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn uno_q_m33_board_ready();
    fn uno_q_m33_board_show_face(face: u8);
    fn uno_q_m33_role_step(step: u32);
}

fn m33_board_ready() {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_board_ready();
    }
}

fn m33_board_show_face(face: u8) {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_board_show_face(face);
    }
    #[cfg(not(target_os = "none"))]
    core::hint::black_box(face);
}

fn m33_role_step(step: u32) {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_role_step(step);
    }
    #[cfg(not(target_os = "none"))]
    core::hint::black_box(step);
}

async fn run_m33_driver<const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    if ROLE != ROLE_M33_LED_KERNEL {
        return ctx.pending().await;
    }

    m33_role_step(0x0100);
    m33_board_ready();

    m33_role_step(0x0200);
    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    m33_role_step(0x0201);
    complete_path_open(&mut ctx, request, FACE_FRAME_PATH, FD_WRITE_RIGHT).await?;

    m33_role_step(0x0500);
    drive_face_frame_loop(&mut ctx).await?;
    m33_role_step(0x0501);
    ctx.pending().await
}

async fn run_boundary<const ROLE: u8>(
    mut ctx: appkit::BoundaryCtx<'_, UnoQCapsule, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    match ROLE {
        ROLE_PSEUDO_LLM => {
            let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
            complete_boundary_path_open(&mut ctx, request, LLM_FRAME_PATH, FD_READ_RIGHT).await?;
            let mut ordinal = 0u8;
            loop {
                pseudo_llm_wait_before_frame(ordinal);
                let branch = ctx.endpoint().offer().await?;
                match branch.label() {
                    LABEL_WASI_FD_READ => {
                        let read = expect_fd_read(branch.decode::<WasiFdReadReqMsg>().await?)?;
                        if read.fd() != LLM_FRAME_FD || read.max_len() < 2 {
                            return Err(UnoQRuntimeError::RuntimeViolation);
                        }
                        let frame = pseudo_llm_frame(ordinal as u8)?;
                        ctx.endpoint()
                            .flow::<WasiFdReadRetMsg>()?
                            .send(&EngineRet::FdReadDone(FdReadDone::new_with_lease(
                                read.fd(),
                                read.lease_id(),
                                &[frame.face(), frame.ordinal()],
                            )?))
                            .await?;
                        ordinal = ordinal.wrapping_add(1);
                    }
                    LABEL_WASI_PROC_EXIT => {
                        let proc_exit = branch.decode::<WasiProcExitReqMsg>().await?;
                        let EngineReq::ProcExit(status) = proc_exit else {
                            return Err(UnoQRuntimeError::RuntimeViolation);
                        };
                        if status.code() != 0 {
                            return Err(UnoQRuntimeError::RuntimeViolation);
                        }
                        break;
                    }
                    _ => return Err(UnoQRuntimeError::RuntimeViolation),
                }
            }
        }
        _ => {}
    }
    ctx.pending().await
}

async fn drive_face_frame_loop<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
) -> Result<(), UnoQRuntimeError> {
    let mut ordinal = 0u8;
    loop {
        m33_role_step(0x0d20_0000 | u32::from(ordinal));
        #[cfg(not(target_os = "none"))]
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!("uno-q-face passive offer ordinal={ordinal}");
        }
        let branch = match ctx.endpoint().offer().await {
            Ok(branch) => branch,
            Err(error) => {
                m33_role_step(0xed20_0000 | u32::from(ordinal));
                return Err(error.into());
            }
        };
        m33_role_step(0x0d21_0000 | (u32::from(ordinal) << 8) | u32::from(branch.label()));
        #[cfg(not(target_os = "none"))]
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!(
                "uno-q-face passive branch ordinal={ordinal} label={}",
                branch.label()
            );
        }
        match branch.label() {
            LABEL_WASI_FD_WRITE => {
                m33_role_step(0x0d22_0000 | u32::from(ordinal));
                let write = match branch.decode::<WasiFdWriteReqMsg>().await {
                    Ok(request) => expect_fd_write(request)?,
                    Err(error) => {
                        m33_role_step(0xed22_0000 | u32::from(ordinal));
                        return Err(error.into());
                    }
                };
                expect_fd_object(
                    &*ctx,
                    write.fd(),
                    FACE_FRAME_OBJECT.object(),
                    FD_WRITE_RIGHT,
                )?;
                let frame = FaceFrame::decode_payload(Payload::new(write.as_bytes()))?;
                m33_role_step(
                    0x0d23_0000 | (u32::from(frame.face()) << 8) | u32::from(frame.ordinal()),
                );
                if frame.ordinal() != ordinal {
                    m33_role_step(
                        0xed23_0000 | (u32::from(frame.ordinal()) << 8) | u32::from(ordinal),
                    );
                    return Err(UnoQRuntimeError::RuntimeViolation);
                }
                m33_role_step(0x0d24_0000 | u32::from(frame.face()));
                m33_board_show_face(frame.face());
                m33_role_step(
                    0x0d25_0000 | (u32::from(frame.face()) << 8) | u32::from(frame.ordinal()),
                );
                send_fd_write_done(ctx, write.fd(), write.len()).await?;
                ordinal = ordinal.wrapping_add(1);
            }
            LABEL_WASI_PROC_EXIT => {
                m33_role_step(0x0d26_0000 | u32::from(ordinal));
                let proc_exit = match branch.decode::<WasiProcExitReqMsg>().await {
                    Ok(request) => request,
                    Err(error) => {
                        m33_role_step(0xed26_0000 | u32::from(ordinal));
                        return Err(error.into());
                    }
                };
                let EngineReq::ProcExit(status) = proc_exit else {
                    m33_role_step(0xed26_1000 | u32::from(ordinal));
                    return Err(UnoQRuntimeError::RuntimeViolation);
                };
                if status.code() != 0 {
                    m33_role_step(0xed26_2000 | u32::from(status.code() as u8));
                    return Err(UnoQRuntimeError::RuntimeViolation);
                }
                m33_role_step(0x0d27_0000 | u32::from(ordinal));
                break;
            }
            label => {
                m33_role_step(0xed21_0000 | (u32::from(ordinal) << 8) | u32::from(label));
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
        }
    }
    Ok(())
}

fn pseudo_llm_wait_before_frame(ordinal: u8) {
    if ordinal == 0 {
        return;
    }
    if !face_loop_forever_enabled() {
        return;
    }
    #[cfg(not(target_os = "none"))]
    std::thread::sleep(std::time::Duration::from_micros(face_hold_us_for_ordinal(
        ordinal.wrapping_sub(1),
    )));
}

fn face_loop_forever_enabled() -> bool {
    #[cfg(not(target_os = "none"))]
    {
        std::env::var_os("UNO_Q_FACE_LOOP_FOREVER").is_some()
    }
    #[cfg(target_os = "none")]
    {
        false
    }
}

#[cfg_attr(target_os = "none", allow(dead_code))]
fn face_hold_us_for_ordinal(ordinal: u8) -> u64 {
    let index = usize::from(ordinal) % UNO_Q_FACE_CYCLE_FRAME_COUNT;
    let face = if index < UNO_Q_FACE_EMOTION_FRAMES.len() {
        UNO_Q_FACE_EMOTION_FRAMES[index]
    } else {
        UNO_Q_FACE_MOUTH_FRAMES[index - UNO_Q_FACE_EMOTION_FRAMES.len()]
    };
    face_hold_us(face)
}

#[cfg_attr(target_os = "none", allow(dead_code))]
fn face_hold_us(face: u8) -> u64 {
    match face {
        FACE_MOUTH_CLOSED | FACE_MOUTH_SMALL | FACE_MOUTH_WIDE | FACE_MOUTH_ROUND => {
            UNO_Q_FACE_MOUTH_HOLD_US
        }
        _ => UNO_Q_FACE_EMOTION_HOLD_US,
    }
}

fn pseudo_llm_frame(ordinal: u8) -> Result<FaceFrame, UnoQRuntimeError> {
    let index = usize::from(ordinal) % UNO_Q_FACE_CYCLE_FRAME_COUNT;
    let face = if index < UNO_Q_FACE_EMOTION_FRAMES.len() {
        UNO_Q_FACE_EMOTION_FRAMES[index]
    } else {
        UNO_Q_FACE_MOUTH_FRAMES[index - UNO_Q_FACE_EMOTION_FRAMES.len()]
    };
    FaceFrame::new(face, ordinal).map_err(Into::into)
}

impl appkit::Localside<UnoQCapsule> for UnoQLocal {
    type Error = UnoQRuntimeError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, UnoQCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, UnoQCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_m33_driver(ctx)
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, UnoQCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_boundary(ctx)
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, UnoQCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, UnoQCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

fn expect_path_open(
    request: EngineReq,
) -> Result<hibana_pico::choreography::protocol::PathOpen, UnoQRuntimeError> {
    match request {
        EngineReq::PathOpen(request) => Ok(request),
        _ => Err(UnoQRuntimeError::RuntimeViolation),
    }
}

fn expect_fd_write(
    request: EngineReq,
) -> Result<hibana_pico::choreography::protocol::FdWrite, UnoQRuntimeError> {
    match request {
        EngineReq::FdWrite(request) => Ok(request),
        _ => Err(UnoQRuntimeError::RuntimeViolation),
    }
}

fn expect_fd_read(
    request: EngineReq,
) -> Result<hibana_pico::choreography::protocol::FdRead, UnoQRuntimeError> {
    match request {
        EngineReq::FdRead(request) => Ok(request),
        _ => Err(UnoQRuntimeError::RuntimeViolation),
    }
}

async fn complete_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
    request: hibana_pico::choreography::protocol::PathOpen,
    expected_path: &[u8],
    expected_rights: u64,
) -> Result<(), UnoQRuntimeError> {
    if request.preopen_fd() != PREOPEN_FD || request.rights_base() != expected_rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    if request.path() != expected_path {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let Some(object) = ctx.choreofs().resolve(expected_path) else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    let Some(fd) = ctx
        .ledger()
        .fds()
        .iter()
        .copied()
        .find(|fact| fact.object() == object && fact.rights() == expected_rights)
    else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    let reply = EngineRet::PathOpened(PathOpened::new(fd.fd() as u8, 0));
    ctx.endpoint()
        .flow::<WasiPathOpenRetMsg>()?
        .send(&reply)
        .await?;
    Ok(())
}

async fn complete_boundary_path_open<const ROLE: u8>(
    ctx: &mut appkit::BoundaryCtx<'_, UnoQCapsule, ROLE>,
    request: hibana_pico::choreography::protocol::PathOpen,
    expected_path: &[u8],
    expected_rights: u64,
) -> Result<(), UnoQRuntimeError> {
    if request.preopen_fd() != PREOPEN_FD || request.rights_base() != expected_rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    if request.path() != expected_path {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let reply = EngineRet::PathOpened(PathOpened::new(LLM_FRAME_FD, 0));
    ctx.endpoint()
        .flow::<WasiPathOpenRetMsg>()?
        .send(&reply)
        .await?;
    Ok(())
}

fn expect_fd_object<const ROLE: u8>(
    ctx: &appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
    fd: u8,
    object: appkit::ObjectId,
    rights: u64,
) -> Result<(), UnoQRuntimeError> {
    let Some(fact) = ctx.ledger().fd(fd as u32) else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    if fact.object() != object || fact.rights() != rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    Ok(())
}

async fn send_fd_write_done<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
    fd: u8,
    len: usize,
) -> Result<(), UnoQRuntimeError> {
    let reply = EngineRet::FdWriteDone(FdWriteDone::new(fd, len as u8));
    ctx.endpoint()
        .flow::<WasiFdWriteRetMsg>()?
        .send(&reply)
        .await?;
    Ok(())
}

impl appkit::ArtifactForImage<UnoQCapsule, site::Local<image::HostLoopbackProof>>
    for UnoQArtifacts
{
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_UNO_Q_LLM_FACE_CELL)
    }
}

impl appkit::ArtifactForImage<UnoQCapsule, site::Local<image::HardwarePeerProof>>
    for UnoQArtifacts
{
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(uno_q_hardware_wasi_guest())
    }
}

impl appkit::ArtifactForImage<UnoQCapsule, site::Local<image::WasiLlmCellProcess>>
    for UnoQArtifacts
{
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(uno_q_hardware_wasi_guest())
    }
}

fn uno_q_hardware_wasi_guest() -> &'static [u8] {
    if face_loop_forever_enabled() {
        return WASM_UNO_Q_LLM_FACE_ROUTER;
    }
    WASM_UNO_Q_LLM_FACE_CELL
}

impl<I> appkit::ArtifactForImage<UnoQCapsule, I> for UnoQArtifacts
where
    I: appkit::LogicalImage<UnoQCapsule, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

pub struct ProofCarrier {
    queues: UnsafeCell<ProofQueues>,
}

pub struct ProofTx {
    local_role: u8,
    session_id: u32,
    lane: u8,
}

pub struct ProofRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    frame_label: Option<hibana::integration::transport::FrameLabel>,
    hint_frame_label: Cell<Option<hibana::integration::transport::FrameLabel>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

#[derive(Clone, Copy)]
struct ProofFrame {
    occupied: bool,
    lane: u8,
    frame_label: hibana::integration::transport::FrameLabel,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

impl ProofFrame {
    const EMPTY: Self = Self {
        occupied: false,
        lane: 0,
        frame_label: hibana::integration::transport::FrameLabel::new(0),
        len: 0,
        bytes: [0; PROOF_CARRIER_FRAME_BYTES],
    };
}

#[derive(Clone, Copy)]
struct ProofQueue {
    frames: [ProofFrame; PROOF_CARRIER_QUEUE_DEPTH],
    head: usize,
    len: usize,
}

impl ProofQueue {
    const EMPTY: Self = Self {
        frames: [ProofFrame::EMPTY; PROOF_CARRIER_QUEUE_DEPTH],
        head: 0,
        len: 0,
    };

    fn push_back(
        &mut self,
        lane: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        payload: Payload<'_>,
    ) -> Result<(), hibana::integration::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > PROOF_CARRIER_FRAME_BYTES || self.len == PROOF_CARRIER_QUEUE_DEPTH {
            return Err(hibana::integration::transport::TransportError::Failed);
        }
        let idx = (self.head + self.len) % PROOF_CARRIER_QUEUE_DEPTH;
        self.frames[idx].occupied = true;
        self.frames[idx].lane = lane;
        self.frames[idx].frame_label = frame_label;
        self.frames[idx].len = bytes.len();
        self.frames[idx].bytes[..bytes.len()].copy_from_slice(bytes);
        self.len += 1;
        Ok(())
    }

    fn push_front(
        &mut self,
        lane: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        bytes: &[u8],
    ) {
        if bytes.len() > PROOF_CARRIER_FRAME_BYTES || self.len == PROOF_CARRIER_QUEUE_DEPTH {
            return;
        }
        self.head = if self.head == 0 {
            PROOF_CARRIER_QUEUE_DEPTH - 1
        } else {
            self.head - 1
        };
        self.frames[self.head].occupied = true;
        self.frames[self.head].lane = lane;
        self.frames[self.head].frame_label = frame_label;
        self.frames[self.head].len = bytes.len();
        self.frames[self.head].bytes[..bytes.len()].copy_from_slice(bytes);
        self.len += 1;
    }

    fn pop_front(&mut self, lane: u8) -> Option<ProofFrame> {
        if self.len == 0 {
            return None;
        }
        let mut matched = None;
        let mut offset = 0usize;
        while offset < self.len {
            let idx = (self.head + offset) % PROOF_CARRIER_QUEUE_DEPTH;
            if self.frames[idx].occupied && self.frames[idx].lane == lane {
                matched = Some(idx);
                break;
            }
            offset += 1;
        }
        let idx = matched?;
        let frame = self.frames[idx];
        let tail = (self.head + self.len - 1) % PROOF_CARRIER_QUEUE_DEPTH;
        let mut cursor = idx;
        while cursor != tail {
            let next = (cursor + 1) % PROOF_CARRIER_QUEUE_DEPTH;
            self.frames[cursor] = self.frames[next];
            cursor = next;
        }
        self.frames[tail] = ProofFrame::EMPTY;
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
        if frame.occupied { Some(frame) } else { None }
    }

    fn front_label(&self, lane: u8) -> Option<hibana::integration::transport::FrameLabel> {
        let mut offset = 0usize;
        while offset < self.len {
            let idx = (self.head + offset) % PROOF_CARRIER_QUEUE_DEPTH;
            if self.frames[idx].occupied && self.frames[idx].lane == lane {
                return Some(self.frames[idx].frame_label);
            }
            offset += 1;
        }
        None
    }
}

struct ProofQueues {
    by_role: [ProofQueue; PROOF_CARRIER_ROLES],
}

impl ProofQueues {
    const EMPTY: Self = Self {
        by_role: [ProofQueue::EMPTY; PROOF_CARRIER_ROLES],
    };
}

impl ProofCarrier {
    pub const fn new() -> Self {
        Self {
            queues: UnsafeCell::new(ProofQueues::EMPTY),
        }
    }

    fn edit<R>(&self, f: impl FnOnce(&mut ProofQueues) -> R) -> R {
        unsafe { f(&mut *self.queues.get()) }
    }
}

#[derive(Clone, Copy)]
struct UartCarrierFrame {
    session_id: u32,
    lane: u8,
    source: u8,
    peer: u8,
    frame_label: hibana::integration::transport::FrameLabel,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

struct UartFrameParser {
    buffer: [u8; UART_CARRIER_FRAME_BYTES],
    len: usize,
}

impl UartFrameParser {
    const fn new() -> Self {
        Self {
            buffer: [0; UART_CARRIER_FRAME_BYTES],
            len: 0,
        }
    }

    fn push(&mut self, byte: u8) -> Option<UartCarrierFrame> {
        if self.len < UART_CARRIER_MAGIC.len() && byte != UART_CARRIER_MAGIC[self.len] {
            self.len = 0;
            return None;
        }
        if self.len == self.buffer.len() {
            self.len = 0;
            return None;
        }

        self.buffer[self.len] = byte;
        self.len += 1;

        if self.len < UART_CARRIER_HEADER_BYTES {
            return None;
        }

        let payload_len = self.buffer[12] as usize;
        if payload_len > PROOF_CARRIER_FRAME_BYTES {
            self.len = 0;
            return None;
        }
        let total_len = UART_CARRIER_HEADER_BYTES + payload_len + 1;
        if self.len < total_len {
            return None;
        }

        self.len = 0;
        let expected =
            uart_frame_checksum(&self.buffer[4..UART_CARRIER_HEADER_BYTES + payload_len]);
        if self.buffer[UART_CARRIER_HEADER_BYTES + payload_len] != expected {
            return None;
        }

        let mut bytes = [0u8; PROOF_CARRIER_FRAME_BYTES];
        bytes[..payload_len].copy_from_slice(
            &self.buffer[UART_CARRIER_HEADER_BYTES..UART_CARRIER_HEADER_BYTES + payload_len],
        );
        Some(UartCarrierFrame {
            session_id: u32::from_le_bytes([
                self.buffer[4],
                self.buffer[5],
                self.buffer[6],
                self.buffer[7],
            ]),
            lane: self.buffer[8],
            source: self.buffer[9],
            peer: self.buffer[10],
            frame_label: hibana::integration::transport::FrameLabel::new(self.buffer[11]),
            len: payload_len,
            bytes,
        })
    }
}

fn uart_frame_checksum(bytes: &[u8]) -> u8 {
    let mut check = UART_CARRIER_CHECK;
    for &byte in bytes {
        check ^= byte;
    }
    check
}

fn encode_uart_frame(
    out: &mut [u8; UART_CARRIER_FRAME_BYTES],
    session_id: u32,
    lane: u8,
    source: u8,
    peer: u8,
    frame_label: hibana::integration::transport::FrameLabel,
    payload: Payload<'_>,
) -> Result<usize, hibana::integration::transport::TransportError> {
    let bytes = payload.as_bytes();
    if bytes.len() > PROOF_CARRIER_FRAME_BYTES {
        return Err(hibana::integration::transport::TransportError::Failed);
    }
    out[..4].copy_from_slice(&UART_CARRIER_MAGIC);
    out[4..8].copy_from_slice(&session_id.to_le_bytes());
    out[8] = lane;
    out[9] = source;
    out[10] = peer;
    out[11] = frame_label.raw();
    out[12] = bytes.len() as u8;
    out[UART_CARRIER_HEADER_BYTES..UART_CARRIER_HEADER_BYTES + bytes.len()].copy_from_slice(bytes);
    let checksum = uart_frame_checksum(&out[4..UART_CARRIER_HEADER_BYTES + bytes.len()]);
    out[UART_CARRIER_HEADER_BYTES + bytes.len()] = checksum;
    Ok(UART_CARRIER_HEADER_BYTES + bytes.len() + 1)
}

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn uno_q_m33_carrier_write(byte: u8);
    fn uno_q_m33_carrier_read() -> i16;
    fn uno_q_m33_carrier_observe_frame(source: u8, peer: u8, label: u8, len: u8);
    fn uno_q_m33_carrier_observe_payload(label: u8, len: u8, byte0: u8, byte1: u8);
    fn uno_q_m33_carrier_observe_tx(peer: u8, label: u8, len: u8);
    fn uno_q_m33_carrier_observe_hint(lane: u8);
    fn uno_q_m33_board_poll();
    fn uno_q_m33_timer_ticks() -> u32;
}

#[cfg(target_os = "none")]
pub struct UnoQUartCarrier {
    queues: UnsafeCell<ProofQueues>,
    parser: UnsafeCell<UartFrameParser>,
}

#[cfg(target_os = "none")]
pub struct UnoQUartTx {
    local_role: u8,
    session_id: u32,
    lane: u8,
}

#[cfg(target_os = "none")]
pub struct UnoQUartRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    frame_label: Option<hibana::integration::transport::FrameLabel>,
    hint_frame_label: Cell<Option<hibana::integration::transport::FrameLabel>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

#[cfg(target_os = "none")]
impl UnoQUartCarrier {
    pub const fn new() -> Self {
        Self {
            queues: UnsafeCell::new(ProofQueues::EMPTY),
            parser: UnsafeCell::new(UartFrameParser::new()),
        }
    }

    fn edit<R>(&self, f: impl FnOnce(&mut ProofQueues) -> R) -> R {
        unsafe { f(&mut *self.queues.get()) }
    }

    fn service_board(&self) {
        unsafe {
            uno_q_m33_board_poll();
        }
    }

    fn drain_uart(&self, session_id: u32) {
        loop {
            self.service_board();
            let byte = unsafe { uno_q_m33_carrier_read() };
            if byte < 0 {
                break;
            }
            let frame = unsafe { (&mut *self.parser.get()).push(byte as u8) };
            let Some(frame) = frame else {
                continue;
            };
            if frame.session_id != session_id
                || frame.peer as usize >= PROOF_CARRIER_ROLES
                || frame.source as usize >= PROOF_CARRIER_ROLES
                || frame.source == frame.peer
            {
                continue;
            }
            unsafe {
                uno_q_m33_carrier_observe_frame(
                    frame.source,
                    frame.peer,
                    frame.frame_label.raw(),
                    frame.len as u8,
                );
            }
            self.edit(|queues| {
                queues.by_role[frame.peer as usize].push_back(
                    frame.lane,
                    frame.frame_label,
                    Payload::new(&frame.bytes[..frame.len]),
                )
            })
            .ok();
        }
    }
}

#[cfg(target_os = "none")]
impl hibana::integration::Transport for UnoQUartCarrier {
    type Error = hibana::integration::transport::TransportError;
    type Tx<'a>
        = UnoQUartTx
    where
        Self: 'a;
    type Rx<'a>
        = UnoQUartRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            UnoQUartTx {
                local_role,
                session_id,
                lane,
            },
            UnoQUartRx {
                local_role,
                session_id,
                lane,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        self.service_board();
        if tx.session_id == 0 || outgoing.peer() == tx.local_role || outgoing.lane() != tx.lane {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        let mut frame = [0u8; UART_CARRIER_FRAME_BYTES];
        let len = encode_uart_frame(
            &mut frame,
            tx.session_id,
            outgoing.lane(),
            tx.local_role,
            outgoing.peer(),
            outgoing.frame_label(),
            outgoing.payload(),
        )?;
        unsafe {
            uno_q_m33_carrier_observe_tx(
                outgoing.peer(),
                outgoing.frame_label().raw(),
                outgoing.payload().as_bytes().len() as u8,
            );
        }
        for &byte in &frame[..len] {
            unsafe {
                uno_q_m33_carrier_write(byte);
            }
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        self.drain_uart(rx.session_id);
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        let Some(frame) = self.edit(|queues| queues.by_role[local_role].pop_front(rx.lane)) else {
            task_context.waker().wake_by_ref();
            return Poll::Pending;
        };
        rx.frame_label = Some(frame.frame_label);
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        unsafe {
            let byte0 = if rx.len > 0 { rx.bytes[0] } else { 0 };
            let byte1 = if rx.len > 1 { rx.bytes[1] } else { 0 };
            uno_q_m33_carrier_observe_payload(
                frame.frame_label.raw(),
                frame.len as u8,
                byte0,
                byte1,
            );
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(Payload::new(&rx.bytes[..rx.len])))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.edit(|queues| {
                    queues.by_role[local_role].push_front(rx.lane, frame_label, &rx.bytes[..rx.len])
                });
            }
        }
        rx.hint_frame_label.set(None);
    }

    fn drain_events(
        &self,
        emit: &mut dyn FnMut(hibana::integration::transport::advanced::TransportEvent),
    ) {
        emit(
            hibana::integration::transport::advanced::TransportEvent::new(
                hibana::integration::transport::advanced::TransportEventKind::Ack,
                0,
                0,
                0,
            ),
        );
    }

    fn recv_frame_hint<'a>(
        &'a self,
        rx: &'a Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameLabel> {
        if let Some(frame_label) = rx.hint_frame_label.take() {
            return Some(frame_label);
        }
        unsafe {
            uno_q_m33_carrier_observe_hint(rx.lane);
        }
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return None;
        }
        let start_ticks = unsafe { uno_q_m33_timer_ticks() };
        loop {
            self.drain_uart(rx.session_id);
            if let Some(frame_label) =
                self.edit(|queues| queues.by_role[local_role].front_label(rx.lane))
            {
                return Some(frame_label);
            }
            let elapsed = unsafe { uno_q_m33_timer_ticks() }.wrapping_sub(start_ticks);
            if elapsed >= UNO_Q_M33_HINT_DRAIN_TICKS {
                break;
            }
            core::hint::spin_loop();
        }
        None
    }

    fn metrics(&self) -> Self::Metrics {}

    fn operational_deadline_ticks(&self) -> Option<u32> {
        Some(UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS)
    }

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box((interval_us, burst_bytes));
    }
}

#[cfg(not(target_os = "none"))]
pub struct HardwarePeerCarrier {
    local: ProofCarrier,
    serial_path: std::string::String,
    serial: std::sync::Mutex<std::fs::File>,
    parser: std::sync::Mutex<UartFrameParser>,
}

#[cfg(not(target_os = "none"))]
pub struct HardwarePeerTx {
    local_role: u8,
    session_id: u32,
    lane: u8,
}

#[cfg(not(target_os = "none"))]
pub struct HardwarePeerRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    frame_label: Option<hibana::integration::transport::FrameLabel>,
    hint_frame_label: Cell<Option<hibana::integration::transport::FrameLabel>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

#[cfg(not(target_os = "none"))]
impl HardwarePeerCarrier {
    pub fn new() -> Self {
        let path =
            std::env::var("UNO_Q_HIBANA_SERIAL").unwrap_or_else(|_| "/dev/ttyHS1".to_owned());
        let serial = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap_or_else(|error| panic!("failed to open hibana UART carrier {path}: {error}"));
        configure_uno_q_uart_modem_ready(&serial).unwrap_or_else(|error| {
            panic!("failed to assert DTR/RTS for hibana UART carrier {path}: {error}")
        });
        Self {
            local: ProofCarrier::new(),
            serial_path: path,
            serial: std::sync::Mutex::new(serial),
            parser: std::sync::Mutex::new(UartFrameParser::new()),
        }
    }

    fn drain_serial(
        &self,
        session_id: u32,
    ) -> Result<(), hibana::integration::transport::TransportError> {
        use std::io::Read;

        let mut bytes = [0u8; 64];
        let read_len = {
            let mut serial = self
                .serial
                .lock()
                .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
            match serial.read(&mut bytes) {
                Ok(len) => len,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    0
                }
                Err(_) => return Err(hibana::integration::transport::TransportError::Failed),
            }
        };
        if read_len == 0 {
            return Ok(());
        }

        let mut parser = self
            .parser
            .lock()
            .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
        for &byte in &bytes[..read_len] {
            let Some(frame) = parser.push(byte) else {
                continue;
            };
            if frame.session_id != session_id
                || frame.peer as usize >= PROOF_CARRIER_ROLES
                || frame.source as usize >= PROOF_CARRIER_ROLES
                || frame.source == frame.peer
            {
                continue;
            }
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-uart rx session={} lane={} {}->{} label={} len={}",
                    frame.session_id,
                    frame.lane,
                    frame.source,
                    frame.peer,
                    frame.frame_label.raw(),
                    frame.len
                );
            }
            self.local.edit(|queues| {
                queues.by_role[frame.peer as usize].push_back(
                    frame.lane,
                    frame.frame_label,
                    Payload::new(&frame.bytes[..frame.len]),
                )
            })?;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "none"))]
impl hibana::integration::Transport for HardwarePeerCarrier {
    type Error = hibana::integration::transport::TransportError;
    type Tx<'a>
        = HardwarePeerTx
    where
        Self: 'a;
    type Rx<'a>
        = HardwarePeerRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            HardwarePeerTx {
                local_role,
                session_id,
                lane,
            },
            HardwarePeerRx {
                local_role,
                session_id,
                lane,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        if tx.session_id == 0 || outgoing.peer() == tx.local_role || outgoing.lane() != tx.lane {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        if outgoing.peer() == ROLE_M33_LED_KERNEL {
            use std::io::Write;

            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-uart tx session={} lane={} {}->{} label={} len={}",
                    tx.session_id,
                    outgoing.lane(),
                    tx.local_role,
                    outgoing.peer(),
                    outgoing.frame_label().raw(),
                    outgoing.payload().as_bytes().len()
                );
            }
            let turnaround_us = std::env::var("UNO_Q_HIBANA_UART_TURNAROUND_US")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(50_000);
            if turnaround_us != 0 {
                std::thread::sleep(std::time::Duration::from_micros(turnaround_us));
            }
            let mut frame = [0u8; UART_CARRIER_FRAME_BYTES];
            let len = encode_uart_frame(
                &mut frame,
                tx.session_id,
                outgoing.lane(),
                tx.local_role,
                outgoing.peer(),
                outgoing.frame_label(),
                outgoing.payload(),
            )?;
            let mut serial = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.serial_path)
                .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
            configure_uno_q_uart_modem_ready(&serial)
                .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
            let byte_delay_us = std::env::var("UNO_Q_HIBANA_UART_BYTE_US")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(10_000);
            for &byte in &frame[..len] {
                serial
                    .write_all(&[byte])
                    .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
                serial
                    .flush()
                    .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
                drain_uno_q_uart_byte(&serial)
                    .map_err(|_| hibana::integration::transport::TransportError::Failed)?;
                if byte_delay_us != 0 {
                    std::thread::sleep(std::time::Duration::from_micros(byte_delay_us));
                }
            }
        } else {
            let peer = outgoing.peer() as usize;
            if peer >= PROOF_CARRIER_ROLES {
                return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
            }
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-local tx session={} lane={} {}->{} label={} len={}",
                    tx.session_id,
                    outgoing.lane(),
                    tx.local_role,
                    outgoing.peer(),
                    outgoing.frame_label().raw(),
                    outgoing.payload().as_bytes().len()
                );
            }
            self.local.edit(|queues| {
                queues.by_role[peer].push_back(
                    outgoing.lane(),
                    outgoing.frame_label(),
                    outgoing.payload(),
                )
            })?;
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        self.drain_serial(rx.session_id)?;
        let Some(frame) = self
            .local
            .edit(|queues| queues.by_role[local_role].pop_front(rx.lane))
        else {
            task_context.waker().wake_by_ref();
            return Poll::Pending;
        };
        rx.frame_label = Some(frame.frame_label);
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(Payload::new(&rx.bytes[..rx.len])))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.local.edit(|queues| {
                    queues.by_role[local_role].push_front(rx.lane, frame_label, &rx.bytes[..rx.len])
                });
            }
        }
        rx.hint_frame_label.set(None);
    }

    fn drain_events(
        &self,
        emit: &mut dyn FnMut(hibana::integration::transport::advanced::TransportEvent),
    ) {
        emit(
            hibana::integration::transport::advanced::TransportEvent::new(
                hibana::integration::transport::advanced::TransportEventKind::Ack,
                0,
                0,
                0,
            ),
        );
    }

    fn recv_frame_hint<'a>(
        &'a self,
        rx: &'a Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameLabel> {
        if let Some(frame_label) = rx.hint_frame_label.take() {
            return Some(frame_label);
        }
        let _ = self.drain_serial(rx.session_id);
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return None;
        }
        self.local
            .edit(|queues| queues.by_role[local_role].front_label(rx.lane))
    }

    fn metrics(&self) -> Self::Metrics {}

    fn operational_deadline_ticks(&self) -> Option<u32> {
        Some(UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS)
    }

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box((interval_us, burst_bytes));
    }
}

impl hibana::integration::Transport for ProofCarrier {
    type Error = hibana::integration::transport::TransportError;
    type Tx<'a>
        = ProofTx
    where
        Self: 'a;
    type Rx<'a>
        = ProofRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            ProofTx {
                local_role,
                session_id,
                lane,
            },
            ProofRx {
                local_role,
                session_id,
                lane,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        if tx.session_id == 0 || outgoing.peer() == tx.local_role || outgoing.lane() != tx.lane {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        let peer = outgoing.peer() as usize;
        if peer >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        self.edit(|queues| {
            queues.by_role[peer].push_back(
                outgoing.lane(),
                outgoing.frame_label(),
                outgoing.payload(),
            )
        })?;
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        if rx.session_id == 0 {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
        }
        let Some(frame) = self.edit(|queues| queues.by_role[local_role].pop_front(rx.lane)) else {
            return Poll::Pending;
        };
        rx.frame_label = Some(frame.frame_label);
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(Payload::new(&rx.bytes[..rx.len])))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.edit(|queues| {
                    queues.by_role[local_role].push_front(rx.lane, frame_label, &rx.bytes[..rx.len])
                });
            }
        }
        rx.hint_frame_label.set(None);
    }

    fn drain_events(
        &self,
        emit: &mut dyn FnMut(hibana::integration::transport::advanced::TransportEvent),
    ) {
        emit(
            hibana::integration::transport::advanced::TransportEvent::new(
                hibana::integration::transport::advanced::TransportEventKind::Ack,
                0,
                0,
                0,
            ),
        );
    }

    fn recv_frame_hint<'a>(
        &'a self,
        rx: &'a Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameLabel> {
        if let Some(frame_label) = rx.hint_frame_label.take() {
            return Some(frame_label);
        }
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return None;
        }
        self.edit(|queues| queues.by_role[local_role].front_label(rx.lane))
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box((interval_us, burst_bytes));
    }
}

macro_rules! impl_nowasi_image {
    ($image:ty, $image_id:expr, $site_id:expr, $roles:expr, $peers:expr, $storage:ident) => {
        impl appkit::LogicalImage<UnoQCapsule> for site::Local<$image> {
            type Artifact = appkit::NoWasi;
            type Exit<R> = appkit::RunReport<R, Self>;
            type Carrier<'a>
                = ProofCarrier
            where
                Self: 'a,
                UnoQCapsule: 'a;

            const IMAGE_ID: appkit::ImageId = appkit::ImageId($image_id);
            const SITE_ID: appkit::SiteId = appkit::SiteId($site_id);
            const REQUESTED_ROLES: appkit::RoleSet = $roles;
            const CARRIER: appkit::CarrierKind = UNO_Q_CARRIER;
            const PEER_IMAGES: appkit::PeerImageSet = $peers;

            fn init() -> Self {
                site::Local::new()
            }

            fn safe_state(&mut self) {}

            fn carrier<'a>() -> Self::Carrier<'a>
            where
                UnoQCapsule: 'a,
            {
                ProofCarrier::new()
            }

            #[cfg(all(not(test), target_os = "none"))]
            fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
                $storage.lease()
            }

            fn driver_facts() -> appkit::DriverFacts<'static> {
                UNO_Q_DRIVER_FACTS.driver_facts()
            }
        }
    };
}

impl appkit::LogicalImage<UnoQCapsule> for site::Local<image::HostLoopbackProof> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(710);
    const SITE_ID: appkit::SiteId = appkit::SiteId(7100);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0x7);
    const CARRIER: appkit::CarrierKind = UNO_Q_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        ProofCarrier::new()
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        HOST_PROOF_ATTACH_STORAGE.lease()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        UNO_Q_DRIVER_FACTS.driver_facts()
    }
}

impl appkit::LogicalImage<UnoQCapsule> for site::Local<image::HardwarePeerProof> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    #[cfg(not(target_os = "none"))]
    type Carrier<'a>
        = HardwarePeerCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;
    #[cfg(target_os = "none")]
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(717);
    const SITE_ID: appkit::SiteId = appkit::SiteId(7107);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(HARDWARE_PEER_ROLE_BITS);
    const CARRIER: appkit::CarrierKind = UNO_Q_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(715));

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    #[cfg(not(target_os = "none"))]
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        HardwarePeerCarrier::new()
    }

    #[cfg(target_os = "none")]
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        ProofCarrier::new()
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        HARDWARE_PEER_ATTACH_STORAGE.lease()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        UNO_Q_DRIVER_FACTS.driver_facts()
    }
}

impl appkit::LogicalImage<UnoQCapsule> for site::Local<image::WasiLlmCellProcess> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(711);
    const SITE_ID: appkit::SiteId = appkit::SiteId(7101);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_WASI_LLM_CELL);
    const CARRIER: appkit::CarrierKind = UNO_Q_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet =
        appkit::PeerImageSet::pair(appkit::ImageId(712), appkit::ImageId(715));

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        ProofCarrier::new()
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        WASI_CELL_ATTACH_STORAGE.lease()
    }
}

impl_nowasi_image!(
    image::PseudoLlmProcess,
    712,
    7102,
    appkit::RoleSet::single(ROLE_PSEUDO_LLM),
    appkit::PeerImageSet::pair(appkit::ImageId(711), appkit::ImageId(715)),
    PSEUDO_LLM_ATTACH_STORAGE
);

impl appkit::LogicalImage<UnoQCapsule> for site::Local<image::M33LedKernelImage> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    #[cfg(target_os = "none")]
    type Carrier<'a>
        = UnoQUartCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;
    #[cfg(not(target_os = "none"))]
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(715);
    const SITE_ID: appkit::SiteId = appkit::SiteId(7105);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_M33_LED_KERNEL);
    const CARRIER: appkit::CarrierKind = UNO_Q_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet =
        appkit::PeerImageSet::pair(appkit::ImageId(717), appkit::ImageId(711));

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    #[cfg(target_os = "none")]
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        UnoQUartCarrier::new()
    }

    #[cfg(not(target_os = "none"))]
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        UnoQCapsule: 'a,
    {
        ProofCarrier::new()
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        M33_LED_ATTACH_STORAGE.lease()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        UNO_Q_DRIVER_FACTS.driver_facts()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage<UnoQCapsule> for site::Local<image::HostLoopbackProof> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage<UnoQCapsule> for site::Local<image::HardwarePeerProof> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage<UnoQCapsule> for site::Local<image::WasiLlmCellProcess> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

pub static ARTIFACTS: UnoQArtifacts = UnoQArtifacts;

pub fn projection_caps() -> appkit::ProjectionCaps {
    appkit::derive_projection_caps::<UnoQCapsule>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_uart_deadline_covers_paced_physical_frames() {
        let full_frame_bytes = UART_CARRIER_FRAME_BYTES as u32;
        assert!(
            UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS
                > UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS * 10
        );
        assert!(UNO_Q_M33_HINT_DRAIN_TICKS > UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS * 20);
        assert!(UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS > full_frame_bytes * 10_000);
    }

    #[test]
    fn face_animation_is_typed_choreography_cadence() {
        assert_eq!(UNO_Q_FACE_EMOTION_FRAMES.len(), 12);
        assert_eq!(UNO_Q_FACE_MOUTH_FRAMES.len(), 8);
        assert_eq!(UNO_Q_FACE_CYCLE_FRAME_COUNT, 20);
        assert_eq!(UNO_Q_FACE_EMOTION_HOLD_US, 500_000);
        assert_eq!(UNO_Q_FACE_MOUTH_HOLD_US, 250_000);
        assert_eq!(
            &UNO_Q_FACE_EMOTION_FRAMES[..4],
            &[FACE_HAPPY, FACE_ANGRY, FACE_SAD, FACE_SURPRISED]
        );
        assert_eq!(
            &UNO_Q_FACE_MOUTH_FRAMES[..4],
            &[
                FACE_MOUTH_CLOSED,
                FACE_MOUTH_SMALL,
                FACE_MOUTH_WIDE,
                FACE_MOUTH_ROUND
            ]
        );
    }

    #[test]
    fn face_animation_uses_route_loop_and_passive_offer_decode() {
        fn text(chars: &[char]) -> String {
            chars.iter().collect()
        }

        let source = include_str!("lib.rs");
        for required in [
            text(&[
                'l', 'e', 't', ' ', 'f', 'a', 'c', 'e', '_', 'f', 'r', 'a', 'm', 'e', '_', 'l',
                'o', 'o', 'p', ' ', '=', ' ', 'g', ':', ':', 'r', 'o', 'u', 't', 'e',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'I', 'm', 'p', 'o', 'r', 't', 'L', 'o', 'o', 'p', 'C', 'o',
                'n', 't', 'i', 'n', 'u', 'e',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'I', 'm', 'p', 'o', 'r', 't', 'L', 'o', 'o', 'p', 'B', 'r',
                'e', 'a', 'k',
            ]),
            text(&[
                'F', 'A', 'C', 'E', '_', 'F', 'R', 'A', 'M', 'E', '_', 'P', 'A', 'T', 'H',
            ]),
            text(&[
                'L', 'L', 'M', '_', 'F', 'R', 'A', 'M', 'E', '_', 'P', 'A', 'T', 'H',
            ]),
            text(&[
                'P', 's', 'e', 'u', 'd', 'o', 'L', 'l', 'm', 'P', 'r', 'o', 'c', 'e', 's', 's',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'F', 'd', 'R', 'e', 'a', 'd', 'R', 'e', 'q', 'M', 's', 'g',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'F', 'd', 'R', 'e', 'a', 'd', 'R', 'e', 't', 'M', 's', 'g',
            ]),
            text(&[
                'o', 'f', 'f', 'e', 'r', '(', ')', '.', 'a', 'w', 'a', 'i', 't',
            ]),
            text(&[
                'b', 'r', 'a', 'n', 'c', 'h', '.', 'd', 'e', 'c', 'o', 'd', 'e',
            ]),
            text(&[
                'p', 's', 'e', 'u', 'd', 'o', '_', 'l', 'l', 'm', '_', 'w', 'a', 'i', 't', '_',
                'b', 'e', 'f', 'o', 'r', 'e', '_', 'f', 'r', 'a', 'm', 'e',
            ]),
        ] {
            assert!(
                source.contains(&required),
                "face animation passive offer path must stay route-loop/transport-drained: missing {required}"
            );
        }

        let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        let wasi = text(&[
            'R', 'O', 'L', 'E', '_', 'W', 'A', 'S', 'I', '_', 'L', 'L', 'M', '_', 'C', 'E', 'L',
            'L',
        ]);
        let m33 = text(&[
            'R', 'O', 'L', 'E', '_', 'M', '3', '3', '_', 'L', 'E', 'D', '_', 'K', 'E', 'R', 'N',
            'E', 'L',
        ]);
        let pseudo = text(&[
            'R', 'O', 'L', 'E', '_', 'P', 'S', 'E', 'U', 'D', 'O', '_', 'L', 'L', 'M',
        ]);
        let read_req = text(&[
            'W', 'a', 's', 'i', 'F', 'd', 'R', 'e', 'a', 'd', 'R', 'e', 'q', 'M', 's', 'g',
        ]);
        let read_ret = text(&[
            'W', 'a', 's', 'i', 'F', 'd', 'R', 'e', 'a', 'd', 'R', 'e', 't', 'M', 's', 'g',
        ]);
        let proc_exit = text(&[
            'W', 'a', 's', 'i', 'P', 'r', 'o', 'c', 'E', 'x', 'i', 't', 'R', 'e', 'q', 'M', 's',
            'g',
        ]);
        assert!(
            compact.contains(&format!("g::Role<{wasi}>,g::Role<{pseudo}>,{read_req}")),
            "WASI must read /llm/frame from the pseudo LLM role through ChoreoFS"
        );
        assert!(
            compact.contains(&format!("g::Role<{pseudo}>,g::Role<{wasi}>,{read_ret}")),
            "pseudo LLM must answer only the WASI fd_read reply"
        );
        assert!(
            compact.contains(&format!("g::Role<{wasi}>,g::Role<{pseudo}>,{proc_exit}")),
            "bounded WASI proc_exit must be visible to the pseudo LLM role"
        );
        assert!(
            compact.contains(&format!("g::Role<{wasi}>,g::Role<{m33}>,{proc_exit}")),
            "bounded WASI proc_exit must be visible to the M33 role"
        );
        for forbidden in [
            format!("g::Role<{m33}>,g::Role<{pseudo}>"),
            format!("g::Role<{pseudo}>,g::Role<{m33}>"),
        ] {
            assert!(
                !compact.contains(&forbidden),
                "M33 and pseudo LLM must not be directly wired; found {forbidden}"
            );
        }

        for forbidden in [
            text(&['F', 'r', 'a', 'm', 'e', 'R', 'e', 'q', 'u', 'e', 's', 't']),
            text(&[
                'L', 'l', 'm', 'F', 'r', 'a', 'm', 'e', 'R', 'e', 'q', 'u', 'e', 's', 't', 'M',
                's', 'g',
            ]),
            text(&[
                'L', 'l', 'm', 'F', 'r', 'a', 'm', 'e', 'R', 'e', 's', 'p', 'o', 'n', 's', 'e',
                'M', 's', 'g',
            ]),
            text(&[
                'F', 'a', 'c', 'e', 'F', 'r', 'a', 'm', 'e', 's', 'A', 'p', 'p', 'l', 'i', 'e', 'd',
            ]),
            text(&[
                'L', 'A', 'B', 'E', 'L', '_', 'L', 'L', 'M', '_', 'F', 'R', 'A', 'M', 'E',
            ]),
            text(&[
                'L', 'A', 'B', 'E', 'L', '_', 'F', 'A', 'C', 'E', '_', 'F', 'R', 'A', 'M', 'E',
                'S', '_', 'A', 'P', 'P', 'L', 'I', 'E', 'D',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'P', 'o', 'l', 'l', 'R', 'e', 'q', 'M', 's', 'g',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'P', 'o', 'l', 'l', 'R', 'e', 't', 'M', 's', 'g',
            ]),
            text(&[
                'F', 'A', 'C', 'E', '_', 'F', 'R', 'A', 'M', 'E', '_', 'L', 'O', 'O', 'P', '_',
                'P', 'O', 'L', 'I', 'C', 'Y',
            ]),
            text(&[
                'f', 'a', 'c', 'e', '_', 'f', 'r', 'a', 'm', 'e', '_', 'l', 'o', 'o', 'p', '_',
                'r', 'e', 's', 'o', 'l', 'v', 'e', 'r',
            ]),
            text(&[
                'p', 'o', 'l', 'i', 'c', 'y', ':', ':', 'R', 'e', 's', 'o', 'l', 'v', 'e', 'r',
                'R', 'e', 'f',
            ]),
            text(&['R', 'O', 'L', 'E', '_', 'I', 'O', 'S']),
            text(&[
                'R', 'O', 'L', 'E', '_', 'C', 'H', 'A', 'L', 'L', 'E', 'N', 'G', 'E', 'R',
            ]),
            text(&[
                'R', 'O', 'L', 'E', '_', 'L', 'L', 'M', '_', 'S', 'I', 'D', 'E', 'C', 'A', 'R',
            ]),
        ] {
            assert!(
                !source.contains(&forbidden),
                "face animation is a static route-loop; remove {forbidden}"
            );
        }
    }

    #[test]
    fn wasi_guest_stays_router_not_face_controller() {
        let proof_guest = include_str!("../wasip1/guest/src/bin/uno-q-llm-face-cell.rs");
        let router_guest = include_str!("../wasip1/guest/src/bin/uno-q-llm-face-router.rs");
        for source in [proof_guest, router_guest] {
            assert!(source.contains("fn main()"));
            assert!(source.contains("choreofs::open_read"));
            assert!(source.contains("choreofs::open_write"));
            assert!(source.contains("read_once"));
            assert!(source.contains("write_once_exact"));
            for forbidden in [
                "#![no_std]",
                "#![no_main]",
                "__main_void",
                "panic_handler",
                "time::sleep",
                "sleep_ms",
                "face_hold",
                "FACE_HAPPY",
                "FACE_ANGRY",
                "FACE_SAD",
                "FACE_SURPRISED",
                "FACE_MOUTH_",
                "EMOTION_FRAMES",
                "MOUTH_FRAMES",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "WASI guest is only the LLM-to-face router; remove {forbidden}"
                );
            }
        }
    }

    #[test]
    fn bounded_break_is_choreography_visible_and_deadline_is_not_caller_knobbed() {
        fn text(chars: &[char]) -> String {
            chars.iter().collect()
        }

        let compact: String = include_str!("lib.rs")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let pseudo_proc_exit_decode = text(&[
            'b', 'r', 'a', 'n', 'c', 'h', '.', 'd', 'e', 'c', 'o', 'd', 'e', ':', ':', '<', 'W',
            'a', 's', 'i', 'P', 'r', 'o', 'c', 'E', 'x', 'i', 't', 'R', 'e', 'q', 'M', 's', 'g',
            '>', '(', ')',
        ]);
        assert!(
            compact.contains(&pseudo_proc_exit_decode),
            "finite pseudo LLM role must observe the projected proc_exit break arm"
        );
        let source = include_str!("lib.rs");
        let hardware_proof = include_str!("bin/uno_q_hardware_proof.rs");
        for forbidden in [
            text(&[
                'U', 'N', 'O', '_', 'Q', '_', 'P', 'R', 'O', 'O', 'F', '_', 'F', 'R', 'A', 'M',
                'E', '_', 'C', 'O', 'U', 'N', 'T',
            ]),
            text(&[
                'U', 'N', 'O', '_', 'Q', '_', 'H', 'I', 'B', 'A', 'N', 'A', '_', 'D', 'E', 'A',
                'D', 'L', 'I', 'N', 'E', '_', 'T', 'I', 'C', 'K', 'S',
            ]),
        ] {
            assert!(
                !source.contains(&forbidden) && !hardware_proof.contains(&forbidden),
                "README forbids local stop/deadline authority: remove {forbidden}"
            );
        }
    }
}
