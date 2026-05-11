#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivationBoundary {
    memory_leases_fenced: bool,
    interrupt_subscriptions_fenced: bool,
    remote_objects_quiesced: bool,
    memory_epoch: u32,
}

impl ActivationBoundary {
    pub const fn new(
        memory_leases_fenced: bool,
        interrupt_subscriptions_fenced: bool,
        remote_objects_quiesced: bool,
        memory_epoch: u32,
    ) -> Self {
        Self {
            memory_leases_fenced,
            interrupt_subscriptions_fenced,
            remote_objects_quiesced,
            memory_epoch,
        }
    }

    pub const fn single_node(
        memory_leases_fenced: bool,
        interrupt_subscriptions_fenced: bool,
        memory_epoch: u32,
    ) -> Self {
        Self::new(
            memory_leases_fenced,
            interrupt_subscriptions_fenced,
            true,
            memory_epoch,
        )
    }

    pub const fn memory_leases_fenced(&self) -> bool {
        self.memory_leases_fenced
    }

    pub const fn interrupt_subscriptions_fenced(&self) -> bool {
        self.interrupt_subscriptions_fenced
    }

    pub const fn remote_objects_quiesced(&self) -> bool {
        self.remote_objects_quiesced
    }

    pub const fn memory_epoch(&self) -> u32 {
        self.memory_epoch
    }

    pub const fn is_safe(&self) -> bool {
        self.memory_leases_fenced
            && self.interrupt_subscriptions_fenced
            && self.remote_objects_quiesced
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyLifecycleError {
    MissingBegin,
    MissingAck,
    MissingCommit,
    GenerationMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyLifecycle {
    generation: u32,
    began: bool,
    acked: bool,
    committed: bool,
}

impl TopologyLifecycle {
    pub const fn new(generation: u32) -> Self {
        Self {
            generation,
            began: false,
            acked: false,
            committed: false,
        }
    }

    pub const fn begin(mut self, generation: u32) -> Result<Self, TopologyLifecycleError> {
        if self.generation != generation {
            return Err(TopologyLifecycleError::GenerationMismatch);
        }
        self.began = true;
        Ok(self)
    }

    pub const fn ack(mut self, generation: u32) -> Result<Self, TopologyLifecycleError> {
        if self.generation != generation {
            return Err(TopologyLifecycleError::GenerationMismatch);
        }
        if !self.began {
            return Err(TopologyLifecycleError::MissingBegin);
        }
        self.acked = true;
        Ok(self)
    }

    pub const fn commit(mut self, generation: u32) -> Result<Self, TopologyLifecycleError> {
        if self.generation != generation {
            return Err(TopologyLifecycleError::GenerationMismatch);
        }
        if !self.began {
            return Err(TopologyLifecycleError::MissingBegin);
        }
        if !self.acked {
            return Err(TopologyLifecycleError::MissingAck);
        }
        self.committed = true;
        Ok(self)
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }

    pub const fn is_committed(&self) -> bool {
        self.began && self.acked && self.committed
    }

    pub const fn require_committed(&self, generation: u32) -> Result<(), TopologyLifecycleError> {
        if self.generation != generation {
            return Err(TopologyLifecycleError::GenerationMismatch);
        }
        if !self.began {
            return Err(TopologyLifecycleError::MissingBegin);
        }
        if !self.acked {
            return Err(TopologyLifecycleError::MissingAck);
        }
        if !self.committed {
            return Err(TopologyLifecycleError::MissingCommit);
        }
        Ok(())
    }
}
