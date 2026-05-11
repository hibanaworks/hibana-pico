#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectTransactionDecision {
    Commit,
    Abort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectTransaction {
    generation: u16,
    decision: ObjectTransactionDecision,
}

impl ObjectTransaction {
    pub const fn commit(generation: u16) -> Self {
        Self {
            generation,
            decision: ObjectTransactionDecision::Commit,
        }
    }

    pub const fn abort(generation: u16) -> Self {
        Self {
            generation,
            decision: ObjectTransactionDecision::Abort,
        }
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn decision(&self) -> ObjectTransactionDecision {
        self.decision
    }

    pub const fn is_commit(&self) -> bool {
        matches!(self.decision, ObjectTransactionDecision::Commit)
    }
}
