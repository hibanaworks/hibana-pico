use crate::protocol::{
    ApprovalAction, ApprovalEvidence, ApprovalRequest, DeviceSigningKey, PicoNodError,
    displayed_hash,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayedIntent {
    request: ApprovalRequest,
}

impl DisplayedIntent {
    pub const fn request(&self) -> ApprovalRequest {
        self.request
    }

    pub fn displayed_hash(&self) -> crate::protocol::Hash {
        displayed_hash(self.request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalApprovalApp {
    key: DeviceSigningKey,
}

impl LocalApprovalApp {
    pub const fn new(key: DeviceSigningKey) -> Self {
        Self { key }
    }

    pub const fn display(&self, request: ApprovalRequest) -> DisplayedIntent {
        core::hint::black_box(self);
        DisplayedIntent { request }
    }

    pub fn decide(
        &self,
        displayed: DisplayedIntent,
        action: ApprovalAction,
    ) -> Result<ApprovalEvidence, PicoNodError> {
        Ok(self.key.sign(displayed.request, action))
    }

    pub const fn cannot_commit_external_actions(&self) -> bool {
        true
    }

    pub const fn cannot_select_routes(&self) -> bool {
        true
    }

    pub const fn cannot_hold_apns_provider_credentials(&self) -> bool {
        true
    }
}
