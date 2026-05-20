#![cfg_attr(all(target_os = "none", not(test)), no_std)]

pub mod protocol;

use core::cell::{Cell, UnsafeCell};
use core::convert::Infallible;
use core::task::Poll;

use hibana::g;
use hibana::integration::{
    program::Projectable,
    runtime::LabelUniverse,
    wire::{CodecError, Payload},
};
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineReq, EngineRet, FdReadDone, FdWriteDone, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET,
        LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
        LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASI_PROC_EXIT, PathOpened, PollReady,
    },
    site,
};
use protocol::{
    CommitMarker, FACE_SPEAKING, FaceCandidate, LABEL_CHALLENGER_PACKET, LABEL_CHALLENGER_READ,
    LABEL_CHALLENGER_READ_RET, LABEL_CHALLENGER_RECEIPT, LABEL_FACE_ACK_COMMIT,
    LABEL_FACE_CANDIDATE_TO_M33, LABEL_FINAL_COMMIT, LABEL_IOS_PROMPT_FACT,
    LABEL_IOS_PROMPT_REQUEST, LABEL_LLM_PROMPT_TO_LINUX, LABEL_LLM_PROPOSAL_TO_LINUX,
    LABEL_LLM_REQUEST_TO_SIDECAR, NetPacket, NetReceipt, ROLE_CHALLENGER_KERNEL,
    ROLE_IOS_PROMPT_INGRESS, ROLE_LINUX_KERNEL, ROLE_LLM_SIDECAR, ROLE_M33_LED_KERNEL,
    ROLE_WASI_LLM_CELL, SmallText,
};

pub struct UnoQCapsule;
pub struct UnoQPlacement;
pub struct UnoQLocal;
pub struct UnoQArtifacts;

#[derive(Clone, Copy, Debug, Default)]
pub struct UnoQLabelUniverse;

impl LabelUniverse for UnoQLabelUniverse {
    const MAX_LABEL: u8 = LABEL_FINAL_COMMIT;
}

pub mod image {
    pub struct HostLoopbackProof;
    pub struct HardwarePeerProof;
    pub struct LinuxKernelProcess;
    pub struct WasiLlmCellProcess;
    pub struct LlmSidecarProcess;
    pub struct IosPromptIngressProcess;
    pub struct M33LedKernelImage;
    pub struct ChallengerNetKernelImage;
}

pub const UNO_Q_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(0x7101);
pub const PREOPEN_FD: u8 = 9;
pub const IOS_PROMPT_FD: u8 = 11;
pub const LLM_PROMPT_FD: u8 = 12;
pub const CHALLENGER_TX_FD: u8 = 13;
pub const CHALLENGER_RX_FD: u8 = 14;
pub const FACE_ACK_FD: u8 = 15;

const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const PROOF_CARRIER_ROLES: usize = 6;
const PROOF_CARRIER_QUEUE_DEPTH: usize = 24;
const PROOF_CARRIER_FRAME_BYTES: usize = 128;
const UART_CARRIER_MAGIC: [u8; 4] = *b"HBU1";
const UART_CARRIER_CHECK: u8 = 0xa7;
const UART_CARRIER_HEADER_BYTES: usize = 13;
const UART_CARRIER_FRAME_BYTES: usize = UART_CARRIER_HEADER_BYTES + PROOF_CARRIER_FRAME_BYTES + 1;
const HARDWARE_PEER_ROLE_BITS: u128 = 0x3e;
const UNO_Q_UART_OPERATIONAL_DEADLINE_TICKS: u32 = 50_000;

const IOS_PROMPT_PATH: &[u8] = b"ios/prompt/inbox";
const LLM_PROMPT_PATH: &[u8] = b"llm/prompt";
const CHALLENGER_TX_PATH: &[u8] = b"net/challenger/tx";
const CHALLENGER_RX_PATH: &[u8] = b"net/challenger/rx";
const FACE_ACK_PATH: &[u8] = b"face/ack";

