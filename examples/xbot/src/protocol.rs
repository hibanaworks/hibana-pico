use core::marker::PhantomData;

use hibana::integration::wire::{CodecError, Payload, WireEncode, WirePayload};

pub const LABEL_REPLY_APPROVAL_REQUEST: u8 = 200;
pub const LABEL_HUMAN_APPROVAL_REQUEST: u8 = 201;
pub const LABEL_HUMAN_APPROVAL_RESPONSE: u8 = 202;
pub const LABEL_AUTO_X_POST: u8 = 203;
pub const LABEL_UNTRUSTED_REPLY: u8 = 204;
pub const LABEL_APPROVE_ROUTE: u8 = 205;
pub const LABEL_REJECT_ROUTE: u8 = 206;
pub const LABEL_FENCE_ROUTE: u8 = 207;
pub const LABEL_NOT_APPROVED_ROUTE: u8 = 208;
pub const LABEL_APPROVED_REPLY_DRAFT: u8 = 209;
pub const LABEL_CODEX_REPLY_REQUEST: u8 = 210;
pub const LABEL_X_POST_COMMITTED: u8 = 211;
pub const LABEL_REPLY_INPUT_REQUEST: u8 = 212;
pub const LABEL_REPLY_INPUT_ADMIT_ROUTE: u8 = 213;
pub const LABEL_REPLY_INPUT_ADMITTED: u8 = 214;
pub const LABEL_REPLY_DRAFT_PROPOSAL: u8 = 215;
pub const LABEL_APPROVED_X_REPLY: u8 = 216;
pub const LABEL_X_REPLY_COMMITTED: u8 = 217;
pub const LABEL_CODEX_REPLY_PROPOSAL: u8 = 218;
pub const LABEL_REJECTED_DRAFT: u8 = 219;
pub const LABEL_SAFE_STOP: u8 = 220;

pub const ROLE_WASI_AGENT: u8 = 0;
pub const ROLE_DRIVER: u8 = 1;
pub const ROLE_APPROVAL_BOUNDARY: u8 = 2;
pub const ROLE_HUMAN_APPROVAL_DEVICE: u8 = 3;
pub const ROLE_X_BOUNDARY: u8 = 4;
pub const ROLE_AUDIT: u8 = 5;
pub const ROLE_LLM_BOUNDARY: u8 = 6;

