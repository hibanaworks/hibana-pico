use core::cell::Cell;

use crate::{
    choreography::protocol::{
        MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd, MgmtImageRollback,
        MgmtStatus, MgmtStatusCode,
    },
    kernel::swarm::{NodeId, SwarmCredential},
    kernel::wasi::{Wasip1Error, Wasip1Module},
};

mod grant;
pub use grant::*;
mod lifecycle;
pub use lifecycle::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageSlotError {
    BadSlot,
    BadChunkIndex,
    ImageTooLarge,
    OffsetMismatch,
    LengthMismatch,
    InvalidImage(Wasip1Error),
    NeedFence,
    BadFenceEpoch,
    AuthFailed,
    BadSessionGeneration,
    RollbackEmpty,
}

impl ImageSlotError {
    pub const fn status_code(&self) -> MgmtStatusCode {
        match self {
            Self::BadSlot => MgmtStatusCode::BadSlot,
            Self::BadChunkIndex => MgmtStatusCode::BadChunkIndex,
            Self::ImageTooLarge => MgmtStatusCode::ImageTooLarge,
            Self::OffsetMismatch => MgmtStatusCode::OffsetMismatch,
            Self::LengthMismatch => MgmtStatusCode::LengthMismatch,
            Self::InvalidImage(_) => MgmtStatusCode::InvalidImage,
            Self::NeedFence => MgmtStatusCode::NeedFence,
            Self::BadFenceEpoch => MgmtStatusCode::BadFenceEpoch,
            Self::AuthFailed => MgmtStatusCode::AuthFailed,
            Self::BadSessionGeneration => MgmtStatusCode::BadSessionGeneration,
            Self::RollbackEmpty => MgmtStatusCode::RollbackEmpty,
        }
    }

