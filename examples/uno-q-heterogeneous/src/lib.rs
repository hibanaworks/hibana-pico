#![cfg_attr(all(target_os = "none", not(test)), no_std)]

pub mod protocol;

use core::cell::{Cell, UnsafeCell};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use hibana::{
    g,
    runtime::{
        program::Projectable,
        wire::{CodecError, Payload, WirePayload},
    },
};
use hibana_pico::appkit;
use hibana_wasip1_runtime::choreofs;
use hibana_wasip1_runtime::protocol::{
    FdBinding, FdRead, FdReadDone, FdReadDoneRet, FdReadReq, FdReadReqMsg, FdReadRetMsg, FdReadRow,
    FdWrite, FdWriteDone, FdWriteDoneRet, FdWriteReq, FdWriteReqMsg, FdWriteRetMsg, FdWriteRow,
    LABEL_WASI_FD_READ, LABEL_WASI_FD_WRITE, PathOpen, PathOpenReq, PathOpenReqMsg, PathOpenRetMsg,
    PathOpened, PathOpenedRet,
};
use protocol::{
    FACE_ANGRY, FACE_HAPPY, FACE_MOUTH_CLOSED, FACE_MOUTH_ROUND, FACE_MOUTH_SMALL, FACE_MOUTH_WIDE,
    FACE_SAD, FACE_SURPRISED, FaceFrame, HumanInputText, LABEL_HUMAN_INPUT_ACK,
    LABEL_HUMAN_INPUT_REQ, LABEL_HUMAN_INPUT_TEXT, LABEL_PICO2W_SENSOR_ACK,
    LABEL_PICO2W_SENSOR_REQ, LABEL_PICO2W_SENSOR_SAMPLE, Pico2wSensorSample, ROLE_HUMAN_INPUT,
    ROLE_LOCAL_LLM, ROLE_M33_LED_KERNEL, ROLE_PICO2W_SENSOR, ROLE_WASI_LLM_CELL,
};
#[cfg(any(not(target_os = "none"), test))]
use protocol::{
    PICO2W_SENSOR_STATUS_FRESH, PICO2W_SENSOR_STATUS_PENDING, PICO2W_SENSOR_STATUS_STALE,
};

pub struct UnoQCapsule;
pub struct UnoQPlacement;
pub struct UnoQLocal;

pub mod image {
    pub struct HostLoopbackProof;
    pub struct HardwarePeerProof;
    pub struct HardwarePeerLoopProof;
    pub struct LocalLlmProcess;
    pub struct HumanInputProcess;
    pub struct Pico2wSensorProcess;
    pub struct WasiLlmCellProcess;
    pub struct M33LedKernelImage;
}

const PREOPEN_FD: u8 = 9;
const LLM_STDIN_FD: u8 = 12;
const LLM_STDOUT_FD: u8 = 13;
const FACE_FRAME_FD: u8 = 15;

const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const PROOF_CARRIER_ROLES: usize = 5;
const PROOF_CARRIER_QUEUE_DEPTH: usize = 24;
const PROOF_CARRIER_FRAME_BYTES: usize = 128;
const UART_CARRIER_MAGIC: [u8; 4] = *b"HBU1";
const UART_CARRIER_CHECK: u8 = 0xa7;
const UART_CARRIER_HEADER_BYTES: usize = 13;
const UART_CARRIER_FRAME_BYTES: usize = UART_CARRIER_HEADER_BYTES + PROOF_CARRIER_FRAME_BYTES + 1;
const HARDWARE_PEER_ROLE_BITS: u16 = (1u16 << ROLE_WASI_LLM_CELL)
    | (1u16 << ROLE_LOCAL_LLM)
    | (1u16 << ROLE_HUMAN_INPUT)
    | (1u16 << ROLE_PICO2W_SENSOR);
#[cfg(test)]
const UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS: u32 = 300_000;
#[cfg(any(test, target_os = "none"))]
const UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS: u32 = 1_000_000_000;
#[cfg(not(target_os = "none"))]
const UNO_Q_FACE_EMOTION_HOLD_US: u64 = 500_000;
#[cfg(not(target_os = "none"))]
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
#[cfg(any(not(target_os = "none"), test))]
const PICO2W_LIGHT_WEAK_RAW: u16 = 400;
#[cfg(any(not(target_os = "none"), test))]
const PICO2W_LIGHT_STRONG_RAW: u16 = 3000;

const LLM_STDIN_PATH: &[u8] = b"llm/stdin";
const LLM_STDOUT_PATH: &[u8] = b"llm/stdout";
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
fn configure_uno_q_uart_nonblocking(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
    }

    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const O_NONBLOCK: i32 = 0x800;

    let flags = unsafe { fcntl(file.as_raw_fd(), F_GETFL, 0) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let rc = unsafe { fcntl(file.as_raw_fd(), F_SETFL, flags | O_NONBLOCK) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
fn configure_uno_q_uart_nonblocking(_file: &std::fs::File) -> std::io::Result<()> {
    Ok(())
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

const FACE_FRAME_OBJECT: choreofs::ChoreoFsObject = choreofs::ChoreoFsObject::writable(
    FACE_FRAME_PATH,
    choreofs::ObjectId(71_005),
    choreofs::FdSpec::new(FACE_FRAME_FD as u32, FD_WRITE_RIGHT, 1),
    FdBinding::write(FdWriteRow::Base),
);

static UNO_Q_DRIVER_FACTS: choreofs::ChoreoFsObjectSet<1> =
    choreofs::ChoreoFsObjectSet::new([FACE_FRAME_OBJECT]);

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_UNO_Q_LLM_FACE_SHELL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-shell.wasm"
));
#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_UNO_Q_LLM_FACE_SHELL_LOOP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-shell-loop.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_UNO_Q_LLM_FACE_SHELL: &[u8] = &[];
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_UNO_Q_LLM_FACE_SHELL_LOOP: &[u8] = &[];

impl image::HostLoopbackProof {
    pub fn wasi_image() -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_bytes(WASM_UNO_Q_LLM_FACE_SHELL)
    }
}

impl image::HardwarePeerProof {
    pub fn wasi_image() -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_bytes(WASM_UNO_Q_LLM_FACE_SHELL)
    }
}

impl image::HardwarePeerLoopProof {
    pub fn wasi_image() -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_bytes(WASM_UNO_Q_LLM_FACE_SHELL_LOOP)
    }
}

impl image::WasiLlmCellProcess {
    pub fn wasi_image() -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_bytes(WASM_UNO_Q_LLM_FACE_SHELL)
    }
}

#[cfg(feature = "runtime-wasip1")]
static mut UNO_Q_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(feature = "runtime-wasip1")]
fn uno_q_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
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
static LOCAL_LLM_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static HUMAN_INPUT_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static PICO2W_SENSOR_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static WASI_CELL_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static M33_LED_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

type HumanInputReqMsg = g::Msg<LABEL_HUMAN_INPUT_REQ, u8>;
type HumanInputTextMsg = g::Msg<LABEL_HUMAN_INPUT_TEXT, HumanInputText>;
type HumanInputAckMsg = g::Msg<LABEL_HUMAN_INPUT_ACK, u8>;
type Pico2wSensorReqMsg = g::Msg<LABEL_PICO2W_SENSOR_REQ, u8>;
type Pico2wSensorSampleMsg = g::Msg<LABEL_PICO2W_SENSOR_SAMPLE, Pico2wSensorSample>;
type Pico2wSensorAckMsg = g::Msg<LABEL_PICO2W_SENSOR_ACK, u8>;

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
    type Placement = UnoQPlacement;
    type Localside = UnoQLocal;

    fn choreography() -> impl Projectable {
        let stdout_write = || {
            g::seq(
                g::send::<ROLE_WASI_LLM_CELL, ROLE_LOCAL_LLM, FdWriteReqMsg>(),
                g::send::<ROLE_LOCAL_LLM, ROLE_WASI_LLM_CELL, FdWriteRetMsg>(),
            )
        };
        let human_input_turn = || {
            g::seq(
                g::send::<ROLE_LOCAL_LLM, ROLE_HUMAN_INPUT, HumanInputReqMsg>(),
                g::seq(
                    g::send::<ROLE_HUMAN_INPUT, ROLE_LOCAL_LLM, HumanInputTextMsg>(),
                    g::send::<ROLE_LOCAL_LLM, ROLE_HUMAN_INPUT, HumanInputAckMsg>(),
                ),
            )
        };
        let pico2w_sensor_turn = || {
            g::seq(
                g::send::<ROLE_LOCAL_LLM, ROLE_PICO2W_SENSOR, Pico2wSensorReqMsg>(),
                g::seq(
                    g::send::<ROLE_PICO2W_SENSOR, ROLE_LOCAL_LLM, Pico2wSensorSampleMsg>(),
                    g::send::<ROLE_LOCAL_LLM, ROLE_PICO2W_SENSOR, Pico2wSensorAckMsg>(),
                ),
            )
        };
        let input_context_turn = || g::par(human_input_turn(), pico2w_sensor_turn());
        let stdin_read = || {
            g::seq(
                g::send::<ROLE_WASI_LLM_CELL, ROLE_LOCAL_LLM, FdReadReqMsg>(),
                g::seq(
                    input_context_turn(),
                    g::send::<ROLE_LOCAL_LLM, ROLE_WASI_LLM_CELL, FdReadRetMsg>(),
                ),
            )
        };
        let face_frame_commit = g::seq(
            g::send::<ROLE_WASI_LLM_CELL, ROLE_M33_LED_KERNEL, FdWriteReqMsg>(),
            g::send::<ROLE_M33_LED_KERNEL, ROLE_WASI_LLM_CELL, FdWriteRetMsg>(),
        );
        let frame_cycle = g::seq(
            stdin_read(),
            g::seq(
                stdout_write(),
                g::seq(stdin_read(), g::seq(face_frame_commit, stdout_write())),
            ),
        );
        let face_frame_loop = frame_cycle.roll();
        g::seq(
            g::send::<ROLE_WASI_LLM_CELL, ROLE_LOCAL_LLM, PathOpenReqMsg>(),
            g::seq(
                g::send::<ROLE_LOCAL_LLM, ROLE_WASI_LLM_CELL, PathOpenRetMsg>(),
                g::seq(
                    g::send::<ROLE_WASI_LLM_CELL, ROLE_LOCAL_LLM, PathOpenReqMsg>(),
                    g::seq(
                        g::send::<ROLE_LOCAL_LLM, ROLE_WASI_LLM_CELL, PathOpenRetMsg>(),
                        g::seq(
                            g::send::<ROLE_WASI_LLM_CELL, ROLE_M33_LED_KERNEL, PathOpenReqMsg>(),
                            g::seq(
                                g::send::<ROLE_M33_LED_KERNEL, ROLE_WASI_LLM_CELL, PathOpenRetMsg>(
                                ),
                                g::seq(stdout_write(), face_frame_loop),
                            ),
                        ),
                    ),
                ),
            ),
        )
    }
}

impl appkit::Placement<UnoQCapsule> for UnoQPlacement {
    fn role_kind<const ROLE: u8>() -> appkit::RoleKind {
        match ROLE {
            ROLE_WASI_LLM_CELL => appkit::RoleKind::Engine,
            ROLE_M33_LED_KERNEL => appkit::RoleKind::Driver,
            ROLE_LOCAL_LLM => appkit::RoleKind::Boundary,
            ROLE_HUMAN_INPUT => appkit::RoleKind::Boundary,
            ROLE_PICO2W_SENSOR => appkit::RoleKind::Boundary,
            _ => panic!("uno-q placement has no role {ROLE}"),
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

async fn run_m33_driver<'endpoint, const ROLE: u8>(
    mut ctx: hibana::Endpoint<'endpoint, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    if ROLE != ROLE_M33_LED_KERNEL {
        return appkit::pending(ctx).await;
    }

    m33_role_step(0x0100);
    m33_board_ready();

    m33_role_step(0x0200);
    let request = expect_path_open(ctx.recv::<PathOpenReqMsg>().await?)?;
    m33_role_step(0x0201);
    complete_path_open(&mut ctx, request, FACE_FRAME_PATH, FD_WRITE_RIGHT).await?;

    m33_role_step(0x0500);
    drive_face_frame_loop(&mut ctx).await?;
    m33_role_step(0x0501);
    appkit::pending(ctx).await
}

async fn run_boundary<'endpoint, const ROLE: u8>(
    mut ctx: hibana::Endpoint<'endpoint, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    match ROLE {
        ROLE_LOCAL_LLM => {
            #[cfg(not(target_os = "none"))]
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q local LLM boundary: open stdin");
            }
            let request = expect_path_open(ctx.recv::<PathOpenReqMsg>().await?)?;
            complete_boundary_path_open(
                &mut ctx,
                request,
                LLM_STDIN_PATH,
                FD_READ_RIGHT,
                LLM_STDIN_FD,
            )
            .await?;
            #[cfg(not(target_os = "none"))]
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q local LLM boundary: open stdout");
            }
            let request = expect_path_open(ctx.recv::<PathOpenReqMsg>().await?)?;
            complete_boundary_path_open(
                &mut ctx,
                request,
                LLM_STDOUT_PATH,
                FD_WRITE_RIGHT,
                LLM_STDOUT_FD,
            )
            .await?;
            let mut source = LocalLlmShellSource::new();
            let write = expect_fd_write(ctx.recv::<FdWriteReqMsg>().await?)?;
            complete_local_llm_stdout_write(&mut ctx, &mut source, write).await?;
            loop {
                let branch = ctx.offer().await?;
                #[cfg(not(target_os = "none"))]
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!("uno-q local LLM boundary branch label={}", branch.label());
                }
                match branch.label() {
                    LABEL_WASI_FD_READ => {
                        let read = expect_fd_read(branch.recv::<FdReadReqMsg>().await?)?;
                        complete_local_llm_stdin_read_with_input_context_read(
                            &mut ctx,
                            &mut source,
                            read,
                        )
                        .await?;

                        let write = expect_fd_write(ctx.recv::<FdWriteReqMsg>().await?)?;
                        complete_local_llm_stdout_write(&mut ctx, &mut source, write).await?;

                        complete_local_llm_stdin_read_with_input_context(&mut ctx, &mut source)
                            .await?;

                        yield_to_peer_roles().await;

                        let write = expect_fd_write(ctx.recv::<FdWriteReqMsg>().await?)?;
                        complete_local_llm_stdout_write(&mut ctx, &mut source, write).await?;
                    }
                    _ => return Err(UnoQRuntimeError::RuntimeViolation),
                }
            }
        }
        ROLE_HUMAN_INPUT => {
            #[cfg(not(target_os = "none"))]
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q human input boundary: start");
            }
            let mut source = HumanInputSource::from_env();
            loop {
                let branch = ctx.offer().await?;
                #[cfg(not(target_os = "none"))]
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!("uno-q human input boundary branch label={}", branch.label());
                }
                match branch.label() {
                    LABEL_HUMAN_INPUT_REQ => {
                        let request = branch.recv::<HumanInputReqMsg>().await?;
                        complete_human_input_turn_after_request(&mut ctx, &mut source, request)
                            .await?;
                        complete_human_input_turn_recv(&mut ctx, &mut source).await?;
                    }
                    _ => return Err(UnoQRuntimeError::RuntimeViolation),
                }
            }
        }
        ROLE_PICO2W_SENSOR => {
            #[cfg(not(target_os = "none"))]
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q pico2w sensor boundary: start");
            }
            let mut source = Pico2wSensorSource::from_env();
            loop {
                let branch = ctx.offer().await?;
                #[cfg(not(target_os = "none"))]
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!(
                        "uno-q pico2w sensor boundary branch label={}",
                        branch.label()
                    );
                }
                match branch.label() {
                    LABEL_PICO2W_SENSOR_REQ => {
                        let request = branch.recv::<Pico2wSensorReqMsg>().await?;
                        complete_pico2w_sensor_turn_after_request(&mut ctx, &mut source, request)
                            .await?;
                        complete_pico2w_sensor_turn_recv(&mut ctx, &mut source).await?;
                    }
                    _ => return Err(UnoQRuntimeError::RuntimeViolation),
                }
            }
        }
        _ => return appkit::pending(ctx).await,
    }
    appkit::pending(ctx).await
}

async fn complete_local_llm_stdout_write<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut LocalLlmShellSource,
    write: hibana_wasip1_runtime::protocol::FdWrite,
) -> Result<(), UnoQRuntimeError> {
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q local LLM stdout fd={} len={} text={:?}",
            write.fd(),
            write.len(),
            core::str::from_utf8(write.as_bytes()).unwrap_or("<binary>")
        );
    }
    if write.fd() != LLM_STDOUT_FD {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    source.observe_shell_output(write.as_bytes());
    ctx.send::<FdWriteRetMsg>(&FdWriteDoneRet(FdWriteDone::new(
        write.fd(),
        write.len() as u8,
    )))
    .await?;
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!("uno-q local LLM stdout ack");
    }
    Ok(())
}

async fn complete_local_llm_stdin_read<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut LocalLlmShellSource,
    read: hibana_wasip1_runtime::protocol::FdRead,
) -> Result<(), UnoQRuntimeError> {
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q local LLM stdin fd={} max={}",
            read.fd(),
            read.max_len()
        );
    }
    if read.fd() != LLM_STDIN_FD || read.max_len() == 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let (command, len) = source.next_command(read.max_len() as usize)?;
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q local LLM stdin reply len={} text={:?}",
            len,
            core::str::from_utf8(&command[..len]).unwrap_or("<binary>")
        );
    }
    let reply = FdReadDone::new(read.fd(), &command[..len])?;
    ctx.send::<FdReadRetMsg>(&FdReadDoneRet(reply)).await?;
    Ok(())
}

