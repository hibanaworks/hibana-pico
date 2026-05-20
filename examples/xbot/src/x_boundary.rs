use crate::audit::{AuditCode, AuditEvent, AuditLog};
use crate::protocol::{
    ApprovedXReply, AutoXPost, BoundedText, DraftObject, Hash, MAX_BODY_BYTES, ReplyId, TxId,
    UntrustedReplyObject, XPostCommitted, XPostId,
};

pub const DRAFT_ROOT_OBJECT: hibana_pico::appkit::ChoreoFsObject =
    hibana_pico::appkit::ChoreoFsObject::new(
        b"xbot/drafts",
        hibana_pico::appkit::ObjectId(10_000),
        hibana_pico::appkit::FdSpec::new(10, 0, 1),
    );

pub const COMMIT_LEDGER_OBJECT: hibana_pico::appkit::ChoreoFsObject =
    hibana_pico::appkit::ChoreoFsObject::new(
        b"xbot/commit-ledger",
        hibana_pico::appkit::ObjectId(10_001),
        hibana_pico::appkit::FdSpec::new(11, 0, 1),
    );

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XApiToken {
    fingerprint: Hash,
}

impl XApiToken {
    pub const fn proof_only(fingerprint: Hash) -> Self {
        Self { fingerprint }
    }

    pub const fn fingerprint(&self) -> Hash {
        self.fingerprint
    }
}

pub trait XApi {
    fn post_to_x(
        &mut self,
        token: &XApiToken,
        body: &BoundedText<MAX_BODY_BYTES>,
    ) -> Result<XPostId, XApiError>;

    fn reply_to_x(
        &mut self,
        token: &XApiToken,
        reply_id: ReplyId,
        body: &BoundedText<MAX_BODY_BYTES>,
    ) -> Result<XPostId, XApiError>;
}

pub struct XBoundary {
    token: XApiToken,
}

impl XBoundary {
    pub const fn new_for_proof(token: XApiToken) -> Self {
        Self { token }
    }

    pub fn post_auto<const D: usize, const L: usize, const A: usize>(
        &self,
        auto: AutoXPost,
        drafts: &DraftStore<D>,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
        api: &mut impl XApi,
    ) -> Result<PostOutcome, XBoundaryError> {
        self.post_auto_inner(auto, drafts, ledger, audit, api, false)
    }

    pub fn reply<const D: usize, const R: usize, const L: usize, const A: usize>(
        &self,
        approved: ApprovedXReply,
        drafts: &DraftStore<D>,
        replies: &ReplyStore<R>,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
        api: &mut impl XApi,
    ) -> Result<PostOutcome, XBoundaryError> {
        let reply = match replies.resolve(approved.reply_id) {
            Some(reply) => reply,
            None => return Err(XBoundaryError::MissingReply),
        };
        core::hint::black_box(reply.body_hash());
        let draft = match drafts.resolve(approved.object_id) {
            Some(draft) => draft,
            None => return Err(XBoundaryError::MissingDraft),
        };
        if draft.draft_hash() != approved.draft_hash || draft.body_hash() != approved.body_hash {
            ledger.record_rejected(approved.tx_id, approved.object_id, approved.body_hash)?;
            return Err(XBoundaryError::DraftHashMismatch);
        }
        match ledger.committed(approved.tx_id, approved.body_hash) {
            Some(committed) => {
                audit.push(AuditEvent {
                    tx_id: approved.tx_id,
                    code: AuditCode::DuplicateCommitted,
                    hash: approved.body_hash,
                    x_post_id: committed.x_post_id,
                })?;
                return Ok(PostOutcome::DuplicateCommitted(committed));
            }
            None => {}
        }
        ledger.ensure_approved(
            approved.tx_id,
            approved.object_id,
            approved.body_hash,
            approved.approval_hash,
        )?;
        let x_post_id = api.reply_to_x(&self.token, approved.reply_id, draft.body())?;
        let committed = ledger.record_committed(
            approved.tx_id,
            approved.object_id,
            approved.body_hash,
            x_post_id,
        )?;
        audit.push(AuditEvent {
            tx_id: approved.tx_id,
            code: AuditCode::Committed,
            hash: approved.body_hash,
            x_post_id,
        })?;
        Ok(PostOutcome::Committed(committed))
    }

    pub fn post_with_lost_local_ack<const D: usize, const L: usize, const A: usize>(
        &self,
        auto: AutoXPost,
        drafts: &DraftStore<D>,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
        api: &mut impl XApi,
    ) -> Result<PostOutcome, XBoundaryError> {
        self.post_auto_inner(auto, drafts, ledger, audit, api, true)
    }