    pub const fn status(&self, slot: u8) -> MgmtStatus {
        MgmtStatus::new(slot, self.status_code())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManagementRejectionTelemetry {
    auth_failed: u16,
    bad_generation: u16,
    bad_slot: u16,
    invalid_image: u16,
    need_fence: u16,
    bad_fence_epoch: u16,
    other: u16,
}

impl ManagementRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            auth_failed: 0,
            bad_generation: 0,
            bad_slot: 0,
            invalid_image: 0,
            need_fence: 0,
            bad_fence_epoch: 0,
            other: 0,
        }
    }

    pub const fn auth_failed(self) -> u16 {
        self.auth_failed
    }

    pub const fn bad_generation(self) -> u16 {
        self.bad_generation
    }

    pub const fn bad_slot(self) -> u16 {
        self.bad_slot
    }

    pub const fn invalid_image(self) -> u16 {
        self.invalid_image
    }

    pub const fn need_fence(self) -> u16 {
        self.need_fence
    }

    pub const fn bad_fence_epoch(self) -> u16 {
        self.bad_fence_epoch
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.auth_failed
            .saturating_add(self.bad_generation)
            .saturating_add(self.bad_slot)
            .saturating_add(self.invalid_image)
            .saturating_add(self.need_fence)
            .saturating_add(self.bad_fence_epoch)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: ImageSlotError) {
        let slot = match error {
            ImageSlotError::AuthFailed => &mut self.auth_failed,
            ImageSlotError::BadSessionGeneration => &mut self.bad_generation,
            ImageSlotError::BadSlot => &mut self.bad_slot,
            ImageSlotError::InvalidImage(_) => &mut self.invalid_image,
            ImageSlotError::NeedFence => &mut self.need_fence,
            ImageSlotError::BadFenceEpoch => &mut self.bad_fence_epoch,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageTransferPlan {
    total_len: u32,
    chunk_count: u16,
    last_chunk_len: u8,
}

impl ImageTransferPlan {
    pub fn new(total_len: usize) -> Result<Self, ImageSlotError> {
        if total_len == 0 {
            return Err(ImageSlotError::LengthMismatch);
        }
        if total_len > u32::MAX as usize {
            return Err(ImageSlotError::ImageTooLarge);
        }
        let chunk_count =
            total_len.div_ceil(crate::choreography::protocol::MGMT_IMAGE_CHUNK_CAPACITY);
        if chunk_count > u16::MAX as usize {
            return Err(ImageSlotError::ImageTooLarge);
        }
        let remainder = total_len % crate::choreography::protocol::MGMT_IMAGE_CHUNK_CAPACITY;
        let last_chunk_len = if remainder == 0 {
            crate::choreography::protocol::MGMT_IMAGE_CHUNK_CAPACITY
        } else {
            remainder
        };
        Ok(Self {
            total_len: total_len as u32,
            chunk_count: chunk_count as u16,
            last_chunk_len: last_chunk_len as u8,
        })
    }

    pub const fn total_len(&self) -> u32 {
        self.total_len
    }

    pub const fn chunk_count(&self) -> u16 {
        self.chunk_count
    }

    pub const fn last_chunk_len(&self) -> u8 {
        self.last_chunk_len
    }

    pub fn chunk_range(&self, index: u16) -> Result<(usize, usize), ImageSlotError> {
        if index >= self.chunk_count {
            return Err(ImageSlotError::BadChunkIndex);
        }
        let start = index as usize * crate::choreography::protocol::MGMT_IMAGE_CHUNK_CAPACITY;
        let end = core::cmp::min(
            start + crate::choreography::protocol::MGMT_IMAGE_CHUNK_CAPACITY,
            self.total_len as usize,
        );
        Ok((start, end))
    }
}

#[derive(Clone, Copy)]
pub struct ImageSlot<const CAP: usize> {
    bytes: [u8; CAP],
    len: u32,
    expected_len: u32,
    generation: u32,
    valid: bool,
}

impl<const CAP: usize> ImageSlot<CAP> {
    pub const fn empty() -> Self {
        Self {
            bytes: [0; CAP],
            len: 0,
            expected_len: 0,
            generation: 0,
            valid: false,
        }
    }

    pub const fn len(&self) -> u32 {
        self.len
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }

    pub const fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    fn begin(&mut self, begin: MgmtImageBegin) -> Result<(), ImageSlotError> {
        if begin.total_len() as usize > CAP {
            return Err(ImageSlotError::ImageTooLarge);
        }
        self.len = 0;
        self.expected_len = begin.total_len();
        self.generation = begin.generation();
        self.valid = false;
        Ok(())
    }

    fn push_chunk(&mut self, chunk: MgmtImageChunk) -> Result<(), ImageSlotError> {
        if chunk.offset() != self.len {
            return Err(ImageSlotError::OffsetMismatch);
        }
        let next_len = self
            .len
            .checked_add(chunk.len() as u32)
            .ok_or(ImageSlotError::ImageTooLarge)?;
        if next_len > self.expected_len || next_len as usize > CAP {
            return Err(ImageSlotError::ImageTooLarge);
        }
        let start = self.len as usize;
        let end = next_len as usize;
        self.bytes[start..end].copy_from_slice(chunk.as_bytes());
        self.len = next_len;
        Ok(())
    }

    fn finish(&mut self, end: MgmtImageEnd) -> Result<(), ImageSlotError> {
        if end.expected_len() != self.expected_len || self.len != self.expected_len {
            return Err(ImageSlotError::LengthMismatch);
        }
        Wasip1Module::install(self.as_bytes()).map_err(ImageSlotError::InvalidImage)?;
        self.valid = true;
        Ok(())
    }
}

pub struct ImageSlotTable<const SLOTS: usize, const CAP: usize> {
    slots: [ImageSlot<CAP>; SLOTS],
    active_slot: Option<u8>,
    rollback: Option<ImageSlot<CAP>>,
    rejection_telemetry: Cell<ManagementRejectionTelemetry>,
}

impl<const SLOTS: usize, const CAP: usize> ImageSlotTable<SLOTS, CAP> {
    pub const fn new() -> Self {
        Self {
            slots: [ImageSlot::empty(); SLOTS],
            active_slot: None,
            rollback: None,
            rejection_telemetry: Cell::new(ManagementRejectionTelemetry::new()),
        }
    }

    pub const fn active_slot(&self) -> Option<u8> {
        self.active_slot
    }

    pub const fn has_rollback(&self) -> bool {
        self.rollback.is_some()
    }

    pub fn rejection_telemetry(&self) -> ManagementRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn begin_with_control(
        &mut self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        begin: MgmtImageBegin,
    ) -> Result<MgmtStatus, ImageSlotError> {
        self.verify_install_control(
            control,
            node_id,
            credential,
            session_generation,
            begin.slot(),
            begin.generation(),
        )?;
        self.begin(begin)
    }

    pub fn chunk_with_control(
        &mut self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        chunk: MgmtImageChunk,
    ) -> Result<MgmtStatus, ImageSlotError> {
        let generation = match self.slot(chunk.slot()) {
            Ok(slot) => slot.generation(),
            Err(error) => return Err(self.record_rejection(error)),
        };
        self.verify_install_control(
            control,
            node_id,
            credential,
            session_generation,
            chunk.slot(),
            generation,
        )?;
        self.chunk(chunk)
    }

    pub fn end_with_control(
        &mut self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        end: MgmtImageEnd,
    ) -> Result<MgmtStatus, ImageSlotError> {
        let generation = match self.slot(end.slot()) {
            Ok(slot) => slot.generation(),
            Err(error) => return Err(self.record_rejection(error)),
        };
        self.verify_install_control(
            control,
            node_id,
            credential,
            session_generation,
            end.slot(),
            generation,
        )?;
        self.end(end)
    }

    pub fn activate_with_control(
        &mut self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        activate: MgmtImageActivate,
        boundary: ActivationBoundary,
    ) -> Result<MgmtStatus, ImageSlotError> {
        let generation = match self.slot(activate.slot()) {
            Ok(slot) => slot.generation(),
            Err(error) => return Err(self.record_rejection(error)),
        };
        self.verify_install_control(
            control,
            node_id,
            credential,
            session_generation,
            activate.slot(),
            generation,
        )?;
        self.activate_at_boundary(activate, boundary)
    }

    pub fn activate_with_topology_control(
        &mut self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        activate: MgmtImageActivate,
        boundary: ActivationBoundary,
        topology: TopologyLifecycle,
    ) -> Result<MgmtStatus, ImageSlotError> {
        topology
            .require_committed(activate.fence_epoch())
            .map_err(|error| match error {
                TopologyLifecycleError::GenerationMismatch => {
                    self.record_rejection(ImageSlotError::BadFenceEpoch)
                }
                _ => self.record_rejection(ImageSlotError::NeedFence),
            })?;
        self.activate_with_control(
            control,
            node_id,
            credential,
            session_generation,
            activate,
            boundary,
        )
    }

    fn verify_install_control(
        &self,
        control: MgmtControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        slot: u8,
        image_generation: u32,
    ) -> Result<(), ImageSlotError> {
        match control {
            MgmtControl::InstallGrant(grant) => grant
                .verify(
                    node_id,
                    credential,
                    session_generation,
                    slot,
                    image_generation,
                )
                .map_err(|error| self.record_rejection(error)),
        }
    }

    pub fn slot(&self, slot: u8) -> Result<&ImageSlot<CAP>, ImageSlotError> {
        self.slots.get(slot as usize).ok_or(ImageSlotError::BadSlot)
    }

    pub fn begin(&mut self, begin: MgmtImageBegin) -> Result<MgmtStatus, ImageSlotError> {
        let result = match self.slots.get_mut(begin.slot() as usize) {
            Some(slot) => slot
                .begin(begin)
                .map(|()| MgmtStatus::new(begin.slot(), MgmtStatusCode::Ok)),
            None => Err(ImageSlotError::BadSlot),
        };
        self.record_result(result)
    }

    pub fn chunk(&mut self, chunk: MgmtImageChunk) -> Result<MgmtStatus, ImageSlotError> {
        let result = match self.slots.get_mut(chunk.slot() as usize) {
            Some(slot) => slot
                .push_chunk(chunk)
                .map(|()| MgmtStatus::new(chunk.slot(), MgmtStatusCode::Ok)),
            None => Err(ImageSlotError::BadSlot),
        };
        self.record_result(result)
    }

    pub fn end(&mut self, end: MgmtImageEnd) -> Result<MgmtStatus, ImageSlotError> {
        let result = match self.slots.get_mut(end.slot() as usize) {
            Some(slot) => slot
                .finish(end)
                .map(|()| MgmtStatus::new(end.slot(), MgmtStatusCode::Ok)),
            None => Err(ImageSlotError::BadSlot),
        };
        self.record_result(result)
    }

    pub fn activate(
        &mut self,
        activate: MgmtImageActivate,
        leases_fenced: bool,
        memory_epoch: u32,
    ) -> Result<MgmtStatus, ImageSlotError> {
        self.activate_at_boundary(
            activate,
            ActivationBoundary::single_node(leases_fenced, true, memory_epoch),
        )
    }

    pub fn activate_at_boundary(
        &mut self,
        activate: MgmtImageActivate,
        boundary: ActivationBoundary,
    ) -> Result<MgmtStatus, ImageSlotError> {
        if !boundary.is_safe() {
            return Err(self.record_rejection(ImageSlotError::NeedFence));
        }
        if activate.fence_epoch() != boundary.memory_epoch() {
            return Err(self.record_rejection(ImageSlotError::BadFenceEpoch));
        }
        let slot = match self.slot(activate.slot()) {
            Ok(slot) => *slot,
            Err(error) => return Err(self.record_rejection(error)),
        };
        if !slot.is_valid() {
            return Err(
                self.record_rejection(ImageSlotError::InvalidImage(Wasip1Error::InvalidModule))
            );
        }
        if let Some(active_slot) = self.active_slot {
            self.rollback = Some(match self.slot(active_slot) {
                Ok(slot) => *slot,
                Err(error) => return Err(self.record_rejection(error)),
            });
        }
        self.active_slot = Some(activate.slot());
        Ok(MgmtStatus::new(activate.slot(), MgmtStatusCode::Ok))
    }

    pub fn rollback(&mut self, rollback: MgmtImageRollback) -> Result<MgmtStatus, ImageSlotError> {
        let result = match self.rollback.take() {
            Some(previous) => match self.slots.get_mut(rollback.slot() as usize) {
                Some(slot) => {
                    *slot = previous;
                    self.active_slot = Some(rollback.slot());
                    Ok(MgmtStatus::new(rollback.slot(), MgmtStatusCode::Ok))
                }
                None => Err(ImageSlotError::BadSlot),
            },
            None => Err(ImageSlotError::RollbackEmpty),
        };
        self.record_result(result)
    }

    fn record_result<T>(&self, result: Result<T, ImageSlotError>) -> Result<T, ImageSlotError> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(self.record_rejection(error)),
        }
    }

    fn record_rejection(&self, error: ImageSlotError) -> ImageSlotError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }
}

