use crate::ingress::WasiIngress;
use crate::protocol::{
    ActionKind, Generation, IntentBodyObject, IntentRequest, IssuerId, PicoNodError, TxId,
    WorkspaceId,
};
use hibana_pico::appkit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportAction {
    FenceWorkspace,
    RevokeFutureIssuerTickets,
    RevokeFutureDeviceTickets,
    RotateKey,
    ExportSignedReceipts,
    MarkIncident,
    ReconcileExternalCommit,
}

impl SupportAction {
    pub const fn code(self) -> u8 {
        match self {
            Self::FenceWorkspace => 1,
            Self::RevokeFutureIssuerTickets => 2,
            Self::RevokeFutureDeviceTickets => 3,
            Self::RotateKey => 4,
            Self::ExportSignedReceipts => 5,
            Self::MarkIncident => 6,
            Self::ReconcileExternalCommit => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportIntent {
    pub action: SupportAction,
    pub body: IntentBodyObject,
    pub request: IntentRequest,
}

impl SupportIntent {
    pub fn new(
        issuer_id: IssuerId,
        workspace_id: WorkspaceId,
        tx_id: TxId,
        generation: Generation,
        action: SupportAction,
        object_id: appkit::ObjectId,
        body: &[u8],
    ) -> Result<Self, PicoNodError> {
        let mut summary = [0u8; 8];
        summary[0] = action.code();
        let (body, request) = WasiIngress::normalize_public_request(
            issuer_id,
            workspace_id,
            tx_id,
            generation,
            ActionKind::LocalCommand,
            object_id,
            body,
            &summary,
        )?;
        Ok(Self {
            action,
            body,
            request,
        })
    }

    pub const fn cannot_commit_without_approval(&self) -> bool {
        true
    }

    pub const fn cannot_select_routes(&self) -> bool {
        true
    }
}
