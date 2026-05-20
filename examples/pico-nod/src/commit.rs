use crate::audit::{AuditCode, AuditEvent, AuditLog};
use crate::protocol::{
    ApprovedIntent, ExternalActionId, Hash, IntentBodyObject, PicoNodError, Signature, TxId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalActionCredential {
    fingerprint: Hash,
}

impl ExternalActionCredential {
    pub const fn proof_only(fingerprint: Hash) -> Self {
        Self { fingerprint }
    }

    pub const fn fingerprint(&self) -> Hash {
        self.fingerprint
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommitState {
    #[default]
    Empty,
    Pending,
    Committed,
    Fenced,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitRecord {
    pub tx_id: TxId,
    pub body_hash: Hash,
    pub external_id: ExternalActionId,
    pub state: CommitState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitFacts<const N: usize> {
    records: [CommitRecord; N],
    len: usize,
}

impl<const N: usize> CommitFacts<N> {
    pub const fn empty() -> Self {
        Self {
            records: [CommitRecord {
                tx_id: TxId(0),
                body_hash: Hash(0),
                external_id: ExternalActionId(0),
                state: CommitState::Empty,
            }; N],
            len: 0,
        }
    }

    pub fn reserve_pending(&mut self, tx_id: TxId, body_hash: Hash) -> Result<(), PicoNodError> {
        match self.find(tx_id) {
            Some(record) if record.body_hash != body_hash => {
                return Err(PicoNodError::DuplicateTxDifferentBody);
            }
            Some(record) if record.state == CommitState::Committed => {
                core::hint::black_box(record);
                return Ok(());
            }
            Some(record) => {
                record.state = CommitState::Pending;
                return Ok(());
            }
            None => {}
        }
        if self.len == N {
            return Err(PicoNodError::CapacityFull);
        }
        self.records[self.len] = CommitRecord {
            tx_id,
            body_hash,
            external_id: ExternalActionId(0),
            state: CommitState::Pending,
        };
        self.len += 1;
        Ok(())
    }

    pub fn record_committed(
        &mut self,
        tx_id: TxId,
        body_hash: Hash,
        external_id: ExternalActionId,
    ) -> Result<CommitRecord, PicoNodError> {
        let record = self.find(tx_id).ok_or(PicoNodError::NotApproved)?;
        if record.body_hash != body_hash {
            return Err(PicoNodError::DuplicateTxDifferentBody);
        }
        record.state = CommitState::Committed;
        record.external_id = external_id;
        Ok(*record)
    }

    pub fn record_fenced(&mut self, tx_id: TxId, body_hash: Hash) -> Result<(), PicoNodError> {
        match self.find(tx_id) {
            Some(record) => {
                if record.body_hash != body_hash {
                    return Err(PicoNodError::DuplicateTxDifferentBody);
                }
                record.state = CommitState::Fenced;
                Ok(())
            }
            None => {
                self.reserve_pending(tx_id, body_hash)?;
                let record = self.find(tx_id).ok_or(PicoNodError::NotApproved)?;
                record.state = CommitState::Fenced;
                Ok(())
            }
        }
    }

    pub fn committed(&self, tx_id: TxId, body_hash: Hash) -> Option<CommitRecord> {
        self.records
            .split_at(self.len)
            .0
            .iter()
            .copied()
            .find(|record| {
                record.tx_id == tx_id
                    && record.body_hash == body_hash
                    && record.state == CommitState::Committed
            })
    }

    pub fn state(&self, tx_id: TxId) -> Option<CommitState> {
        self.records
            .split_at(self.len)
            .0
            .iter()
            .find(|record| record.tx_id == tx_id)
            .map(|record| record.state)
    }

    fn find(&mut self, tx_id: TxId) -> Option<&mut CommitRecord> {
        self.records
            .split_at_mut(self.len)
            .0
            .iter_mut()
            .find(|record| record.tx_id == tx_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomeReceipt {
    pub tx_id: TxId,
    pub body_hash: Hash,
    pub external_id: ExternalActionId,
    pub signature: Signature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed(OutcomeReceipt),
    DuplicateCommitted(OutcomeReceipt),
    Fenced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalActionError {
    UnknownWithoutIdempotencyEvidence,
    FailedClosed,
}

pub trait ExternalActionApi {
    fn commit(
        &mut self,
        credential: ExternalActionCredential,
        tx_id: TxId,
        body_hash: Hash,
    ) -> Result<ExternalActionId, ExternalActionError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitBoundary {
    credential: ExternalActionCredential,
    receipt_key: Hash,
}

impl CommitBoundary {
    pub const fn new(credential: ExternalActionCredential, receipt_key: Hash) -> Self {
        Self {
            credential,
            receipt_key,
        }
    }

    pub fn commit<const C: usize, const A: usize>(
        &self,
        approved: ApprovedIntent,
        body: &IntentBodyObject,
        facts: &mut CommitFacts<C>,
        audit: &mut AuditLog<A>,
        api: &mut impl ExternalActionApi,
    ) -> Result<CommitOutcome, PicoNodError> {
        self.ensure_approved_matches(approved, body)?;
        if let Some(committed) = facts.committed(approved.tx_id, approved.body_hash) {
            let receipt = self.receipt(committed.tx_id, committed.body_hash, committed.external_id);
            audit.push(AuditEvent::with_external(
                approved.tx_id,
                AuditCode::DuplicateCommitted,
                approved.body_hash,
                committed.external_id,
            ))?;
            return Ok(CommitOutcome::DuplicateCommitted(receipt));
        }
        facts.reserve_pending(approved.tx_id, approved.body_hash)?;
        audit.push(AuditEvent::new(
            approved.tx_id,
            AuditCode::CommitPending,
            approved.body_hash,
        ))?;
        match api.commit(self.credential, approved.tx_id, approved.body_hash) {
            Ok(external_id) => {
                facts.record_committed(approved.tx_id, approved.body_hash, external_id)?;
                audit.push(AuditEvent::with_external(
                    approved.tx_id,
                    AuditCode::Committed,
                    approved.body_hash,
                    external_id,
                ))?;
                Ok(CommitOutcome::Committed(self.receipt(
                    approved.tx_id,
                    approved.body_hash,
                    external_id,
                )))
            }
            Err(ExternalActionError::UnknownWithoutIdempotencyEvidence) => {
                facts.record_fenced(approved.tx_id, approved.body_hash)?;
                audit.push(AuditEvent::new(
                    approved.tx_id,
                    AuditCode::Fenced,
                    approved.body_hash,
                ))?;
                Ok(CommitOutcome::Fenced)
            }
            Err(ExternalActionError::FailedClosed) => {
                facts.record_fenced(approved.tx_id, approved.body_hash)?;
                audit.push(AuditEvent::new(
                    approved.tx_id,
                    AuditCode::TerminalFault,
                    approved.body_hash,
                ))?;
                Err(PicoNodError::ExternalFailed)
            }
        }
    }

    pub fn commit_with_lost_local_ack<const C: usize, const A: usize>(
        &self,
        approved: ApprovedIntent,
        body: &IntentBodyObject,
        facts: &mut CommitFacts<C>,
        audit: &mut AuditLog<A>,
        api: &mut impl ExternalActionApi,
    ) -> Result<OutcomeReceipt, PicoNodError> {
        match self.commit(approved, body, facts, audit, api)? {
            CommitOutcome::Committed(receipt) | CommitOutcome::DuplicateCommitted(receipt) => {
                Ok(receipt)
            }
            CommitOutcome::Fenced => Err(PicoNodError::MissingIdempotencyEvidence),
        }
    }

    fn ensure_approved_matches(
        &self,
        approved: ApprovedIntent,
        body: &IntentBodyObject,
    ) -> Result<(), PicoNodError> {
        if approved.permit.tx_id() != approved.tx_id
            || approved.permit.generation() != approved.generation
            || approved.permit.object_id() != approved.object_id
            || approved.permit.body_hash() != approved.body_hash
        {
            return Err(PicoNodError::NotApproved);
        }
        if body.object_id() != approved.object_id || body.body_hash() != approved.body_hash {
            return Err(PicoNodError::ApprovalMismatch);
        }
        Ok(())
    }

    fn receipt(
        &self,
        tx_id: TxId,
        body_hash: Hash,
        external_id: ExternalActionId,
    ) -> OutcomeReceipt {
        let signature = Signature(
            crate::protocol::hash_fields(&[
                self.receipt_key.0,
                tx_id.0,
                body_hash.0,
                external_id.0,
            ])
            .0,
        );
        OutcomeReceipt {
            tx_id,
            body_hash,
            external_id,
            signature,
        }
    }
}
