use hibana_pico::appkit;

use crate::protocol::{
    ActionKind, BoundedBytes, Generation, IntentBodyObject, IntentRequest, IssuerId,
    MAX_BODY_BYTES, MAX_SUMMARY_BYTES, PicoNodError, TxId, WorkspaceId,
};

pub struct WasiIngress;

impl WasiIngress {
    pub fn normalize_public_request(
        issuer_id: IssuerId,
        workspace_id: WorkspaceId,
        tx_id: TxId,
        generation: Generation,
        action_kind: ActionKind,
        object_id: appkit::ObjectId,
        body_bytes: &[u8],
        summary_bytes: &[u8],
    ) -> Result<(IntentBodyObject, IntentRequest), PicoNodError> {
        let body = IntentBodyObject::new(object_id, body_bytes)?;
        let summary: BoundedBytes<MAX_SUMMARY_BYTES> = BoundedBytes::new_summary(summary_bytes)?;
        let intent = IntentRequest::new(
            issuer_id,
            workspace_id,
            tx_id,
            generation,
            action_kind,
            &body,
            &summary,
        );
        Ok((body, intent))
    }

    pub const fn cannot_hold_credentials() -> bool {
        true
    }

    pub const fn cannot_select_routes() -> bool {
        true
    }

    pub const fn maximum_body_bytes() -> usize {
        MAX_BODY_BYTES
    }
}
