use crate::protocol::{AdmittedReplyInput, DraftObject, Generation, ReplyDraftProposal, TxId};

pub struct WasiAgent;

impl WasiAgent {
    pub const fn propose_reply(
        tx_id: TxId,
        generation: Generation,
        admitted: &AdmittedReplyInput,
        object: &DraftObject,
        risk_hint: u8,
    ) -> ReplyDraftProposal {
        ReplyDraftProposal {
            tx_id,
            generation,
            reply_id: admitted.reply_id,
            object_id: object.object_id(),
            draft_hash: object.draft_hash(),
            body_hash: object.body_hash(),
            risk_hint,
        }
    }
}
