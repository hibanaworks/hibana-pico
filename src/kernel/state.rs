#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateSnapshotFact {
    generation: u16,
    memory_epoch: u32,
}

impl StateSnapshotFact {
    pub const fn new(generation: u16, memory_epoch: u32) -> Self {
        Self {
            generation,
            memory_epoch,
        }
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn memory_epoch(&self) -> u32 {
        self.memory_epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRestoreFact {
    snapshot_generation: u16,
    target_memory_epoch: u32,
}

impl StateRestoreFact {
    pub const fn from_snapshot(snapshot: StateSnapshotFact, target_memory_epoch: u32) -> Self {
        Self {
            snapshot_generation: snapshot.generation,
            target_memory_epoch,
        }
    }

    pub const fn snapshot_generation(&self) -> u16 {
        self.snapshot_generation
    }

    pub const fn target_memory_epoch(&self) -> u32 {
        self.target_memory_epoch
    }
}
