#![no_std]

pub mod approval_boundary;
pub mod audit;
pub mod driver;
pub mod llm_boundary;
pub mod protocol;
pub mod wasi_agent;
pub mod x_boundary;

use core::cell::{Cell, UnsafeCell};
use core::convert::Infallible;
use core::task::Poll;

use hibana::g;
use hibana::integration::{
    cap::control::RouteDecisionKind,
    policy::{DecisionArm, DecisionResolution, ResolverError, ResolverRef},
    program::Projectable,
    runtime::LabelUniverse,
};
use hibana_pico::choreography::protocol::{
    EngineReq, EngineRet, FdReadDone, FdWriteDone, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET,
    LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET,
    LABEL_WASI_PROC_EXIT, PathOpened,
};
use hibana_pico::{appkit, site};

use protocol::{
    CodexProposalObject, LABEL_APPROVE_ROUTE, LABEL_APPROVED_REPLY_DRAFT, LABEL_APPROVED_X_REPLY,
    LABEL_AUTO_X_POST, LABEL_CODEX_REPLY_PROPOSAL, LABEL_CODEX_REPLY_REQUEST, LABEL_FENCE_ROUTE,
    LABEL_HUMAN_APPROVAL_REQUEST, LABEL_HUMAN_APPROVAL_RESPONSE, LABEL_NOT_APPROVED_ROUTE,
    LABEL_REJECT_ROUTE, LABEL_REJECTED_DRAFT, LABEL_REPLY_APPROVAL_REQUEST,
    LABEL_REPLY_INPUT_ADMITTED, LABEL_REPLY_INPUT_REQUEST, LABEL_SAFE_STOP, LABEL_UNTRUSTED_REPLY,
    LABEL_X_POST_COMMITTED, LABEL_X_REPLY_COMMITTED, ROLE_APPROVAL_BOUNDARY, ROLE_AUDIT,
    ROLE_DRIVER, ROLE_HUMAN_APPROVAL_DEVICE, ROLE_LLM_BOUNDARY, ROLE_WASI_AGENT, ROLE_X_BOUNDARY,
};

pub struct XBotCapsule;
pub struct XBotPlacement;
pub struct XBotLocal;
pub struct XBotArtifacts;
#[derive(Clone, Copy, Debug, Default)]
pub struct XBotLabelUniverse;

impl LabelUniverse for XBotLabelUniverse {
    const MAX_LABEL: u8 = LABEL_SAFE_STOP;
}

pub const XBOT_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(70_01);
pub const XBOT_PREOPEN_FD: u8 = 8;
pub const XBOT_PROPOSAL_FD: u8 = 12;
pub const XBOT_DRAFT_FD: u8 = 13;
const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const PROOF_CARRIER_ROLES: usize = 8;
const PROOF_CARRIER_QUEUE_DEPTH: usize = 16;
const PROOF_CARRIER_FRAME_BYTES: usize = 256;
const PROOF_CODEX_PROPOSAL: &[u8] = b"safe";
const XBOT_APPROVAL_LEFT_POLICY: u16 = 70;
const XBOT_APPROVAL_RIGHT_POLICY: u16 = 71;

pub const CODEX_PROPOSAL_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"xbot/codex-proposal.txt",
    appkit::ObjectId(10_010),
    appkit::FdSpec::new(XBOT_PROPOSAL_FD as u32, FD_READ_RIGHT, 1),
);

pub const REPLY_DRAFT_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"xbot/reply-draft.txt",
    appkit::ObjectId(10_011),
    appkit::FdSpec::new(XBOT_DRAFT_FD as u32, FD_WRITE_RIGHT, 1),
);

static XBOT_DRIVER_FACTS: appkit::ChoreoFsObjectSet<4> = appkit::ChoreoFsObjectSet::new([
    x_boundary::DRAFT_ROOT_OBJECT,
    x_boundary::COMMIT_LEDGER_OBJECT,
    CODEX_PROPOSAL_OBJECT,
    REPLY_DRAFT_OBJECT,
]);

type NotApprovedRouteMsg = g::Msg<LABEL_NOT_APPROVED_ROUTE, (), RouteDecisionKind>;
type ApproveRouteMsg = g::Msg<LABEL_APPROVE_ROUTE, (), RouteDecisionKind>;
type RejectRouteMsg = g::Msg<LABEL_REJECT_ROUTE, (), RouteDecisionKind>;
type FenceRouteMsg = g::Msg<LABEL_FENCE_ROUTE, (), RouteDecisionKind>;