async fn complete_local_llm_stdin_read_with_input_context<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut LocalLlmShellSource,
) -> Result<(), UnoQRuntimeError> {
    let read = expect_fd_read(ctx.recv::<FdReadReqMsg>().await?)?;
    complete_local_llm_stdin_read_with_input_context_read(ctx, source, read).await
}

async fn complete_local_llm_stdin_read_with_input_context_read<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut LocalLlmShellSource,
    read: hibana_wasip1_runtime::protocol::FdRead,
) -> Result<(), UnoQRuntimeError> {
    yield_to_peer_roles().await;
    ctx.send::<HumanInputReqMsg>(&0).await?;
    ctx.send::<Pico2wSensorReqMsg>(&0).await?;
    yield_to_peer_roles().await;
    let human_input = ctx.recv::<HumanInputTextMsg>().await?;
    let sensor_sample = ctx.recv::<Pico2wSensorSampleMsg>().await?;
    source.observe_human_input(human_input)?;
    source.observe_pico2w_sensor_sample(sensor_sample);
    ctx.send::<HumanInputAckMsg>(&0).await?;
    ctx.send::<Pico2wSensorAckMsg>(&0).await?;
    yield_to_peer_roles().await;
    complete_local_llm_stdin_read(ctx, source, read).await
}

async fn complete_human_input_turn_recv<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut HumanInputSource,
) -> Result<(), UnoQRuntimeError> {
    let request = ctx.recv::<HumanInputReqMsg>().await?;
    complete_human_input_turn_after_request(ctx, source, request).await
}

async fn complete_human_input_turn_after_request<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut HumanInputSource,
    request: u8,
) -> Result<(), UnoQRuntimeError> {
    if request != 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let input = source.next_input()?;
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!("uno-q human input boundary send len={}", input.len());
    }
    ctx.send::<HumanInputTextMsg>(&input).await?;
    let ack = ctx.recv::<HumanInputAckMsg>().await?;
    if ack != 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    Ok(())
}

async fn complete_pico2w_sensor_turn_recv<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut Pico2wSensorSource,
) -> Result<(), UnoQRuntimeError> {
    let request = ctx.recv::<Pico2wSensorReqMsg>().await?;
    complete_pico2w_sensor_turn_after_request(ctx, source, request).await
}

async fn complete_pico2w_sensor_turn_after_request<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    source: &mut Pico2wSensorSource,
    request: u8,
) -> Result<(), UnoQRuntimeError> {
    if request != 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let sample = source.next_sample()?;
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q pico2w sensor boundary send status={} seq={}",
            sample.status(),
            sample.seq()
        );
    }
    ctx.send::<Pico2wSensorSampleMsg>(&sample).await?;
    let ack = ctx.recv::<Pico2wSensorAckMsg>().await?;
    if ack != 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    Ok(())
}

struct YieldToPeerRoles {
    yielded: bool,
}

impl Future for YieldToPeerRoles {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn yield_to_peer_roles() -> YieldToPeerRoles {
    YieldToPeerRoles { yielded: false }
}

async fn drive_face_frame_loop<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<(), UnoQRuntimeError> {
    let mut ordinal = 0u8;
    loop {
        m33_role_step(0x0d20_0000 | u32::from(ordinal));
        #[cfg(not(target_os = "none"))]
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!("uno-q-face passive offer ordinal={ordinal}");
        }
        let branch = match ctx.offer().await {
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
                let write = match branch.recv::<FdWriteReqMsg>().await {
                    Ok(request) => match expect_fd_write(request) {
                        Ok(write) => write,
                        Err(error) => {
                            m33_role_step(0xed21_0000 | u32::from(ordinal));
                            return Err(error);
                        }
                    },
                    Err(error) => {
                        m33_role_step(0xed22_0000 | u32::from(ordinal));
                        return Err(error.into());
                    }
                };
                m33_role_step(0x0d22_0100 | u32::from(ordinal));
                if let Err(error) = expect_fd_object(
                    &*ctx,
                    write.fd(),
                    FACE_FRAME_OBJECT.object(),
                    FD_WRITE_RIGHT,
                ) {
                    m33_role_step(0xed24_0000 | u32::from(ordinal));
                    return Err(error);
                }
                m33_role_step(0x0d22_0200 | u32::from(ordinal));
                let frame = match FaceFrame::decode_payload(Payload::new(write.as_bytes())) {
                    Ok(frame) => frame,
                    Err(error) => {
                        m33_role_step(0xed25_0000 | u32::from(ordinal));
                        return Err(error.into());
                    }
                };
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
                yield_to_peer_roles().await;
            }
            label => {
                m33_role_step(0xed21_0000 | (u32::from(ordinal) << 8) | u32::from(label));
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
        }
    }
    Ok(())
}

const LOCAL_LLM_TRANSCRIPT_BYTES: usize = 768;
const LOCAL_LLM_COMMAND_BYTES: usize = 96;
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_LOCAL_LLM_BIN_DIR: &str = "/data/local/tmp/uno-q-local-llm/bin";
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_LOCAL_LLM_LIB_DIR: &str = "/data/local/tmp/uno-q-local-llm/lib";
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_LOCAL_LLM_SERVER: &str = "/data/local/tmp/uno-q-local-llm/bin/llama-server";
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_LOCAL_LLM_MODEL: &str =
    "/data/local/tmp/uno-q-local-llm/models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf";
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_LOCAL_LLM_SERVER_PORT: u16 = 18080;
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_PICO2W_SENSOR_UDP_BIND: &str = "0.0.0.0:8787";
#[cfg(not(target_os = "none"))]
const DEFAULT_UNO_Q_PICO2W_SENSOR_UDP_STALE_MS: u64 = 5_000;

#[cfg(not(target_os = "none"))]
fn decode_pico2w_sensor_udp_payload(input: &[u8]) -> Option<Pico2wSensorSample> {
    if input.len() != protocol::PICO2W_SENSOR_SAMPLE_BYTES {
        return None;
    }
    Pico2wSensorSample::decode_payload(Payload::new(input)).ok()
}

struct LocalLlmShellSource {
    transcript: [u8; LOCAL_LLM_TRANSCRIPT_BYTES],
    transcript_len: usize,
    read_phase: u8,
    ordinal: u8,
    #[cfg(not(target_os = "none"))]
    human_request: Option<String>,
    #[cfg(not(target_os = "none"))]
    pico2w_sensor_sample: Pico2wSensorSample,
    #[cfg(not(target_os = "none"))]
    command: LocalLlmCommandSource,
}

impl LocalLlmShellSource {
    fn new() -> Self {
        Self {
            transcript: [0; LOCAL_LLM_TRANSCRIPT_BYTES],
            transcript_len: 0,
            read_phase: 0,
            ordinal: 0,
            #[cfg(not(target_os = "none"))]
            human_request: None,
            #[cfg(not(target_os = "none"))]
            pico2w_sensor_sample: Pico2wSensorSample::pending(0),
            #[cfg(not(target_os = "none"))]
            command: LocalLlmCommandSource::from_env(),
        }
    }

    fn observe_shell_output(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.transcript_len == self.transcript.len() {
                self.transcript.copy_within(1.., 0);
                self.transcript_len -= 1;
            }
            self.transcript[self.transcript_len] = byte;
            self.transcript_len += 1;
        }
    }

    fn observe_human_input(&mut self, input: HumanInputText) -> Result<(), UnoQRuntimeError> {
        #[cfg(not(target_os = "none"))]
        {
            if input.is_empty() {
                self.human_request = None;
                return Ok(());
            }
            self.human_request = Some(input.as_str()?.to_owned());
            Ok(())
        }
        #[cfg(target_os = "none")]
        {
            let _ = input;
            Ok(())
        }
    }

    fn observe_pico2w_sensor_sample(&mut self, sample: Pico2wSensorSample) {
        #[cfg(not(target_os = "none"))]
        {
            self.pico2w_sensor_sample = sample;
        }
        #[cfg(target_os = "none")]
        {
            let _ = sample;
        }
    }

    fn next_command(
        &mut self,
        max_len: usize,
    ) -> Result<([u8; LOCAL_LLM_COMMAND_BYTES], usize), UnoQRuntimeError> {
        if self.read_phase == 0 {
            self.wait_before_cycle();
        }

        let mut command = [0u8; LOCAL_LLM_COMMAND_BYTES];
        let len = {
            #[cfg(not(target_os = "none"))]
            {
                let context = LocalLlmPromptContext {
                    human_request: self.human_request.as_deref(),
                    sensor_sample: self.pico2w_sensor_sample,
                };
                self.command.next_command(
                    &self.transcript[..self.transcript_len],
                    self.read_phase,
                    self.ordinal,
                    context,
                    &mut command,
                )?
            }
            #[cfg(target_os = "none")]
            {
                scripted_local_llm_shell_command(self.read_phase, self.ordinal, &mut command)?
            }
        };
        if len == 0 || len > max_len {
            return Err(UnoQRuntimeError::RuntimeViolation);
        }
        self.observe_shell_output(&command[..len]);
        if self.read_phase == 0 {
            self.read_phase = 1;
        } else {
            self.read_phase = 0;
            self.ordinal = self.ordinal.wrapping_add(1);
        }
        Ok((command, len))
    }

    fn wait_before_cycle(&self) {
        if self.ordinal == 0 {
            return;
        }
        if !face_loop_pacing_enabled() {
            return;
        }
        #[cfg(not(target_os = "none"))]
        std::thread::sleep(std::time::Duration::from_micros(face_hold_us_for_ordinal(
            self.ordinal.wrapping_sub(1),
        )));
    }
}

#[cfg(not(target_os = "none"))]
struct HumanInputSource {
    stream: Option<HumanInputStream>,
    latest: HumanInputText,
}

#[cfg(not(target_os = "none"))]
struct HumanInputStream {
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
}

#[cfg(not(target_os = "none"))]
impl HumanInputStream {
    fn new(receiver: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self { receiver }
    }
}

#[cfg(not(target_os = "none"))]
impl HumanInputSource {
    fn from_env() -> Self {
        let mode = human_input_mode_from_env();
        let stream = match mode {
            HumanInputMode::Prompt => spawn_prompt_human_input(),
            HumanInputMode::Voice => spawn_voice_human_input(),
            HumanInputMode::Off => None,
        };
        let latest = std::env::var("UNO_Q_HUMAN_INPUT_TEXT")
            .ok()
            .and_then(|text| HumanInputText::new(&text).ok())
            .unwrap_or_else(HumanInputText::empty);
        Self { stream, latest }
    }

    fn next_input(&mut self) -> Result<HumanInputText, UnoQRuntimeError> {
        if let Some(stream) = &self.stream {
            while let Ok(bytes) = stream.receiver.try_recv() {
                self.latest = HumanInputText::from_bytes(&bytes)?;
            }
        }
        Ok(self.latest)
    }
}

#[cfg(target_os = "none")]
struct HumanInputSource;

#[cfg(target_os = "none")]
impl HumanInputSource {
    fn from_env() -> Self {
        Self
    }

    fn next_input(&mut self) -> Result<HumanInputText, UnoQRuntimeError> {
        Ok(HumanInputText::empty())
    }
}

#[cfg(not(target_os = "none"))]
struct Pico2wSensorSource {
    stream: Option<Pico2wSensorStream>,
    latest: Pico2wSensorSample,
    latest_at: Option<std::time::Instant>,
    seq: u16,
    mode: Pico2wSensorMode,
}

#[cfg(not(target_os = "none"))]
struct Pico2wSensorStream {
    receiver: std::sync::mpsc::Receiver<Pico2wSensorSample>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(not(target_os = "none"))]
impl Pico2wSensorStream {
    fn new(
        receiver: std::sync::mpsc::Receiver<Pico2wSensorSample>,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            receiver,
            shutdown,
            handle: Some(handle),
        }
    }
}

#[cfg(not(target_os = "none"))]
impl Drop for Pico2wSensorStream {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(not(target_os = "none"))]
impl Pico2wSensorSource {
    fn from_env() -> Self {
        let mode = pico2w_sensor_mode_from_env();
        let stream = match mode {
            Pico2wSensorMode::Udp => spawn_pico2w_sensor_udp(),
            Pico2wSensorMode::Off => None,
        };
        Self {
            stream,
            latest: Pico2wSensorSample::pending(0),
            latest_at: None,
            seq: 0,
            mode,
        }
    }

    fn next_sample(&mut self) -> Result<Pico2wSensorSample, UnoQRuntimeError> {
        if let Some(stream) = &self.stream {
            while let Ok(sample) = stream.receiver.try_recv() {
                self.latest = sample.with_status_and_seq(PICO2W_SENSOR_STATUS_FRESH, self.seq)?;
                self.latest_at = Some(std::time::Instant::now());
            }
        }

        self.seq = self.seq.wrapping_add(1);
        if matches!(self.mode, Pico2wSensorMode::Udp) {
            let Some(latest_at) = self.latest_at else {
                return Ok(Pico2wSensorSample::pending(self.seq));
            };
            let age = latest_at.elapsed();
            if age > pico2w_sensor_stale_timeout() {
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!("uno-q pico2w sensor UDP stale after {} ms", age.as_millis());
                }
                return self
                    .latest
                    .with_status_and_seq(PICO2W_SENSOR_STATUS_STALE, self.seq)
                    .map_err(Into::into);
            }
        }
        self.latest
            .with_status_and_seq(self.latest.status(), self.seq)
            .map_err(Into::into)
    }
}

#[cfg(target_os = "none")]
struct Pico2wSensorSource {
    seq: u16,
}

#[cfg(target_os = "none")]
impl Pico2wSensorSource {
    fn from_env() -> Self {
        Self { seq: 0 }
    }

    fn next_sample(&mut self) -> Result<Pico2wSensorSample, UnoQRuntimeError> {
        self.seq = self.seq.wrapping_add(1);
        Ok(Pico2wSensorSample::pending(self.seq))
    }
}

#[cfg(not(target_os = "none"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HumanInputMode {
    Prompt,
    Voice,
    Off,
}

#[cfg(not(target_os = "none"))]
fn human_input_mode_from_env() -> HumanInputMode {
    match std::env::var("UNO_Q_HUMAN_INPUT_MODE").as_deref() {
        Ok("prompt") | Ok("prompt-shell") => HumanInputMode::Prompt,
        Ok("voice") | Ok("voice-shell") => HumanInputMode::Voice,
        Ok("off") | Ok("none") | Ok("0") => HumanInputMode::Off,
        Ok(_) => HumanInputMode::Off,
        Err(_) => HumanInputMode::Off,
    }
}

#[cfg(not(target_os = "none"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pico2wSensorMode {
    Udp,
    Off,
}

#[cfg(not(target_os = "none"))]
fn pico2w_sensor_mode_from_env() -> Pico2wSensorMode {
    match std::env::var("UNO_Q_PICO2W_SENSOR_MODE").as_deref() {
        Ok("udp") => Pico2wSensorMode::Udp,
        Ok("off") | Ok("none") | Ok("0") => Pico2wSensorMode::Off,
        Ok(_) => Pico2wSensorMode::Off,
        Err(_) => Pico2wSensorMode::Off,
    }
}

#[cfg(not(target_os = "none"))]
fn pico2w_sensor_udp_bind_from_env() -> String {
    std::env::var("UNO_Q_PICO2W_SENSOR_UDP_BIND")
        .unwrap_or_else(|_| DEFAULT_UNO_Q_PICO2W_SENSOR_UDP_BIND.to_owned())
}

#[cfg(not(target_os = "none"))]
fn pico2w_sensor_stale_timeout() -> std::time::Duration {
    let ms = std::env::var("UNO_Q_PICO2W_SENSOR_UDP_STALE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_UNO_Q_PICO2W_SENSOR_UDP_STALE_MS);
    std::time::Duration::from_millis(ms)
}

#[cfg(not(target_os = "none"))]
fn spawn_prompt_human_input() -> Option<HumanInputStream> {
    use std::io::BufRead as _;

    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("uno-q-human-prompt-shell".to_owned())
        .spawn(move || {
            eprintln!(
                "uno-q input role prompt shell: type a request; line bytes go unchanged to the LLM role"
            );
            let stdin = std::io::stdin();
            let mut locked = stdin.lock();
            let mut line = Vec::new();
            loop {
                line.clear();
                match locked.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        strip_terminal_line_delimiter(&mut line);
                        if line.len() > protocol::HUMAN_INPUT_TEXT_BYTES {
                            eprintln!(
                                "uno-q input role ignored over-capacity line: {} bytes",
                                line.len()
                            );
                            continue;
                        }
                        if sender.send(line.clone()).is_err() {
                            break;
                        }
                    }
                }
            }
        })
        .ok()?;
    Some(HumanInputStream::new(receiver))
}

