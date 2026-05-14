use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineAbortReason {
    GuestTrap,
    GuestWrapperError,
    UnsupportedImport,
    FuelExhausted,
    MemoryFault,
    BadImportShape,
}

impl EngineAbortReason {
    pub const fn tag(self) -> u8 {
        match self {
            Self::GuestTrap => 1,
            Self::GuestWrapperError => 2,
            Self::UnsupportedImport => 3,
            Self::FuelExhausted => 4,
            Self::MemoryFault => 5,
            Self::BadImportShape => 6,
        }
    }

    fn decode(tag: u8) -> Result<Self, CodecError> {
        match tag {
            1 => Ok(Self::GuestTrap),
            2 => Ok(Self::GuestWrapperError),
            3 => Ok(Self::UnsupportedImport),
            4 => Ok(Self::FuelExhausted),
            5 => Ok(Self::MemoryFault),
            6 => Ok(Self::BadImportShape),
            _ => Err(CodecError::Invalid("unknown engine abort reason")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineAbort {
    reason: EngineAbortReason,
    code: u16,
}

impl EngineAbort {
    pub const fn new(reason: EngineAbortReason, code: u16) -> Self {
        Self { reason, code }
    }

    pub const fn reason(&self) -> EngineAbortReason {
        self.reason
    }

    pub const fn code(&self) -> u16 {
        self.code
    }
}

impl WireEncode for EngineAbort {
    fn encoded_len(&self) -> Option<usize> {
        Some(3)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 3 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.reason.tag();
        out[1..3].copy_from_slice(&self.code.to_be_bytes());
        Ok(3)
    }
}

impl WirePayload for EngineAbort {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 3 {
            return Err(CodecError::Truncated);
        }
        if bytes.len() > 3 {
            return Err(CodecError::Invalid(
                "unexpected trailing engine abort bytes",
            ));
        }
        let reason = EngineAbortReason::decode(bytes[0])?;
        let code = u16::from_be_bytes([bytes[1], bytes[2]]);
        Ok(Self { reason, code })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineAbortBeginKind;

impl ResourceKind for EngineAbortBeginKind {
    type Handle = AbortControlWireHandle;
    const TAG: u8 = 0x41;
    const NAME: &'static str = "EngineAbortBegin";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_abort_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_abort_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for EngineAbortBeginKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Abort;
    const TAP_ID: u16 = 0x0500;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::AbortBegin;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(scope.raw());
        (sid.raw(), lane.raw() as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineAbortFenceKind;

impl ResourceKind for EngineAbortFenceKind {
    type Handle = AbortControlWireHandle;
    const TAG: u8 = 0x42;
    const NAME: &'static str = "EngineAbortFence";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_abort_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_abort_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for EngineAbortFenceKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Abort;
    const TAP_ID: u16 = 0x0501;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::Fence;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(scope.raw());
        (sid.raw(), lane.raw() as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineAbortAckKind;

impl ResourceKind for EngineAbortAckKind {
    type Handle = AbortControlWireHandle;
    const TAG: u8 = 0x43;
    const NAME: &'static str = "EngineAbortAck";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_abort_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_abort_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for EngineAbortAckKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Abort;
    const TAP_ID: u16 = 0x0502;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::AbortAck;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(scope.raw());
        (sid.raw(), lane.raw() as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivationAuthorityKind;

impl ResourceKind for ActivationAuthorityKind {
    type Handle = LifecycleControlWireHandle;
    const TAG: u8 = 0x44;
    const NAME: &'static str = "ActivationAuthority";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for ActivationAuthorityKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Delegate;
    const TAP_ID: u16 = 0x0510;
    const SHOT: CapShot = CapShot::Many;
    const PATH: ControlPath = ControlPath::Local;
    const OP: ControlOp = ControlOp::CapDelegate;
    const AUTO_MINT_WIRE: bool = false;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (0, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivationKind;

impl ResourceKind for ActivationKind {
    type Handle = LifecycleControlWireHandle;
    const TAG: u8 = 0x45;
    const NAME: &'static str = "Activation";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for ActivationKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Delegate;
    const TAP_ID: u16 = 0x0511;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Local;
    const OP: ControlOp = ControlOp::CapDelegate;
    const AUTO_MINT_WIRE: bool = false;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (1, scope.raw())
    }
}

pub type ReentryPermitKind = ActivationAuthorityKind;
pub type ActivationPermitKind = ActivationKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyBeginKind;

impl ResourceKind for TopologyBeginKind {
    type Handle = TopologyControlWireHandle;
    const TAG: u8 = 0x46;
    const NAME: &'static str = "TopologyBegin";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for TopologyBeginKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Topology;
    const TAP_ID: u16 = 0x0520;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::TopologyBegin;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (0, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyAckKind;

impl ResourceKind for TopologyAckKind {
    type Handle = TopologyControlWireHandle;
    const TAG: u8 = 0x47;
    const NAME: &'static str = "TopologyAck";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for TopologyAckKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Topology;
    const TAP_ID: u16 = 0x0521;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::TopologyAck;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (1, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyCommitKind;

impl ResourceKind for TopologyCommitKind {
    type Handle = TopologyControlWireHandle;
    const TAG: u8 = 0x48;
    const NAME: &'static str = "TopologyCommit";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for TopologyCommitKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Topology;
    const TAP_ID: u16 = 0x0522;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::TopologyCommit;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (2, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxCommitKind;

impl ResourceKind for TxCommitKind {
    type Handle = TransactionControlWireHandle;
    const TAG: u8 = 0x49;
    const NAME: &'static str = "TxCommit";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for TxCommitKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Policy;
    const TAP_ID: u16 = 0x0530;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::TxCommit;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (0, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxAbortKind;

impl ResourceKind for TxAbortKind {
    type Handle = TransactionControlWireHandle;
    const TAG: u8 = 0x4a;
    const NAME: &'static str = "TxAbort";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for TxAbortKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::Policy;
    const TAP_ID: u16 = 0x0531;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::TxAbort;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (1, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateSnapshotKind;

impl ResourceKind for StateSnapshotKind {
    type Handle = StateControlWireHandle;
    const TAG: u8 = 0x4b;
    const NAME: &'static str = "StateSnapshot";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for StateSnapshotKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::State;
    const TAP_ID: u16 = 0x0540;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::StateSnapshot;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (0, scope.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRestoreKind;

impl ResourceKind for StateRestoreKind {
    type Handle = StateControlWireHandle;
    const TAG: u8 = 0x4c;
    const NAME: &'static str = "StateRestore";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        encode_control_handle(*handle)
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        decode_control_handle(data)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl ControlResourceKind for StateRestoreKind {
    const SCOPE: ControlScopeKind = ControlScopeKind::State;
    const TAP_ID: u16 = 0x0541;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Wire;
    const OP: ControlOp = ControlOp::StateRestore;
    const AUTO_MINT_WIRE: bool = true;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (1, scope.raw())
    }
}

pub type EngineAbortBeginControl = Msg<
    LABEL_ENGINE_ABORT_BEGIN_CONTROL,
    GenericCapToken<EngineAbortBeginKind>,
    EngineAbortBeginKind,
>;
pub type EngineAbortMsg = Msg<LABEL_ENGINE_ABORT_REASON, EngineAbort>;
pub type EngineAbortFenceControl = Msg<
    LABEL_ENGINE_ABORT_FENCE_CONTROL,
    GenericCapToken<EngineAbortFenceKind>,
    EngineAbortFenceKind,
>;
pub type EngineAbortAckControl =
    Msg<LABEL_ENGINE_ABORT_ACK_CONTROL, GenericCapToken<EngineAbortAckKind>, EngineAbortAckKind>;
pub type ReentryPermitControl =
    Msg<LABEL_REENTRY_PERMIT_CONTROL, GenericCapToken<ReentryPermitKind>, ReentryPermitKind>;
pub type ActivationPermitControl = Msg<
    LABEL_ACTIVATION_PERMIT_CONTROL,
    GenericCapToken<ActivationPermitKind>,
    ActivationPermitKind,
>;
pub type ActivationAuthorityControl = Msg<
    LABEL_ACTIVATION_AUTHORITY_CONTROL,
    GenericCapToken<ActivationAuthorityKind>,
    ActivationAuthorityKind,
>;
pub type ActivationControl =
    Msg<LABEL_ACTIVATION_CONTROL, GenericCapToken<ActivationKind>, ActivationKind>;
pub type TopologyBeginControl =
    Msg<LABEL_TOPOLOGY_BEGIN_CONTROL, GenericCapToken<TopologyBeginKind>, TopologyBeginKind>;
pub type TopologyAckControl =
    Msg<LABEL_TOPOLOGY_ACK_CONTROL, GenericCapToken<TopologyAckKind>, TopologyAckKind>;
pub type TopologyCommitControl =
    Msg<LABEL_TOPOLOGY_COMMIT_CONTROL, GenericCapToken<TopologyCommitKind>, TopologyCommitKind>;
pub type TxCommitControl =
    Msg<LABEL_TX_COMMIT_CONTROL, GenericCapToken<TxCommitKind>, TxCommitKind>;
pub type TxAbortControl = Msg<LABEL_TX_ABORT_CONTROL, GenericCapToken<TxAbortKind>, TxAbortKind>;
pub type StateSnapshotControl =
    Msg<LABEL_STATE_SNAPSHOT_CONTROL, GenericCapToken<StateSnapshotKind>, StateSnapshotKind>;
pub type StateRestoreControl =
    Msg<LABEL_STATE_RESTORE_CONTROL, GenericCapToken<StateRestoreKind>, StateRestoreKind>;
fn encode_control_handle(handle: (u8, u64)) -> [u8; CAP_HANDLE_LEN] {
    let mut buf = [0u8; CAP_HANDLE_LEN];
    buf[0] = handle.0;
    buf[1..9].copy_from_slice(&handle.1.to_le_bytes());
    buf
}

fn encode_abort_control_handle(handle: AbortControlWireHandle) -> [u8; CAP_HANDLE_LEN] {
    let mut buf = [0u8; CAP_HANDLE_LEN];
    buf[0..4].copy_from_slice(&handle.0.to_le_bytes());
    buf[4..6].copy_from_slice(&handle.1.to_le_bytes());
    buf
}

fn decode_control_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<(u8, u64), CapError> {
    let mut scope_bytes = [0u8; 8];
    scope_bytes.copy_from_slice(&data[1..9]);
    Ok((data[0], u64::from_le_bytes(scope_bytes)))
}

fn decode_abort_control_handle(
    data: [u8; CAP_HANDLE_LEN],
) -> Result<AbortControlWireHandle, CapError> {
    if data[6..].iter().any(|byte| *byte != 0) {
        return Err(CapError::Mismatch);
    }
    Ok((
        u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        u16::from_le_bytes([data[4], data[5]]),
    ))
}
