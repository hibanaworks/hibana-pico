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

    wire_payload_via_decode!();

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

pub type EngineAbortBeginControl = Msg<LABEL_ENGINE_ABORT_BEGIN_CONTROL, ()>;
pub type EngineAbortMsg = Msg<LABEL_ENGINE_ABORT_REASON, EngineAbort>;
pub type EngineAbortFenceControl = Msg<LABEL_ENGINE_ABORT_FENCE_CONTROL, ()>;
pub type EngineAbortAckControl = Msg<LABEL_ENGINE_ABORT_ACK_CONTROL, ()>;
pub type TopologyBeginControl = Msg<LABEL_TOPOLOGY_BEGIN_CONTROL, ()>;
pub type TopologyAckControl = Msg<LABEL_TOPOLOGY_ACK_CONTROL, ()>;
pub type TopologyCommitControl = Msg<LABEL_TOPOLOGY_COMMIT_CONTROL, ()>;
pub type TxCommitControl = Msg<LABEL_TX_COMMIT_CONTROL, ()>;
pub type TxAbortControl = Msg<LABEL_TX_ABORT_CONTROL, ()>;
pub type StateSnapshotControl = Msg<LABEL_STATE_SNAPSHOT_CONTROL, ()>;
pub type StateRestoreControl = Msg<LABEL_STATE_RESTORE_CONTROL, ()>;