#[cfg(not(target_os = "none"))]
fn spawn_voice_human_input() -> Option<HumanInputStream> {
    use std::io::BufRead as _;

    let command = std::env::var("UNO_Q_HUMAN_INPUT_VOICE_CMD").ok()?;
    let mut parts = split_local_llm_args(&command);
    if parts.is_empty() {
        return None;
    }
    let executable = parts.remove(0);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("uno-q-human-voice-shell".to_owned())
        .spawn(move || {
            let mut child = match std::process::Command::new(&executable)
                .args(&parts)
                .stdout(std::process::Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(error) => {
                    eprintln!("uno-q input role failed to start voice shell: {error}");
                    return;
                }
            };
            let Some(stdout) = child.stdout.take() else {
                return;
            };
            let reader = std::io::BufReader::new(stdout);
            for line in reader.split(b'\n') {
                let Ok(mut bytes) = line else {
                    break;
                };
                strip_terminal_line_delimiter(&mut bytes);
                if bytes.len() > protocol::HUMAN_INPUT_TEXT_BYTES {
                    eprintln!(
                        "uno-q input role ignored over-capacity voice line: {} bytes",
                        bytes.len()
                    );
                    continue;
                }
                if sender.send(bytes).is_err() {
                    break;
                }
            }
            let _ = child.wait();
        })
        .ok()?;
    Some(HumanInputStream::new(receiver))
}

#[cfg(not(target_os = "none"))]
fn spawn_pico2w_sensor_udp() -> Option<Pico2wSensorStream> {
    let bind = pico2w_sensor_udp_bind_from_env();
    let port = match pico2w_sensor_udp_port(&bind) {
        Some(port) => port,
        None => {
            eprintln!("uno-q pico2w sensor UDP invalid bind address: {bind}");
            return None;
        }
    };
    let raw_socket = match open_pico2w_sensor_raw_socket() {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("uno-q pico2w sensor UDP raw socket failed: {error}");
            return None;
        }
    };
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_shutdown = shutdown.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("uno-q-pico2w-sensor-udp".to_owned())
        .spawn(move || {
            eprintln!("uno-q pico2w sensor raw UDP listening on wlan0:{port}");
            let mut packet = [0u8; 1536];
            let mut ack_frame = [0u8; 96];
            while !thread_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                match raw_socket.recv(&mut packet, std::time::Duration::from_millis(250)) {
                    Ok(Some(len)) => {
                        let Some(frame) = parse_pico2w_sensor_udp_frame(&packet[..len], port)
                        else {
                            continue;
                        };
                        let Some(sample) = decode_pico2w_sensor_udp_payload(&frame.payload) else {
                            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                                eprintln!(
                                    "uno-q pico2w sensor UDP ignored malformed raw packet from {}.{}.{}.{}:{} len={len}",
                                    frame.src_ip.0[0],
                                    frame.src_ip.0[1],
                                    frame.src_ip.0[2],
                                    frame.src_ip.0[3],
                                    frame.src_port
                                );
                            }
                            continue;
                        };
                        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                            eprintln!(
                                "uno-q pico2w sensor UDP from {}.{}.{}.{}:{}: status={} temp_x10={} hum_x10={} light={}",
                                frame.src_ip.0[0],
                                frame.src_ip.0[1],
                                frame.src_ip.0[2],
                                frame.src_ip.0[3],
                                frame.src_port,
                                sample.status(),
                                sample.temperature_c_x10(),
                                sample.humidity_pct_x10(),
                                sample.light_raw()
                            );
                        }
                        let ack_len =
                            match build_pico2w_sensor_udp_ack_frame(frame, sample.seq(), &mut ack_frame) {
                                Some(len) => len,
                                None => {
                                    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                                        eprintln!("uno-q pico2w sensor UDP ack frame build failed");
                                    }
                                    continue;
                                }
                            };
                        if let Err(error) = raw_socket.send(&ack_frame[..ack_len]) {
                            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                                eprintln!(
                                    "uno-q pico2w sensor UDP raw ack failed to {}.{}.{}.{}:{}: {error}",
                                    frame.src_ip.0[0],
                                    frame.src_ip.0[1],
                                    frame.src_ip.0[2],
                                    frame.src_ip.0[3],
                                    frame.src_port
                                );
                            }
                            continue;
                        }
                        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                            eprintln!(
                                "uno-q pico2w sensor UDP raw ack to {}.{}.{}.{}:{}: seq={}",
                                frame.src_ip.0[0],
                                frame.src_ip.0[1],
                                frame.src_ip.0[2],
                                frame.src_ip.0[3],
                                frame.src_port,
                                sample.seq()
                            );
                        }
                        if sender.send(sample).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(error) => {
                        eprintln!("uno-q pico2w sensor UDP receive error: {error}");
                        break;
                    }
                }
            }
        })
        .ok()?;
    Some(Pico2wSensorStream::new(receiver, shutdown, handle))
}

#[cfg(not(target_os = "none"))]
fn pico2w_sensor_udp_port(bind: &str) -> Option<u16> {
    bind.parse::<std::net::SocketAddr>()
        .ok()
        .map(|addr| addr.port())
}

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
struct Pico2wRawSocket {
    fd: std::os::fd::RawFd,
}

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
impl Pico2wRawSocket {
    fn recv(&self, out: &mut [u8], timeout: std::time::Duration) -> std::io::Result<Option<usize>> {
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut pollfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if ready == 0 {
            return Ok(None);
        }
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(error);
        }
        let len = unsafe { libc::recv(self.fd, out.as_mut_ptr().cast(), out.len(), 0) };
        if len < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                Ok(None)
            } else {
                Err(error)
            }
        } else {
            Ok(Some(len as usize))
        }
    }

    fn send(&self, frame: &[u8]) -> std::io::Result<()> {
        let written = unsafe { libc::send(self.fd, frame.as_ptr().cast(), frame.len(), 0) };
        if written < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if written as usize != frame.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "short raw ethernet send",
            ));
        }
        Ok(())
    }
}

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
impl Drop for Pico2wRawSocket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(all(not(target_os = "none"), target_os = "linux"))]
fn open_pico2w_sensor_raw_socket() -> std::io::Result<Pico2wRawSocket> {
    let protocol = i32::from((libc::ETH_P_ALL as u16).to_be());
    let fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, protocol) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let socket = Pico2wRawSocket { fd };
    let iface = std::ffi::CString::new("wlan0").expect("static interface name");
    let ifindex = unsafe { libc::if_nametoindex(iface.as_ptr()) };
    if ifindex == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut addr: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    addr.sll_family = libc::AF_PACKET as u16;
    addr.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
    addr.sll_ifindex = ifindex as i32;
    let result = unsafe {
        libc::bind(
            socket.fd,
            (&addr as *const libc::sockaddr_ll).cast(),
            std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(socket)
}

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
struct Pico2wRawSocket;

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
impl Pico2wRawSocket {
    fn recv(
        &self,
        _out: &mut [u8],
        _timeout: std::time::Duration,
    ) -> std::io::Result<Option<usize>> {
        Ok(None)
    }

    fn send(&self, _frame: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(not(target_os = "none"), not(target_os = "linux")))]
fn open_pico2w_sensor_raw_socket() -> std::io::Result<Pico2wRawSocket> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "AF_PACKET raw socket requires Linux",
    ))
}

#[cfg(not(target_os = "none"))]
#[derive(Clone, Copy)]
struct Pico2wSensorUdpFrame {
    src_mac: hibana_wifi::proto::ethernet::MacAddr,
    dst_mac: hibana_wifi::proto::ethernet::MacAddr,
    src_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
    dst_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: [u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES],
}

#[cfg(not(target_os = "none"))]
fn parse_pico2w_sensor_udp_frame(frame: &[u8], local_port: u16) -> Option<Pico2wSensorUdpFrame> {
    use hibana_wifi::proto::{
        ethernet::{ETH_HEADER_LEN, IP_PROTO_UDP, IPV4_HEADER_LEN, Ipv4Addr, MacAddr},
        udp::parse_udp_ipv4_packet,
    };

    if frame.len() < ETH_HEADER_LEN + IPV4_HEADER_LEN + 8 {
        return None;
    }
    let dst_mac = MacAddr([frame[0], frame[1], frame[2], frame[3], frame[4], frame[5]]);
    let src_mac = MacAddr([frame[6], frame[7], frame[8], frame[9], frame[10], frame[11]]);
    let ip = &frame[ETH_HEADER_LEN..];
    if ip[0] >> 4 != 4 || ip[9] != IP_PROTO_UDP {
        return None;
    }
    let ihl = usize::from(ip[0] & 0x0f) * 4;
    if ihl < IPV4_HEADER_LEN || ip.len() < ihl + 8 {
        return None;
    }
    let dst_ip = Ipv4Addr([ip[16], ip[17], ip[18], ip[19]]);
    let packet = parse_udp_ipv4_packet::<{ protocol::PICO2W_SENSOR_SAMPLE_BYTES }>(
        frame, dst_mac, dst_ip, local_port,
    )?;
    if packet.payload().len() != protocol::PICO2W_SENSOR_SAMPLE_BYTES {
        return None;
    }
    let mut payload = [0u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES];
    payload.copy_from_slice(packet.payload());
    Some(Pico2wSensorUdpFrame {
        src_mac,
        dst_mac,
        src_ip: packet.src_ip(),
        dst_ip: packet.dst_ip(),
        src_port: packet.src_port(),
        dst_port: packet.dst_port(),
        payload,
    })
}

#[cfg(not(target_os = "none"))]
fn build_pico2w_sensor_udp_ack_frame(
    frame: Pico2wSensorUdpFrame,
    seq: u16,
    out: &mut [u8],
) -> Option<usize> {
    hibana_wifi::proto::ethernet::build_udp_ipv4(
        out,
        frame.dst_mac,
        frame.src_mac,
        frame.dst_ip,
        frame.src_ip,
        frame.dst_port,
        frame.src_port,
        &protocol::pico2w_sensor_udp_ack(seq),
    )
    .ok()
}

#[cfg(not(target_os = "none"))]
fn strip_terminal_line_delimiter(bytes: &mut Vec<u8>) {
    if bytes.last().copied() == Some(b'\n') {
        bytes.pop();
    }
    if bytes.last().copied() == Some(b'\r') {
        bytes.pop();
    }
}

#[cfg(not(target_os = "none"))]
#[derive(Clone, Copy)]
struct LocalLlmPromptContext<'a> {
    human_request: Option<&'a str>,
    sensor_sample: Pico2wSensorSample,
}

#[cfg(not(target_os = "none"))]
enum LocalLlmCommandSource {
    Server(LocalLlmServer),
    External(LocalLlmExternalCommand),
    Scripted,
    Missing,
}

#[cfg(not(target_os = "none"))]
struct LocalLlmServer {
    endpoint: String,
    child: Option<std::process::Child>,
}

#[cfg(not(target_os = "none"))]
struct LocalLlmExternalCommand {
    executable: String,
    args: Vec<String>,
    prompt: Option<String>,
    add_transcript_affixes: bool,
    current_dir: Option<String>,
    ld_library_path: Option<String>,
}

#[cfg(not(target_os = "none"))]
impl LocalLlmCommandSource {
    fn from_env() -> Self {
        if std::env::var_os("UNO_Q_LOCAL_LLM_SCRIPTED").is_some() {
            return Self::Scripted;
        }

        let explicit = std::env::var("UNO_Q_LOCAL_LLM_CMD").ok();
        if let Some(command) = explicit {
            let mut parts = split_local_llm_args(&command);
            if parts.is_empty() {
                return Self::Missing;
            }
            let executable = parts.remove(0);
            return Self::External(LocalLlmExternalCommand {
                executable,
                args: parts,
                prompt: None,
                add_transcript_affixes: false,
                current_dir: std::env::var("UNO_Q_LOCAL_LLM_WORK_DIR").ok(),
                ld_library_path: local_llm_library_path_from_env(),
            });
        }

        let explicit_cli = std::env::var("UNO_Q_LOCAL_LLM_CLI").ok();
        let model = std::env::var("UNO_Q_LOCAL_LLM_MODEL")
            .ok()
            .or_else(|| local_llm_existing_path(DEFAULT_UNO_Q_LOCAL_LLM_MODEL));
        let Some(model) = model else {
            return Self::Missing;
        };

        let Some(executable) = explicit_cli else {
            if let Some(server) = LocalLlmServer::from_env(&model) {
                return Self::Server(server);
            }
            return Self::Missing;
        };

        let mut args = Vec::new();
        args.push("-m".to_owned());
        args.push(model);
        let add_transcript_affixes = if let Ok(extra) = std::env::var("UNO_Q_LOCAL_LLM_ARGS") {
            args.extend(split_local_llm_args(&extra));
            false
        } else {
            args.extend([
                "--no-display-prompt".to_owned(),
                "--simple-io".to_owned(),
                "--no-warmup".to_owned(),
                "-t".to_owned(),
                "4".to_owned(),
                "-n".to_owned(),
                "8".to_owned(),
                "--temp".to_owned(),
                "0".to_owned(),
            ]);
            true
        };

        let prompt = Some(
            std::env::var("UNO_Q_LOCAL_LLM_PROMPT")
                .unwrap_or_else(|_| default_local_llm_shell_prompt().to_owned()),
        );

        Self::External(LocalLlmExternalCommand {
            executable,
            args,
            prompt,
            add_transcript_affixes,
            current_dir: local_llm_work_dir_from_env(),
            ld_library_path: local_llm_library_path_from_env(),
        })
    }

    fn next_command(
        &mut self,
        transcript: &[u8],
        phase: u8,
        ordinal: u8,
        context: LocalLlmPromptContext<'_>,
        out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
    ) -> Result<usize, UnoQRuntimeError> {
        match self {
            Self::Server(server) => server.next_command(transcript, phase, ordinal, context, out),
            Self::External(command) => {
                command.next_command(transcript, phase, ordinal, context, out)
            }
            Self::Scripted => {
                if phase != 0 {
                    if let Some(request) = context.human_request
                        && let Some(len) = scripted_human_request_shell_command(request, out)?
                    {
                        return Ok(len);
                    }
                    if let Some(len) = scripted_pico2w_sensor_shell_command(
                        context.sensor_sample,
                        context.human_request.is_some(),
                        out,
                    )? {
                        return Ok(len);
                    }
                }
                scripted_local_llm_shell_command(phase, ordinal, out)
            }
            Self::Missing => Err(UnoQRuntimeError::RuntimeViolation),
        }
    }
}

#[cfg(not(target_os = "none"))]
impl LocalLlmServer {
    fn from_env(model: &str) -> Option<Self> {
        if let Ok(endpoint) = std::env::var("UNO_Q_LOCAL_LLM_SERVER_ENDPOINT") {
            match Self::attach(endpoint) {
                Ok(server) => return Some(server),
                Err(()) => {
                    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                        eprintln!("uno-q local LLM server endpoint is not healthy");
                    }
                    return None;
                }
            }
        }

