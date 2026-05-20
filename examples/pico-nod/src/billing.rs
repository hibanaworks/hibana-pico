use crate::protocol::{
    Hash, KeyId, PicoNodError, Signature, TicketClock, WorkspaceId, hash_fields,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntitlementState {
    Active,
    Grace,
    Expired,
    Revoked,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreEvidence {
    pub workspace_id: WorkspaceId,
    pub state: EntitlementState,
    pub expires_at: u64,
    pub key_id: KeyId,
    pub signature: Signature,
}

impl StoreEvidence {
    pub fn new(
        workspace_id: WorkspaceId,
        state: EntitlementState,
        expires_at: u64,
        key_id: KeyId,
        signing_hash: Hash,
    ) -> Self {
        let unsigned = Self {
            workspace_id,
            state,
            expires_at,
            key_id,
            signature: Signature(0),
        };
        Self {
            signature: sign_store_evidence(signing_hash, unsigned),
            ..unsigned
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntitlementFact {
    pub workspace_id: WorkspaceId,
    pub state: EntitlementState,
    pub expires_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BillingBoundary {
    signing_hash: Hash,
}

impl BillingBoundary {
    pub const fn new(signing_hash: Hash) -> Self {
        Self { signing_hash }
    }

    pub fn verify(
        &self,
        evidence: StoreEvidence,
        clock: TicketClock,
    ) -> Result<EntitlementFact, PicoNodError> {
        if sign_store_evidence(
            self.signing_hash,
            StoreEvidence {
                signature: Signature(0),
                ..evidence
            },
        ) != evidence.signature
        {
            return Err(PicoNodError::SignatureMismatch);
        }
        let state = if clock.now > evidence.expires_at.saturating_add(clock.skew) {
            EntitlementState::Expired
        } else {
            evidence.state
        };
        Ok(EntitlementFact {
            workspace_id: evidence.workspace_id,
            state,
            expires_at: evidence.expires_at,
        })
    }
}

impl EntitlementFact {
    pub fn require_paid_feature(self) -> Result<Self, PicoNodError> {
        match self.state {
            EntitlementState::Active | EntitlementState::Grace => Ok(self),
            EntitlementState::Expired | EntitlementState::Revoked | EntitlementState::Unknown => {
                Err(PicoNodError::EntitlementInactive)
            }
        }
    }

    pub const fn cannot_approve(&self) -> bool {
        true
    }

    pub const fn cannot_commit_external_actions(&self) -> bool {
        true
    }
}

pub fn sign_store_evidence(signing_hash: Hash, evidence: StoreEvidence) -> Signature {
    let state_value = match evidence.state {
        EntitlementState::Active => 1,
        EntitlementState::Grace => 2,
        EntitlementState::Expired => 3,
        EntitlementState::Revoked => 4,
        EntitlementState::Unknown => 5,
    };
    Signature(
        hash_fields(&[
            signing_hash.0,
            evidence.workspace_id.0,
            state_value,
            evidence.expires_at,
            evidence.key_id.0 as u64,
        ])
        .0,
    )
}
