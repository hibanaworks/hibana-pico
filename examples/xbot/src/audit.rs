use crate::protocol::{Hash, TxId, XPostId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuditEvent {
    pub tx_id: TxId,
    pub code: AuditCode,
    pub hash: Hash,
    pub x_post_id: XPostId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuditCode {
    #[default]
    None,
    ApprovalRequested,
    ReplyInputAdmitted,
    Approved,
    Rejected,
    Fenced,
    Committed,
    DuplicateCommitted,
    TerminalFault,
}

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
                x_post_id: XPostId(0),
            }; N],
            len: 0,
        }
    }

    pub fn push(&mut self, event: AuditEvent) -> Result<(), AuditError> {
        if self.len == N {
            return Err(AuditError::Full);
        }
        self.events[self.len] = event;
        self.len += 1;
        Ok(())
    }

    pub fn events(&self) -> &[AuditEvent] {
        self.events.split_at(self.len).0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditError {
    Full,
}