        let executable = std::env::var("UNO_Q_LOCAL_LLM_SERVER")
            .ok()
            .or_else(|| local_llm_existing_path(DEFAULT_UNO_Q_LOCAL_LLM_SERVER))?;
        let port = local_llm_server_port_from_env();
        let endpoint = format!("http://127.0.0.1:{port}");
        match Self::start(executable, model.to_owned(), endpoint) {
            Ok(server) => Some(server),
            Err(()) => {
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!("uno-q local LLM server failed to start");
                }
                None
            }
        }
    }

    fn attach(endpoint: String) -> Result<Self, ()> {
        if !local_llm_server_health_ok(&endpoint, std::time::Duration::from_millis(250)) {
            return Err(());
        }
        Ok(Self {
            endpoint,
            child: None,
        })
    }

    fn start(executable: String, model: String, endpoint: String) -> Result<Self, ()> {
        if local_llm_server_health_ok(&endpoint, std::time::Duration::from_millis(250)) {
            return Ok(Self {
                endpoint,
                child: None,
            });
        }

        let (_host, port) = local_llm_http_endpoint_parts(&endpoint).ok_or(())?;
        let mut args = vec![
            "-m".to_owned(),
            model,
            "--host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            port.to_string(),
            "-t".to_owned(),
            "4".to_owned(),
            "-c".to_owned(),
            "1024".to_owned(),
            "-np".to_owned(),
            "1".to_owned(),
            "--no-warmup".to_owned(),
            "--no-webui".to_owned(),
            "--no-slots".to_owned(),
            "--temp".to_owned(),
            "0".to_owned(),
            "-n".to_owned(),
            "24".to_owned(),
        ];
        if let Ok(extra) = std::env::var("UNO_Q_LOCAL_LLM_SERVER_ARGS") {
            args.extend(split_local_llm_args(&extra));
        }

        let mut command = std::process::Command::new(&executable);
        command
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            command.stderr(std::process::Stdio::inherit());
        } else {
            command.stderr(std::process::Stdio::null());
        }
        if let Some(dir) = local_llm_work_dir_from_env() {
            command.current_dir(dir);
        }
        if let Some(path) = local_llm_library_path_from_env() {
            command.env("LD_LIBRARY_PATH", path);
        }

        let child = command.spawn().map_err(|_| ())?;
        let mut server = Self {
            endpoint,
            child: Some(child),
        };
        server.wait_until_ready(std::time::Duration::from_secs(90))?;
        Ok(server)
    }

    fn wait_until_ready(&mut self, timeout: std::time::Duration) -> Result<(), ()> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if local_llm_server_health_ok(&self.endpoint, std::time::Duration::from_millis(500)) {
                return Ok(());
            }
            if let Some(child) = self.child.as_mut() {
                if child.try_wait().map_err(|_| ())?.is_some() {
                    return Err(());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        Err(())
    }

    fn next_command(
        &mut self,
        transcript: &[u8],
        phase: u8,
        ordinal: u8,
        context: LocalLlmPromptContext<'_>,
        out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
    ) -> Result<usize, UnoQRuntimeError> {
        let mut retry_transcript = transcript.to_vec();
        for attempt in 0..3 {
            let response = self
                .chat_complete(&retry_transcript, phase, ordinal, context)
                .or_else(|_| {
                    let prompt =
                        local_llm_prompt_for_server(&retry_transcript, phase, ordinal, context)?;
                    self.complete(&prompt)
                })?;
            if let Some(len) = copy_llm_terminal_input_from_output(&response, out) {
                if local_llm_terminal_command_admitted_for_phase(phase, &out[..len]) {
                    return Ok(len);
                }
                if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                    eprintln!(
                        "uno-q local LLM server shell retry attempt={} phase={} rejected={:?}",
                        attempt + 1,
                        phase,
                        core::str::from_utf8(&out[..len]).unwrap_or("<binary>")
                    );
                }
                append_local_llm_shell_error(&mut retry_transcript, &out[..len]);
                continue;
            }
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q local LLM server produced no terminal input: {response}");
            }
            break;
        }
        Err(UnoQRuntimeError::RuntimeViolation)
    }

    fn chat_complete(
        &mut self,
        transcript: &[u8],
        phase: u8,
        ordinal: u8,
        context: LocalLlmPromptContext<'_>,
    ) -> Result<String, UnoQRuntimeError> {
        use std::io::{Read, Write};

        let (host, port) = local_llm_http_endpoint_parts(&self.endpoint)
            .ok_or(UnoQRuntimeError::RuntimeViolation)?;
        let system = local_llm_chat_system_prompt();
        let user = local_llm_chat_user_prompt(transcript, phase, ordinal, context)?;
        let body = format!(
            "{{\"model\":\"local\",\"messages\":[{{\"role\":\"system\",\"content\":{}}},{{\"role\":\"user\",\"content\":{}}}],\"max_tokens\":24,\"temperature\":0}}",
            local_llm_json_string(system),
            local_llm_json_string(&user)
        );
        let mut stream = std::net::TcpStream::connect((host.as_str(), port))
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        let timeout = Some(std::time::Duration::from_secs(120));
        stream
            .set_read_timeout(timeout)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        stream
            .set_write_timeout(timeout)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        write!(
            stream,
            "POST /v1/chat/completions HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q local LLM server chat HTTP failure: {response}");
            }
            return Err(UnoQRuntimeError::RuntimeViolation);
        }
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or(response.as_str());
        local_llm_json_string_field(body, "content").ok_or(UnoQRuntimeError::RuntimeViolation)
    }

    fn complete(&mut self, prompt: &str) -> Result<String, UnoQRuntimeError> {
        use std::io::{Read, Write};

        let (host, port) = local_llm_http_endpoint_parts(&self.endpoint)
            .ok_or(UnoQRuntimeError::RuntimeViolation)?;
        let body = format!(
            "{{\"prompt\":{},\"n_predict\":8,\"temperature\":0,\"stop\":[\"\\n\"]}}",
            local_llm_json_string(prompt)
        );
        let mut stream = std::net::TcpStream::connect((host.as_str(), port))
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        let timeout = Some(std::time::Duration::from_secs(120));
        stream
            .set_read_timeout(timeout)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        stream
            .set_write_timeout(timeout)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        write!(
            stream,
            "POST /completion HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!("uno-q local LLM server HTTP failure: {response}");
            }
            return Err(UnoQRuntimeError::RuntimeViolation);
        }
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or(response.as_str());
        local_llm_json_string_field(body, "content").ok_or(UnoQRuntimeError::RuntimeViolation)
    }
}

#[cfg(not(target_os = "none"))]
impl Drop for LocalLlmServer {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(not(target_os = "none"))]
impl LocalLlmExternalCommand {
    fn next_command(
        &mut self,
        transcript: &[u8],
        phase: u8,
        ordinal: u8,
        context: LocalLlmPromptContext<'_>,
        out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
    ) -> Result<usize, UnoQRuntimeError> {
        let output = self.run(transcript, phase, ordinal, context)?;
        let text = String::from_utf8_lossy(&output);
        if let Some(len) = copy_llm_terminal_input_from_output(&text, out) {
            return Ok(len);
        }
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!("uno-q local LLM produced no terminal input: {text}");
        }
        Err(UnoQRuntimeError::RuntimeViolation)
    }

    fn run(
        &mut self,
        transcript: &[u8],
        phase: u8,
        ordinal: u8,
        context: LocalLlmPromptContext<'_>,
    ) -> Result<Vec<u8>, UnoQRuntimeError> {
        use std::io::Write;

        let mut args = self.args.clone();
        let pass_transcript_on_stdin = self.prompt.is_none() || phase != 0;
        if self.add_transcript_affixes && phase != 0 {
            args.push("--in-prefix".to_owned());
            args.push("\nTranscript:\n".to_owned());
            args.push("--in-suffix".to_owned());
            args.push("\nCommand:".to_owned());
        }
        if let Some(prompt) = &self.prompt {
            args.push("-p".to_owned());
            let prompt_text = if std::env::var_os("UNO_Q_LOCAL_LLM_PROMPT").is_some() {
                prompt.clone()
            } else {
                local_llm_prompt_for_phase(phase, ordinal, context)?
            };
            args.push(prompt_text);
        }
        let mut command = std::process::Command::new(&self.executable);
        command
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(dir) = &self.current_dir {
            command.current_dir(dir);
        }
        if let Some(path) = &self.ld_library_path {
            command.env("LD_LIBRARY_PATH", path);
        }
        let mut child = command
            .spawn()
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        if pass_transcript_on_stdin {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin
                    .write_all(transcript)
                    .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
            }
        }
        drop(child.stdin.take());
        if !pass_transcript_on_stdin {
            let _ = transcript;
        }
        let output = child
            .wait_with_output()
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
        if !output.status.success() {
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "uno-q local LLM command failed: status={:?} stderr={}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return Err(UnoQRuntimeError::RuntimeViolation);
        }
        Ok(output.stdout)
    }
}

#[cfg(not(target_os = "none"))]
fn local_llm_existing_path(path: &str) -> Option<String> {
    std::path::Path::new(path).exists().then(|| path.to_owned())
}

#[cfg(not(target_os = "none"))]
fn local_llm_work_dir_from_env() -> Option<String> {
    std::env::var("UNO_Q_LOCAL_LLM_WORK_DIR")
        .ok()
        .or_else(|| local_llm_existing_path(DEFAULT_UNO_Q_LOCAL_LLM_BIN_DIR))
}

#[cfg(not(target_os = "none"))]
fn local_llm_library_path_from_env() -> Option<String> {
    if let Ok(path) = std::env::var("UNO_Q_LOCAL_LLM_LD_LIBRARY_PATH") {
        return Some(path);
    }

    let mut parts = split_local_llm_args(
        &std::env::var("UNO_Q_LOCAL_LLM_LIB_DIRS").unwrap_or_else(|_| {
            format!(
                "{} {}",
                DEFAULT_UNO_Q_LOCAL_LLM_BIN_DIR, DEFAULT_UNO_Q_LOCAL_LLM_LIB_DIR
            )
        }),
    );
    parts.retain(|path| std::path::Path::new(path).exists());
    if parts.is_empty() {
        return None;
    }
    let mut path = parts.join(":");
    if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
        if !existing.is_empty() {
            path.push(':');
            path.push_str(&existing);
        }
    }
    Some(path)
}

