use crate::audit::{AuditCode, AuditEvent, AuditLog};
use crate::protocol::{
    ApprovalAction, ApprovalEvidence, ApprovalRequest, ApprovedIntent, DevicePublicKey, Hash,
    IntentCommitPermit, IntentRequest, Nonce, PicoNodError, displayed_hash, sign_approval,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    Nod(ApprovedIntent),
    Reject,
    Fence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalBoundary {
    device_key: DevicePublicKey,
}

impl ApprovalBoundary {
    pub const fn new(device_key: DevicePublicKey) -> Self {
        Self { device_key }
    }

    pub const fn request(&self, intent: IntentRequest, nonce: Nonce) -> ApprovalRequest {
        core::hint::black_box(self.device_key);
        ApprovalRequest {
            tx_id: intent.tx_id,
            generation: intent.generation,
            workspace_id: intent.workspace_id,
            object_id: intent.object_id,
            body_hash: intent.body_hash,
            summary_hash: intent.summary_hash,
            nonce,
        }
    }

    pub fn decide<const N: usize>(
        &self,
        request: ApprovalRequest,
        evidence: ApprovalEvidence,
        audit: &mut AuditLog<N>,
    ) -> Result<ApprovalDecision, PicoNodError> {
        if evidence.device_id != self.device_key.device_id {
            return Err(PicoNodError::SignatureMismatch);
        }
        if evidence.tx_id != request.tx_id
            || evidence.generation != request.generation
            || evidence.workspace_id != request.workspace_id
            || evidence.object_id != request.object_id
            || evidence.body_hash != request.body_hash
            || evidence.summary_hash != request.summary_hash
            || evidence.nonce != request.nonce
        {
            return Err(PicoNodError::ApprovalMismatch);
        }
        let displayed = displayed_hash(request);
        if evidence.displayed_hash != displayed {
            return Err(PicoNodError::DisplayMismatch);
        }
        let expected = sign_approval(
            self.device_key.verification_hash,
            request,
            evidence.action,
            displayed,
        );
        if evidence.signature != expected {
            return Err(PicoNodError::SignatureMismatch);
        }
        match evidence.action {
            ApprovalAction::Nod => {
                let permit = IntentCommitPermit::new(
                    request.generation,
                    request.tx_id,
                    request.object_id,
                    request.body_hash,
                );
                let approved = ApprovedIntent {
                    tx_id: request.tx_id,
                    generation: request.generation,
                    workspace_id: request.workspace_id,
                    object_id: request.object_id,
                    body_hash: request.body_hash,
                    permit,
                };
                audit.push(AuditEvent::new(
                    request.tx_id,
                    AuditCode::Approved,
                    request.body_hash,
                ))?;
                Ok(ApprovalDecision::Nod(approved))
            }
            ApprovalAction::Reject => {
                audit.push(AuditEvent::new(
                    request.tx_id,
                    AuditCode::Rejected,
                    request.body_hash,
                ))?;
                Ok(ApprovalDecision::Reject)
            }
            ApprovalAction::Fence => {
                audit.push(AuditEvent::new(request.tx_id, AuditCode::Fenced, Hash(0)))?;
                Ok(ApprovalDecision::Fence)
            }
        }
    }
}