pub const IOS_PROMPT_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    IOS_PROMPT_PATH,
    appkit::ObjectId(71_001),
    appkit::FdSpec::new(IOS_PROMPT_FD as u32, FD_READ_RIGHT, 1),
);
pub const LLM_PROMPT_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    LLM_PROMPT_PATH,
    appkit::ObjectId(71_002),
    appkit::FdSpec::new(LLM_PROMPT_FD as u32, FD_WRITE_RIGHT, 1),
);
pub const CHALLENGER_TX_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    CHALLENGER_TX_PATH,
    appkit::ObjectId(71_003),
    appkit::FdSpec::new(CHALLENGER_TX_FD as u32, FD_WRITE_RIGHT, 1),
);
pub const CHALLENGER_RX_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    CHALLENGER_RX_PATH,
    appkit::ObjectId(71_004),
    appkit::FdSpec::new(CHALLENGER_RX_FD as u32, FD_READ_RIGHT, 1),
);
pub const FACE_ACK_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    FACE_ACK_PATH,
    appkit::ObjectId(71_005),
    appkit::FdSpec::new(FACE_ACK_FD as u32, FD_WRITE_RIGHT, 1),
);

static UNO_Q_DRIVER_FACTS: appkit::ChoreoFsObjectSet<5> = appkit::ChoreoFsObjectSet::new([
    IOS_PROMPT_OBJECT,
    LLM_PROMPT_OBJECT,
    CHALLENGER_TX_OBJECT,
    CHALLENGER_RX_OBJECT,
    FACE_ACK_OBJECT,
]);

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_UNO_Q_LLM_FACE_CELL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-cell.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_UNO_Q_LLM_FACE_CELL: &[u8] = &[];

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
static LINUX_KERNEL_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static WASI_CELL_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static LLM_SIDECAR_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static IOS_INGRESS_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static M33_LED_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(not(test), target_os = "none"))]
static CHALLENGER_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<UNO_Q_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

type WasiPathOpenReqMsg = g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>;
type WasiPathOpenRetMsg = g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>;
type WasiFdReadReqMsg = g::Msg<LABEL_WASI_FD_READ, EngineReq>;
type WasiFdReadRetMsg = g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>;
type WasiFdWriteReqMsg = g::Msg<LABEL_WASI_FD_WRITE, EngineReq>;
type WasiFdWriteRetMsg = g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>;
type WasiPollReqMsg = g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>;
type WasiPollRetMsg = g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>;
type WasiProcExitReqMsg = g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>;