#[cfg(not(target_os = "none"))]
fn local_llm_server_port_from_env() -> u16 {
    std::env::var("UNO_Q_LOCAL_LLM_SERVER_PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(DEFAULT_UNO_Q_LOCAL_LLM_SERVER_PORT)
}

#[cfg(not(target_os = "none"))]
fn local_llm_http_endpoint_parts(endpoint: &str) -> Option<(String, u16)> {
    let rest = endpoint.strip_prefix("http://")?;
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = authority.rsplit_once(':')?;
    Some((host.to_owned(), port.parse().ok()?))
}

#[cfg(not(target_os = "none"))]
fn local_llm_server_health_ok(endpoint: &str, timeout: std::time::Duration) -> bool {
    use std::io::{Read, Write};

    let Some((host, port)) = local_llm_http_endpoint_parts(endpoint) else {
        return false;
    };
    let Ok(mut stream) = std::net::TcpStream::connect((host.as_str(), port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    if write!(
        stream,
        "GET /health HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    )
    .is_err()
    {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok()
        && (response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
        && response.contains("\"ok\"")
}

#[cfg(not(target_os = "none"))]
fn local_llm_prompt_for_phase(
    phase: u8,
    ordinal: u8,
    context: LocalLlmPromptContext<'_>,
) -> Result<String, UnoQRuntimeError> {
    if let Ok(prompt) = std::env::var("UNO_Q_LOCAL_LLM_PROMPT") {
        return Ok(prompt);
    }
    if phase == 0 {
        return Ok(local_llm_discovery_prompt());
    }

    let self_mood_prompt =
        local_llm_self_mood_enabled().then(|| local_llm_self_mood_prompt(ordinal));
    if context.human_request.is_some()
        || self_mood_prompt.is_some()
        || context.sensor_sample.status() != PICO2W_SENSOR_STATUS_PENDING
    {
        return Ok(local_llm_joined_face_prompt(
            context,
            self_mood_prompt.as_deref(),
        ));
    }

    Ok(local_llm_default_face_prompt(
        core::str::from_utf8(local_llm_face_label_for_ordinal(ordinal)?)
            .map_err(|_| UnoQRuntimeError::RuntimeViolation)?,
    ))
}

#[cfg(not(target_os = "none"))]
fn local_llm_prompt_for_server(
    transcript: &[u8],
    phase: u8,
    ordinal: u8,
    context: LocalLlmPromptContext<'_>,
) -> Result<String, UnoQRuntimeError> {
    let prompt = local_llm_prompt_for_phase(phase, ordinal, context)?;
    if phase != 0 || transcript.is_empty() {
        return Ok(prompt);
    }
    let transcript = local_llm_transcript_tail(transcript);
    Ok(format!(
        "You are controlling the same WASI shell. Shell transcript so far:\n\
{transcript}\n\nReturn one next terminal input line.\n{prompt}"
    ))
}

#[cfg(not(target_os = "none"))]
fn local_llm_chat_system_prompt() -> &'static str {
    "You infer Uno Q's mood from sensor values, then control its WASI shell. \
Output exactly one command, no prose. Valid commands: ls; echo h > /face/frame; \
echo a > /face/frame; echo s > /face/frame; echo u > /face/frame; \
echo mc > /face/frame; echo ms > /face/frame; echo mw > /face/frame; \
echo mr > /face/frame. Use ls only for initial discovery. \
Bare commands like echo h or w /face/frame are invalid; include > /face/frame. \
For fresh Pico 2 W sensor turns without a human override, choose the face from light intensity L. \
Weak or dark light maps to echo s > /face/frame; strong or bright light maps to echo u > /face/frame; \
normal light maps to echo h > /face/frame. \
If the sensor status is stale, output echo s > /face/frame unless human input explicitly overrides it. \
For sensor turns, use only echo h > /face/frame, echo a > /face/frame, \
echo s > /face/frame, or echo u > /face/frame. \
Use echo a > /face/frame only when human input explicitly asks for anger or irritation. \
Use mc/ms/mw/mr only when the human input explicitly asks for mouth or speaking animation."
}

#[cfg(not(target_os = "none"))]
fn local_llm_chat_user_prompt(
    transcript: &[u8],
    phase: u8,
    ordinal: u8,
    context: LocalLlmPromptContext<'_>,
) -> Result<String, UnoQRuntimeError> {
    if phase == 0 {
        return Ok("Output exactly: ls".to_owned());
    }
    let feedback = local_llm_shell_feedback_line(transcript);
    if context.human_request.is_none()
        && context.sensor_sample.status() == PICO2W_SENSOR_STATUS_STALE
    {
        return Ok(format!(
            "{feedback}Do not output ls.\n\
The Pico 2 W sensor sample status is stale. Hold the tired/sad face until fresh samples return.\n\
Output exactly: echo s > /face/frame\n\
{}",
            pico2w_sensor_context_line(context.sensor_sample)
        ));
    }
    if context.human_request.is_some()
        || context.sensor_sample.status() == PICO2W_SENSOR_STATUS_FRESH
    {
        return Ok(local_llm_joined_chat_user_prompt(
            feedback.as_str(),
            context,
        ));
    }
    let label = core::str::from_utf8(local_llm_face_label_for_ordinal(ordinal)?)
        .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
    Ok(format!(
        "{feedback}Do not output ls.\nOutput exactly: echo {label} > /face/frame"
    ))
}

#[cfg(not(target_os = "none"))]
fn local_llm_transcript_tail(transcript: &[u8]) -> String {
    let start = transcript.len().saturating_sub(96);
    String::from_utf8_lossy(&transcript[start..])
        .trim()
        .to_owned()
}

#[cfg(not(target_os = "none"))]
fn local_llm_shell_feedback_line(transcript: &[u8]) -> String {
    let tail = local_llm_transcript_tail(transcript);
    if tail.contains("err ") {
        format!(
            "Shell feedback: {tail}\nCorrection: use the full redirect form, e.g. echo h > /face/frame.\n"
        )
    } else {
        String::new()
    }
}

#[cfg(not(target_os = "none"))]
fn local_llm_json_string(value: &str) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(not(target_os = "none"))]
fn local_llm_json_string_field(input: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let rest = input.split_once(&needle)?.1;
    let rest = rest.split_once(':')?.1.trim_start();
    local_llm_decode_json_string(rest)
}

#[cfg(not(target_os = "none"))]
fn local_llm_decode_json_string(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    if bytes.first().copied() != Some(b'"') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(out),
            b'\\' => {
                i += 1;
                let escaped = *bytes.get(i)?;
                match escaped {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let value = local_llm_decode_json_hex4(bytes.get(i + 1..i + 5)?)?;
                        out.push(char::from_u32(value).unwrap_or('\u{fffd}'));
                        i += 4;
                    }
                    _ => return None,
                }
                i += 1;
            }
            byte if byte < 0x20 => return None,
            _ => {
                let ch = input[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

#[cfg(not(target_os = "none"))]
fn local_llm_decode_json_hex4(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 4 {
        return None;
    }
    let mut value = 0u32;
    for &byte in bytes {
        value <<= 4;
        value |= match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a' + 10),
            b'A'..=b'F' => u32::from(byte - b'A' + 10),
            _ => return None,
        };
    }
    Some(value)
}

#[cfg(not(target_os = "none"))]
fn local_llm_self_mood_enabled() -> bool {
    std::env::var_os("UNO_Q_LOCAL_LLM_SELF_MOOD").is_some()
}

#[cfg(not(target_os = "none"))]
fn local_llm_self_mood_prompt(ordinal: u8) -> String {
    std::env::var("UNO_Q_LOCAL_LLM_SELF_MOOD_PROMPT").unwrap_or_else(|_| {
        let mood = match ordinal % 4 {
            0 => "I am happy and pleased.",
            1 => "I am frustrated and focused.",
            2 => "I am sad and tired.",
            _ => "I am surprised and curious.",
        };
        format!(
            "For this turn, the simulated assistant mood is: {mood} You do not \
need to explain the mood; return only the matching shell command."
        )
    })
}

fn face_loop_pacing_enabled() -> bool {
    #[cfg(not(target_os = "none"))]
    {
        std::env::var_os("UNO_Q_FACE_LOOP_FOREVER").is_some()
    }
    #[cfg(target_os = "none")]
    {
        false
    }
}

#[cfg(not(target_os = "none"))]
fn face_hold_us_for_ordinal(ordinal: u8) -> u64 {
    let index = usize::from(ordinal) % UNO_Q_FACE_CYCLE_FRAME_COUNT;
    let face = if index < UNO_Q_FACE_EMOTION_FRAMES.len() {
        UNO_Q_FACE_EMOTION_FRAMES[index]
    } else {
        UNO_Q_FACE_MOUTH_FRAMES[index - UNO_Q_FACE_EMOTION_FRAMES.len()]
    };
    face_hold_us(face)
}

#[cfg(not(target_os = "none"))]
fn face_hold_us(face: u8) -> u64 {
    match face {
        FACE_MOUTH_CLOSED | FACE_MOUTH_SMALL | FACE_MOUTH_WIDE | FACE_MOUTH_ROUND => {
            UNO_Q_FACE_MOUTH_HOLD_US
        }
        _ => UNO_Q_FACE_EMOTION_HOLD_US,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(not(target_os = "none"))]
enum LocalLlmTerminalCommand {
    Catalog,
    Face,
    Other,
}

#[cfg(not(target_os = "none"))]
fn classify_local_llm_terminal_command(command: &[u8]) -> LocalLlmTerminalCommand {
    let command = trim_terminal_command(command);
    if command == b"ls" {
        return LocalLlmTerminalCommand::Catalog;
    }
    if decode_local_face_echo_command(command) {
        return LocalLlmTerminalCommand::Face;
    }
    LocalLlmTerminalCommand::Other
}

#[cfg(not(target_os = "none"))]
fn trim_terminal_command(command: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = command.len();
    while start < end && command[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && command[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &command[start..end]
}

#[cfg(not(target_os = "none"))]
fn decode_local_face_echo_command(command: &[u8]) -> bool {
    let prefix = b"echo ";
    let redirect = b" > /face/frame";
    if command.len() <= prefix.len() + redirect.len()
        || &command[..prefix.len()] != prefix
        || &command[command.len() - redirect.len()..] != redirect
    {
        return false;
    }
    matches!(
        &command[prefix.len()..command.len() - redirect.len()],
        b"h" | b"s" | b"a" | b"u" | b"v" | b"mc" | b"ms" | b"mw" | b"mr"
    )
}

#[cfg(not(target_os = "none"))]
fn local_llm_terminal_command_admitted_for_phase(phase: u8, command: &[u8]) -> bool {
    match classify_local_llm_terminal_command(command) {
        LocalLlmTerminalCommand::Face => phase != 0,
        LocalLlmTerminalCommand::Catalog | LocalLlmTerminalCommand::Other => phase == 0,
    }
}

#[cfg(not(target_os = "none"))]
fn append_local_llm_shell_error(transcript: &mut Vec<u8>, command: &[u8]) {
    transcript.extend_from_slice(command);
    if !command.ends_with(b"\n") {
        transcript.push(b'\n');
    }
    transcript.extend_from_slice(b"err /face/frame h,a,s,u,mw\n$ ");
    if transcript.len() > LOCAL_LLM_TRANSCRIPT_BYTES {
        let excess = transcript.len() - LOCAL_LLM_TRANSCRIPT_BYTES;
        transcript.drain(..excess);
    }
}

fn scripted_local_llm_shell_command(
    phase: u8,
    ordinal: u8,
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<usize, UnoQRuntimeError> {
    if phase == 0 {
        return copy_command_bytes(b"ls\n", out);
    }
    let label = local_llm_face_label_for_ordinal(ordinal)?;
    copy_face_echo_command(label, out)
}

#[cfg(not(target_os = "none"))]
fn scripted_human_request_shell_command(
    request: &str,
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<Option<usize>, UnoQRuntimeError> {
    for (needle, label) in [
        ("echo h > /face/frame", b"h" as &[u8]),
        ("echo a > /face/frame", b"a" as &[u8]),
        ("echo s > /face/frame", b"s" as &[u8]),
        ("echo u > /face/frame", b"u" as &[u8]),
        ("echo mc > /face/frame", b"mc" as &[u8]),
        ("echo ms > /face/frame", b"ms" as &[u8]),
        ("echo mw > /face/frame", b"mw" as &[u8]),
        ("echo mr > /face/frame", b"mr" as &[u8]),
    ] {
        if request.contains(needle) {
            return copy_face_echo_command(label, out).map(Some);
        }
    }
    Ok(None)
}

#[cfg(not(target_os = "none"))]
fn scripted_pico2w_sensor_shell_command(
    sample: Pico2wSensorSample,
    human_input_present: bool,
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<Option<usize>, UnoQRuntimeError> {
    if sample.status() == PICO2W_SENSOR_STATUS_PENDING {
        return Ok(None);
    }
    if sample.status() == PICO2W_SENSOR_STATUS_STALE && !human_input_present {
        return copy_face_echo_command(b"s", out).map(Some);
    }
    if sample.status() != PICO2W_SENSOR_STATUS_FRESH {
        return Ok(None);
    }
    copy_face_echo_command(pico2w_light_face_label(sample.light_raw()), out).map(Some)
}

#[cfg(not(target_os = "none"))]
fn pico2w_light_face_label(light_raw: u16) -> &'static [u8] {
    if light_raw <= PICO2W_LIGHT_WEAK_RAW {
        b"s"
    } else if light_raw >= PICO2W_LIGHT_STRONG_RAW {
        b"u"
    } else {
        b"h"
    }
}

fn copy_face_echo_command(
    label: &[u8],
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<usize, UnoQRuntimeError> {
    let mut len = 0usize;
    for bytes in [b"echo " as &[u8], label, b" > /face/frame\n"] {
        if len + bytes.len() > out.len() {
            return Err(UnoQRuntimeError::RuntimeViolation);
        }
        out[len..len + bytes.len()].copy_from_slice(bytes);
        len += bytes.len();
    }
    Ok(len)
}

#[cfg(not(target_os = "none"))]
fn copy_command_str_line(
    command: &str,
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<usize, UnoQRuntimeError> {
    let bytes = command.as_bytes();
    if bytes.is_empty() || bytes.len() + 1 > out.len() {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    out[..bytes.len()].copy_from_slice(bytes);
    out[bytes.len()] = b'\n';
    Ok(bytes.len() + 1)
}

fn local_llm_face_label_for_ordinal(ordinal: u8) -> Result<&'static [u8], UnoQRuntimeError> {
    let index = usize::from(ordinal) % UNO_Q_FACE_CYCLE_FRAME_COUNT;
    let face = if index < UNO_Q_FACE_EMOTION_FRAMES.len() {
        UNO_Q_FACE_EMOTION_FRAMES[index]
    } else {
        UNO_Q_FACE_MOUTH_FRAMES[index - UNO_Q_FACE_EMOTION_FRAMES.len()]
    };
    face_shell_label(face)
}

fn copy_command_bytes(
    bytes: &[u8],
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Result<usize, UnoQRuntimeError> {
    if bytes.is_empty() || bytes.len() > out.len() {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(bytes.len())
}

fn face_shell_label(face: u8) -> Result<&'static [u8], UnoQRuntimeError> {
    match face {
        FACE_HAPPY => Ok(b"h"),
        FACE_ANGRY => Ok(b"a"),
        FACE_SAD => Ok(b"s"),
        FACE_SURPRISED => Ok(b"u"),
        FACE_MOUTH_CLOSED => Ok(b"mc"),
        FACE_MOUTH_SMALL => Ok(b"ms"),
        FACE_MOUTH_WIDE => Ok(b"mw"),
        FACE_MOUTH_ROUND => Ok(b"mr"),
        _ => Err(UnoQRuntimeError::RuntimeViolation),
    }
}

#[cfg(not(target_os = "none"))]
fn split_local_llm_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(core::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[cfg(not(target_os = "none"))]
fn default_local_llm_shell_prompt() -> &'static str {
    "You are controlling a WASI shell. Read the transcript from stdin and return \
one terminal input line. First run `ls` to discover ChoreoFS. After the shell \
lists ChoreoFS, you may run `echo <code> > /face/frame` to change the face. \
The WASI shell and hibana choreography decide which effects are admitted."
}

#[cfg(not(target_os = "none"))]
fn local_llm_discovery_prompt() -> String {
    "Shell command examples:\nTask: list files\nCommand: ls\nTask: list directory\nCommand: ls\nTask: start by listing files\nCommand:"
        .to_owned()
}

#[cfg(all(test, not(target_os = "none")))]
fn local_llm_human_face_prompt(request: &str) -> String {
    let context = LocalLlmPromptContext {
        human_request: Some(request),
        sensor_sample: Pico2wSensorSample::pending(0),
    };
    local_llm_joined_face_prompt(context, None)
}

#[cfg(not(target_os = "none"))]
fn local_llm_joined_face_prompt(
    context: LocalLlmPromptContext<'_>,
    assistant_mood: Option<&str>,
) -> String {
    let human = match context.human_request {
        Some(request) => format!("Human input:\n{request}\n\n"),
        None => String::new(),
    };
    let assistant = match assistant_mood {
        Some(request) => format!("Assistant mood instruction:\n{request}\n\n"),
        None => String::new(),
    };
    let sensor = pico2w_sensor_context_line(context.sensor_sample);
    let stale_rule = if context.sensor_sample.status() == PICO2W_SENSOR_STATUS_STALE
        && context.human_request.is_none()
        && assistant_mood.is_none()
    {
        "The Pico 2 W sensor status is stale; return echo s > /face/frame.\n"
    } else {
        ""
    };
    format!(
        "System prompt: You control Uno Q's face through a WASI ChoreoFS shell.\n\
{human}{assistant}Pico 2 W sensor:\n{sensor}\n\n\
Respect explicit human override when present; otherwise choose Uno Q's mood from the \
Pico 2 W sensor light intensity when it is fresh: weak/dark light is sad, normal light is happy, \
and strong/bright light is surprised.\n\
{stale_rule}\
Valid commands are echo h > /face/frame, echo a > /face/frame, echo s > /face/frame, \
echo u > /face/frame, echo mc > /face/frame, echo ms > /face/frame, \
echo mw > /face/frame, and echo mr > /face/frame.\n\
Bare commands like echo h are invalid; include > /face/frame.\n\
For fresh sensor input choose s, h, or u from light intensity. Use a and mouth commands only for explicit human input.\n\
For plain text, choose the closest valid face command from the user's requested mood.\n\
Return exactly one shell command and no explanation.\n\
Command:"
    )
}

#[cfg(not(target_os = "none"))]
fn local_llm_joined_chat_user_prompt(feedback: &str, context: LocalLlmPromptContext<'_>) -> String {
    let human = match context.human_request {
        Some(request) => format!("Human input:\n{request}\n"),
        None => String::new(),
    };
    let sensor = pico2w_sensor_context_line(context.sensor_sample);
    format!(
        "{feedback}Do not output ls.\n\
{human}Pico 2 W sensor:\n{sensor}\n\
Respect explicit human override when present; otherwise choose the mood from light intensity L: weak/dark=s, normal=h, strong/bright=u.\n\
For sensor input choose only s, h, or u unless human input explicitly asks for another valid face.\n\
Use the exact full form, for example: echo h > /face/frame"
    )
}

#[cfg(not(target_os = "none"))]
fn pico2w_sensor_context_line(sample: Pico2wSensorSample) -> String {
    match sample.status() {
        PICO2W_SENSOR_STATUS_FRESH => format!(
            "status=fresh seq={} T={:.1}C H={:.1}% L={}",
            sample.seq(),
            f32::from(sample.temperature_c_x10()) / 10.0,
            f32::from(sample.humidity_pct_x10()) / 10.0,
            sample.light_raw()
        ),
        PICO2W_SENSOR_STATUS_STALE => format!(
            "status=stale seq={} last_T={:.1}C last_H={:.1}% last_L={}",
            sample.seq(),
            f32::from(sample.temperature_c_x10()) / 10.0,
            f32::from(sample.humidity_pct_x10()) / 10.0,
            sample.light_raw()
        ),
        PICO2W_SENSOR_STATUS_PENDING => {
            format!("status=pending seq={} no sample yet", sample.seq())
        }
        _ => "status=invalid".to_owned(),
    }
}

#[cfg(not(target_os = "none"))]
fn local_llm_default_face_prompt(label: &str) -> String {
    format!(
        "Task: write the single face code {label}. Valid shell syntax is echo CODE > /face/frame.\n\
Answer with exactly: echo {label} > /face/frame\nAnswer:"
    )
}

#[cfg(not(target_os = "none"))]
fn copy_llm_terminal_input_from_output(
    output: &str,
    out: &mut [u8; LOCAL_LLM_COMMAND_BYTES],
) -> Option<usize> {
    for line in output.lines() {
        let input = line.trim();
        if input.is_empty() || input == "[end of text]" {
            continue;
        }
        let bytes = input.as_bytes();
        let copy_len = bytes.len().min(out.len());
        if copy_len == 0 {
            return None;
        }
        out[..copy_len].copy_from_slice(&bytes[..copy_len]);
        if copy_len == out.len() {
            return Some(copy_len);
        }
        out[copy_len] = b'\n';
        return Some(copy_len + 1);
    }
    None
}

impl appkit::Localside<UnoQCapsule> for UnoQLocal {
    type Error = UnoQRuntimeError;

    fn engine<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn driver<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_m33_driver(ctx)
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_boundary(ctx)
    }
}

fn expect_path_open(request: PathOpenReq) -> Result<PathOpen, UnoQRuntimeError> {
    Ok(request.0)
}

fn expect_fd_write(request: FdWriteReq) -> Result<FdWrite, UnoQRuntimeError> {
    Ok(request.0)
}

fn expect_fd_read(request: FdReadReq) -> Result<FdRead, UnoQRuntimeError> {
    Ok(request.0)
}

async fn complete_path_open<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    request: PathOpen,
    expected_path: &[u8],
    expected_rights: u64,
) -> Result<(), UnoQRuntimeError> {
    if request.preopen_fd() != PREOPEN_FD || request.rights_base() != expected_rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    if request.path() != expected_path {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let Some(object) = UNO_Q_DRIVER_FACTS.choreofs().facts().resolve(expected_path) else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    let Some(fd) = UNO_Q_DRIVER_FACTS
        .choreofs()
        .ledger()
        .fds()
        .iter()
        .copied()
        .find(|fact| fact.object() == object && fact.rights() == expected_rights)
    else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    let binding = fd_binding_for_rights(expected_rights)?;
    ctx.send::<PathOpenRetMsg>(&PathOpenedRet(PathOpened::new_with_binding(
        fd.fd() as u8,
        0,
        binding,
    )))
    .await?;
    Ok(())
}

async fn complete_boundary_path_open<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    request: PathOpen,
    expected_path: &[u8],
    expected_rights: u64,
    returned_fd: u8,
) -> Result<(), UnoQRuntimeError> {
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q boundary path_open role={} preopen={} rights={} path={:?} expected_rights={} expected_path={:?}",
            ROLE,
            request.preopen_fd(),
            request.rights_base(),
            core::str::from_utf8(request.path()).unwrap_or("<binary>"),
            expected_rights,
            core::str::from_utf8(expected_path).unwrap_or("<binary>")
        );
    }
    if request.preopen_fd() != PREOPEN_FD || request.rights_base() != expected_rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    if request.path() != expected_path {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let reply = PathOpenedRet(PathOpened::new_with_binding(
        returned_fd,
        0,
        fd_binding_for_rights(expected_rights)?,
    ));
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q boundary path_open_ret role={} fd={} path={:?}",
            ROLE,
            returned_fd,
            core::str::from_utf8(expected_path).unwrap_or("<binary>")
        );
    }
    ctx.send::<PathOpenRetMsg>(&reply).await?;
    #[cfg(not(target_os = "none"))]
    if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
        eprintln!(
            "uno-q boundary path_open_ret sent role={} fd={}",
            ROLE, returned_fd
        );
    }
    Ok(())
}

fn fd_binding_for_rights(rights: u64) -> Result<FdBinding, UnoQRuntimeError> {
    match rights {
        FD_READ_RIGHT => Ok(FdBinding::read(FdReadRow::Base)),
        FD_WRITE_RIGHT => Ok(FdBinding::write(FdWriteRow::Base)),
        _ => Err(UnoQRuntimeError::RuntimeViolation),
    }
}

fn expect_fd_object<const ROLE: u8>(
    ctx: &hibana::Endpoint<'_, ROLE>,
    fd: u8,
    object: choreofs::ObjectId,
    rights: u64,
) -> Result<(), UnoQRuntimeError> {
    let Some(fact) = UNO_Q_DRIVER_FACTS.choreofs().ledger().fd(fd as u32) else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    if fact.object() != object || fact.rights() != rights {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    Ok(())
}

async fn send_fd_write_done<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    fd: u8,
    len: usize,
) -> Result<(), UnoQRuntimeError> {
    ctx.send::<FdWriteRetMsg>(&FdWriteDoneRet(FdWriteDone::new(fd, len as u8)))
        .await?;
    Ok(())
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
    source_role: u8,
    frame_label: Option<u8>,
    hint_frame_label: Cell<Option<u8>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

fn proof_frame_header(
    session_id: u32,
    lane: u8,
    source_role: u8,
    peer_role: u8,
    frame_label: u8,
) -> hibana::runtime::transport::FrameHeader {
    let session = session_id.to_be_bytes();
    hibana::runtime::transport::FrameHeader::from_bytes([
        session[0],
        session[1],
        session[2],
        session[3],
        lane,
        source_role,
        peer_role,
        frame_label,
    ])
}

fn proof_received_frame<'a>(
    session_id: u32,
    lane: u8,
    source_role: u8,
    peer_role: u8,
    frame_label: u8,
    bytes: &'a [u8],
) -> hibana::runtime::transport::ReceivedFrame<'a> {
    hibana::runtime::transport::ReceivedFrame::framed(
        proof_frame_header(session_id, lane, source_role, peer_role, frame_label),
        Payload::new(bytes),
    )
}

#[derive(Clone, Copy)]
struct ProofFrame {
    occupied: bool,
    lane: u8,
    source: u8,
    frame_label: u8,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

impl ProofFrame {
    const EMPTY: Self = Self {
        occupied: false,
        lane: 0,
        source: 0,
        frame_label: 0,
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
        source: u8,
        frame_label: u8,
        payload: Payload<'_>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > PROOF_CARRIER_FRAME_BYTES || self.len == PROOF_CARRIER_QUEUE_DEPTH {
            return Err(hibana::runtime::transport::TransportError::Failed);
        }
        let idx = (self.head + self.len) % PROOF_CARRIER_QUEUE_DEPTH;
        self.frames[idx].occupied = true;
        self.frames[idx].lane = lane;
        self.frames[idx].source = source;
        self.frames[idx].frame_label = frame_label;
        self.frames[idx].len = bytes.len();
        self.frames[idx].bytes[..bytes.len()].copy_from_slice(bytes);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, lane: u8, source: u8, frame_label: u8, bytes: &[u8]) {
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
        self.frames[self.head].source = source;
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
}

struct ProofQueues {
    by_role: [ProofQueue; PROOF_CARRIER_ROLES],
}

impl ProofQueues {
    const EMPTY: Self = Self {
        by_role: [ProofQueue::EMPTY; PROOF_CARRIER_ROLES],
    };
}

#[cfg(not(target_os = "none"))]
static HARDWARE_PEER_LOCAL_QUEUES: std::sync::Mutex<ProofQueues> =
    std::sync::Mutex::new(ProofQueues::EMPTY);

#[cfg(not(target_os = "none"))]
fn edit_hardware_peer_local_queues<R>(
    f: impl FnOnce(&mut ProofQueues) -> R,
) -> Result<R, hibana::runtime::transport::TransportError> {
    let mut queues = HARDWARE_PEER_LOCAL_QUEUES
        .lock()
        .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
    Ok(f(&mut queues))
}

impl ProofCarrier {
    const fn new() -> Self {
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
    frame_label: u8,
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
            frame_label: self.buffer[11],
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
    frame_label: hibana::runtime::transport::FrameLabel,
    payload: Payload<'_>,
) -> Result<usize, hibana::runtime::transport::TransportError> {
    let bytes = payload.as_bytes();
    if bytes.len() > PROOF_CARRIER_FRAME_BYTES {
        return Err(hibana::runtime::transport::TransportError::Failed);
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
    fn uno_q_m33_carrier_observe_open(role: u8, lane: u8, session_id: u32);
    fn uno_q_m33_carrier_observe_parsed(session_id: u32, source: u8, peer: u8, label: u8, len: u8);
    fn uno_q_m33_carrier_observe_reject(
        reason: u8,
        expected_session_id: u32,
        frame_session_id: u32,
        source: u8,
        peer: u8,
        label: u8,
        len: u8,
    );
    fn uno_q_m33_carrier_observe_payload(label: u8, len: u8, byte0: u8, byte1: u8);
    fn uno_q_m33_carrier_observe_tx(peer: u8, label: u8, len: u8);
    fn uno_q_m33_carrier_observe_deadline(op: u8, role: u8, lane: u8, elapsed: u32);
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
    deadline_start_ticks: u32,
}

#[cfg(target_os = "none")]
pub struct UnoQUartRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    source_role: u8,
    frame_label: Option<u8>,
    hint_frame_label: Cell<Option<u8>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
    deadline_start_ticks: u32,
}

#[cfg(target_os = "none")]
impl UnoQUartCarrier {
    const fn new() -> Self {
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

    fn timer_ticks(&self) -> u32 {
        unsafe { uno_q_m33_timer_ticks() }
    }

    fn deadline_elapsed(&self, start_ticks: u32) -> Option<u32> {
        let elapsed = self.timer_ticks().wrapping_sub(start_ticks);
        if elapsed >= UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS {
            Some(elapsed)
        } else {
            None
        }
    }

    fn observe_deadline(&self, op: u8, role: u8, lane: u8, elapsed: u32) {
        unsafe {
            uno_q_m33_carrier_observe_deadline(op, role, lane, elapsed);
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
            unsafe {
                uno_q_m33_carrier_observe_parsed(
                    frame.session_id,
                    frame.source,
                    frame.peer,
                    frame.frame_label,
                    frame.len as u8,
                );
            }
            let reject_reason = if frame.session_id != session_id {
                Some(1)
            } else if frame.peer as usize >= PROOF_CARRIER_ROLES {
                Some(2)
            } else if frame.source as usize >= PROOF_CARRIER_ROLES {
                Some(3)
            } else if frame.source == frame.peer {
                Some(4)
            } else {
                None
            };
            if let Some(reason) = reject_reason {
                unsafe {
                    uno_q_m33_carrier_observe_reject(
                        reason,
                        session_id,
                        frame.session_id,
                        frame.source,
                        frame.peer,
                        frame.frame_label,
                        frame.len as u8,
                    );
                }
                continue;
            }
            unsafe {
                uno_q_m33_carrier_observe_frame(
                    frame.source,
                    frame.peer,
                    frame.frame_label,
                    frame.len as u8,
                );
            }
            self.edit(|queues| {
                queues.by_role[frame.peer as usize].push_back(
                    frame.lane,
                    frame.source,
                    frame.frame_label,
                    Payload::new(&frame.bytes[..frame.len]),
                )
            })
            .ok();
        }
    }
}

#[cfg(target_os = "none")]
impl hibana::runtime::transport::Transport for UnoQUartCarrier {
    type Tx<'a>
        = UnoQUartTx
    where
        Self: 'a;
    type Rx<'a>
        = UnoQUartRx
    where
        Self: 'a;
    fn open<'a>(
        &'a self,
        port: hibana::runtime::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane();
        let deadline_start_ticks = self.timer_ticks();
        unsafe {
            uno_q_m33_carrier_observe_open(local_role, lane, session_id);
        }
        (
            UnoQUartTx {
                local_role,
                session_id,
                lane,
                deadline_start_ticks,
            },
            UnoQUartRx {
                local_role,
                session_id,
                lane,
                source_role: 0,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
                deadline_start_ticks,
            },
        )
    }

    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::runtime::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), hibana::runtime::transport::TransportError>>
    where
        'a: 'f,
    {
        self.service_board();
        if tx.session_id == 0
            || outgoing.target_role() == tx.local_role
            || outgoing.lane() != tx.lane
        {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        let mut frame = [0u8; UART_CARRIER_FRAME_BYTES];
        let len = encode_uart_frame(
            &mut frame,
            tx.session_id,
            outgoing.lane(),
            tx.local_role,
            outgoing.target_role(),
            outgoing.frame_label(),
            outgoing.payload(),
        )?;
        unsafe {
            uno_q_m33_carrier_observe_tx(
                outgoing.target_role(),
                outgoing.frame_label().raw(),
                outgoing.payload().as_bytes().len() as u8,
            );
        }
        for &byte in &frame[..len] {
            unsafe {
                uno_q_m33_carrier_write(byte);
            }
            if let Some(elapsed) = self.deadline_elapsed(tx.deadline_start_ticks) {
                self.observe_deadline(1, tx.local_role, tx.lane, elapsed);
                return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
            }
        }
        tx.deadline_start_ticks = self.timer_ticks();
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<
        Result<
            hibana::runtime::transport::ReceivedFrame<'a>,
            hibana::runtime::transport::TransportError,
        >,
    > {
        self.drain_uart(rx.session_id);
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        let Some(frame) = self.edit(|queues| queues.by_role[local_role].pop_front(rx.lane)) else {
            if let Some(elapsed) = self.deadline_elapsed(rx.deadline_start_ticks) {
                self.observe_deadline(2, rx.local_role, rx.lane, elapsed);
                return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
            }
            task_context.waker().wake_by_ref();
            return Poll::Pending;
        };
        rx.deadline_start_ticks = self.timer_ticks();
        rx.frame_label = Some(frame.frame_label);
        rx.source_role = frame.source;
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        unsafe {
            let byte0 = if rx.len > 0 { rx.bytes[0] } else { 0 };
            let byte1 = if rx.len > 1 { rx.bytes[1] } else { 0 };
            uno_q_m33_carrier_observe_payload(frame.frame_label, frame.len as u8, byte0, byte1);
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(proof_received_frame(
            rx.session_id,
            rx.lane,
            frame.source,
            rx.local_role,
            frame.frame_label,
            &rx.bytes[..rx.len],
        )))
    }

    fn requeue<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.edit(|queues| {
                    queues.by_role[local_role].push_front(
                        rx.lane,
                        rx.source_role,
                        frame_label,
                        &rx.bytes[..rx.len],
                    )
                });
            }
        }
        rx.hint_frame_label.set(None);
        Ok(())
    }
}

#[cfg(not(target_os = "none"))]
pub struct HardwarePeerCarrier {
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
    source_role: u8,
    frame_label: Option<u8>,
    hint_frame_label: Cell<Option<u8>>,
    len: usize,
    bytes: [u8; PROOF_CARRIER_FRAME_BYTES],
}

#[cfg(not(target_os = "none"))]
impl HardwarePeerCarrier {
    fn new() -> Self {
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
        configure_uno_q_uart_nonblocking(&serial).unwrap_or_else(|error| {
            panic!("failed to put hibana UART carrier {path} in nonblocking mode: {error}")
        });
        Self {
            serial_path: path,
            serial: std::sync::Mutex::new(serial),
            parser: std::sync::Mutex::new(UartFrameParser::new()),
        }
    }

    fn drain_serial(
        &self,
        session_id: u32,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        use std::io::Read;

        let mut bytes = [0u8; 64];
        let read_len = {
            let mut serial = self
                .serial
                .lock()
                .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
            match serial.read(&mut bytes) {
                Ok(len) => len,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    0
                }
                Err(_) => return Err(hibana::runtime::transport::TransportError::Failed),
            }
        };
        if read_len == 0 {
            return Ok(());
        }

        let mut parser = self
            .parser
            .lock()
            .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
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
                    frame.frame_label,
                    frame.len
                );
            }
            let pushed = edit_hardware_peer_local_queues(|queues| {
                queues.by_role[frame.peer as usize].push_back(
                    frame.lane,
                    frame.source,
                    frame.frame_label,
                    Payload::new(&frame.bytes[..frame.len]),
                )
            })?;
            pushed?;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "none"))]
impl hibana::runtime::transport::Transport for HardwarePeerCarrier {
    type Tx<'a>
        = HardwarePeerTx
    where
        Self: 'a;
    type Rx<'a>
        = HardwarePeerRx
    where
        Self: 'a;
    fn open<'a>(
        &'a self,
        port: hibana::runtime::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane();
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
                source_role: 0,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::runtime::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), hibana::runtime::transport::TransportError>>
    where
        'a: 'f,
    {
        if tx.session_id == 0
            || outgoing.target_role() == tx.local_role
            || outgoing.lane() != tx.lane
        {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        if outgoing.target_role() == ROLE_M33_LED_KERNEL {
            use std::io::Write;

            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-uart tx session={} lane={} {}->{} label={} len={}",
                    tx.session_id,
                    outgoing.lane(),
                    tx.local_role,
                    outgoing.target_role(),
                    outgoing.frame_label().raw(),
                    outgoing.payload().as_bytes().len()
                );
            }
            let turnaround_us = std::env::var("UNO_Q_HIBANA_UART_TURNAROUND_US")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(200_000);
            if turnaround_us != 0 {
                std::thread::sleep(std::time::Duration::from_micros(turnaround_us));
            }
            let mut frame = [0u8; UART_CARRIER_FRAME_BYTES];
            let len = encode_uart_frame(
                &mut frame,
                tx.session_id,
                outgoing.lane(),
                tx.local_role,
                outgoing.target_role(),
                outgoing.frame_label(),
                outgoing.payload(),
            )?;
            let mut serial = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.serial_path)
                .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
            configure_uno_q_uart_modem_ready(&serial)
                .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
            let byte_delay_us = std::env::var("UNO_Q_HIBANA_UART_BYTE_US")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(50_000);
            for &byte in &frame[..len] {
                serial
                    .write_all(&[byte])
                    .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
                serial
                    .flush()
                    .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
                drain_uno_q_uart_byte(&serial)
                    .map_err(|_| hibana::runtime::transport::TransportError::Failed)?;
                if byte_delay_us != 0 {
                    std::thread::sleep(std::time::Duration::from_micros(byte_delay_us));
                }
            }
        } else {
            let peer = outgoing.target_role() as usize;
            if peer >= PROOF_CARRIER_ROLES {
                return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
            }
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-local tx session={} lane={} {}->{} label={} len={}",
                    tx.session_id,
                    outgoing.lane(),
                    tx.local_role,
                    outgoing.target_role(),
                    outgoing.frame_label().raw(),
                    outgoing.payload().as_bytes().len()
                );
            }
            let pushed = edit_hardware_peer_local_queues(|queues| {
                queues.by_role[peer].push_back(
                    outgoing.lane(),
                    tx.local_role,
                    outgoing.frame_label().raw(),
                    outgoing.payload(),
                )
            })?;
            pushed?;
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<
        Result<
            hibana::runtime::transport::ReceivedFrame<'a>,
            hibana::runtime::transport::TransportError,
        >,
    > {
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        if let Some(frame) =
            edit_hardware_peer_local_queues(|queues| queues.by_role[local_role].pop_front(rx.lane))?
        {
            rx.frame_label = Some(frame.frame_label);
            rx.source_role = frame.source;
            rx.hint_frame_label.set(Some(frame.frame_label));
            rx.len = frame.len;
            rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
            if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
                eprintln!(
                    "hibana-local rx session={} lane={} role={} label={} len={} bytes={:?}",
                    rx.session_id,
                    rx.lane,
                    rx.local_role,
                    frame.frame_label,
                    frame.len,
                    &rx.bytes[..rx.len]
                );
            }
            task_context.waker().wake_by_ref();
            return Poll::Ready(Ok(proof_received_frame(
                rx.session_id,
                rx.lane,
                frame.source,
                rx.local_role,
                frame.frame_label,
                &rx.bytes[..rx.len],
            )));
        }
        self.drain_serial(rx.session_id)?;
        let Some(frame) = edit_hardware_peer_local_queues(|queues| {
            queues.by_role[local_role].pop_front(rx.lane)
        })?
        else {
            task_context.waker().wake_by_ref();
            return Poll::Pending;
        };
        rx.frame_label = Some(frame.frame_label);
        rx.source_role = frame.source;
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!(
                "hibana-local rx session={} lane={} role={} label={} len={} bytes={:?}",
                rx.session_id,
                rx.lane,
                rx.local_role,
                frame.frame_label,
                frame.len,
                &rx.bytes[..rx.len]
            );
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(proof_received_frame(
            rx.session_id,
            rx.lane,
            frame.source,
            rx.local_role,
            frame.frame_label,
            &rx.bytes[..rx.len],
        )))
    }

    fn requeue<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                edit_hardware_peer_local_queues(|queues| {
                    queues.by_role[local_role].push_front(
                        rx.lane,
                        rx.source_role,
                        frame_label,
                        &rx.bytes[..rx.len],
                    )
                })?;
            }
        }
        rx.hint_frame_label.set(None);
        Ok(())
    }
}