fn xbot_approval_left_resolver() -> Result<DecisionResolution, ResolverError> {
    Ok(DecisionResolution::Arm(DecisionArm::Left))
}

fn xbot_approval_right_resolver() -> Result<DecisionResolution, ResolverError> {
    Ok(DecisionResolution::Arm(DecisionArm::Right))
}

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_XBOT_REPLY_NORMALIZER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-xbot-reply-normalizer.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_XBOT_REPLY_NORMALIZER: &[u8] = &[];

#[cfg(feature = "runtime-wasip1")]
static mut XBOT_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(feature = "runtime-wasip1")]
fn xbot_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
    core::hint::black_box(ROLE);
    unsafe { (&mut *core::ptr::addr_of_mut!(XBOT_WASI_GUEST_ARENA)).lease() }
}

pub mod image {
    pub struct WasiAgentProcess;
    pub struct DriverProcess;
    pub struct ApprovalBoundaryProcess;
    pub struct HumanApprovalDeviceProcess;
    pub struct XBoundaryProcess;
    pub struct AuditProcess;
    pub struct LlmBoundaryProcess;
    pub struct HostProofProcess;
}

impl appkit::Capsule for XBotCapsule {
    type Universe = XBotLabelUniverse;
    type Placement = XBotPlacement;
    type Local = XBotLocal;
    type Report = Infallible;

