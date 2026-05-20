use crate::protocol::{ExternalActionId, Hash, PicoNodError, TxId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuditCode {
    #[default]
    None,
    Approved,
    Rejected,
    Fenced,
    CommitPending,
    Committed,
    DuplicateCommitted,
    TerminalFault,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuditEvent {
    pub tx_id: TxId,
    pub code: AuditCode,
    pub hash: Hash,
    pub external_id: ExternalActionId,
}

impl AuditEvent {
    pub const fn new(tx_id: TxId, code: AuditCode, hash: Hash) -> Self {
        Self {
            tx_id,
            code,
            hash,
            external_id: ExternalActionId(0),
        }
    }

    pub const fn with_external(
        tx_id: TxId,
        code: AuditCode,
        hash: Hash,
        external_id: ExternalActionId,
    ) -> Self {
        Self {
            tx_id,
            code,
            hash,
            external_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditLog<const N: usize> {
    events: [AuditEvent; N],
    len: usize,
}

impl<const N: usize> AuditLog<N> {
    pub const fn empty() -> Self {
        Self {
            events: [AuditEvent {
                tx_id: TxId(0),
                code: AuditCode::None,
                hash: Hash(0),
                external_id: ExternalActionId(0),
            }; N],
            len: 0,
        }
    }

    pub fn push(&mut self, event: AuditEvent) -> Result<(), PicoNodError> {
        if self.len == N {
            return Err(PicoNodError::CapacityFull);
        }
        self.events[self.len] = event;
        self.len += 1;
        Ok(())
    }

    pub fn events(&self) -> &[AuditEvent] {
        self.events.split_at(self.len).0
    }

    pub const fn len(&self) -> usize {
        self.len
    }
}