impl hibana::runtime::transport::Transport for ProofCarrier {
    type Tx<'a>
        = ProofTx
    where
        Self: 'a;
    type Rx<'a>
        = ProofRx
    where
        Self: 'a;
    fn open<'a>(
        &'a self,
        port: hibana::runtime::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane();
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
                source_role: 0,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; PROOF_CARRIER_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::runtime::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), hibana::runtime::transport::TransportError>>
    where
        'a: 'f,
    {
        if tx.session_id == 0
            || outgoing.target_role() == tx.local_role
            || outgoing.lane() != tx.lane
        {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        let peer = outgoing.target_role() as usize;
        if peer >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        #[cfg(not(target_os = "none"))]
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!(
                "proof-carrier tx session={} lane={} {}->{} label={} len={}",
                tx.session_id,
                outgoing.lane(),
                tx.local_role,
                outgoing.target_role(),
                outgoing.frame_label().raw(),
                outgoing.payload().as_bytes().len()
            );
        }
        self.edit(|queues| {
            queues.by_role[peer].push_back(
                outgoing.lane(),
                tx.local_role,
                outgoing.frame_label().raw(),
                outgoing.payload(),
            )
        })?;
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<
        Result<
            hibana::runtime::transport::ReceivedFrame<'a>,
            hibana::runtime::transport::TransportError,
        >,
    > {
        if rx.session_id == 0 {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return Poll::Ready(Err(hibana::runtime::transport::TransportError::Failed));
        }
        let Some(frame) = self.edit(|queues| queues.by_role[local_role].pop_front(rx.lane)) else {
            return Poll::Pending;
        };
        rx.frame_label = Some(frame.frame_label);
        rx.source_role = frame.source;
        rx.hint_frame_label.set(Some(frame.frame_label));
        rx.len = frame.len;
        rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        #[cfg(not(target_os = "none"))]
        if std::env::var_os("UNO_Q_HIBANA_TRACE").is_some() {
            eprintln!(
                "proof-carrier rx session={} lane={} role={} label={} len={} bytes={:?}",
                rx.session_id,
                rx.lane,
                rx.local_role,
                frame.frame_label,
                frame.len,
                &rx.bytes[..rx.len]
            );
        }
        task_context.waker().wake_by_ref();
        Poll::Ready(Ok(proof_received_frame(
            rx.session_id,
            rx.lane,
            frame.source,
            rx.local_role,
            frame.frame_label,
            &rx.bytes[..rx.len],
        )))
    }

    fn requeue<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.edit(|queues| {
                    queues.by_role[local_role].push_front(
                        rx.lane,
                        rx.source_role,
                        frame_label,
                        &rx.bytes[..rx.len],
                    )
                });
            }
        }
        rx.hint_frame_label.set(None);
        Ok(())
    }
}