    fn choreography() -> impl Projectable {
        let auto_post = g::seq(
            g::send::<ROLE_DRIVER, ROLE_X_BOUNDARY, g::Msg<LABEL_AUTO_X_POST, u8>, 0>(),
            g::send::<ROLE_X_BOUNDARY, ROLE_AUDIT, g::Msg<LABEL_X_POST_COMMITTED, u8>, 0>(),
        );

        let reply_action_rejected = || {
            g::seq(
                g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_APPROVAL_BOUNDARY, RejectRouteMsg, 0>()
                    .policy::<XBOT_APPROVAL_LEFT_POLICY>(),
                g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_AUDIT, g::Msg<LABEL_REJECTED_DRAFT, u8>, 0>(
                ),
            )
        };
        let reply_action_fenced = || {
            g::seq(
                g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_APPROVAL_BOUNDARY, FenceRouteMsg, 0>()
                    .policy::<XBOT_APPROVAL_RIGHT_POLICY>(),
                g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_AUDIT, g::Msg<LABEL_SAFE_STOP, u8>, 0>(),
            )
        };
        let reply_action_approved = g::seq(
            g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_APPROVAL_BOUNDARY, ApproveRouteMsg, 0>()
                .policy::<XBOT_APPROVAL_LEFT_POLICY>(),
            g::seq(
                g::send::<
                    ROLE_APPROVAL_BOUNDARY,
                    ROLE_DRIVER,
                    g::Msg<LABEL_APPROVED_REPLY_DRAFT, u8>,
                    0,
                >(),
                g::seq(
                    g::send::<ROLE_DRIVER, ROLE_X_BOUNDARY, g::Msg<LABEL_APPROVED_X_REPLY, u8>, 0>(
                    ),
                    g::send::<ROLE_X_BOUNDARY, ROLE_AUDIT, g::Msg<LABEL_X_REPLY_COMMITTED, u8>, 0>(
                    ),
                ),
            ),
        );
        let reply_action_not_approved = g::seq(
            g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_APPROVAL_BOUNDARY, NotApprovedRouteMsg, 0>()
                .policy::<XBOT_APPROVAL_RIGHT_POLICY>(),
            g::route(reply_action_rejected(), reply_action_fenced()),
        );
        let wasi_reply_read_prefix = g::seq(
            g::send::<ROLE_WASI_AGENT, ROLE_DRIVER, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 0>(),
            g::seq(
                g::send::<
                    ROLE_DRIVER,
                    ROLE_WASI_AGENT,
                    g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>,
                    0,
                >(),
                g::send::<ROLE_WASI_AGENT, ROLE_DRIVER, g::Msg<LABEL_WASI_FD_READ, EngineReq>, 0>(),
            ),
        );
        let wasi_reply_finish_path = g::seq(
            g::send::<ROLE_DRIVER, ROLE_WASI_AGENT, g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 0>(),
            g::seq(
                g::send::<ROLE_WASI_AGENT, ROLE_DRIVER, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(
                ),
                g::seq(
                    g::send::<
                        ROLE_DRIVER,
                        ROLE_WASI_AGENT,
                        g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>,
                        1,
                    >(),
                    g::seq(
                        g::send::<
                            ROLE_WASI_AGENT,
                            ROLE_DRIVER,
                            g::Msg<LABEL_WASI_FD_WRITE, EngineReq>,
                            1,
                        >(),
                        g::seq(
                            g::send::<
                                ROLE_DRIVER,
                                ROLE_WASI_AGENT,
                                g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>,
                                1,
                            >(),
                            g::send::<
                                ROLE_WASI_AGENT,
                                ROLE_DRIVER,
                                g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>,
                                1,
                            >(),
                        ),
                    ),
                ),
            ),
        );
        let reply_action_path = g::seq(
            wasi_reply_finish_path,
            g::seq(
                g::send::<
                    ROLE_DRIVER,
                    ROLE_APPROVAL_BOUNDARY,
                    g::Msg<LABEL_REPLY_APPROVAL_REQUEST, u8>,
                    0,
                >(),
                g::seq(
                    g::send::<
                        ROLE_APPROVAL_BOUNDARY,
                        ROLE_HUMAN_APPROVAL_DEVICE,
                        g::Msg<LABEL_HUMAN_APPROVAL_REQUEST, u8>,
                        0,
                    >(),
                    g::seq(
                        g::send::<
                            ROLE_HUMAN_APPROVAL_DEVICE,
                            ROLE_APPROVAL_BOUNDARY,
                            g::Msg<LABEL_HUMAN_APPROVAL_RESPONSE, u8>,
                            0,
                        >(),
                        g::route(reply_action_approved, reply_action_not_approved),
                    ),
                ),
            ),
        );

        let codex_reply_proposal_path = g::seq(
            g::send::<ROLE_DRIVER, ROLE_LLM_BOUNDARY, g::Msg<LABEL_CODEX_REPLY_REQUEST, u8>, 0>(),
            g::send::<
                ROLE_LLM_BOUNDARY,
                ROLE_DRIVER,
                g::Msg<LABEL_CODEX_REPLY_PROPOSAL, CodexProposalObject>,
                0,
            >(),
        );
        let reply_input_admitted = g::seq(
            g::send::<ROLE_APPROVAL_BOUNDARY, ROLE_DRIVER, g::Msg<LABEL_REPLY_INPUT_ADMITTED, u8>, 0>(
            ),
            g::seq(codex_reply_proposal_path, reply_action_path),
        );
        let reply_input_path = g::seq(
            g::send::<ROLE_X_BOUNDARY, ROLE_DRIVER, g::Msg<LABEL_UNTRUSTED_REPLY, u8>, 0>(),
            g::seq(
                g::send::<
                    ROLE_DRIVER,
                    ROLE_APPROVAL_BOUNDARY,
                    g::Msg<LABEL_REPLY_INPUT_REQUEST, u8>,
                    0,
                >(),
                g::seq(
                    g::send::<
                        ROLE_APPROVAL_BOUNDARY,
                        ROLE_HUMAN_APPROVAL_DEVICE,
                        g::Msg<LABEL_HUMAN_APPROVAL_REQUEST, u8>,
                        0,
                    >(),
                    g::seq(
                        g::send::<
                            ROLE_HUMAN_APPROVAL_DEVICE,
                            ROLE_APPROVAL_BOUNDARY,
                            g::Msg<LABEL_HUMAN_APPROVAL_RESPONSE, u8>,
                            0,
                        >(),
                        reply_input_admitted,
                    ),
                ),
            ),
        );
        g::seq(wasi_reply_read_prefix, g::seq(auto_post, reply_input_path))
    }

    fn register_resolvers<'cfg, R>(registry: &mut R)
    where
        R: appkit::ResolverRegistry<'cfg, Self>,
    {
        registry.policy::<XBOT_APPROVAL_LEFT_POLICY, ROLE_APPROVAL_BOUNDARY>(
            ResolverRef::decision_fn(xbot_approval_left_resolver),
        );
        registry.policy::<XBOT_APPROVAL_RIGHT_POLICY, ROLE_APPROVAL_BOUNDARY>(
            ResolverRef::decision_fn(xbot_approval_right_resolver),
        );
    }
}

