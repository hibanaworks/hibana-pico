use crate::protocol::{
    AdmittedReplyInput, ApprovedReplyDraft, ApprovedXReply, AutoPostPermit, AutoXPost, DraftObject,
    Generation, Hash, ProtocolError, ReplyApprovalRequest, ReplyDraftProposal, ReplyInputRequest,
    TxId, UntrustedReplyObject,
};

pub struct Driver;

impl Driver {
    pub fn auto_post(tx_id: TxId, generation: Generation, object: &DraftObject) -> AutoXPost {
        let permit = AutoPostPermit::new(
            generation,
            tx_id,
            object.object_id(),
            object.draft_hash(),
            object.body_hash(),
        );
        AutoXPost {
            tx_id,
            generation,
            object_id: object.object_id(),
            draft_hash: object.draft_hash(),
            body_hash: object.body_hash(),
            permit,
        }
    }

    pub const fn request_reply_input(
        tx_id: TxId,
        generation: Generation,
        reply: &UntrustedReplyObject,
        reason_hash: Hash,
    ) -> ReplyInputRequest {
        ReplyInputRequest {
            tx_id,
            generation,
            reply_id: reply.reply_id(),
            object_id: reply.object_id(),
            body_hash: reply.body_hash(),
            reason_hash,
        }
    }

    pub const fn request_reply_approval(
        proposal: ReplyDraftProposal,
        summary_hash: Hash,
    ) -> ReplyApprovalRequest {
        ReplyApprovalRequest {
            tx_id: proposal.tx_id,
            generation: proposal.generation,
            reply_id: proposal.reply_id,
            object_id: proposal.object_id,
            draft_hash: proposal.draft_hash,
            body_hash: proposal.body_hash,
            summary_hash,
        }
    }

    pub fn approved_reply(
        approved: ApprovedReplyDraft,
        admitted: &AdmittedReplyInput,
    ) -> Result<ApprovedXReply, ProtocolError> {
        if approved.reply_id != admitted.reply_id || approved.generation != admitted.generation {
            return Err(ProtocolError::ReplyApprovalMismatch);
        }
        Ok(ApprovedXReply::new(approved))
    }
}