macro_rules! impl_nowasi_image {
    ($image:ty, $roles:expr, $storage:ident) => {
        impl appkit::LogicalImage for $image {
            type Capsule = UnoQCapsule;

            type Carrier<'a>
                = ProofCarrier
            where
                Self: 'a,
                UnoQCapsule: 'a;
            const REQUESTED_ROLES: appkit::RoleSet = $roles;

            fn init() -> Self {
                Self
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
        }
    };
}

impl appkit::LogicalImage for image::HostLoopbackProof {
    type Capsule = UnoQCapsule;

    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0x1f);

    fn init() -> Self {
        Self
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
}

macro_rules! impl_hardware_peer_wasi_image {
    ($image:ty) => {
        impl appkit::LogicalImage for $image {
            type Capsule = UnoQCapsule;

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
            const REQUESTED_ROLES: appkit::RoleSet =
                appkit::RoleSet::from_bits(HARDWARE_PEER_ROLE_BITS);

            fn init() -> Self {
                Self
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
        }
    };
}

impl_hardware_peer_wasi_image!(image::HardwarePeerProof);
impl_hardware_peer_wasi_image!(image::HardwarePeerLoopProof);

impl appkit::LogicalImage for image::WasiLlmCellProcess {
    type Capsule = UnoQCapsule;

    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        UnoQCapsule: 'a;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_WASI_LLM_CELL);

    fn init() -> Self {
        Self
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
    image::LocalLlmProcess,
    appkit::RoleSet::single(ROLE_LOCAL_LLM),
    LOCAL_LLM_ATTACH_STORAGE
);

impl_nowasi_image!(
    image::HumanInputProcess,
    appkit::RoleSet::single(ROLE_HUMAN_INPUT),
    HUMAN_INPUT_ATTACH_STORAGE
);

impl_nowasi_image!(
    image::Pico2wSensorProcess,
    appkit::RoleSet::single(ROLE_PICO2W_SENSOR),
    PICO2W_SENSOR_ATTACH_STORAGE
);

impl appkit::LogicalImage for image::M33LedKernelImage {
    type Capsule = UnoQCapsule;

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
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_M33_LED_KERNEL);

    fn init() -> Self {
        Self
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
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage for image::HostLoopbackProof {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage for image::HardwarePeerProof {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage for image::HardwarePeerLoopProof {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage for image::WasiLlmCellProcess {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        uno_q_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hibana::runtime::wire::WireEncode;

    #[cfg(not(target_os = "none"))]
    fn prompt_context(
        human_request: Option<&str>,
        sensor_sample: Pico2wSensorSample,
    ) -> LocalLlmPromptContext<'_> {
        LocalLlmPromptContext {
            human_request,
            sensor_sample,
        }
    }

    #[test]
    fn embedded_uart_deadline_covers_paced_physical_frames() {
        let full_frame_bytes = UART_CARRIER_FRAME_BYTES as u32;
        assert!(
            UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS
                > UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS * 10
        );
        assert!(
            UNO_Q_M33_UART_OPERATIONAL_DEADLINE_TICKS
                > UNO_Q_HOST_UART_OPERATIONAL_DEADLINE_TICKS * 20
        );
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
            text(&['.', 'r', 'o', 'l', 'l', '(', ')', ';']),
            text(&[
                'W', 'a', 's', 'i', 'P', 'r', 'o', 'c', 'E', 'x', 'i', 't', 'R', 'e', 'q', 'M',
                's', 'g',
            ]),
            text(&[
                'F', 'A', 'C', 'E', '_', 'F', 'R', 'A', 'M', 'E', '_', 'P', 'A', 'T', 'H',
            ]),
            text(&[
                'L', 'L', 'M', '_', 'S', 'T', 'D', 'I', 'N', '_', 'P', 'A', 'T', 'H',
            ]),
            text(&[
                'L', 'L', 'M', '_', 'S', 'T', 'D', 'O', 'U', 'T', '_', 'P', 'A', 'T', 'H',
            ]),
            text(&[
                'L', 'o', 'c', 'a', 'l', 'L', 'l', 'm', 'P', 'r', 'o', 'c', 'e', 's', 's',
            ]),
            text(&[
                'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'P', 'r', 'o', 'c', 'e', 's', 's',
            ]),
            text(&[
                'P', 'i', 'c', 'o', '2', 'w', 'S', 'e', 'n', 's', 'o', 'r', 'P', 'r', 'o', 'c',
                'e', 's', 's',
            ]),
            text(&[
                'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'A', 'c', 'k', 'M', 's', 'g',
            ]),
            text(&[
                'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'T', 'e', 'x', 't', 'M', 's', 'g',
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
            text(&['b', 'r', 'a', 'n', 'c', 'h', '.', 'r', 'e', 'c', 'v']),
            text(&[
                'o', 'b', 's', 'e', 'r', 'v', 'e', '_', 's', 'h', 'e', 'l', 'l', '_', 'o', 'u',
                't', 'p', 'u', 't',
            ]),
            text(&['n', 'e', 'x', 't', '_', 'c', 'o', 'm', 'm', 'a', 'n', 'd']),
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
        let local_llm = text(&[
            'R', 'O', 'L', 'E', '_', 'L', 'O', 'C', 'A', 'L', '_', 'L', 'L', 'M',
        ]);
        let human_input = text(&[
            'R', 'O', 'L', 'E', '_', 'H', 'U', 'M', 'A', 'N', '_', 'I', 'N', 'P', 'U', 'T',
        ]);
        let pico2w_sensor = text(&[
            'R', 'O', 'L', 'E', '_', 'P', 'I', 'C', 'O', '2', 'W', '_', 'S', 'E', 'N', 'S', 'O',
            'R',
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
        let human_text = text(&[
            'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'T', 'e', 'x', 't', 'M', 's', 'g',
        ]);
        let human_ack = text(&[
            'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'A', 'c', 'k', 'M', 's', 'g',
        ]);
        let sensor_sample = text(&[
            'P', 'i', 'c', 'o', '2', 'w', 'S', 'e', 'n', 's', 'o', 'r', 'S', 'a', 'm', 'p', 'l',
            'e', 'M', 's', 'g',
        ]);
        let sensor_ack = text(&[
            'P', 'i', 'c', 'o', '2', 'w', 'S', 'e', 'n', 's', 'o', 'r', 'A', 'c', 'k', 'M', 's',
            'g',
        ]);
        assert!(
            compact.contains(&format!("{wasi},{local_llm},{read_req}")),
            "WASI shell must read terminal commands from the local LLM role through ChoreoFS"
        );
        assert!(
            compact.contains(&format!("{local_llm},{wasi},{read_ret}")),
            "local LLM must answer only the WASI fd_read reply"
        );
        assert!(
            compact.contains(&format!("{wasi},{local_llm},{proc_exit}")),
            "bounded WASI proc_exit must be visible to the local LLM role"
        );
        assert!(
            compact.contains(&format!("{wasi},{m33},{proc_exit}")),
            "bounded WASI proc_exit must be visible to the M33 role"
        );
        assert!(
            compact.contains(&format!("{wasi},{pico2w_sensor},{proc_exit}")),
            "bounded WASI proc_exit must be visible to the Pico 2 W sensor role"
        );
        assert!(
            compact.contains(&format!("{human_input},{local_llm},{human_text}")),
            "input role must pass arbitrary human text to the local LLM role as one typed message"
        );
        assert!(
            compact.contains(&format!("{local_llm},{human_input},{human_ack}")),
            "local LLM must acknowledge the input turn as one projected typed message"
        );
        assert!(
            compact.contains(&format!("{pico2w_sensor},{local_llm},{sensor_sample}")),
            "sensor role must pass one typed sensor sample to the local LLM role"
        );
        assert!(
            compact.contains(&format!("{local_llm},{pico2w_sensor},{sensor_ack}")),
            "local LLM must acknowledge the sensor turn as one projected typed message"
        );
        assert!(
            compact.contains("g::par(human_input_turn(),pico2w_sensor_turn())"),
            "human input and Pico 2 W sensor turns must be joined through g::par"
        );
        assert!(
            compact.contains("HumanInputReqMsg,1")
                && compact.contains("HumanInputTextMsg,1")
                && compact.contains("HumanInputAckMsg,1")
                && compact.contains("Pico2wSensorReqMsg,2")
                && compact.contains("Pico2wSensorSampleMsg,2")
                && compact.contains("Pico2wSensorAckMsg,2"),
            "human and sensor branches must use separate lanes"
        );
        assert!(
            compact.contains(&format!(
                "g::seq(input_context_turn(),g::send::<{local_llm},{wasi},{read_ret}>()"
            )),
            "WASI fd_read_ret must be emitted only after the parallel input context joins"
        );
        for forbidden in [
            format!("{m33},{local_llm}"),
            format!("{local_llm},{m33}"),
            format!("{human_input},{m33}"),
            format!("{m33},{human_input}"),
            format!("{pico2w_sensor},{m33}"),
            format!("{m33},{pico2w_sensor}"),
        ] {
            let forbidden_send = format!("g::send::<{forbidden}");
            assert!(
                !compact.contains(&forbidden_send),
                "M33 must only observe WASI ChoreoFS face writes; found {forbidden_send}"
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
                'L', 'L', 'M', '_', 'F', 'R', 'A', 'M', 'E', '_', 'P', 'A', 'T', 'H',
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
    fn wasi_guest_is_the_llm_visible_choreofs_shell() {
        fn text(chars: &[char]) -> String {
            chars.iter().collect()
        }

        let shell_guest = include_str!("../wasip1/guest/src/bin/uno-q-llm-face-shell.rs");
        let shell_loop_guest = include_str!("../wasip1/guest/src/bin/uno-q-llm-face-shell-loop.rs");
        let removed_llm_frame_path =
            text(&['"', '/', 'l', 'l', 'm', '/', 'f', 'r', 'a', 'm', 'e', '"']);
        for source in [shell_guest, shell_loop_guest] {
            assert!(source.contains("fn main()"));
            assert!(source.contains("fs::{File, OpenOptions}"));
            assert!(source.contains("io::{self, Read, Write}"));
            assert!(source.contains("OpenOptions::new().read(true).open(\"/llm/stdin\")"));
            assert!(source.contains("OpenOptions::new().write(true).open(\"/llm/stdout\")"));
            assert!(source.contains("OpenOptions::new().write(true).open(\"/face/frame\")"));
            assert!(source.contains("\"/llm/stdin\""));
            assert!(source.contains("\"/llm/stdout\""));
            assert!(source.contains("\"/face/frame\""));
            assert!(source.contains("ShellCommand::Catalog"));
            assert!(source.contains("find ChoreoFS -type f"));
            assert!(source.contains("is_catalog_discovery_command"));
            assert!(source.contains("face[0] == b'u'"));
            assert!(source.contains("echo "));
            assert!(source.contains(" > /face/frame"));
            assert!(source.contains("SHELL_INVALID_COMMAND"));
            assert!(source.contains("ShellCommand::Invalid"));
            assert!(source.contains("stdin.read(&mut buffer)?"));
            assert!(source.contains("file.write_all(bytes)?"));
            for forbidden in [
                "#![no_std]",
                "#![no_main]",
                "__main_void",
                "panic_handler",
                concat!("hibana_", "wasip1_", "guest"),
                "choreofs::",
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
                "face[0] == b'v'",
                "surprised_accepts_model_alias_v",
                "model alias",
                removed_llm_frame_path.as_str(),
            ] {
                assert!(
                    !source.contains(forbidden),
                    "WASI guest must remain the LLM-visible ChoreoFS shell; remove {forbidden}"
                );
            }
        }
    }

    #[test]
    fn passive_roles_offer_projected_shell_routes_without_llm_output_filtering() {
        fn text(chars: &[char]) -> String {
            chars.iter().collect()
        }

        let source = include_str!("lib.rs");
        assert!(
            source.contains("branch.recv::<FdWriteReqMsg>()"),
            "local LLM must decode projected stdout writes through offer"
        );
        assert!(
            source.contains("branch.recv::<FdReadReqMsg>()"),
            "local LLM must decode projected stdin reads through offer"
        );
        assert!(
            source.contains("complete_local_llm_stdin_read"),
            "local LLM stdin replies must stay behind the WASI fd_read path"
        );
        assert!(
            source.contains("yield_to_peer_roles().await"),
            "passive hardware split peers must yield after local handshakes so the next projected role can advance"
        );
        assert!(
            source.contains("cx.waker().wake_by_ref()"),
            "local scheduler yield must wake itself so hardware split tasks are polled again"
        );
        assert!(
            {
                let removed_human_poll = text(&[
                    'H', 'u', 'm', 'a', 'n', 'I', 'n', 'p', 'u', 't', 'P', 'o', 'l', 'l', 'M', 's',
                    'g',
                ]);
                source.contains("recv::<HumanInputTextMsg>().await?")
                    && source.contains("flow::<HumanInputAckMsg>()?")
                    && source.contains("flow::<HumanInputReqMsg>()?")
                    && source.contains("LABEL_HUMAN_INPUT_TEXT")
                    && !source.contains(&removed_human_poll)
            },
            "human input must use a choreography-visible request, one typed send, and one ack; not a separate poll protocol"
        );
        assert!(
            source.contains("recv::<Pico2wSensorSampleMsg>().await?")
                && source.contains("flow::<Pico2wSensorReqMsg>()?")
                && source.contains("flow::<Pico2wSensorAckMsg>()?")
                && source.contains("LABEL_PICO2W_SENSOR_SAMPLE"),
            "Pico 2 W sensor must use a choreography-visible request, one fixed sample, and one ack"
        );
        let forbidden_fd_write_route_hook = [
            text(&[
                'W', 'a', 's', 'i', 'F', 'd', 'W', 'r', 'i', 't', 'e', 'B', 'o', 'u', 'n', 'd',
                'a', 'r', 'y', 'R', 'o', 'u', 't', 'e', 'C', 'o', 'n', 't', 'r', 'o', 'l',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'F', 'd', 'W', 'r', 'i', 't', 'e', 'D', 'r', 'i', 'v', 'e',
                'r', 'R', 'o', 'u', 't', 'e', 'C', 'o', 'n', 't', 'r', 'o', 'l',
            ]),
            text(&[
                'W', 'a', 's', 'i', 'F', 'd', 'W', 'r', 'i', 't', 'e', 'B', 'o', 'u', 'n', 'd',
                'a', 'r', 'y', 'P', 'e', 'e', 'r', 'R', 'o', 'u', 't', 'e', 'M', 's', 'g',
            ]),
            text(&[
                'f', 'n', ' ', 'w', 'a', 's', 'i', '_', 'f', 'd', '_', 'w', 'r', 'i', 't', 'e',
                '_', 'r', 'o', 'u', 't', 'e',
            ]),
        ];
        for forbidden in forbidden_fd_write_route_hook {
            assert!(
                !source.contains(forbidden.as_str()),
                "fd_write target routing must not be hidden behind appkit fd evidence: remove {forbidden}"
            );
        }
        let removed_phase_filter = text(&[
            'c', 'o', 'p', 'y', '_', 'l', 'l', 'm', '_', 't', 'e', 'r', 'm', 'i', 'n', 'a', 'l',
            '_', 'i', 'n', 'p', 'u', 't', '_', 'f', 'r', 'o', 'm', '_', 'o', 'u', 't', 'p', 'u',
            't', '_', 'f', 'o', 'r', '_', 'p', 'h', 'a', 's', 'e',
        ]);
        let removed_face_filter = text(&[
            'l', 'o', 'c', 'a', 'l', '_', 'l', 'l', 'm', '_', 'i', 's', '_', 'f', 'a', 'c', 'e',
            '_', 'w', 'r', 'i', 't', 'e', '_', 'c', 'o', 'm', 'm', 'a', 'n', 'd',
        ]);
        let removed_repair_prompt = text(&[
            'l', 'o', 'c', 'a', 'l', '_', 'l', 'l', 'm', '_', 'f', 'a', 'c', 'e', '_', 'c', 'o',
            'm', 'm', 'a', 'n', 'd', '_', 'r', 'e', 'p', 'a', 'i', 'r', '_', 'p', 'r', 'o', 'm',
            'p', 't',
        ]);
        assert!(
            !source.contains(removed_phase_filter.as_str())
                && !source.contains(removed_face_filter.as_str())
                && !source.contains(removed_repair_prompt.as_str()),
            "LLM terminal input must not be filtered outside the WASI shell/choreography path"
        );
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
        let local_llm_proc_exit_decode = text(&[
            'b', 'r', 'a', 'n', 'c', 'h', '.', 'r', 'e', 'c', 'v', ':', ':', '<', 'W', 'a', 's',
            'i', 'P', 'r', 'o', 'c', 'E', 'x', 'i', 't', 'R', 'e', 'q', 'M', 's', 'g', '>', '(',
            ')', '.', 'a', 'w', 'a', 'i', 't',
        ]);
        assert!(
            compact.contains(&local_llm_proc_exit_decode),
            "finite local LLM role must observe the projected proc_exit break arm"
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

    #[test]
    fn local_llm_output_is_terminal_input_not_choreography_filter() {
        let mut out = [0u8; LOCAL_LLM_COMMAND_BYTES];
        assert_eq!(
            copy_llm_terminal_input_from_output(
                "echo mc > /face/frame\n [end of text]\n",
                &mut out,
            ),
            Some(22)
        );
        assert_eq!(&out[..22], b"echo mc > /face/frame\n");

        let mut out = [0u8; LOCAL_LLM_COMMAND_BYTES];
        let invalid = "cat /etc/passwd\n";
        assert_eq!(
            copy_llm_terminal_input_from_output(invalid, &mut out),
            Some(invalid.len())
        );
        assert_eq!(&out[..invalid.len()], invalid.as_bytes());
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn local_llm_default_source_keeps_one_persistent_llama_server() {
        let source = include_str!("lib.rs");
        assert!(source.contains("DEFAULT_UNO_Q_LOCAL_LLM_SERVER"));
        assert!(source.contains("llama-server"));
        assert!(source.contains("LocalLlmServer"));
        assert!(source.contains("Self::Server(server)"));
        assert!(source.contains("POST /v1/chat/completions"));
        assert!(source.contains("POST /completion"));
        assert!(source.contains("GET /health"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_SERVER_ENDPOINT"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_SERVER_PORT"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_SERVER_ARGS"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_CLI"));
        assert!(!source.contains("DEFAULT_UNO_Q_LOCAL_LLM_COMPLETION"));
        assert!(!source.contains("copy_watchdog_local_llm_command"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_SCRIPTED"));
        assert!(source.contains("struct HumanInputSource"));
        assert!(source.contains("std::sync::mpsc::Receiver<Vec<u8>>"));
        assert!(source.contains("UNO_Q_HUMAN_INPUT_MODE"));
        assert!(source.contains("UNO_Q_HUMAN_INPUT_TEXT"));
        assert!(source.contains("UNO_Q_HUMAN_INPUT_VOICE_CMD"));
        assert!(source.contains("struct Pico2wSensorSource"));
        assert!(source.contains("Pico2wSensorSample"));
        assert!(source.contains("UNO_Q_PICO2W_SENSOR_MODE"));
        assert!(source.contains("UNO_Q_PICO2W_SENSOR_UDP_BIND"));
        assert!(source.contains("spawn_pico2w_sensor_udp"));
        assert!(source.contains("decode_pico2w_sensor_udp_payload"));
        assert!(source.contains("HumanInputText::from_bytes"));
        assert!(source.contains("strip_terminal_line_delimiter"));
        assert!(source.contains("observe_human_input"));
        assert!(source.contains("observe_pico2w_sensor_sample"));
        assert!(source.contains("UNO_Q_LOCAL_LLM_SELF_MOOD"));
        assert!(source.contains("Assistant mood instruction"));
        assert!(source.contains("local_llm_human_face_prompt"));
        assert!(source.contains("Self::Missing => Err"));
        let removed_sensor_udp_human = ["spawn_", "sensor_udp_human_input"].concat();
        let removed_sensor_to_human = ["uno_q_sensor_payload_to_", "human_input"].concat();
        assert!(!source.contains(&removed_sensor_udp_human));
        assert!(!source.contains(&removed_sensor_to_human));
        let removed_prompt_file_const = ["DEFAULT_UNO_Q_LOCAL_LLM_", "USER_PROMPT_FILE"].concat();
        let removed_prompt_file_env = ["UNO_Q_LOCAL_LLM_", "USER_PROMPT_FILE"].concat();
        let removed_mood_classifier = ["local_llm_", "mood_key"].concat();
        let removed_mood_words_helper = ["local_llm_", "context_has_any"].concat();
        assert!(!source.contains(&removed_prompt_file_const));
        assert!(!source.contains(&removed_prompt_file_env));
        assert!(!source.contains(&removed_mood_classifier));
        assert!(!source.contains(&removed_mood_words_helper));
        let removed_interactive_env = ["UNO_Q_LOCAL_LLM_", "INTERACTIVE"].concat();
        let removed_user_prompt_env = ["UNO_Q_LOCAL_LLM_", "USER_PROMPT"].concat();
        let removed_prompt_bytes = ["local_llm_", "prompt_from_bytes"].concat();
        assert!(!source.contains(&removed_interactive_env));
        assert!(!source.contains(&removed_user_prompt_env));
        assert!(!source.contains(&removed_prompt_bytes));
        let removed_keyword_bucket = ["angry\", \"frustrated", "\", \"mad\", \"upset"].concat();
        assert!(!source.contains(&removed_keyword_bucket));
        let removed_prompt_file_script = ["inject_llm_", "prompt.sh"].concat();
        assert!(
            !std::path::Path::new(&format!(
                "examples/uno-q-heterogeneous/scripts/{removed_prompt_file_script}"
            ))
            .exists()
        );
        let llama_grammar_flag = ["--", "grammar"].concat();
        assert!(!source.contains(&llama_grammar_flag));
        let removed_grammar_helper = ["local_llm_", "grammar_for_phase"].concat();
        assert!(!source.contains(&removed_grammar_helper));
        let enough_predict_tokens = ["\"", "8", "\".to_owned()"].concat();
        assert!(source.contains(&enough_predict_tokens));
        let piped_stderr = [".stderr(std::process::Stdio::", "piped())"].concat();
        assert!(source.contains(&piped_stderr));
        let removed_optional_source = ["command: ", "Option<LocalLlmCommandSource>"].concat();
        assert!(!source.contains(&removed_optional_source));

        let prompt = default_local_llm_shell_prompt();
        assert!(prompt.contains("WASI shell"));
        assert!(prompt.contains("choreography"));
        let server_prompt = local_llm_prompt_for_server(
            b"llm/stdout\n",
            1,
            0,
            prompt_context(Some("怒った感じで"), Pico2wSensorSample::pending(0)),
        )
        .unwrap();
        assert!(!server_prompt.contains("Shell transcript so far"));
        assert!(server_prompt.contains("怒った感じで"));
        assert!(server_prompt.contains("Pico 2 W sensor"));
        assert!(server_prompt.ends_with("Command:"));
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn local_llm_prompt_guidance_is_not_llama_grammar() {
        assert!(local_llm_discovery_prompt().contains("Command: ls"));
        assert!(
            local_llm_human_face_prompt("悲しい感じで")
                .contains("choose the closest valid face command")
        );
        assert!(
            local_llm_human_face_prompt("T=23.0C H=50% L=1500; face happy=comfy")
                .contains("Pico 2 W sensor")
        );
        assert!(
            local_llm_default_face_prompt("h")
                .contains("Answer with exactly: echo h > /face/frame\nAnswer:")
        );
        assert!(local_llm_default_face_prompt("h").contains("Valid shell syntax"));
        assert!(local_llm_chat_system_prompt().contains("infer Uno Q's mood"));
        assert!(local_llm_chat_system_prompt().contains("echo h > /face/frame"));
        assert!(local_llm_chat_system_prompt().contains("Use ls only for initial discovery"));
        assert!(local_llm_chat_system_prompt().contains("Bare commands like echo h"));
        assert!(local_llm_chat_system_prompt().contains("For sensor turns, use only"));
        assert!(local_llm_chat_system_prompt().contains("light intensity L"));
        assert!(local_llm_chat_system_prompt().contains("normal light maps to echo h"));
        assert!(local_llm_chat_system_prompt().contains("sensor status is stale"));
        assert_eq!(
            local_llm_chat_user_prompt(
                b"w /face/frame FaceFrame\n$ ",
                0,
                0,
                prompt_context(None, Pico2wSensorSample::pending(0)),
            )
            .unwrap(),
            "Output exactly: ls"
        );
        let fresh_sample =
            Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 230, 600, 42, 7).unwrap();
        let chat_user = local_llm_chat_user_prompt(
            b"echo c > /face/frame\nerr /face/frame h,a,s,u,mw\n$ ",
            1,
            0,
            prompt_context(Some("calm manual override"), fresh_sample),
        )
        .unwrap();
        assert!(chat_user.contains("Human input:"));
        assert!(chat_user.contains("calm manual override"));
        assert!(chat_user.contains("Pico 2 W sensor:"));
        assert!(chat_user.contains("status=fresh seq=7 T=23.0C H=60.0% L=42"));
        assert!(chat_user.contains("choose the mood from light intensity L"));
        assert!(chat_user.contains("choose only s, h, or u"));
        assert!(chat_user.contains("exact full form"));
        assert!(chat_user.contains("Do not output ls"));
        assert!(chat_user.contains("err /face/frame"));
        assert!(chat_user.contains("full redirect form"));
        assert!(chat_user.contains("h,a,s,u,mw"));
        let stale_sample = fresh_sample
            .with_status_and_seq(PICO2W_SENSOR_STATUS_STALE, 8)
            .unwrap();
        let stale_user =
            local_llm_chat_user_prompt(b"$ ", 1, 0, prompt_context(None, stale_sample)).unwrap();
        assert!(stale_user.contains("Output exactly: echo s > /face/frame"));
        assert!(stale_user.contains("status is stale"));
        assert!(!stale_user.contains("infer the mood from the fresh sensor sample"));
        let mut scripted = [0u8; LOCAL_LLM_COMMAND_BYTES];
        let len = scripted_pico2w_sensor_shell_command(stale_sample, false, &mut scripted)
            .unwrap()
            .unwrap();
        assert_eq!(&scripted[..len], b"echo s > /face/frame\n");
    }

    #[test]
    fn local_llm_terminal_command_state_distinguishes_catalog_from_face() {
        assert_eq!(
            classify_local_llm_terminal_command(b"ls\n"),
            LocalLlmTerminalCommand::Catalog
        );
        assert_eq!(
            classify_local_llm_terminal_command(b"echo h > /face/frame\n"),
            LocalLlmTerminalCommand::Face
        );
        assert_eq!(
            classify_local_llm_terminal_command(b"echo c > /face/frame\n"),
            LocalLlmTerminalCommand::Other
        );
        assert!(local_llm_terminal_command_admitted_for_phase(0, b"ls\n"));
        assert!(!local_llm_terminal_command_admitted_for_phase(
            0,
            b"echo h > /face/frame\n"
        ));
        assert!(local_llm_terminal_command_admitted_for_phase(
            1,
            b"echo h > /face/frame\n"
        ));
        assert!(!local_llm_terminal_command_admitted_for_phase(1, b"ls\n"));

        let mut transcript = b"w /face/frame FaceFrame\n$ ".to_vec();
        append_local_llm_shell_error(&mut transcript, b"ls\n");
        let transcript = core::str::from_utf8(&transcript).unwrap();
        assert!(transcript.contains("ls\nerr /face/frame h,a,s,u,mw\n$ "));
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn local_llm_server_json_codec_handles_completion_content() {
        let prompt = "Transcript:\nls\nCommand: echo a > /face/frame";
        let encoded = local_llm_json_string(prompt);
        assert!(encoded.contains("\\n"));
        assert!(encoded.contains("echo a > /face/frame"));

        let body = "{\"index\":0,\"content\":\" echo a > /face/frame\",\"tokens_predicted\":5}";
        assert_eq!(
            local_llm_json_string_field(body, "content").as_deref(),
            Some(" echo a > /face/frame")
        );
        let escaped = "{\"content\":\"echo s > /face/frame\\n\"}";
        assert_eq!(
            local_llm_json_string_field(escaped, "content").as_deref(),
            Some("echo s > /face/frame\n")
        );
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn human_input_role_forwards_exact_text_as_prompt_context_not_runtime_authority() {
        let input = HumanInputText::new("怒った感じで  keep spaces  ").unwrap();
        assert_eq!(input.as_str().unwrap(), "怒った感じで  keep spaces  ");
        assert_eq!(input.as_bytes(), "怒った感じで  keep spaces  ".as_bytes());

        let too_long = vec![b'x'; protocol::HUMAN_INPUT_TEXT_BYTES + 1];
        assert_eq!(
            HumanInputText::from_bytes(&too_long),
            Err(protocol::ProtocolError::HumanInputTooLong)
        );

        let mut line = b"happy now  \r\n".to_vec();
        strip_terminal_line_delimiter(&mut line);
        assert_eq!(&line, b"happy now  ");

        let mut out = [0u8; LOCAL_LLM_COMMAND_BYTES];
        let arbitrary = "echo hello > /tmp/free-shell\n";
        assert_eq!(
            copy_llm_terminal_input_from_output(arbitrary, &mut out),
            Some(arbitrary.len())
        );
        assert_eq!(&out[..arbitrary.len()], arbitrary.as_bytes());
    }

    #[test]
    fn pico2w_sensor_sample_is_fixed_length_codec_payload() {
        let sample =
            Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, -123, 1000, 4095, 42).unwrap();
        let mut encoded = [0u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES];
        assert_eq!(
            sample.encode_into(&mut encoded).unwrap(),
            protocol::PICO2W_SENSOR_SAMPLE_BYTES
        );
        assert_eq!(encoded, [0, 133, 255, 232, 3, 255, 15, 42, 0]);
        let decoded = Pico2wSensorSample::decode_payload(Payload::new(&encoded)).unwrap();
        assert_eq!(decoded, sample);

        let mut short = [0u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES - 1];
        assert_eq!(sample.encode_into(&mut short), Err(CodecError::Truncated));
        assert!(Pico2wSensorSample::decode_payload(Payload::new(&encoded[..8])).is_err());
        let mut invalid_status = encoded;
        invalid_status[0] = 99;
        assert!(Pico2wSensorSample::decode_payload(Payload::new(&invalid_status)).is_err());
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn sensor_udp_payload_becomes_typed_pico2w_sensor_sample() {
        let typed = Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 226, 600, 2500, 7).unwrap();
        let mut bytes = [0u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES];
        typed.encode_into(&mut bytes).unwrap();
        assert_eq!(decode_pico2w_sensor_udp_payload(&bytes), Some(typed));
        assert_eq!(
            decode_pico2w_sensor_udp_payload(b"T:22.60C H:60%\nL:2500\n"),
            None
        );
        assert_eq!(
            decode_pico2w_sensor_udp_payload(b"{\"temperature\":21.5,\"rh\":45.0,\"lux\":900}"),
            None
        );
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn pico2w_raw_udp_ack_uses_observed_l2_peer() {
        use hibana_wifi::proto::{
            ethernet::{Ipv4Addr, MacAddr, build_udp_ipv4},
            udp::parse_udp_ipv4_packet,
        };

        let pico_mac = MacAddr([0x02, 0x12, 0x34, 0x56, 0x78, 0x9a]);
        let uno_q_mac = MacAddr([0x14, 0xb5, 0xcd, 0x0f, 0x41, 0x7d]);
        let pico_ip = Ipv4Addr([192, 168, 96, 98]);
        let uno_q_ip = Ipv4Addr([192, 168, 96, 99]);
        let sample = Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 239, 577, 0, 7).unwrap();
        let mut payload = [0u8; protocol::PICO2W_SENSOR_SAMPLE_BYTES];
        sample.encode_into(&mut payload).unwrap();
        let mut frame = [0u8; 96];
        let len = build_udp_ipv4(
            &mut frame, pico_mac, uno_q_mac, pico_ip, uno_q_ip, 43210, 8787, &payload,
        )
        .unwrap();

        let parsed = parse_pico2w_sensor_udp_frame(&frame[..len], 8787).unwrap();
        assert_eq!(parsed.src_mac, pico_mac);
        assert_eq!(parsed.dst_mac, uno_q_mac);
        assert_eq!(parsed.src_ip, pico_ip);
        assert_eq!(parsed.dst_ip, uno_q_ip);
        assert_eq!(parsed.src_port, 43210);
        assert_eq!(parsed.dst_port, 8787);
        assert_eq!(
            decode_pico2w_sensor_udp_payload(&parsed.payload),
            Some(sample)
        );

        let mut ack = [0u8; 96];
        let ack_len = build_pico2w_sensor_udp_ack_frame(parsed, sample.seq(), &mut ack).unwrap();
        let packet = parse_udp_ipv4_packet::<{ protocol::PICO2W_SENSOR_UDP_ACK_BYTES }>(
            &ack[..ack_len],
            pico_mac,
            pico_ip,
            43210,
        )
        .unwrap();
        assert_eq!(packet.src_ip(), uno_q_ip);
        assert_eq!(packet.dst_ip(), pico_ip);
        assert_eq!(packet.src_port(), 8787);
        assert_eq!(packet.dst_port(), 43210);
        assert_eq!(
            protocol::decode_pico2w_sensor_udp_ack(packet.payload()),
            Some(7)
        );
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn scripted_llm_changes_face_from_typed_sensor_light_strength() {
        let mut source = LocalLlmCommandSource::Scripted;
        let mut out = [0u8; LOCAL_LLM_COMMAND_BYTES];
        let dim = Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 238, 610, 100, 1).unwrap();
        let len = source
            .next_command(b"", 1, 9, prompt_context(None, dim), &mut out)
            .unwrap();
        assert_eq!(&out[..len], b"echo s > /face/frame\n");

        let normal = Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 320, 900, 900, 2).unwrap();
        let len = source
            .next_command(b"", 1, 9, prompt_context(None, normal), &mut out)
            .unwrap();
        assert_eq!(&out[..len], b"echo h > /face/frame\n");

        let bright =
            Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, -50, 100, 4095, 3).unwrap();
        let len = source
            .next_command(b"", 1, 9, prompt_context(None, bright), &mut out)
            .unwrap();
        assert_eq!(&out[..len], b"echo u > /face/frame\n");
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn self_mood_mode_is_prompt_guidance_only() {
        let prompt = local_llm_self_mood_prompt(1);
        assert!(prompt.contains("simulated assistant mood"));
        assert!(prompt.contains("frustrated"));
        assert!(prompt.contains("return only"));

        let face_prompt = local_llm_human_face_prompt(&prompt);
        assert!(face_prompt.contains("Human input:"));
        assert!(face_prompt.contains("Pico 2 W sensor:"));
        assert!(face_prompt.contains("choose Uno Q's mood"));
        assert!(face_prompt.ends_with("Command:"));
    }

    #[cfg(not(target_os = "none"))]
    #[test]
    fn local_llm_command_args_support_quoted_prompt_fragments() {
        assert_eq!(
            split_local_llm_args("llama-cli -m model.gguf -p 'echo mc > /face/frame'"),
            vec![
                "llama-cli",
                "-m",
                "model.gguf",
                "-p",
                "echo mc > /face/frame"
            ]
        );
    }
}