type IosPromptRequestMsg = g::Msg<LABEL_IOS_PROMPT_REQUEST, SmallText>;
type IosPromptFactMsg = g::Msg<LABEL_IOS_PROMPT_FACT, SmallText>;
type LlmPromptToLinuxMsg = g::Msg<LABEL_LLM_PROMPT_TO_LINUX, SmallText>;
type LlmRequestToSidecarMsg = g::Msg<LABEL_LLM_REQUEST_TO_SIDECAR, SmallText>;
type LlmProposalToLinuxMsg = g::Msg<LABEL_LLM_PROPOSAL_TO_LINUX, protocol::LlmProposal>;
type FaceCandidateToM33Msg = g::Msg<LABEL_FACE_CANDIDATE_TO_M33, FaceCandidate>;
type ChallengerPacketMsg = g::Msg<LABEL_CHALLENGER_PACKET, NetPacket>;
type ChallengerReceiptMsg = g::Msg<LABEL_CHALLENGER_RECEIPT, NetReceipt>;
type ChallengerReadMsg = g::Msg<LABEL_CHALLENGER_READ, NetPacket>;
type ChallengerReadRetMsg = g::Msg<LABEL_CHALLENGER_READ_RET, NetReceipt>;
type FaceAckCommitMsg = g::Msg<LABEL_FACE_ACK_COMMIT, CommitMarker>;
type FinalCommitMsg = g::Msg<LABEL_FINAL_COMMIT, CommitMarker>;

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
        let ios_prompt_read = g::seq(
            g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiFdReadReqMsg, 0>(
            ),
            g::seq(
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_LINUX_KERNEL>,
                    IosPromptRequestMsg,
                    0,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_LINUX_KERNEL>,
                        g::Role<ROLE_IOS_PROMPT_INGRESS>,
                        IosPromptRequestMsg,
                        0,
                    >(),
                    g::seq(
                        g::send::<
                            g::Role<ROLE_IOS_PROMPT_INGRESS>,
                            g::Role<ROLE_LINUX_KERNEL>,
                            IosPromptFactMsg,
                            0,
                        >(),
                        g::seq(
                            g::send::<
                                g::Role<ROLE_LINUX_KERNEL>,
                                g::Role<ROLE_M33_LED_KERNEL>,
                                IosPromptFactMsg,
                                0,
                            >(),
                            g::send::<
                                g::Role<ROLE_M33_LED_KERNEL>,
                                g::Role<ROLE_WASI_LLM_CELL>,
                                WasiFdReadRetMsg,
                                0,
                            >(),
                        ),
                    ),
                ),
            ),
        );
        let llm_prompt_write = g::seq(
            g::send::<
                g::Role<ROLE_WASI_LLM_CELL>,
                g::Role<ROLE_M33_LED_KERNEL>,
                WasiFdWriteReqMsg,
                1,
            >(),
            g::seq(
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_LINUX_KERNEL>,
                    LlmPromptToLinuxMsg,
                    1,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_LINUX_KERNEL>,
                        g::Role<ROLE_LLM_SIDECAR>,
                        LlmRequestToSidecarMsg,
                        1,
                    >(),
                    g::seq(
                        g::send::<
                            g::Role<ROLE_LLM_SIDECAR>,
                            g::Role<ROLE_LINUX_KERNEL>,
                            LlmProposalToLinuxMsg,
                            1,
                        >(),
                        g::seq(
                            g::send::<
                                g::Role<ROLE_LINUX_KERNEL>,
                                g::Role<ROLE_M33_LED_KERNEL>,
                                FaceCandidateToM33Msg,
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
            ),
        );
        let challenger_write = g::seq(
            g::send::<
                g::Role<ROLE_WASI_LLM_CELL>,
                g::Role<ROLE_M33_LED_KERNEL>,
                WasiFdWriteReqMsg,
                2,
            >(),
            g::seq(
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_CHALLENGER_KERNEL>,
                    ChallengerPacketMsg,
                    2,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_CHALLENGER_KERNEL>,
                        g::Role<ROLE_M33_LED_KERNEL>,
                        ChallengerReceiptMsg,
                        2,
                    >(),
                    g::send::<
                        g::Role<ROLE_M33_LED_KERNEL>,
                        g::Role<ROLE_WASI_LLM_CELL>,
                        WasiFdWriteRetMsg,
                        2,
                    >(),
                ),
            ),
        );
        let challenger_read = g::seq(
            g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiFdReadReqMsg, 3>(
            ),
            g::seq(
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_CHALLENGER_KERNEL>,
                    ChallengerReadMsg,
                    3,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_CHALLENGER_KERNEL>,
                        g::Role<ROLE_M33_LED_KERNEL>,
                        ChallengerReadRetMsg,
                        3,
                    >(),
                    g::send::<
                        g::Role<ROLE_M33_LED_KERNEL>,
                        g::Role<ROLE_WASI_LLM_CELL>,
                        WasiFdReadRetMsg,
                        3,
                    >(),
                ),
            ),
        );
        let face_ack_write = g::seq(
            g::send::<
                g::Role<ROLE_WASI_LLM_CELL>,
                g::Role<ROLE_M33_LED_KERNEL>,
                WasiFdWriteReqMsg,
                4,
            >(),
            g::seq(
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_LINUX_KERNEL>,
                    FaceAckCommitMsg,
                    4,
                >(),
                g::send::<
                    g::Role<ROLE_M33_LED_KERNEL>,
                    g::Role<ROLE_WASI_LLM_CELL>,
                    WasiFdWriteRetMsg,
                    4,
                >(),
            ),
        );
        g::seq(
            g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPathOpenReqMsg, 0>(),
            g::seq(
                g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPathOpenRetMsg, 0>(),
                g::seq(
                    ios_prompt_read,
                    g::seq(
                        g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPathOpenReqMsg, 1>(),
                        g::seq(
                            g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPathOpenRetMsg, 1>(),
                            g::seq(
                                llm_prompt_write,
                                g::seq(
                                    g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPollReqMsg, 1>(),
                                    g::seq(
                                        g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPollRetMsg, 1>(),
                                        g::seq(
                                            g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPathOpenReqMsg, 2>(),
                                            g::seq(
                                                g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPathOpenRetMsg, 2>(),
                                                g::seq(
                                                    challenger_write,
                                                    g::seq(
                                                        g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPathOpenReqMsg, 3>(),
                                                        g::seq(
                                                            g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPathOpenRetMsg, 3>(),
                                                            g::seq(
                                                                challenger_read,
                                                                g::seq(
                                                                    g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiPathOpenReqMsg, 4>(),
                                                                    g::seq(
                                                                        g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_WASI_LLM_CELL>, WasiPathOpenRetMsg, 4>(),
                                                                        g::seq(
                                                                            face_ack_write,
                                                                            g::seq(
                                                                                g::send::<g::Role<ROLE_WASI_LLM_CELL>, g::Role<ROLE_M33_LED_KERNEL>, WasiProcExitReqMsg, 4>(),
                                                                                g::send::<g::Role<ROLE_M33_LED_KERNEL>, g::Role<ROLE_LINUX_KERNEL>, FinalCommitMsg, 4>(),
                                                                            ),
                                                                        ),
                                                                    ),
                                                                ),
                                                            ),
                                                        ),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
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
            ROLE_LINUX_KERNEL
            | ROLE_LLM_SIDECAR
            | ROLE_CHALLENGER_KERNEL
            | ROLE_IOS_PROMPT_INGRESS => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn uno_q_m33_board_ready();
    fn uno_q_m33_board_accept_candidate(face: u8, mouth_frames: u8);
    fn uno_q_m33_board_commit_face(face: u8);
}

fn m33_board_ready() {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_board_ready();
    }
}

fn m33_board_accept_candidate(face: u8, mouth_frames: u8) {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_board_accept_candidate(face, mouth_frames);
    }
    #[cfg(not(target_os = "none"))]
    core::hint::black_box((face, mouth_frames));
}

fn m33_board_commit_face(face: u8) {
    #[cfg(target_os = "none")]
    unsafe {
        uno_q_m33_board_commit_face(face);
    }
    #[cfg(not(target_os = "none"))]
    core::hint::black_box(face);
}

async fn run_m33_driver<const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, UnoQCapsule, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    if ROLE != ROLE_M33_LED_KERNEL {
        return ctx.pending().await;
    }

    m33_board_ready();

    let selected_face: u8;
    let challenger_status: u8;

    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    complete_path_open(&mut ctx, request, IOS_PROMPT_PATH, FD_READ_RIGHT).await?;

    let read = expect_fd_read(ctx.endpoint().recv::<WasiFdReadReqMsg>().await?)?;
    expect_fd_object(&ctx, read.fd(), IOS_PROMPT_OBJECT.object(), FD_READ_RIGHT)?;
    let prompt_request = SmallText::new(b"ios prompt")?;
    ctx.endpoint()
        .flow::<IosPromptRequestMsg>()?
        .send(&prompt_request)
        .await?;
    let prompt = ctx.endpoint().recv::<IosPromptFactMsg>().await?;
    let reply = EngineRet::FdReadDone(FdReadDone::new_with_lease(
        read.fd(),
        read.lease_id(),
        bounded(prompt.as_bytes(), read.max_len() as usize),
    )?);
    ctx.endpoint()
        .flow::<WasiFdReadRetMsg>()?
        .send(&reply)
        .await?;

    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    complete_path_open(&mut ctx, request, LLM_PROMPT_PATH, FD_WRITE_RIGHT).await?;

    let write = expect_fd_write(ctx.endpoint().recv::<WasiFdWriteReqMsg>().await?)?;
    expect_fd_object(&ctx, write.fd(), LLM_PROMPT_OBJECT.object(), FD_WRITE_RIGHT)?;
    let prompt = SmallText::new(write.as_bytes())?;
    ctx.endpoint()
        .flow::<LlmPromptToLinuxMsg>()?
        .send(&prompt)
        .await?;
    let candidate = ctx.endpoint().recv::<FaceCandidateToM33Msg>().await?;
    if candidate.face() != FACE_SPEAKING || candidate.mouth_frames() < 3 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    m33_board_accept_candidate(candidate.face(), candidate.mouth_frames());
    selected_face = candidate.face();
    send_fd_write_done(&mut ctx, write.fd(), write.len()).await?;

    let poll = ctx.endpoint().recv::<WasiPollReqMsg>().await?;
    let EngineReq::PollOneoff(poll) = poll else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    if poll.timeout_tick() != 10 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    ctx.endpoint()
        .flow::<WasiPollRetMsg>()?
        .send(&EngineRet::PollReady(PollReady::new(1)))
        .await?;

    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    complete_path_open(&mut ctx, request, CHALLENGER_TX_PATH, FD_WRITE_RIGHT).await?;

    let write = expect_fd_write(ctx.endpoint().recv::<WasiFdWriteReqMsg>().await?)?;
    expect_fd_object(
        &ctx,
        write.fd(),
        CHALLENGER_TX_OBJECT.object(),
        FD_WRITE_RIGHT,
    )?;
    let packet = NetPacket::new(1, write.as_bytes())?;
    ctx.endpoint()
        .flow::<ChallengerPacketMsg>()?
        .send(&packet)
        .await?;
    let receipt = ctx.endpoint().recv::<ChallengerReceiptMsg>().await?;
    if receipt.packet_id() != 1 || receipt.status() != 1 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    challenger_status = receipt.status();
    send_fd_write_done(&mut ctx, write.fd(), write.len()).await?;

    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    complete_path_open(&mut ctx, request, CHALLENGER_RX_PATH, FD_READ_RIGHT).await?;

    let read = expect_fd_read(ctx.endpoint().recv::<WasiFdReadReqMsg>().await?)?;
    expect_fd_object(
        &ctx,
        read.fd(),
        CHALLENGER_RX_OBJECT.object(),
        FD_READ_RIGHT,
    )?;
    let read_packet = NetPacket::new(2, b"read receipt")?;
    ctx.endpoint()
        .flow::<ChallengerReadMsg>()?
        .send(&read_packet)
        .await?;
    let read_receipt = ctx.endpoint().recv::<ChallengerReadRetMsg>().await?;
    if read_receipt.packet_id() != 2 || read_receipt.status() != 1 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let reply = EngineRet::FdReadDone(FdReadDone::new_with_lease(
        read.fd(),
        read.lease_id(),
        bounded(read_receipt.body().as_bytes(), read.max_len() as usize),
    )?);
    ctx.endpoint()
        .flow::<WasiFdReadRetMsg>()?
        .send(&reply)
        .await?;

    let request = expect_path_open(ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?)?;
    complete_path_open(&mut ctx, request, FACE_ACK_PATH, FD_WRITE_RIGHT).await?;

    let write = expect_fd_write(ctx.endpoint().recv::<WasiFdWriteReqMsg>().await?)?;
    expect_fd_object(&ctx, write.fd(), FACE_ACK_OBJECT.object(), FD_WRITE_RIGHT)?;
    if write.as_bytes() != b"face committed" {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let commit = CommitMarker::new(selected_face, challenger_status)?;
    ctx.endpoint()
        .flow::<FaceAckCommitMsg>()?
        .send(&commit)
        .await?;
    send_fd_write_done(&mut ctx, write.fd(), write.len()).await?;

    let proc_exit = ctx.endpoint().recv::<WasiProcExitReqMsg>().await?;
    let EngineReq::ProcExit(status) = proc_exit else {
        return Err(UnoQRuntimeError::RuntimeViolation);
    };
    if status.code() != 0 {
        return Err(UnoQRuntimeError::RuntimeViolation);
    }
    let final_commit = CommitMarker::new(selected_face, challenger_status)?;
    m33_board_commit_face(final_commit.face());
    ctx.endpoint()
        .flow::<FinalCommitMsg>()?
        .send(&final_commit)
        .await?;
    ctx.pending().await
}

async fn run_boundary<const ROLE: u8>(
    mut ctx: appkit::BoundaryCtx<'_, UnoQCapsule, ROLE>,
) -> appkit::RoleResult<UnoQRuntimeError> {
    match ROLE {
        ROLE_LINUX_KERNEL => {
            let prompt_request = ctx.endpoint().recv::<IosPromptRequestMsg>().await?;
            ctx.endpoint()
                .flow::<IosPromptRequestMsg>()?
                .send(&prompt_request)
                .await?;
            let prompt = ctx.endpoint().recv::<IosPromptFactMsg>().await?;
            ctx.endpoint()
                .flow::<IosPromptFactMsg>()?
                .send(&prompt)
                .await?;

            let prompt = ctx.endpoint().recv::<LlmPromptToLinuxMsg>().await?;
            ctx.endpoint()
                .flow::<LlmRequestToSidecarMsg>()?
                .send(&prompt)
                .await?;
            let proposal = ctx.endpoint().recv::<LlmProposalToLinuxMsg>().await?;
            let candidate = FaceCandidate::new(proposal.emotion(), 3)?;
            ctx.endpoint()
                .flow::<FaceCandidateToM33Msg>()?
                .send(&candidate)
                .await?;

            let ack = ctx.endpoint().recv::<FaceAckCommitMsg>().await?;
            if ack.face() != FACE_SPEAKING || ack.challenger_status() != 1 {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
            let final_commit = ctx.endpoint().recv::<FinalCommitMsg>().await?;
            if final_commit.face() != ack.face()
                || final_commit.challenger_status() != ack.challenger_status()
            {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
        }
        ROLE_IOS_PROMPT_INGRESS => {
            let request = ctx.endpoint().recv::<IosPromptRequestMsg>().await?;
            if request.as_bytes() != b"ios prompt" {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
            let prompt = ios_prompt_fact()?;
            ctx.endpoint()
                .flow::<IosPromptFactMsg>()?
                .send(&prompt)
                .await?;
        }
        ROLE_LLM_SIDECAR => {
            let prompt = ctx.endpoint().recv::<LlmRequestToSidecarMsg>().await?;
            if prompt.as_bytes().is_empty() {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
            let proposal =
                protocol::LlmProposal::new(FACE_SPEAKING, b"hibana choreography speaks")?;
            ctx.endpoint()
                .flow::<LlmProposalToLinuxMsg>()?
                .send(&proposal)
                .await?;
        }
        ROLE_CHALLENGER_KERNEL => {
            let packet = ctx.endpoint().recv::<ChallengerPacketMsg>().await?;
            if packet.packet_id() != 1 || packet.body().as_bytes() != b"challenger ping" {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
            let receipt = NetReceipt::new(1, 1, b"packet accepted")?;
            ctx.endpoint()
                .flow::<ChallengerReceiptMsg>()?
                .send(&receipt)
                .await?;

            let read = ctx.endpoint().recv::<ChallengerReadMsg>().await?;
            if read.packet_id() != 2 {
                return Err(UnoQRuntimeError::RuntimeViolation);
            }
            let read_reply = NetReceipt::new(2, 1, b"challenger ok happy")?;
            ctx.endpoint()
                .flow::<ChallengerReadRetMsg>()?
                .send(&read_reply)
                .await?;
        }
        _ => {}
    }
    ctx.pending().await
}

#[cfg(not(target_os = "none"))]
fn ios_prompt_fact() -> Result<SmallText, UnoQRuntimeError> {
    if std::env::var_os("UNO_Q_IOS_PROMPT_TCP").is_none() {
        return Ok(SmallText::new(b"face happy say hibana")?);
    }

    use std::io::{Read, Write};

    let addr = std::env::var("UNO_Q_IOS_PROMPT_ADDR").unwrap_or_else(|_| "0.0.0.0:7105".into());
    let listener =
        std::net::TcpListener::bind(addr).map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
    let (mut stream, peer) = listener
        .accept()
        .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
    core::hint::black_box(peer);

    let mut input = [0u8; 512];
    let len = stream
        .read(&mut input)
        .map_err(|_| UnoQRuntimeError::RuntimeViolation)?;
    let prompt = ios_prompt_from_wire(&input[..len]);
    let response =
        b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 16\r\n\r\nhibana accepted\n";
    let response_sent = stream.write_all(response).is_ok();
    core::hint::black_box(response_sent);
    SmallText::new(prompt).map_err(Into::into)
}

#[cfg(target_os = "none")]
fn ios_prompt_fact() -> Result<SmallText, UnoQRuntimeError> {
    SmallText::new(b"face happy say hibana").map_err(Into::into)
}

#[cfg(not(target_os = "none"))]
fn ios_prompt_from_wire(input: &[u8]) -> &[u8] {
    let body = match find_subslice(input, b"\r\n\r\n") {
        Some(index) => &input[index + 4..],
        None => input,
    };
    let body = body.strip_prefix(b"prompt=").unwrap_or(body);
    let trimmed = trim_ascii(body);
    if trimmed.is_empty() {
        b"face happy say hibana"
    } else {
        bounded(trimmed, protocol::MAX_TEXT_BYTES)
    }
}

#[cfg(not(target_os = "none"))]
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let last_start = haystack.len() - needle.len();
    let mut index = 0usize;
    while index <= last_start {
        if &haystack[index..index + needle.len()] == needle {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg(not(target_os = "none"))]
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
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

fn expect_fd_read(
    request: EngineReq,
) -> Result<hibana_pico::choreography::protocol::FdRead, UnoQRuntimeError> {
    match request {
        EngineReq::FdRead(request) => Ok(request),
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

fn bounded(bytes: &[u8], max_len: usize) -> &[u8] {
    &bytes[..bytes.len().min(max_len)]
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
        appkit::WasiImage::from_static(WASM_UNO_Q_LLM_FACE_CELL)
    }
}

impl appkit::ArtifactForImage<UnoQCapsule, site::Local<image::WasiLlmCellProcess>>
    for UnoQArtifacts
{
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_UNO_Q_LLM_FACE_CELL)
    }
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
    fn uno_q_m33_carrier_observe_tx(peer: u8, label: u8, len: u8);
    fn uno_q_m33_board_poll();
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

    fn operational_deadline_ticks(&self) -> Option<u32> {
        Some(UNO_Q_UART_OPERATIONAL_DEADLINE_TICKS)
    }

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box((interval_us, burst_bytes));
    }
}

#[cfg(not(target_os = "none"))]
pub struct HardwarePeerCarrier {
    local: ProofCarrier,
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
        Self {
            local: ProofCarrier::new(),
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
            let mut serial = self
                .serial
                .lock()
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
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return None;
        }
        self.local
            .edit(|queues| queues.by_role[local_role].front_label(rx.lane))
    }

    fn metrics(&self) -> Self::Metrics {}

    fn operational_deadline_ticks(&self) -> Option<u32> {
        match std::env::var("UNO_Q_HIBANA_DEADLINE_TICKS") {
            Ok(value) => value
                .parse::<u32>()
                .ok()
                .or(Some(UNO_Q_UART_OPERATIONAL_DEADLINE_TICKS)),
            Err(_) => Some(UNO_Q_UART_OPERATIONAL_DEADLINE_TICKS),
        }
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
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0x3f);
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
        appkit::PeerImageSet::pair(appkit::ImageId(712), appkit::ImageId(713));

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
    image::LinuxKernelProcess,
    712,
    7102,
    appkit::RoleSet::single(ROLE_LINUX_KERNEL),
    appkit::PeerImageSet::pair(appkit::ImageId(711), appkit::ImageId(713)),
    LINUX_KERNEL_ATTACH_STORAGE
);
impl_nowasi_image!(
    image::LlmSidecarProcess,
    713,
    7103,
    appkit::RoleSet::single(ROLE_LLM_SIDECAR),
    appkit::PeerImageSet::single(appkit::ImageId(712)),
    LLM_SIDECAR_ATTACH_STORAGE
);
impl_nowasi_image!(
    image::IosPromptIngressProcess,
    714,
    7104,
    appkit::RoleSet::single(ROLE_IOS_PROMPT_INGRESS),
    appkit::PeerImageSet::single(appkit::ImageId(712)),
    IOS_INGRESS_ATTACH_STORAGE
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

impl_nowasi_image!(
    image::ChallengerNetKernelImage,
    716,
    7106,
    appkit::RoleSet::single(ROLE_CHALLENGER_KERNEL),
    appkit::PeerImageSet::pair(appkit::ImageId(715), appkit::ImageId(712)),
    CHALLENGER_ATTACH_STORAGE
);

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