    pub fn retry_after_lost_local_ack<const L: usize, const A: usize>(
        &self,
        tx_id: TxId,
        body_hash: Hash,
        ledger: &CommitLedger<L>,
        audit: &mut AuditLog<A>,
    ) -> Result<PostOutcome, XBoundaryError> {
        match ledger.committed(tx_id, body_hash) {
            Some(committed) => {
                audit.push(AuditEvent {
                    tx_id,
                    code: AuditCode::DuplicateCommitted,
                    hash: body_hash,
                    x_post_id: committed.x_post_id,
                })?;
                Ok(PostOutcome::DuplicateCommitted(committed))
            }
            None => Err(XBoundaryError::Ledger(LedgerError::MissingTx)),
        }
    }

    fn post_auto_inner<const D: usize, const L: usize, const A: usize>(
        &self,
        auto: AutoXPost,
        drafts: &DraftStore<D>,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
        api: &mut impl XApi,
        lose_local_ack: bool,
    ) -> Result<PostOutcome, XBoundaryError> {
        let draft = match drafts.resolve(auto.object_id) {
            Some(draft) => draft,
            None => return Err(XBoundaryError::MissingDraft),
        };
        if draft.draft_hash() != auto.draft_hash || draft.body_hash() != auto.body_hash {
            ledger.record_rejected(auto.tx_id, auto.object_id, auto.body_hash)?;
            return Err(XBoundaryError::DraftHashMismatch);
        }
        match ledger.committed(auto.tx_id, auto.body_hash) {
            Some(committed) => {
                audit.push(AuditEvent {
                    tx_id: auto.tx_id,
                    code: AuditCode::DuplicateCommitted,
                    hash: auto.body_hash,
                    x_post_id: committed.x_post_id,
                })?;
                return Ok(PostOutcome::DuplicateCommitted(committed));
            }
            None => {}
        }
        ledger.record_pending(auto.tx_id, auto.object_id, auto.body_hash)?;
        let x_post_id = api.post_to_x(&self.token, draft.body())?;
        let committed =
            ledger.record_auto_committed(auto.tx_id, auto.object_id, auto.body_hash, x_post_id)?;
        audit.push(AuditEvent {
            tx_id: auto.tx_id,
            code: AuditCode::Committed,
            hash: auto.body_hash,
            x_post_id,
        })?;
        if lose_local_ack {
            return Err(XBoundaryError::LocalAckLost(committed));
        }
        Ok(PostOutcome::Committed(committed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftStore<const N: usize> {
    entries: [DraftSlot; N],
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyStore<const N: usize> {
    entries: [ReplySlot; N],
    len: usize,
}

impl<const N: usize> ReplyStore<N> {
    pub const fn empty() -> Self {
        Self {
            entries: [ReplySlot::EMPTY; N],
            len: 0,
        }
    }

    pub fn ingest(&mut self, reply: UntrustedReplyObject) -> Result<(), ReplyStoreError> {
        if self.resolve(reply.reply_id()).is_some() {
            return Err(ReplyStoreError::DuplicateReply);
        }
        if self.len == N {
            return Err(ReplyStoreError::Full);
        }
        self.entries[self.len] = ReplySlot {
            occupied: true,
            reply,
        };
        self.len += 1;
        Ok(())
    }

    pub fn resolve(&self, reply_id: ReplyId) -> Option<&UntrustedReplyObject> {
        let mut index = 0usize;
        while index < self.len {
            let slot = self.entries[index];
            if slot.occupied && slot.reply.reply_id() == reply_id {
                return Some(&self.entries[index].reply);
            }
            index += 1;
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplySlot {
    occupied: bool,
    reply: UntrustedReplyObject,
}

impl ReplySlot {
    const EMPTY: Self = Self {
        occupied: false,
        reply: UntrustedReplyObject::empty(),
    };
}

impl<const N: usize> DraftStore<N> {
    pub const fn empty() -> Self {
        Self {
            entries: [DraftSlot::EMPTY; N],
            len: 0,
        }
    }

    pub fn insert(&mut self, draft: DraftObject) -> Result<(), DraftStoreError> {
        if self.resolve(draft.object_id()).is_some() {
            return Err(DraftStoreError::DuplicateObject);
        }
        if self.len == N {
            return Err(DraftStoreError::Full);
        }
        self.entries[self.len] = DraftSlot {
            occupied: true,
            draft,
        };
        self.len += 1;
        Ok(())
    }

    pub fn resolve(&self, object_id: hibana_pico::appkit::ObjectId) -> Option<&DraftObject> {
        let mut index = 0usize;
        while index < self.len {
            let slot = self.entries[index];
            if slot.occupied && slot.draft.object_id() == object_id {
                return Some(&self.entries[index].draft);
            }
            index += 1;
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DraftSlot {
    occupied: bool,
    draft: DraftObject,
}

impl DraftSlot {
    const EMPTY: Self = Self {
        occupied: false,
        draft: DraftObject::empty(),
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitLedger<const N: usize> {
    entries: [LedgerEntry; N],
    len: usize,
}

impl<const N: usize> CommitLedger<N> {
    pub const fn empty() -> Self {
        Self {
            entries: [LedgerEntry::EMPTY; N],
            len: 0,
        }
    }

    pub fn record_pending(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index_or_insert(tx_id)?;
        let entry = self.entries[index];
        match entry.state {
            LedgerState::Unseen => {
                self.entries[index].object_id = object_id;
                self.entries[index].body_hash = body_hash;
                self.entries[index].state = LedgerState::Pending;
                Ok(())
            }
            LedgerState::Pending | LedgerState::ApprovalRequested => {
                if entry.object_id == object_id && entry.body_hash == body_hash {
                    Ok(())
                } else {
                    Err(LedgerError::BodyChanged)
                }
            }
            LedgerState::Approved
            | LedgerState::InputAdmitted
            | LedgerState::Committed
            | LedgerState::Rejected
            | LedgerState::TerminalFault => Err(LedgerError::TerminalState),
        }
    }

    pub fn record_approval_requested_for_reply_action(
        &mut self,
        request: crate::protocol::ReplyApprovalRequest,
        nonce: crate::protocol::Nonce,
    ) -> Result<(), LedgerError> {
        self.record_pending(request.tx_id, request.object_id, request.body_hash)?;
        let index = self.index(request.tx_id).ok_or(LedgerError::MissingTx)?;
        self.entries[index].draft_hash = request.draft_hash;
        self.entries[index].nonce = nonce;
        self.entries[index].state = LedgerState::ApprovalRequested;
        Ok(())
    }

    pub fn record_approval_requested_for_reply_input(
        &mut self,
        request: crate::protocol::ReplyInputRequest,
        nonce: crate::protocol::Nonce,
    ) -> Result<(), LedgerError> {
        self.record_pending(request.tx_id, request.object_id, request.body_hash)?;
        let index = self.index(request.tx_id).ok_or(LedgerError::MissingTx)?;
        self.entries[index].nonce = nonce;
        self.entries[index].state = LedgerState::ApprovalRequested;
        Ok(())
    }

    pub fn record_input_admitted(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        approval_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index(tx_id).ok_or(LedgerError::MissingTx)?;
        let entry = self.entries[index];
        if entry.object_id != object_id || entry.body_hash != body_hash {
            return Err(LedgerError::BodyChanged);
        }
        match entry.state {
            LedgerState::ApprovalRequested | LedgerState::InputAdmitted => {
                self.entries[index].approval_hash = approval_hash;
                self.entries[index].state = LedgerState::InputAdmitted;
                Ok(())
            }
            _ => Err(LedgerError::WrongState),
        }
    }

    pub fn record_approved(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        approval_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index(tx_id).ok_or(LedgerError::MissingTx)?;
        let entry = self.entries[index];
        if entry.object_id != object_id || entry.body_hash != body_hash {
            return Err(LedgerError::BodyChanged);
        }
        match entry.state {
            LedgerState::ApprovalRequested | LedgerState::Approved => {
                self.entries[index].approval_hash = approval_hash;
                self.entries[index].state = LedgerState::Approved;
                Ok(())
            }
            _ => Err(LedgerError::WrongState),
        }
    }

    pub fn ensure_approved(
        &self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        approval_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index(tx_id).ok_or(LedgerError::MissingTx)?;
        let entry = self.entries[index];
        if entry.object_id != object_id || entry.body_hash != body_hash {
            return Err(LedgerError::BodyChanged);
        }
        if entry.approval_hash != approval_hash {
            return Err(LedgerError::ApprovalMismatch);
        }
        match entry.state {
            LedgerState::Approved | LedgerState::Committed => Ok(()),
            _ => Err(LedgerError::WrongState),
        }
    }

    pub fn record_committed(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        x_post_id: XPostId,
    ) -> Result<XPostCommitted, LedgerError> {
        let index = self.index(tx_id).ok_or(LedgerError::MissingTx)?;
        let entry = self.entries[index];
        if entry.object_id != object_id || entry.body_hash != body_hash {
            return Err(LedgerError::BodyChanged);
        }
        match entry.state {
            LedgerState::Approved | LedgerState::Committed => {
                if entry.state == LedgerState::Committed {
                    return Ok(XPostCommitted {
                        tx_id,
                        x_post_id: entry.x_post_id,
                        body_hash,
                    });
                }
                self.entries[index].x_post_id = x_post_id;
                self.entries[index].state = LedgerState::Committed;
                Ok(XPostCommitted {
                    tx_id,
                    x_post_id,
                    body_hash,
                })
            }
            _ => Err(LedgerError::WrongState),
        }
    }

    pub fn record_auto_committed(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        body_hash: Hash,
        x_post_id: XPostId,
    ) -> Result<XPostCommitted, LedgerError> {
        let index = self.index(tx_id).ok_or(LedgerError::MissingTx)?;
        let entry = self.entries[index];
        if entry.object_id != object_id || entry.body_hash != body_hash {
            return Err(LedgerError::BodyChanged);
        }
        match entry.state {
            LedgerState::Pending | LedgerState::Committed => {
                if entry.state == LedgerState::Committed {
                    return Ok(XPostCommitted {
                        tx_id,
                        x_post_id: entry.x_post_id,
                        body_hash,
                    });
                }
                self.entries[index].x_post_id = x_post_id;
                self.entries[index].state = LedgerState::Committed;
                Ok(XPostCommitted {
                    tx_id,
                    x_post_id,
                    body_hash,
                })
            }
            _ => Err(LedgerError::WrongState),
        }
    }

    pub fn record_rejected(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        reason_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index_or_insert(tx_id)?;
        self.entries[index].object_id = object_id;
        self.entries[index].reason_hash = reason_hash;
        self.entries[index].state = LedgerState::Rejected;
        Ok(())
    }

    pub fn record_terminal_fault(
        &mut self,
        tx_id: TxId,
        object_id: hibana_pico::appkit::ObjectId,
        reason_hash: Hash,
    ) -> Result<(), LedgerError> {
        let index = self.index_or_insert(tx_id)?;
        self.entries[index].object_id = object_id;
        self.entries[index].reason_hash = reason_hash;
        self.entries[index].state = LedgerState::TerminalFault;
        Ok(())
    }

    pub fn committed(&self, tx_id: TxId, body_hash: Hash) -> Option<XPostCommitted> {
        let index = self.index(tx_id)?;
        let entry = self.entries[index];
        if entry.state == LedgerState::Committed && entry.body_hash == body_hash {
            return Some(XPostCommitted {
                tx_id,
                x_post_id: entry.x_post_id,
                body_hash,
            });
        }
        None
    }

    pub fn state(&self, tx_id: TxId) -> Option<LedgerState> {
        let index = self.index(tx_id)?;
        Some(self.entries[index].state)
    }

    fn index(&self, tx_id: TxId) -> Option<usize> {
        let mut index = 0usize;
        while index < self.len {
            if self.entries[index].tx_id == tx_id {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn index_or_insert(&mut self, tx_id: TxId) -> Result<usize, LedgerError> {
        match self.index(tx_id) {
            Some(index) => Ok(index),
            None => {
                if self.len == N {
                    return Err(LedgerError::Full);
                }
                let index = self.len;
                self.entries[index] = LedgerEntry {
                    tx_id,
                    ..LedgerEntry::EMPTY
                };
                self.len += 1;
                Ok(index)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LedgerState {
    #[default]
    Unseen,
    Pending,
    ApprovalRequested,
    InputAdmitted,
    Approved,
    Committed,
    Rejected,
    TerminalFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LedgerEntry {
    tx_id: TxId,
    object_id: hibana_pico::appkit::ObjectId,
    draft_hash: Hash,
    body_hash: Hash,
    approval_hash: Hash,
    reason_hash: Hash,
    nonce: crate::protocol::Nonce,
    x_post_id: XPostId,
    state: LedgerState,
}

impl LedgerEntry {
    const EMPTY: Self = Self {
        tx_id: TxId(0),
        object_id: hibana_pico::appkit::ObjectId(0),
        draft_hash: Hash(0),
        body_hash: Hash(0),
        approval_hash: Hash(0),
        reason_hash: Hash(0),
        nonce: crate::protocol::Nonce(0),
        x_post_id: XPostId(0),
        state: LedgerState::Unseen,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    Committed(XPostCommitted),
    DuplicateCommitted(XPostCommitted),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XBoundaryError {
    MissingDraft,
    MissingReply,
    DraftHashMismatch,
    Ledger(LedgerError),
    XApi(XApiError),
    Audit(crate::audit::AuditError),
    LocalAckLost(XPostCommitted),
}

impl From<LedgerError> for XBoundaryError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

impl From<XApiError> for XBoundaryError {
    fn from(error: XApiError) -> Self {
        Self::XApi(error)
    }
}

impl From<crate::audit::AuditError> for XBoundaryError {
    fn from(error: crate::audit::AuditError) -> Self {
        Self::Audit(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XApiError {
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerError {
    Full,
    MissingTx,
    WrongState,
    BodyChanged,
    ApprovalMismatch,
    TerminalState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftStoreError {
    DuplicateObject,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyStoreError {
    DuplicateReply,
    Full,
}