impl appkit::Placement<XBotCapsule> for XBotPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            ROLE_WASI_AGENT => appkit::RoleKind::Engine,
            ROLE_DRIVER => appkit::RoleKind::Driver,
            ROLE_APPROVAL_BOUNDARY
            | ROLE_HUMAN_APPROVAL_DEVICE
            | ROLE_X_BOUNDARY
            | ROLE_LLM_BOUNDARY => appkit::RoleKind::Boundary,
            ROLE_AUDIT => appkit::RoleKind::Supervisor,
            _ => appkit::RoleKind::Link,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum XBotRuntimeError {
    Endpoint(hibana::EndpointError),
    Wire(hibana::integration::wire::CodecError),
    Protocol(protocol::ProtocolError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for XBotRuntimeError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<hibana::integration::wire::CodecError> for XBotRuntimeError {
    fn from(error: hibana::integration::wire::CodecError) -> Self {
        Self::Wire(error)
    }
}

impl From<protocol::ProtocolError> for XBotRuntimeError {
    fn from(error: protocol::ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

type AutoXPostMsg = g::Msg<LABEL_AUTO_X_POST, u8>;
type XPostCommittedMsg = g::Msg<LABEL_X_POST_COMMITTED, u8>;
type UntrustedReplyMsg = g::Msg<LABEL_UNTRUSTED_REPLY, u8>;
type ReplyInputRequestMsg = g::Msg<LABEL_REPLY_INPUT_REQUEST, u8>;
type HumanApprovalRequestMsg = g::Msg<LABEL_HUMAN_APPROVAL_REQUEST, u8>;
type HumanApprovalResponseMsg = g::Msg<LABEL_HUMAN_APPROVAL_RESPONSE, u8>;
type ReplyInputAdmittedMsg = g::Msg<LABEL_REPLY_INPUT_ADMITTED, u8>;
type CodexReplyRequestMsg = g::Msg<LABEL_CODEX_REPLY_REQUEST, u8>;
type CodexReplyProposalMsg = g::Msg<LABEL_CODEX_REPLY_PROPOSAL, CodexProposalObject>;
type ReplyApprovalRequestMsg = g::Msg<LABEL_REPLY_APPROVAL_REQUEST, u8>;
type ApprovedReplyDraftMsg = g::Msg<LABEL_APPROVED_REPLY_DRAFT, u8>;
type ApprovedXReplyMsg = g::Msg<LABEL_APPROVED_X_REPLY, u8>;
type XReplyCommittedMsg = g::Msg<LABEL_X_REPLY_COMMITTED, u8>;
type WasiPathOpenReqMsg = g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>;
type WasiPathOpenRetMsg = g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>;
type WasiFdReadReqMsg = g::Msg<LABEL_WASI_FD_READ, EngineReq>;
type WasiFdReadRetMsg = g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>;
type WasiFdWriteReqMsg = g::Msg<LABEL_WASI_FD_WRITE, EngineReq>;
type WasiFdWriteRetMsg = g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>;
type WasiProcExitReqMsg = g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>;

async fn run_xbot_driver<const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, XBotCapsule, ROLE>,
) -> appkit::RoleResult<XBotRuntimeError> {
    if ROLE != ROLE_DRIVER {
        return ctx.pending().await;
    }

    let path_open = ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?;
    let EngineReq::PathOpen(proposal_open) = path_open else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    complete_path_open(
        &mut ctx,
        proposal_open,
        CODEX_PROPOSAL_OBJECT.path(),
        FD_READ_RIGHT,
    )
    .await?;

    let read_req = ctx.endpoint().recv::<WasiFdReadReqMsg>().await?;
    let EngineReq::FdRead(read) = read_req else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    let proposal_fact = ctx.ledger().fd(read.fd() as u32);
    if proposal_fact.map(|fact| fact.object()) != Some(CODEX_PROPOSAL_OBJECT.object()) {
        return Err(XBotRuntimeError::RuntimeViolation);
    }

    ctx.endpoint().flow::<AutoXPostMsg>()?.send(&1).await?;
    accept_marker(ctx.endpoint().recv::<UntrustedReplyMsg>().await?)?;
    ctx.endpoint()
        .flow::<ReplyInputRequestMsg>()?
        .send(&1)
        .await?;
    accept_marker(ctx.endpoint().recv::<ReplyInputAdmittedMsg>().await?)?;
    ctx.endpoint()
        .flow::<CodexReplyRequestMsg>()?
        .send(&1)
        .await?;
    let proposal = ctx.endpoint().recv::<CodexReplyProposalMsg>().await?;

    let reply = EngineRet::FdReadDone(FdReadDone::new_with_lease(
        read.fd(),
        read.lease_id(),
        bounded_prefix(proposal.as_bytes(), read.max_len() as usize),
    )?);
    ctx.endpoint()
        .flow::<WasiFdReadRetMsg>()?
        .send(&reply)
        .await?;

    let path_open = ctx.endpoint().recv::<WasiPathOpenReqMsg>().await?;
    let EngineReq::PathOpen(draft_open) = path_open else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    complete_path_open(
        &mut ctx,
        draft_open,
        REPLY_DRAFT_OBJECT.path(),
        FD_WRITE_RIGHT,
    )
    .await?;

    let write_req = ctx.endpoint().recv::<WasiFdWriteReqMsg>().await?;
    let EngineReq::FdWrite(write) = write_req else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    let draft_fact = ctx.ledger().fd(write.fd() as u32);
    if draft_fact.map(|fact| fact.object()) != Some(REPLY_DRAFT_OBJECT.object()) {
        return Err(XBotRuntimeError::RuntimeViolation);
    }
    let mut expected = [0u8; protocol::MAX_BODY_BYTES];
    let expected_len = normalize_codex_reply(proposal.as_bytes(), &mut expected);
    if write.as_bytes() != &expected[..expected_len] {
        return Err(XBotRuntimeError::RuntimeViolation);
    }
    let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
    ctx.endpoint()
        .flow::<WasiFdWriteRetMsg>()?
        .send(&reply)
        .await?;

    let proc_exit = ctx.endpoint().recv::<WasiProcExitReqMsg>().await?;
    let EngineReq::ProcExit(status) = proc_exit else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    if status.code() != 0 {
        return Err(XBotRuntimeError::RuntimeViolation);
    }

    ctx.endpoint()
        .flow::<ReplyApprovalRequestMsg>()?
        .send(&1)
        .await?;
    let branch = ctx.endpoint().offer().await?;
    accept_marker(branch.decode::<ApprovedReplyDraftMsg>().await?)?;
    ctx.endpoint().flow::<ApprovedXReplyMsg>()?.send(&1).await?;
    ctx.pending().await
}

async fn complete_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, XBotCapsule, ROLE>,
    request: hibana_pico::choreography::protocol::PathOpen,
    expected_path: &[u8],
    expected_rights: u64,
) -> Result<(), XBotRuntimeError> {
    if request.preopen_fd() != XBOT_PREOPEN_FD || request.rights_base() != expected_rights {
        return Err(XBotRuntimeError::RuntimeViolation);
    }
    if request.path() != expected_path {
        return Err(XBotRuntimeError::RuntimeViolation);
    }
    let Some(object) = ctx.choreofs().resolve(expected_path) else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    let Some(fd) = ctx
        .ledger()
        .fds()
        .iter()
        .copied()
        .find(|fact| fact.object() == object)
    else {
        return Err(XBotRuntimeError::RuntimeViolation);
    };
    if fd.rights() != expected_rights {
        return Err(XBotRuntimeError::RuntimeViolation);
    }
    let reply = EngineRet::PathOpened(PathOpened::new(fd.fd() as u8, 0));
    ctx.endpoint()
        .flow::<WasiPathOpenRetMsg>()?
        .send(&reply)
        .await?;
    Ok(())
}

fn bounded_prefix(bytes: &[u8], max_len: usize) -> &[u8] {
    let len = bytes.len().min(max_len);
    &bytes[..len]
}

fn normalize_codex_reply(input: &[u8], out: &mut [u8; protocol::MAX_BODY_BYTES]) -> usize {
    let mut in_idx = 0usize;
    let mut out_idx = 0usize;
    while in_idx < input.len() && out_idx < out.len() {
        let byte = input[in_idx];
        let normalized = match byte {
            b'\r' | b'\n' | b'\t' => b' ',
            _ => byte,
        };
        if normalized != b' ' || out_idx == 0 || out[out_idx - 1] != b' ' {
            out[out_idx] = normalized;
            out_idx += 1;
        }
        in_idx += 1;
    }
    while out_idx > 0 && out[out_idx - 1] == b' ' {
        out_idx -= 1;
    }
    out_idx
}

fn accept_marker(value: u8) -> Result<(), XBotRuntimeError> {
    if value == 1 {
        Ok(())
    } else {
        Err(XBotRuntimeError::RuntimeViolation)
    }
}

async fn run_xbot_boundary<const ROLE: u8>(
    mut ctx: appkit::BoundaryCtx<'_, XBotCapsule, ROLE>,
) -> appkit::RoleResult<XBotRuntimeError> {
    match ROLE {
        ROLE_X_BOUNDARY => {
            accept_marker(ctx.endpoint().recv::<AutoXPostMsg>().await?)?;
            ctx.endpoint().flow::<XPostCommittedMsg>()?.send(&1).await?;
            ctx.endpoint().flow::<UntrustedReplyMsg>()?.send(&1).await?;
            let branch = ctx.endpoint().offer().await?;
            accept_marker(branch.decode::<ApprovedXReplyMsg>().await?)?;
            ctx.endpoint()
                .flow::<XReplyCommittedMsg>()?
                .send(&1)
                .await?;
        }
        ROLE_APPROVAL_BOUNDARY => {
            accept_marker(ctx.endpoint().recv::<ReplyInputRequestMsg>().await?)?;
            ctx.endpoint()
                .flow::<HumanApprovalRequestMsg>()?
                .send(&1)
                .await?;
            accept_marker(ctx.endpoint().recv::<HumanApprovalResponseMsg>().await?)?;
            ctx.endpoint()
                .flow::<ReplyInputAdmittedMsg>()?
                .send(&1)
                .await?;
            accept_marker(ctx.endpoint().recv::<ReplyApprovalRequestMsg>().await?)?;
            ctx.endpoint()
                .flow::<HumanApprovalRequestMsg>()?
                .send(&1)
                .await?;
            accept_marker(ctx.endpoint().recv::<HumanApprovalResponseMsg>().await?)?;
            ctx.endpoint().flow::<ApproveRouteMsg>()?.send(&()).await?;
            ctx.endpoint()
                .flow::<ApprovedReplyDraftMsg>()?
                .send(&1)
                .await?;
        }
        ROLE_HUMAN_APPROVAL_DEVICE => {
            accept_marker(ctx.endpoint().recv::<HumanApprovalRequestMsg>().await?)?;
            ctx.endpoint()
                .flow::<HumanApprovalResponseMsg>()?
                .send(&1)
                .await?;
            accept_marker(ctx.endpoint().recv::<HumanApprovalRequestMsg>().await?)?;
            ctx.endpoint()
                .flow::<HumanApprovalResponseMsg>()?
                .send(&1)
                .await?;
        }
        ROLE_LLM_BOUNDARY => {
            accept_marker(ctx.endpoint().recv::<CodexReplyRequestMsg>().await?)?;
            let proposal = CodexProposalObject::new(PROOF_CODEX_PROPOSAL)?;
            ctx.endpoint()
                .flow::<CodexReplyProposalMsg>()?
                .send(&proposal)
                .await?;
        }
        _ => {}
    }
    ctx.pending().await
}

async fn run_xbot_audit<const ROLE: u8>(
    mut ctx: appkit::SupervisorCtx<'_, XBotCapsule, ROLE>,
) -> appkit::RoleResult<XBotRuntimeError> {
    if ROLE == ROLE_AUDIT {
        accept_marker(ctx.endpoint().recv::<XPostCommittedMsg>().await?)?;
        let branch = ctx.endpoint().offer().await?;
        accept_marker(branch.decode::<XReplyCommittedMsg>().await?)?;
    }
    ctx.pending().await
}

impl appkit::Localside<XBotCapsule> for XBotLocal {
    type Error = XBotRuntimeError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, XBotCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, XBotCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_xbot_driver(ctx)
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, XBotCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_xbot_boundary(ctx)
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, XBotCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, XBotCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        run_xbot_audit(ctx)
    }
}

impl appkit::ArtifactForImage<XBotCapsule, site::Local<image::WasiAgentProcess>> for XBotArtifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_XBOT_REPLY_NORMALIZER)
    }
}

