use crate::audit::{AuditCode, AuditEvent, AuditLog};
use crate::protocol::{
    AdmittedReplyInput, ApprovalAction, ApprovalDeviceIdentity, ApprovedReplyDraft, Generation,
    Hash, HumanApprovalRequest, HumanApprovalResponse, InputAdmitPermit, Nonce, RejectedDraft,
    ReplyApprovalRequest, ReplyCommitPermit, ReplyInputRequest, SafeStop, TxId, hash_pair,
};
use crate::x_boundary::{CommitLedger, LedgerError};

pub const APNS_APPROVAL_CATEGORY: &[u8] = b"hibana-xbot-approval";
pub const APNS_APPROVE_ACTION: &[u8] = b"approve";
pub const APNS_REJECT_ACTION: &[u8] = b"reject";
pub const APNS_FENCE_ACTION: &[u8] = b"fence";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApnsProviderCredential {
    fingerprint: Hash,
}

impl ApnsProviderCredential {
    pub const fn proof_only(fingerprint: Hash) -> Self {
        Self { fingerprint }
    }

    pub const fn fingerprint(&self) -> Hash {
        self.fingerprint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApnsDeviceToken {
    fingerprint: Hash,
}

impl ApnsDeviceToken {
    pub const fn proof_only(fingerprint: Hash) -> Self {
        Self { fingerprint }
    }

    pub const fn fingerprint(&self) -> Hash {
        self.fingerprint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalBoundary {
    device: ApprovalDeviceIdentity,
}

impl ApprovalBoundary {
    pub const fn new(device: ApprovalDeviceIdentity) -> Self {
        Self { device }
    }

    pub const fn human_reply_approval_request(
        &self,
        request: ReplyApprovalRequest,
        nonce: Nonce,
    ) -> HumanApprovalRequest {
        core::hint::black_box(self.device);
        HumanApprovalRequest {
            tx_id: request.tx_id,
            generation: request.generation,
            object_id: request.object_id,
            draft_hash: request.draft_hash,
            body_hash: request.body_hash,
            nonce,
        }
    }

    pub const fn human_reply_input_request(
        &self,
        request: ReplyInputRequest,
        nonce: Nonce,
    ) -> HumanApprovalRequest {
        core::hint::black_box(self.device);
        HumanApprovalRequest {
            tx_id: request.tx_id,
            generation: request.generation,
            object_id: request.object_id,
            draft_hash: request.body_hash,
            body_hash: request.body_hash,
            nonce,
        }
    }

    pub fn decide_reply<const L: usize, const A: usize>(
        &self,
        request: ReplyApprovalRequest,
        response: HumanApprovalResponse,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
    ) -> Result<ReplyApprovalDecision, ApprovalError> {
        self.validate_reply_approval_response(request, response)?;
        ledger.record_approval_requested_for_reply_action(request, response.nonce)?;
        audit.push(AuditEvent {
            tx_id: request.tx_id,
            code: AuditCode::ApprovalRequested,
            hash: request.body_hash,
            x_post_id: crate::protocol::XPostId(0),
        })?;
        match response.action {
            ApprovalAction::Approve => {
                let approval_hash = approval_hash(response);
                let permit = ReplyCommitPermit::new(
                    request.generation,
                    request.tx_id,
                    request.reply_id,
                    request.object_id,
                    request.draft_hash,
                    request.body_hash,
                    approval_hash,
                );
                let approved = ApprovedReplyDraft::new(
                    request.tx_id,
                    request.generation,
                    request.reply_id,
                    request.object_id,
                    request.draft_hash,
                    request.body_hash,
                    approval_hash,
                    permit,
                );
                ledger.record_approved(
                    approved.tx_id,
                    approved.object_id,
                    approved.body_hash,
                    approved.approval_hash,
                )?;
                audit.push(AuditEvent {
                    tx_id: approved.tx_id,
                    code: AuditCode::Approved,
                    hash: approved.body_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyApprovalDecision::Approve(approved))
            }
            ApprovalAction::Reject => {
                let rejected = RejectedDraft {
                    tx_id: request.tx_id,
                    object_id: request.object_id,
                    reason_hash: response.reason_hash,
                };
                ledger.record_rejected(rejected.tx_id, rejected.object_id, rejected.reason_hash)?;
                audit.push(AuditEvent {
                    tx_id: rejected.tx_id,
                    code: AuditCode::Rejected,
                    hash: rejected.reason_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyApprovalDecision::Reject(rejected))
            }
            ApprovalAction::Fence => {
                let safe_stop = SafeStop {
                    tx_id: request.tx_id,
                    object_id: request.object_id,
                    reason_hash: response.reason_hash,
                };
                ledger.record_terminal_fault(
                    safe_stop.tx_id,
                    safe_stop.object_id,
                    safe_stop.reason_hash,
                )?;
                audit.push(AuditEvent {
                    tx_id: safe_stop.tx_id,
                    code: AuditCode::Fenced,
                    hash: safe_stop.reason_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyApprovalDecision::Fence(safe_stop))
            }
        }
    }

    fn validate_reply_approval_response(
        &self,
        request: ReplyApprovalRequest,
        response: HumanApprovalResponse,
    ) -> Result<(), ApprovalError> {
        if response.tx_id != request.tx_id {
            return Err(ApprovalError::WrongTx);
        }
        if response.generation != request.generation {
            return Err(ApprovalError::WrongGeneration);
        }
        if response.object_id != request.object_id {
            return Err(ApprovalError::WrongObject);
        }
        if response.body_hash != request.body_hash {
            return Err(ApprovalError::WrongBodyHash);
        }
        if response.nonce == Nonce(0) {
            return Err(ApprovalError::StaleNonce);
        }
        if response.device != self.device {
            return Err(ApprovalError::WrongDevice);
        }
        Ok(())
    }

    pub fn decide_reply_input<const L: usize, const A: usize>(
        &self,
        request: ReplyInputRequest,
        response: HumanApprovalResponse,
        ledger: &mut CommitLedger<L>,
        audit: &mut AuditLog<A>,
    ) -> Result<ReplyInputDecision, ApprovalError> {
        self.validate_reply_input_response(request, response)?;
        ledger.record_approval_requested_for_reply_input(request, response.nonce)?;
        audit.push(AuditEvent {
            tx_id: request.tx_id,
            code: AuditCode::ApprovalRequested,
            hash: request.body_hash,
            x_post_id: crate::protocol::XPostId(0),
        })?;
        match response.action {
            ApprovalAction::Approve => {
                let approval_hash = approval_hash(response);
                let permit = InputAdmitPermit::new(
                    request.generation,
                    request.tx_id,
                    request.reply_id,
                    request.object_id,
                    request.body_hash,
                    approval_hash,
                );
                let admitted = AdmittedReplyInput::new(
                    request.tx_id,
                    request.generation,
                    request.reply_id,
                    request.object_id,
                    request.body_hash,
                    approval_hash,
                    permit,
                );
                ledger.record_input_admitted(
                    request.tx_id,
                    request.object_id,
                    request.body_hash,
                    approval_hash,
                )?;
                audit.push(AuditEvent {
                    tx_id: request.tx_id,
                    code: AuditCode::ReplyInputAdmitted,
                    hash: request.body_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyInputDecision::Admit(admitted))
            }
            ApprovalAction::Reject => {
                let rejected = RejectedDraft {
                    tx_id: request.tx_id,
                    object_id: request.object_id,
                    reason_hash: response.reason_hash,
                };
                ledger.record_rejected(rejected.tx_id, rejected.object_id, rejected.reason_hash)?;
                audit.push(AuditEvent {
                    tx_id: rejected.tx_id,
                    code: AuditCode::Rejected,
                    hash: rejected.reason_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyInputDecision::Reject(rejected))
            }
            ApprovalAction::Fence => {
                let safe_stop = SafeStop {
                    tx_id: request.tx_id,
                    object_id: request.object_id,
                    reason_hash: response.reason_hash,
                };
                ledger.record_terminal_fault(
                    safe_stop.tx_id,
                    safe_stop.object_id,
                    safe_stop.reason_hash,
                )?;
                audit.push(AuditEvent {
                    tx_id: safe_stop.tx_id,
                    code: AuditCode::Fenced,
                    hash: safe_stop.reason_hash,
                    x_post_id: crate::protocol::XPostId(0),
                })?;
                Ok(ReplyInputDecision::Fence(safe_stop))
            }
        }
    }

    fn validate_reply_input_response(
        &self,
        request: ReplyInputRequest,
        response: HumanApprovalResponse,
    ) -> Result<(), ApprovalError> {
        if response.tx_id != request.tx_id {
            return Err(ApprovalError::WrongTx);
        }
        if response.generation != request.generation {
            return Err(ApprovalError::WrongGeneration);
        }
        if response.object_id != request.object_id {
            return Err(ApprovalError::WrongObject);
        }
        if response.body_hash != request.body_hash {
            return Err(ApprovalError::WrongBodyHash);
        }
        if response.nonce == Nonce(0) {
            return Err(ApprovalError::StaleNonce);
        }
        if response.device != self.device {
            return Err(ApprovalError::WrongDevice);
        }
        Ok(())
    }
}

pub fn approval_response(
    request: HumanApprovalRequest,
    device: ApprovalDeviceIdentity,
    action: ApprovalAction,
    reason_hash: Hash,
) -> HumanApprovalResponse {
    HumanApprovalResponse {
        tx_id: request.tx_id,
        generation: request.generation,
        object_id: request.object_id,
        body_hash: request.body_hash,
        nonce: request.nonce,
        device,
        action,
        reason_hash,
    }
}

pub fn stale_approval_response(
    tx_id: TxId,
    generation: Generation,
    object_id: hibana_pico::appkit::ObjectId,
    body_hash: Hash,
    device: ApprovalDeviceIdentity,
    action: ApprovalAction,
    reason_hash: Hash,
) -> HumanApprovalResponse {
    HumanApprovalResponse {
        tx_id,
        generation,
        object_id,
        body_hash,
        nonce: Nonce(0),
        device,
        action,
        reason_hash,
    }
}

pub fn approval_hash(response: HumanApprovalResponse) -> Hash {
    hash_pair(
        response.body_hash,
        Hash(response.nonce.0 ^ response.device.0 ^ response.reason_hash.0),
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplyApprovalDecision {
    Approve(ApprovedReplyDraft),
    Reject(RejectedDraft),
    Fence(SafeStop),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplyInputDecision {
    Admit(AdmittedReplyInput),
    Reject(RejectedDraft),
    Fence(SafeStop),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    WrongTx,
    WrongGeneration,
    WrongObject,
    WrongBodyHash,
    StaleNonce,
    WrongDevice,
    Ledger(LedgerError),
    Audit(crate::audit::AuditError),
}

impl From<LedgerError> for ApprovalError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

impl From<crate::audit::AuditError> for ApprovalError {
    fn from(error: crate::audit::AuditError) -> Self {
        Self::Audit(error)
    }
}