pub const MAX_BODY_BYTES: usize = 280;
pub const MAX_REASON_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hash(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nonce(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XPostId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplyId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApprovalDeviceIdentity(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct One;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalAction {
    Approve,
    Reject,
    Fence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundedText<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> BoundedText<N> {
    pub const EMPTY: Self = Self {
        bytes: [0; N],
        len: 0,
    };

    pub fn new(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > N {
            return Err(ProtocolError::TextTooLong);
        }
        let mut out = [0; N];
        let mut idx = 0usize;
        while idx < bytes.len() {
            out[idx] = bytes[idx];
            idx += 1;
        }
        Ok(Self {
            bytes: out,
            len: bytes.len(),
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }

    pub fn hash(&self) -> Hash {
        hash_bytes(self.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexProposalObject {
    body: BoundedText<MAX_BODY_BYTES>,
}

impl CodexProposalObject {
    pub fn new(bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            body: BoundedText::new(bytes)?,
        })
    }

    pub const fn body(&self) -> &BoundedText<MAX_BODY_BYTES> {
        &self.body
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.body.as_bytes()
    }
}

impl WireEncode for CodexProposalObject {
    fn encoded_len(&self) -> Option<usize> {
        Some(2 + self.body.as_bytes().len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let bytes = self.body.as_bytes();
        if bytes.len() > u16::MAX as usize {
            return Err(CodecError::Invalid("codex proposal too large"));
        }
        let len = bytes.len();
        if out.len() < 2 + len {
            return Err(CodecError::Truncated);
        }
        out[..2].copy_from_slice(&(len as u16).to_be_bytes());
        out[2..2 + len].copy_from_slice(bytes);
        Ok(2 + len)
    }
}

impl WirePayload for CodexProposalObject {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 2 {
            return Err(CodecError::Truncated);
        }
        let len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if len > MAX_BODY_BYTES {
            return Err(CodecError::Invalid(
                "codex proposal length exceeds capacity",
            ));
        }
        if bytes.len() != 2 + len {
            return Err(CodecError::Invalid("codex proposal length mismatch"));
        }
        match Self::new(&bytes[2..]) {
            Ok(value) => Ok(value),
            Err(_) => Err(CodecError::Invalid("invalid codex proposal")),
        }
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.len() < 2 {
            return Err(CodecError::Truncated);
        }
        scratch[0] = 0;
        scratch[1] = 0;
        Ok(Payload::new(&scratch[..2]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftObject {
    object_id: hibana_pico::appkit::ObjectId,
    draft_hash: Hash,
    body_hash: Hash,
    body: BoundedText<MAX_BODY_BYTES>,
}

impl DraftObject {
    pub const fn empty() -> Self {
        Self {
            object_id: hibana_pico::appkit::ObjectId(0),
            draft_hash: Hash(0),
            body_hash: Hash(0),
            body: BoundedText::EMPTY,
        }
    }

    pub fn new(
        object_id: hibana_pico::appkit::ObjectId,
        body: BoundedText<MAX_BODY_BYTES>,
    ) -> Self {
        let body_hash = body.hash();
        let draft_hash = hash_pair(Hash(object_id.0 as u64), body_hash);
        Self {
            object_id,
            draft_hash,
            body_hash,
            body,
        }
    }

    pub const fn object_id(&self) -> hibana_pico::appkit::ObjectId {
        self.object_id
    }

    pub const fn draft_hash(&self) -> Hash {
        self.draft_hash
    }

    pub const fn body_hash(&self) -> Hash {
        self.body_hash
    }

    pub const fn body(&self) -> &BoundedText<MAX_BODY_BYTES> {
        &self.body
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyDraftProposal {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub risk_hint: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyApprovalRequest {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub summary_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyInputRequest {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub body_hash: Hash,
    pub reason_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanApprovalRequest {
    pub tx_id: TxId,
    pub generation: Generation,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub nonce: Nonce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanApprovalResponse {
    pub tx_id: TxId,
    pub generation: Generation,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub body_hash: Hash,
    pub nonce: Nonce,
    pub device: ApprovalDeviceIdentity,
    pub action: ApprovalAction,
    pub reason_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UntrustedReplyObject {
    reply_id: ReplyId,
    object_id: hibana_pico::appkit::ObjectId,
    body_hash: Hash,
    author_hash: Hash,
    body: BoundedText<MAX_BODY_BYTES>,
}

impl UntrustedReplyObject {
    pub const fn empty() -> Self {
        Self {
            reply_id: ReplyId(0),
            object_id: hibana_pico::appkit::ObjectId(0),
            body_hash: Hash(0),
            author_hash: Hash(0),
            body: BoundedText::EMPTY,
        }
    }

    pub fn new(
        reply_id: ReplyId,
        object_id: hibana_pico::appkit::ObjectId,
        author_hash: Hash,
        body: BoundedText<MAX_BODY_BYTES>,
    ) -> Self {
        Self {
            reply_id,
            object_id,
            body_hash: body.hash(),
            author_hash,
            body,
        }
    }

    pub const fn reply_id(&self) -> ReplyId {
        self.reply_id
    }

    pub const fn object_id(&self) -> hibana_pico::appkit::ObjectId {
        self.object_id
    }

    pub const fn body_hash(&self) -> Hash {
        self.body_hash
    }

    pub const fn author_hash(&self) -> Hash {
        self.author_hash
    }

    pub const fn body(&self) -> &BoundedText<MAX_BODY_BYTES> {
        &self.body
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AutoPostPermit<Shot> {
    pub generation: Generation,
    pub tx_id: TxId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    shot: PhantomData<fn() -> Shot>,
}

impl AutoPostPermit<One> {
    pub(crate) fn new(
        generation: Generation,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        draft_hash: Hash,
        body_hash: Hash,
    ) -> Self {
        Self {
            generation,
            tx_id,
            object_id,
            draft_hash,
            body_hash,
            shot: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InputAdmitPermit<Shot> {
    pub generation: Generation,
    pub tx_id: TxId,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub body_hash: Hash,
    pub approval_hash: Hash,
    shot: PhantomData<fn() -> Shot>,
}

impl InputAdmitPermit<One> {
    pub(crate) fn new(
        generation: Generation,
        tx_id: TxId,
        reply_id: ReplyId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        approval_hash: Hash,
    ) -> Self {
        Self {
            generation,
            tx_id,
            reply_id,
            object_id,
            body_hash,
            approval_hash,
            shot: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReplyCommitPermit<Shot> {
    pub generation: Generation,
    pub tx_id: TxId,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub approval_hash: Hash,
    shot: PhantomData<fn() -> Shot>,
}

impl ReplyCommitPermit<One> {
    pub(crate) fn new(
        generation: Generation,
        tx_id: TxId,
        reply_id: ReplyId,
        object_id: hibana_pico::appkit::ObjectId,
        draft_hash: Hash,
        body_hash: Hash,
        approval_hash: Hash,
    ) -> Self {
        Self {
            generation,
            tx_id,
            reply_id,
            object_id,
            draft_hash,
            body_hash,
            approval_hash,
            shot: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AutoXPost {
    pub tx_id: TxId,
    pub generation: Generation,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub permit: AutoPostPermit<One>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ApprovedReplyDraft {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub approval_hash: Hash,
    pub permit: ReplyCommitPermit<One>,
}

impl ApprovedReplyDraft {
    pub(crate) fn new(
        tx_id: TxId,
        generation: Generation,
        reply_id: ReplyId,
        object_id: hibana_pico::appkit::ObjectId,
        draft_hash: Hash,
        body_hash: Hash,
        approval_hash: Hash,
        permit: ReplyCommitPermit<One>,
    ) -> Self {
        Self {
            tx_id,
            generation,
            reply_id,
            object_id,
            draft_hash,
            body_hash,
            approval_hash,
            permit,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedReplyInput {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub body_hash: Hash,
    pub approval_hash: Hash,
    pub permit: InputAdmitPermit<One>,
}

impl AdmittedReplyInput {
    pub(crate) fn new(
        tx_id: TxId,
        generation: Generation,
        reply_id: ReplyId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        approval_hash: Hash,
        permit: InputAdmitPermit<One>,
    ) -> Self {
        Self {
            tx_id,
            generation,
            reply_id,
            object_id,
            body_hash,
            approval_hash,
            permit,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ApprovedXReply {
    pub tx_id: TxId,
    pub generation: Generation,
    pub reply_id: ReplyId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub draft_hash: Hash,
    pub body_hash: Hash,
    pub approval_hash: Hash,
    pub permit: ReplyCommitPermit<One>,
}

impl ApprovedXReply {
    pub(crate) fn new(approved: ApprovedReplyDraft) -> Self {
        Self {
            tx_id: approved.tx_id,
            generation: approved.generation,
            reply_id: approved.reply_id,
            object_id: approved.object_id,
            draft_hash: approved.draft_hash,
            body_hash: approved.body_hash,
            approval_hash: approved.approval_hash,
            permit: approved.permit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XPostCommitted {
    pub tx_id: TxId,
    pub x_post_id: XPostId,
    pub body_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RejectedDraft {
    pub tx_id: TxId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub reason_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeStop {
    pub tx_id: TxId,
    pub object_id: hibana_pico::appkit::ObjectId,
    pub reason_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    TextTooLong,
    ReplyNotAdmitted,
    ReplyApprovalMismatch,
}

pub const fn hash_bytes(bytes: &[u8]) -> Hash {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut idx = 0usize;
    while idx < bytes.len() {
        hash ^= bytes[idx] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        idx += 1;
    }
    Hash(hash)
}

pub const fn hash_pair(left: Hash, right: Hash) -> Hash {
    Hash(left.0.rotate_left(17) ^ right.0.rotate_right(11) ^ 0x9e37_79b9_7f4a_7c15)
}