impl appkit::ArtifactForImage<XBotCapsule, site::Local<image::HostProofProcess>> for XBotArtifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_XBOT_REPLY_NORMALIZER)
    }
}

impl<I> appkit::ArtifactForImage<XBotCapsule, I> for XBotArtifacts
where
    I: appkit::LogicalImage<XBotCapsule, Artifact = appkit::NoWasi>,
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
        payload: hibana::integration::wire::Payload<'_>,
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

impl hibana::integration::transport::Transport for ProofCarrier {
    type Error = hibana::integration::transport::TransportError;
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
        port: hibana::integration::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane().as_wire();
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
        &self,
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
    ) -> Poll<Result<hibana::integration::transport::Incoming<'a>, Self::Error>> {
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
        Poll::Ready(Ok(hibana::integration::transport::Incoming::new(
            hibana::integration::transport::FrameHeader::new(
                hibana::integration::ids::SessionId::new(rx.session_id),
                hibana::integration::ids::Lane::new(rx.lane as u32),
                0,
                rx.local_role,
                frame.frame_label,
            ),
            hibana::integration::wire::Payload::new(&rx.bytes[..rx.len]),
        )))
    }

    fn requeue<'a>(&self, rx: &mut Self::Rx<'a>) -> Result<(), Self::Error> {
        if let Some(frame_label) = rx.frame_label.take() {
            let local_role = rx.local_role as usize;
            if local_role < PROOF_CARRIER_ROLES {
                self.edit(|queues| {
                    queues.by_role[local_role].push_front(rx.lane, frame_label, &rx.bytes[..rx.len])
                });
            }
        }
        rx.hint_frame_label.set(None);
        Ok(())
    }

    fn peek_recv_frame<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameHeader> {
        if let Some(frame_label) = rx.hint_frame_label.get() {
            return Some(hibana::integration::transport::FrameHeader::new(
                hibana::integration::ids::SessionId::new(rx.session_id),
                hibana::integration::ids::Lane::new(rx.lane as u32),
                0,
                rx.local_role,
                frame_label,
            ));
        }
        let local_role = rx.local_role as usize;
        if local_role >= PROOF_CARRIER_ROLES {
            return None;
        }
        self.edit(|queues| queues.by_role[local_role].front_label(rx.lane))
            .map(|frame_label| {
                hibana::integration::transport::FrameHeader::new(
                    hibana::integration::ids::SessionId::new(rx.session_id),
                    hibana::integration::ids::Lane::new(rx.lane as u32),
                    0,
                    rx.local_role,
                    frame_label,
                )
            })
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::WasiAgentProcess> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(70);
    const SITE_ID: appkit::SiteId = appkit::SiteId(700);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_WASI_AGENT);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::DriverProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(71);
    const SITE_ID: appkit::SiteId = appkit::SiteId(701);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_DRIVER);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        XBOT_DRIVER_FACTS.driver_facts()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::ApprovalBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(72);
    const SITE_ID: appkit::SiteId = appkit::SiteId(702);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_APPROVAL_BOUNDARY);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        XBOT_DRIVER_FACTS.driver_facts()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::HumanApprovalDeviceProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(73);
    const SITE_ID: appkit::SiteId = appkit::SiteId(703);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_HUMAN_APPROVAL_DEVICE);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::XBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(74);
    const SITE_ID: appkit::SiteId = appkit::SiteId(704);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_X_BOUNDARY);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        XBOT_DRIVER_FACTS.driver_facts()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::LlmBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(76);
    const SITE_ID: appkit::SiteId = appkit::SiteId(706);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_LLM_BOUNDARY);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::AuditProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(75);
    const SITE_ID: appkit::SiteId = appkit::SiteId(705);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_AUDIT);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }
}

impl appkit::LogicalImage<XBotCapsule> for site::Local<image::HostProofProcess> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = ProofCarrier
    where
        Self: 'a,
        XBotCapsule: 'a;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(77);
    const SITE_ID: appkit::SiteId = appkit::SiteId(707);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0x7f);
    const CARRIER: appkit::CarrierKind = XBOT_CARRIER;

    fn init() -> Self {
        site::Local::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        XBotCapsule: 'a,
    {
        ProofCarrier::new()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        XBOT_DRIVER_FACTS.driver_facts()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage<XBotCapsule> for site::Local<image::WasiAgentProcess> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        xbot_wasi_guest_lease::<ROLE>()
    }
}

#[cfg(feature = "runtime-wasip1")]
impl appkit::WasiGuestImage<XBotCapsule> for site::Local<image::HostProofProcess> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        xbot_wasi_guest_lease::<ROLE>()
    }
}

pub fn projection_caps() -> appkit::ProjectionCaps {
    appkit::derive_projection_caps::<XBotCapsule>()
}