impl<const SLOTS: usize, const CAP: usize> Default for ImageSlotTable<SLOTS, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TelemetrySnapshot {
    active_slot: Option<u8>,
    has_rollback: bool,
    outstanding_leases: u8,
    memory_epoch: u32,
}

impl TelemetrySnapshot {
    pub const fn new(
        active_slot: Option<u8>,
        has_rollback: bool,
        outstanding_leases: u8,
        memory_epoch: u32,
    ) -> Self {
        Self {
            active_slot,
            has_rollback,
            outstanding_leases,
            memory_epoch,
        }
    }

    pub const fn active_slot(&self) -> Option<u8> {
        self.active_slot
    }

    pub const fn has_rollback(&self) -> bool {
        self.has_rollback
    }

    pub const fn outstanding_leases(&self) -> u8 {
        self.outstanding_leases
    }

    pub const fn memory_epoch(&self) -> u32 {
        self.memory_epoch
    }
}

impl<const SLOTS: usize, const CAP: usize> ImageSlotTable<SLOTS, CAP> {
    pub fn telemetry(&self, outstanding_leases: u8, memory_epoch: u32) -> TelemetrySnapshot {
        TelemetrySnapshot::new(
            self.active_slot(),
            self.has_rollback(),
            outstanding_leases,
            memory_epoch,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationBoundary, ImageSlotError, ImageSlotTable, ImageTransferPlan, MgmtControl,
    };
    use crate::choreography::protocol::{
        MGMT_IMAGE_CHUNK_CAPACITY, MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd,
        MgmtImageRollback, MgmtStatusCode,
    };
    use crate::kernel::swarm::{NodeId, SwarmCredential};

    static WASIP1_STDOUT_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write hibana wasip1 stdout\n";
    static WASIP1_STDERR_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write hibana wasip1 stderr\n";

    fn install_image<const CAP: usize>(
        table: &mut ImageSlotTable<2, CAP>,
        slot: u8,
        generation: u32,
        image: &[u8],
    ) {
        let plan = ImageTransferPlan::new(image.len()).expect("plan image transfer");
        table
            .begin(MgmtImageBegin::new(slot, plan.total_len(), generation))
            .expect("begin image");
        for index in 0..plan.chunk_count() {
            let (offset, end) = plan.chunk_range(index).expect("image chunk range");
            table
                .chunk(
                    MgmtImageChunk::new(slot, offset as u32, &image[offset..end])
                        .expect("image chunk"),
                )
                .expect("append chunk");
        }
        table
            .end(MgmtImageEnd::new(slot, plan.total_len()))
            .expect("end image");
    }

    #[test]
    fn image_transfer_plan_bounds_chunk_ranges() {
        let plan =
            ImageTransferPlan::new(MGMT_IMAGE_CHUNK_CAPACITY * 2 + 7).expect("plan transfer");
        assert_eq!(plan.total_len(), 71);
        assert_eq!(plan.chunk_count(), 3);
        assert_eq!(plan.last_chunk_len(), 7);
        assert_eq!(plan.chunk_range(0), Ok((0, MGMT_IMAGE_CHUNK_CAPACITY)));
        assert_eq!(
            plan.chunk_range(1),
            Ok((MGMT_IMAGE_CHUNK_CAPACITY, MGMT_IMAGE_CHUNK_CAPACITY * 2))
        );
        assert_eq!(
            plan.chunk_range(2),
            Ok((
                MGMT_IMAGE_CHUNK_CAPACITY * 2,
                MGMT_IMAGE_CHUNK_CAPACITY * 2 + 7
            ))
        );
        assert_eq!(plan.chunk_range(3), Err(ImageSlotError::BadChunkIndex));
        assert_eq!(
            ImageTransferPlan::new(0),
            Err(ImageSlotError::LengthMismatch)
        );
    }

    #[test]
    fn image_slot_installs_valid_module_and_requires_fence_to_activate() {
        let mut table: ImageSlotTable<2, 131_072> = ImageSlotTable::new();
        install_image(&mut table, 0, 1, WASIP1_STDOUT_GUEST);
        assert!(table.slot(0).expect("slot").is_valid());

        assert_eq!(
            table.activate_at_boundary(
                MgmtImageActivate::new(0, 2),
                ActivationBoundary::single_node(false, true, 1)
            ),
            Err(ImageSlotError::NeedFence)
        );
        table
            .activate_at_boundary(
                MgmtImageActivate::new(0, 2),
                ActivationBoundary::single_node(true, true, 2),
            )
            .expect("activate image");
        assert_eq!(table.active_slot(), Some(0));
    }

    #[test]
    fn image_activation_boundary_requires_interrupt_and_remote_quiescence() {
        let mut table: ImageSlotTable<2, 131_072> = ImageSlotTable::new();
        install_image(&mut table, 0, 1, WASIP1_STDOUT_GUEST);
        let activate = MgmtImageActivate::new(0, 2);

        assert_eq!(
            table.activate_at_boundary(activate, ActivationBoundary::new(true, false, true, 2)),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(
            table.activate_at_boundary(activate, ActivationBoundary::new(true, true, false, 2)),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(
            table.activate_at_boundary(activate, ActivationBoundary::new(true, true, true, 1)),
            Err(ImageSlotError::BadFenceEpoch)
        );
        table
            .activate_at_boundary(activate, ActivationBoundary::new(true, true, true, 2))
            .expect("activate after every boundary is quiesced");
        assert_eq!(table.active_slot(), Some(0));
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.need_fence(), 2);
        assert_eq!(telemetry.bad_fence_epoch(), 1);
        assert_eq!(telemetry.total(), 3);
    }

    #[test]
    fn authenticated_management_install_rejects_forged_or_stale_grants() {
        let mut table: ImageSlotTable<2, 131_072> = ImageSlotTable::new();
        let node = NodeId::new(7);
        let credential = SwarmCredential::new(0x4849_4241);
        let begin = MgmtImageBegin::new(0, WASIP1_STDOUT_GUEST.len() as u32, 41);
        let grant =
            MgmtControl::install_grant(node, credential, 9, begin.slot(), begin.generation());

        assert_eq!(
            table.begin_with_control(
                MgmtControl::install_grant(node, credential, 8, begin.slot(), begin.generation()),
                node,
                credential,
                9,
                begin,
            ),
            Err(ImageSlotError::BadSessionGeneration)
        );
        assert_eq!(
            ImageSlotError::BadSessionGeneration
                .status(begin.slot())
                .code(),
            MgmtStatusCode::BadSessionGeneration
        );

        assert_eq!(
            table.begin_with_control(
                MgmtControl::install_grant(
                    node,
                    SwarmCredential::new(0x5752_4f4e),
                    9,
                    begin.slot(),
                    begin.generation(),
                ),
                node,
                credential,
                9,
                begin,
            ),
            Err(ImageSlotError::AuthFailed)
        );
        assert_eq!(
            ImageSlotError::AuthFailed.status(begin.slot()).code(),
            MgmtStatusCode::AuthFailed
        );
        assert_eq!(
            table.begin_with_control(
                MgmtControl::install_grant(node, credential, 9, 1, begin.generation()),
                node,
                credential,
                9,
                begin,
            ),
            Err(ImageSlotError::AuthFailed)
        );

        table
            .begin_with_control(grant, node, credential, 9, begin)
            .expect("authorized image begin");
        let plan = ImageTransferPlan::new(WASIP1_STDOUT_GUEST.len()).expect("image plan");
        let (offset, end) = plan.chunk_range(0).expect("first chunk range");
        let chunk = MgmtImageChunk::new(0, offset as u32, &WASIP1_STDOUT_GUEST[offset..end])
            .expect("image chunk");
        assert_eq!(
            table.chunk_with_control(
                MgmtControl::install_grant(
                    node,
                    SwarmCredential::new(0x5752_4f4e),
                    9,
                    0,
                    begin.generation(),
                ),
                node,
                credential,
                9,
                chunk,
            ),
            Err(ImageSlotError::AuthFailed)
        );
        table
            .chunk_with_control(grant, node, credential, 9, chunk)
            .expect("authorized image chunk");
        for index in 1..plan.chunk_count() {
            let (offset, end) = plan.chunk_range(index).expect("chunk range");
            table
                .chunk_with_control(
                    grant,
                    node,
                    credential,
                    9,
                    MgmtImageChunk::new(0, offset as u32, &WASIP1_STDOUT_GUEST[offset..end])
                        .expect("image chunk"),
                )
                .expect("authorized image chunk");
        }
        table
            .end_with_control(
                grant,
                node,
                credential,
                9,
                MgmtImageEnd::new(0, WASIP1_STDOUT_GUEST.len() as u32),
            )
            .expect("authorized image end");

        let activate = MgmtImageActivate::new(0, 2);
        assert_eq!(
            table.activate_with_control(
                MgmtControl::install_grant(
                    node,
                    SwarmCredential::new(0x5752_4f4e),
                    9,
                    0,
                    begin.generation(),
                ),
                node,
                credential,
                9,
                activate,
                ActivationBoundary::single_node(true, true, 2),
            ),
            Err(ImageSlotError::AuthFailed)
        );
        table
            .activate_with_control(
                grant,
                node,
                credential,
                9,
                activate,
                ActivationBoundary::single_node(true, true, 2),
            )
            .expect("authorized activation");
        assert_eq!(table.active_slot(), Some(0));
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.auth_failed(), 4);
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.total(), 5);
    }

    #[test]
    fn image_slot_rejects_invalid_or_misaligned_images() {
        let mut table: ImageSlotTable<2, 64> = ImageSlotTable::new();
        assert_eq!(
            table.begin(MgmtImageBegin::new(0, 65, 1)),
            Err(ImageSlotError::ImageTooLarge)
        );

        table
            .begin(MgmtImageBegin::new(0, 8, 1))
            .expect("begin small invalid image");
        assert_eq!(
            table.chunk(MgmtImageChunk::new(0, 1, b"bad").expect("chunk")),
            Err(ImageSlotError::OffsetMismatch)
        );
        table
            .chunk(MgmtImageChunk::new(0, 0, b"not-wasm").expect("chunk"))
            .expect("chunk invalid image");
        assert!(matches!(
            table.end(MgmtImageEnd::new(0, 8)),
            Err(ImageSlotError::InvalidImage(_))
        ));
    }

    #[test]
    fn image_slot_preserves_previous_active_image_for_rollback() {
        let mut table: ImageSlotTable<2, 131_072> = ImageSlotTable::new();
        install_image(&mut table, 0, 1, WASIP1_STDOUT_GUEST);
        table
            .activate(MgmtImageActivate::new(0, 2), true, 2)
            .expect("activate first image");

        install_image(&mut table, 1, 2, WASIP1_STDERR_GUEST);
        table
            .activate(MgmtImageActivate::new(1, 3), true, 3)
            .expect("activate second image");
        assert_eq!(table.active_slot(), Some(1));
        assert!(table.has_rollback());

        table
            .rollback(MgmtImageRollback::new(0))
            .expect("rollback into slot 0");
        assert_eq!(table.active_slot(), Some(0));
        assert!(!table.has_rollback());
        assert_eq!(table.slot(0).expect("slot").generation(), 1);
    }

    #[test]
    fn image_slot_rejects_without_rollback() {
        let mut table: ImageSlotTable<2, 1024> = ImageSlotTable::new();
        assert_eq!(
            table.rollback(MgmtImageRollback::new(0)),
            Err(ImageSlotError::RollbackEmpty)
        );
    }

    #[test]
    fn image_slot_reports_telemetry_snapshot() {
        let mut table: ImageSlotTable<2, 131_072> = ImageSlotTable::new();
        install_image(&mut table, 0, 1, WASIP1_STDOUT_GUEST);
        table
            .activate(MgmtImageActivate::new(0, 2), true, 2)
            .expect("activate image");
        let telemetry = table.telemetry(3, 9);
        assert_eq!(telemetry.active_slot(), Some(0));
        assert!(!telemetry.has_rollback());
        assert_eq!(telemetry.outstanding_leases(), 3);
        assert_eq!(telemetry.memory_epoch(), 9);
    }
}
