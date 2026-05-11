use core::cell::Cell;

use crate::{
    choreography::protocol::{
        ClockNow, MEM_LEASE_NONE, MemBorrow, MemCommit, MemFence, MemGrant, MemRelease, MemRights,
        RandomSeed, StderrChunk, StdinChunk, StdinRequest, StdoutChunk, Wasip1ExitStatus,
        Wasip1StreamChunk,
    },
    kernel::features::{
        WASIP1_PREVIEW1_IMPORTS, WASIP1_PREVIEW1_MODULE, Wasip1HandlerSet, Wasip1ImportName,
        Wasip1Syscall,
    },
    kernel::policy::PolicySlotTable,
};
use hibana::substrate::wire::CodecError;

#[cfg(feature = "profile-host-linux-wasip1-full")]
pub mod host_runner;

const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const WASM_MODULE_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

const IMPORT_MODULE: &[u8] = WASIP1_PREVIEW1_MODULE.as_bytes();
const IMPORT_FD_WRITE: &[u8] = Wasip1ImportName::FdWrite.name().as_bytes();
const IMPORT_FD_READ: &[u8] = Wasip1ImportName::FdRead.name().as_bytes();
const IMPORT_FD_FDSTAT_GET: &[u8] = Wasip1ImportName::FdFdstatGet.name().as_bytes();
const IMPORT_FD_CLOSE: &[u8] = Wasip1ImportName::FdClose.name().as_bytes();
const IMPORT_CLOCK_RES_GET: &[u8] = Wasip1ImportName::ClockResGet.name().as_bytes();
const IMPORT_CLOCK_TIME_GET: &[u8] = Wasip1ImportName::ClockTimeGet.name().as_bytes();
const IMPORT_POLL_ONEOFF: &[u8] = Wasip1ImportName::PollOneoff.name().as_bytes();
const IMPORT_RANDOM_GET: &[u8] = Wasip1ImportName::RandomGet.name().as_bytes();
const IMPORT_PROC_EXIT: &[u8] = Wasip1ImportName::ProcExit.name().as_bytes();
const IMPORT_PROC_RAISE: &[u8] = Wasip1ImportName::ProcRaise.name().as_bytes();
const IMPORT_SCHED_YIELD: &[u8] = Wasip1ImportName::SchedYield.name().as_bytes();
const IMPORT_ARGS_SIZES_GET: &[u8] = Wasip1ImportName::ArgsSizesGet.name().as_bytes();
const IMPORT_ARGS_GET: &[u8] = Wasip1ImportName::ArgsGet.name().as_bytes();
const IMPORT_ENVIRON_SIZES_GET: &[u8] = Wasip1ImportName::EnvironSizesGet.name().as_bytes();
const IMPORT_ENVIRON_GET: &[u8] = Wasip1ImportName::EnvironGet.name().as_bytes();
const DISALLOWED_WASI_PREFIX: &[u8] = &[119, 97, 115, 105, 58];
const DISALLOWED_VERSION_SUFFIX: &[u8] = &[64, 48, 46, 50];

pub const WASIP1_STATIC_ARGS_CAPACITY: usize = 4;
pub const WASIP1_STATIC_ENV_CAPACITY: usize = 4;
pub const WASIP1_STATIC_ARG_BYTES_CAPACITY: usize = 64;
pub const WASIP1_STATIC_ENV_BYTES_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wasip1Error {
    Truncated,
    InvalidModule,
    MissingWasiModule,
    MissingFdWriteImport,
    MissingFdReadImport,
    MissingFdStatImport,
    MissingFdCloseImport,
    MissingClockResGetImport,
    MissingClockTimeGetImport,
    MissingPollOneoffImport,
    MissingRandomGetImport,
    MissingProcExitImport,
    MissingProcRaiseImport,
    MissingSchedYieldImport,
    MissingArgsSizesGetImport,
    MissingArgsGetImport,
    MissingEnvironSizesGetImport,
    MissingEnvironGetImport,
    MissingPathMinimalImport,
    MissingPathFullImport,
    MissingSocketImport,
    UnsupportedImport,
    UnsupportedByProfile,
    StdoutNotFound,
    StderrNotFound,
    StdoutTooLarge,
    StderrTooLarge,
    StdinTooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLeaseError {
    Empty,
    TooLarge,
    EpochMismatch,
    OutOfBounds,
    TableFull,
    InvalidLeaseId,
    UnknownLease,
    RightsMismatch,
    LeaseMismatch,
    LengthExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticArgEnvError {
    TooManyArgs,
    ArgsTooLarge,
    TooManyEnvironmentPairs,
    EnvironmentTooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoFdError {
    TableFull,
    BadFd,
    BadGeneration,
    PermissionDenied,
    WrongResource,
    BadRoute,
    BadSessionGeneration,
    PolicyDenied,
    Revoked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PicoFdRejectionTelemetry {
    bad_fd: u16,
    bad_generation: u16,
    permission_denied: u16,
    wrong_resource: u16,
    bad_route: u16,
    revoked: u16,
    other: u16,
}

impl PicoFdRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            bad_fd: 0,
            bad_generation: 0,
            permission_denied: 0,
            wrong_resource: 0,
            bad_route: 0,
            revoked: 0,
            other: 0,
        }
    }

    pub const fn bad_fd(self) -> u16 {
        self.bad_fd
    }

    pub const fn bad_generation(self) -> u16 {
        self.bad_generation
    }

    pub const fn permission_denied(self) -> u16 {
        self.permission_denied
    }

    pub const fn wrong_resource(self) -> u16 {
        self.wrong_resource
    }

    pub const fn bad_route(self) -> u16 {
        self.bad_route
    }

    pub const fn revoked(self) -> u16 {
        self.revoked
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.bad_fd
            .saturating_add(self.bad_generation)
            .saturating_add(self.permission_denied)
            .saturating_add(self.wrong_resource)
            .saturating_add(self.bad_route)
            .saturating_add(self.revoked)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: PicoFdError) {
        let slot = match error {
            PicoFdError::BadFd => &mut self.bad_fd,
            PicoFdError::BadGeneration | PicoFdError::BadSessionGeneration => {
                &mut self.bad_generation
            }
            PicoFdError::PermissionDenied | PicoFdError::PolicyDenied => {
                &mut self.permission_denied
            }
            PicoFdError::WrongResource => &mut self.wrong_resource,
            PicoFdError::BadRoute => &mut self.bad_route,
            PicoFdError::Revoked => &mut self.revoked,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryLeaseRejectionTelemetry {
    bad_generation: u16,
    invalid_lease: u16,
    rights_mismatch: u16,
    length_exceeded: u16,
    out_of_bounds: u16,
    table_full: u16,
    other: u16,
}

impl MemoryLeaseRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            bad_generation: 0,
            invalid_lease: 0,
            rights_mismatch: 0,
            length_exceeded: 0,
            out_of_bounds: 0,
            table_full: 0,
            other: 0,
        }
    }

    pub const fn bad_generation(self) -> u16 {
        self.bad_generation
    }

    pub const fn invalid_lease(self) -> u16 {
        self.invalid_lease
    }

    pub const fn rights_mismatch(self) -> u16 {
        self.rights_mismatch
    }

    pub const fn length_exceeded(self) -> u16 {
        self.length_exceeded
    }

    pub const fn out_of_bounds(self) -> u16 {
        self.out_of_bounds
    }

    pub const fn table_full(self) -> u16 {
        self.table_full
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.bad_generation
            .saturating_add(self.invalid_lease)
            .saturating_add(self.rights_mismatch)
            .saturating_add(self.length_exceeded)
            .saturating_add(self.out_of_bounds)
            .saturating_add(self.table_full)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: MemoryLeaseError) {
        let slot = match error {
            MemoryLeaseError::EpochMismatch => &mut self.bad_generation,
            MemoryLeaseError::InvalidLeaseId
            | MemoryLeaseError::UnknownLease
            | MemoryLeaseError::LeaseMismatch => &mut self.invalid_lease,
            MemoryLeaseError::RightsMismatch => &mut self.rights_mismatch,
            MemoryLeaseError::LengthExceeded | MemoryLeaseError::TooLarge => {
                &mut self.length_exceeded
            }
            MemoryLeaseError::OutOfBounds => &mut self.out_of_bounds,
            MemoryLeaseError::TableFull => &mut self.table_full,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoFdRights {
    None,
    Read,
    Write,
    ReadWrite,
}

impl PicoFdRights {
    const fn bits(self) -> u8 {
        match self {
            Self::None => 0b00,
            Self::Read => 0b01,
            Self::Write => 0b10,
            Self::ReadWrite => 0b11,
        }
    }

    pub const fn allows(self, required: Self) -> bool {
        self.bits() & required.bits() == required.bits()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoreoResourceKind {
    Stdin,
    Stdout,
    Stderr,
    Gpio,
    Uart,
    Timer,
    LocalSensor,
    LocalActuator,
    NetworkDatagram,
    NetworkStream,
    Telemetry,
    Management,
    Gateway,
    InterruptSubscription,
    PreopenRoot,
    ChoreoObject,
    DirectoryView,
    NetworkListener,
    RemoteObject,
    EphemeralPipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoFdMaterializedView {
    fd: u8,
    generation: u16,
    source: PicoFdViewSource,
    rights: PicoFdRights,
    resource: ChoreoResourceKind,
    choreo_resource_kind: ChoreoResourceKind,
    choreo_object_id: u16,
    choreo_object_generation: u16,
    route: PicoFdRoute,
}

impl PicoFdMaterializedView {
    pub const fn fd(self) -> u8 {
        self.fd
    }

    pub const fn generation(self) -> u16 {
        self.generation
    }

    pub const fn source(self) -> PicoFdViewSource {
        self.source
    }

    pub const fn rights(self) -> PicoFdRights {
        self.rights
    }

    pub const fn resource(self) -> ChoreoResourceKind {
        self.resource
    }

    pub const fn choreo_resource_kind(self) -> ChoreoResourceKind {
        self.choreo_resource_kind
    }

    pub const fn choreo_object_id(self) -> u16 {
        self.choreo_object_id
    }

    pub const fn choreo_object_generation(self) -> u16 {
        self.choreo_object_generation
    }

    pub const fn route(self) -> PicoFdRoute {
        self.route
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoFdViewSource {
    Grant,
    Mint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoFdGrant {
    fd: u8,
    rights: PicoFdRights,
    resource: ChoreoResourceKind,
    lane: u8,
    route_label: u8,
    choreo_object_id: u16,
    target_node: u8,
    target_role: u16,
    session_generation: u16,
    choreo_object_generation: u16,
    policy_slot: u8,
}

impl PicoFdGrant {
    pub const fn new(
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        choreo_object_id: u16,
        route: PicoFdRoute,
    ) -> Self {
        Self {
            fd,
            rights,
            resource,
            lane: route.lane,
            route_label: route.route_label,
            choreo_object_id,
            target_node: route.target_node,
            target_role: route.target_role,
            session_generation: route.session_generation,
            choreo_object_generation: route.session_generation,
            policy_slot: route.policy_slot,
        }
    }

    pub const fn new_mint(
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        choreo_object_id: u16,
        choreo_object_generation: u16,
        route: PicoFdRoute,
    ) -> Self {
        Self {
            fd,
            rights,
            resource,
            lane: route.lane,
            route_label: route.route_label,
            choreo_object_id,
            target_node: route.target_node,
            target_role: route.target_role,
            session_generation: route.session_generation,
            choreo_object_generation,
            policy_slot: route.policy_slot,
        }
    }

    pub const fn fd(self) -> u8 {
        self.fd
    }

    pub const fn rights(self) -> PicoFdRights {
        self.rights
    }

    pub const fn resource(self) -> ChoreoResourceKind {
        self.resource
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoFdControl {
    CapGrant(PicoFdGrant),
    CapMint(PicoFdGrant),
    CapRestrict {
        fd: u8,
        generation: u16,
        rights: PicoFdRights,
    },
    CapRevoke {
        fd: u8,
    },
    CapClose {
        fd: u8,
        generation: u16,
    },
}

impl PicoFdControl {
    pub const fn cap_grant(
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        choreo_object_id: u16,
        route: PicoFdRoute,
    ) -> Self {
        Self::CapGrant(PicoFdGrant::new(
            fd,
            rights,
            resource,
            choreo_object_id,
            route,
        ))
    }

    pub const fn cap_mint(
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        choreo_object_id: u16,
        choreo_object_generation: u16,
        route: PicoFdRoute,
    ) -> Self {
        Self::CapMint(PicoFdGrant::new_mint(
            fd,
            rights,
            resource,
            choreo_object_id,
            choreo_object_generation,
            route,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoFdViewEntry {
    fd: u8,
    generation: u16,
    source: PicoFdViewSource,
    rights: PicoFdRights,
    resource: ChoreoResourceKind,
    lane: u8,
    route_label: u8,
    wait_or_subscription_id: u16,
    target_node: u8,
    target_role: u16,
    session_generation: u16,
    choreo_object_generation: u16,
    policy_slot: u8,
    revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoFdRoute {
    target_node: u8,
    target_role: u16,
    lane: u8,
    route_label: u8,
    session_generation: u16,
    policy_slot: u8,
}

impl PicoFdRoute {
    pub const fn new(
        target_node: u8,
        target_role: u16,
        lane: u8,
        route_label: u8,
        session_generation: u16,
        policy_slot: u8,
    ) -> Self {
        Self {
            target_node,
            target_role,
            lane,
            route_label,
            session_generation,
            policy_slot,
        }
    }

    pub const fn target_node(&self) -> u8 {
        self.target_node
    }

    pub const fn target_role(&self) -> u16 {
        self.target_role
    }

    pub const fn lane(&self) -> u8 {
        self.lane
    }

    pub const fn route_label(&self) -> u8 {
        self.route_label
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn policy_slot(&self) -> u8 {
        self.policy_slot
    }
}

impl PicoFdViewEntry {
    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn source(&self) -> PicoFdViewSource {
        self.source
    }

    pub const fn rights(&self) -> PicoFdRights {
        self.rights
    }

    pub const fn resource(&self) -> ChoreoResourceKind {
        self.resource
    }

    pub const fn lane(&self) -> u8 {
        self.lane
    }

    pub const fn route_label(&self) -> u8 {
        self.route_label
    }

    pub const fn wait_or_subscription_id(&self) -> u16 {
        self.wait_or_subscription_id
    }

    pub const fn target_node(&self) -> u8 {
        self.target_node
    }

    pub const fn target_role(&self) -> u16 {
        self.target_role
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn policy_slot(&self) -> u8 {
        self.policy_slot
    }

    pub const fn route(&self) -> PicoFdRoute {
        PicoFdRoute::new(
            self.target_node,
            self.target_role,
            self.lane,
            self.route_label,
            self.session_generation,
            self.policy_slot,
        )
    }

    pub const fn is_revoked(&self) -> bool {
        self.revoked
    }

    pub const fn materialized_rights(&self) -> PicoFdRights {
        self.rights
    }

    pub const fn choreo_resource_kind(&self) -> ChoreoResourceKind {
        self.resource
    }

    pub const fn choreo_object_id(&self) -> u16 {
        self.wait_or_subscription_id
    }

    pub const fn choreo_object_generation(&self) -> u16 {
        self.choreo_object_generation
    }

    pub const fn materialized_view(&self) -> PicoFdMaterializedView {
        PicoFdMaterializedView {
            fd: self.fd,
            generation: self.generation,
            source: self.source,
            rights: self.rights,
            resource: self.resource,
            choreo_resource_kind: self.choreo_resource_kind(),
            choreo_object_id: self.choreo_object_id(),
            choreo_object_generation: self.choreo_object_generation(),
            route: self.route(),
        }
    }
}

pub struct PicoFdView<const N: usize> {
    slots: [Option<PicoFdViewEntry>; N],
    next_generation: u16,
    rejection_telemetry: Cell<PicoFdRejectionTelemetry>,
}

impl<const N: usize> PicoFdView<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            next_generation: 1,
            rejection_telemetry: Cell::new(PicoFdRejectionTelemetry::new()),
        }
    }

    pub fn rejection_telemetry(&self) -> PicoFdRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn apply_local_cap_grant(
        &mut self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        lane: u8,
        route_label: u8,
        wait_or_subscription_id: u16,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        let route = PicoFdRoute::new(0, 0, lane, route_label, 0, 0);
        self.apply_cap_grant(fd, rights, resource, wait_or_subscription_id, route)
    }

    pub fn apply_cap_grant(
        &mut self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        wait_or_subscription_id: u16,
        route: PicoFdRoute,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        self.apply_control(PicoFdControl::cap_grant(
            fd,
            rights,
            resource,
            wait_or_subscription_id,
            route,
        ))
    }

    pub fn apply_cap_mint(
        &mut self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        choreo_object_id: u16,
        choreo_object_generation: u16,
        route: PicoFdRoute,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        self.apply_control(PicoFdControl::cap_mint(
            fd,
            rights,
            resource,
            choreo_object_id,
            choreo_object_generation,
            route,
        ))
    }

    pub fn apply_control(
        &mut self,
        control: PicoFdControl,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        match control {
            PicoFdControl::CapGrant(grant) => self.materialize(grant, PicoFdViewSource::Grant),
            PicoFdControl::CapMint(grant) => self.materialize(grant, PicoFdViewSource::Mint),
            PicoFdControl::CapRestrict {
                fd,
                generation,
                rights,
            } => self.restrict(fd, generation, rights),
            PicoFdControl::CapRevoke { fd } => {
                self.revoke_fd(fd)?;
                self.find(fd)
            }
            PicoFdControl::CapClose { fd, generation } => self.close(fd, generation),
        }
    }

    fn materialize(
        &mut self,
        grant: PicoFdGrant,
        source: PicoFdViewSource,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        if self
            .slots
            .iter()
            .flatten()
            .any(|entry| entry.fd == grant.fd && !entry.revoked)
        {
            return Err(self.record_rejection(PicoFdError::BadFd));
        }

        let generation = self.bump_generation();
        let entry = PicoFdViewEntry {
            fd: grant.fd,
            generation,
            source,
            rights: grant.rights,
            resource: grant.resource,
            lane: grant.lane,
            route_label: grant.route_label,
            wait_or_subscription_id: grant.choreo_object_id,
            target_node: grant.target_node,
            target_role: grant.target_role,
            session_generation: grant.session_generation,
            choreo_object_generation: grant.choreo_object_generation,
            policy_slot: grant.policy_slot,
            revoked: false,
        };

        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|existing| existing.fd == grant.fd))
        {
            *slot = Some(entry);
            return Ok(entry);
        }

        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return Err(self.record_rejection(PicoFdError::TableFull));
        };
        *slot = Some(entry);
        Ok(entry)
    }

    pub fn resolve_current(
        &self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        self.record_result(
            self.find(fd)
                .and_then(|entry| self.validate(entry, entry.generation, rights, resource)),
        )
    }

    pub fn resolve(
        &self,
        fd: u8,
        generation: u16,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        self.record_result(
            self.find(fd)
                .and_then(|entry| self.validate(entry, generation, rights, resource)),
        )
    }

    pub fn resolve_routed_current(
        &self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        route: PicoFdRoute,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        let entry = self.resolve_current(fd, rights, resource)?;
        if let Err(error) = Self::validate_route(entry, route) {
            return Err(self.record_rejection(error));
        }
        Ok(entry)
    }

    pub fn resolve_routed_authorized_current<const P: usize>(
        &self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        route: PicoFdRoute,
        policy: &PolicySlotTable<P>,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        if !policy.is_allowed(route.policy_slot()) {
            return Err(self.record_rejection(PicoFdError::PolicyDenied));
        }
        self.resolve_routed_current(fd, rights, resource, route)
    }

    pub fn resolve_routed(
        &self,
        fd: u8,
        generation: u16,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        route: PicoFdRoute,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        let entry = self.resolve(fd, generation, rights, resource)?;
        if let Err(error) = Self::validate_route(entry, route) {
            return Err(self.record_rejection(error));
        }
        Ok(entry)
    }

    pub fn resolve_routed_authorized<const P: usize>(
        &self,
        fd: u8,
        generation: u16,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        route: PicoFdRoute,
        policy: &PolicySlotTable<P>,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        if !policy.is_allowed(route.policy_slot()) {
            return Err(self.record_rejection(PicoFdError::PolicyDenied));
        }
        self.resolve_routed(fd, generation, rights, resource, route)
    }

    pub fn close_current(&mut self, fd: u8) -> Result<PicoFdViewEntry, PicoFdError> {
        match self.find(fd) {
            Ok(entry) => self.apply_control(PicoFdControl::CapClose {
                fd,
                generation: entry.generation,
            }),
            Err(error) => Err(self.record_rejection(error)),
        }
    }

    pub fn close(&mut self, fd: u8, generation: u16) -> Result<PicoFdViewEntry, PicoFdError> {
        let result = match self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|entry| entry.fd == fd))
        {
            Some(slot) => {
                let mut entry = (*slot).expect("matched fd entry must exist");
                if entry.generation != generation {
                    Err(PicoFdError::BadGeneration)
                } else if entry.revoked {
                    Err(PicoFdError::Revoked)
                } else {
                    entry.revoked = true;
                    *slot = Some(entry);
                    Ok(entry)
                }
            }
            None => Err(PicoFdError::BadFd),
        };
        self.record_result(result)
    }

    pub fn revoke_fd(&mut self, fd: u8) -> Result<(), PicoFdError> {
        let result = match self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|entry| entry.fd == fd))
        {
            Some(slot) => {
                let mut entry = (*slot).expect("matched fd entry must exist");
                entry.revoked = true;
                *slot = Some(entry);
                Ok(())
            }
            None => Err(PicoFdError::BadFd),
        };
        self.record_result(result)
    }

    pub fn restrict_current(
        &mut self,
        fd: u8,
        rights: PicoFdRights,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        match self.find(fd) {
            Ok(entry) => self.apply_control(PicoFdControl::CapRestrict {
                fd,
                generation: entry.generation,
                rights,
            }),
            Err(error) => Err(self.record_rejection(error)),
        }
    }

    pub fn restrict(
        &mut self,
        fd: u8,
        generation: u16,
        rights: PicoFdRights,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        let result = match self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|entry| entry.fd == fd))
        {
            Some(slot) => {
                let mut entry = (*slot).expect("matched fd entry must exist");
                if entry.generation != generation {
                    Err(PicoFdError::BadGeneration)
                } else if entry.revoked {
                    Err(PicoFdError::Revoked)
                } else if !entry.rights.allows(rights) {
                    Err(PicoFdError::PermissionDenied)
                } else {
                    entry.rights = rights;
                    *slot = Some(entry);
                    Ok(entry)
                }
            }
            None => Err(PicoFdError::BadFd),
        };
        self.record_result(result)
    }

    pub fn has_active(&self) -> bool {
        self.slots.iter().flatten().any(|entry| !entry.revoked)
    }

    pub fn active_count(&self) -> usize {
        self.slots
            .iter()
            .flatten()
            .filter(|entry| !entry.revoked)
            .count()
    }

    pub fn fence_all(&mut self) -> usize {
        let mut revoked = 0usize;
        for index in 0..self.slots.len() {
            let Some(mut entry) = self.slots[index] else {
                continue;
            };
            if entry.revoked {
                continue;
            }
            entry.revoked = true;
            entry.generation = self.bump_generation();
            self.slots[index] = Some(entry);
            revoked += 1;
        }
        revoked
    }

    fn find(&self, fd: u8) -> Result<PicoFdViewEntry, PicoFdError> {
        self.slots
            .iter()
            .flatten()
            .find(|entry| entry.fd == fd)
            .copied()
            .ok_or(PicoFdError::BadFd)
    }

    fn validate(
        &self,
        entry: PicoFdViewEntry,
        generation: u16,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
    ) -> Result<PicoFdViewEntry, PicoFdError> {
        if entry.generation != generation {
            return Err(PicoFdError::BadGeneration);
        }
        if entry.revoked {
            return Err(PicoFdError::Revoked);
        }
        if entry.resource != resource {
            return Err(PicoFdError::WrongResource);
        }
        if !entry.rights.allows(rights) {
            return Err(PicoFdError::PermissionDenied);
        }
        Ok(entry)
    }

    fn validate_route(entry: PicoFdViewEntry, route: PicoFdRoute) -> Result<(), PicoFdError> {
        if entry.session_generation != route.session_generation {
            return Err(PicoFdError::BadSessionGeneration);
        }
        if entry.target_node != route.target_node
            || entry.target_role != route.target_role
            || entry.lane != route.lane
            || entry.route_label != route.route_label
            || entry.policy_slot != route.policy_slot
        {
            return Err(PicoFdError::BadRoute);
        }
        Ok(())
    }

    fn bump_generation(&mut self) -> u16 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        if self.next_generation == 0 {
            self.next_generation = 1;
        }
        generation
    }

    fn record_result<T>(&self, result: Result<T, PicoFdError>) -> Result<T, PicoFdError> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(self.record_rejection(error)),
        }
    }

    fn record_rejection(&self, error: PicoFdError) -> PicoFdError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }
}

impl<const N: usize> Default for PicoFdView<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticArgEnvSnapshot<'a, const ARGS: usize, const ENV: usize> {
    args: [&'a [u8]; ARGS],
    arg_count: usize,
    environment: [(&'a [u8], &'a [u8]); ENV],
    environment_count: usize,
}

pub type Wasip1StaticArgEnv<'a> =
    StaticArgEnvSnapshot<'a, WASIP1_STATIC_ARGS_CAPACITY, WASIP1_STATIC_ENV_CAPACITY>;

impl<'a, const ARGS: usize, const ENV: usize> StaticArgEnvSnapshot<'a, ARGS, ENV> {
    pub const fn empty() -> Self {
        Self {
            args: [&[]; ARGS],
            arg_count: 0,
            environment: [(&[], &[]); ENV],
            environment_count: 0,
        }
    }

    pub fn new(
        args: &[&'a [u8]],
        environment: &[(&'a [u8], &'a [u8])],
    ) -> Result<Self, StaticArgEnvError> {
        if args.len() > ARGS {
            return Err(StaticArgEnvError::TooManyArgs);
        }
        if environment.len() > ENV {
            return Err(StaticArgEnvError::TooManyEnvironmentPairs);
        }

        let mut out = Self::empty();
        let mut arg_bytes = 0usize;
        for (index, arg) in args.iter().enumerate() {
            arg_bytes = arg_bytes
                .checked_add(arg.len())
                .ok_or(StaticArgEnvError::ArgsTooLarge)?;
            if arg_bytes > WASIP1_STATIC_ARG_BYTES_CAPACITY {
                return Err(StaticArgEnvError::ArgsTooLarge);
            }
            out.args[index] = arg;
        }
        out.arg_count = args.len();

        let mut environment_bytes = 0usize;
        for (index, pair) in environment.iter().enumerate() {
            environment_bytes = environment_bytes
                .checked_add(pair.0.len())
                .and_then(|len| len.checked_add(pair.1.len()))
                .and_then(|len| len.checked_add(1))
                .ok_or(StaticArgEnvError::EnvironmentTooLarge)?;
            if environment_bytes > WASIP1_STATIC_ENV_BYTES_CAPACITY {
                return Err(StaticArgEnvError::EnvironmentTooLarge);
            }
            out.environment[index] = *pair;
        }
        out.environment_count = environment.len();
        Ok(out)
    }

    pub const fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub const fn environment_count(&self) -> usize {
        self.environment_count
    }

    pub fn args(&self) -> &[&'a [u8]] {
        &self.args[..self.arg_count]
    }

    pub fn environment(&self) -> &[(&'a [u8], &'a [u8])] {
        &self.environment[..self.environment_count]
    }
}

pub struct MemoryLeaseTable<const N: usize> {
    memory_len: u32,
    epoch: u32,
    slots: [Option<MemGrant>; N],
    rejection_telemetry: Cell<MemoryLeaseRejectionTelemetry>,
}

impl<const N: usize> MemoryLeaseTable<N> {
    pub const fn new(memory_len: u32, epoch: u32) -> Self {
        Self {
            memory_len,
            epoch,
            slots: [None; N],
            rejection_telemetry: Cell::new(MemoryLeaseRejectionTelemetry::new()),
        }
    }

    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    pub fn has_outstanding_leases(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    pub fn outstanding_lease_count(&self) -> u8 {
        self.slots.iter().filter(|slot| slot.is_some()).count() as u8
    }

    pub fn rejection_telemetry(&self) -> MemoryLeaseRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn grant_read(&mut self, borrow: MemBorrow) -> Result<MemGrant, MemoryLeaseError> {
        self.grant(borrow, MemRights::Read)
    }

    pub fn grant_write(&mut self, borrow: MemBorrow) -> Result<MemGrant, MemoryLeaseError> {
        self.grant(borrow, MemRights::Write)
    }

    pub fn validate_read_chunk(&self, chunk: &Wasip1StreamChunk) -> Result<(), MemoryLeaseError> {
        let grant = self.get(chunk.lease_id())?;
        if grant.rights() != MemRights::Read {
            return Err(self.record_rejection(MemoryLeaseError::RightsMismatch));
        }
        if chunk.len() > grant.len() as usize {
            return Err(self.record_rejection(MemoryLeaseError::LengthExceeded));
        }
        Ok(())
    }

    pub fn validate_write_request(&self, request: &StdinRequest) -> Result<(), MemoryLeaseError> {
        let grant = self.get(request.lease_id())?;
        if grant.rights() != MemRights::Write {
            return Err(self.record_rejection(MemoryLeaseError::RightsMismatch));
        }
        if request.max_len() > grant.len() {
            return Err(self.record_rejection(MemoryLeaseError::LengthExceeded));
        }
        Ok(())
    }

    pub fn validate_write_chunk(&self, chunk: &Wasip1StreamChunk) -> Result<(), MemoryLeaseError> {
        let grant = self.get(chunk.lease_id())?;
        if grant.rights() != MemRights::Write {
            return Err(self.record_rejection(MemoryLeaseError::RightsMismatch));
        }
        if chunk.len() > grant.len() as usize {
            return Err(self.record_rejection(MemoryLeaseError::LengthExceeded));
        }
        Ok(())
    }

    pub fn commit(&self, commit: MemCommit) -> Result<(), MemoryLeaseError> {
        let grant = self.get(commit.lease_id())?;
        if grant.rights() != MemRights::Write {
            return Err(self.record_rejection(MemoryLeaseError::RightsMismatch));
        }
        if commit.written() > grant.len() {
            return Err(self.record_rejection(MemoryLeaseError::LengthExceeded));
        }
        Ok(())
    }

    pub fn release(&mut self, release: MemRelease) -> Result<MemGrant, MemoryLeaseError> {
        let lease_id = release.lease_id();
        if lease_id == MEM_LEASE_NONE {
            return Err(self.record_rejection(MemoryLeaseError::InvalidLeaseId));
        }
        for slot in &mut self.slots {
            if let Some(grant) = slot {
                if grant.lease_id() == lease_id {
                    let out = *grant;
                    *slot = None;
                    return Ok(out);
                }
            }
        }
        Err(self.record_rejection(MemoryLeaseError::UnknownLease))
    }

    pub fn fence(&mut self, fence: MemFence) {
        for slot in &mut self.slots {
            *slot = None;
        }
        self.epoch = fence.new_epoch();
    }

    fn grant(
        &mut self,
        borrow: MemBorrow,
        rights: MemRights,
    ) -> Result<MemGrant, MemoryLeaseError> {
        if borrow.len() == 0 {
            return Err(self.record_rejection(MemoryLeaseError::Empty));
        }
        if borrow.len() as usize > crate::choreography::protocol::WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(self.record_rejection(MemoryLeaseError::TooLarge));
        }
        if borrow.epoch() != self.epoch {
            return Err(self.record_rejection(MemoryLeaseError::EpochMismatch));
        }
        let end = borrow
            .ptr()
            .checked_add(borrow.len() as u32)
            .ok_or_else(|| self.record_rejection(MemoryLeaseError::OutOfBounds))?;
        if end > self.memory_len {
            return Err(self.record_rejection(MemoryLeaseError::OutOfBounds));
        }
        let Some(slot_index) = self.slots.iter().position(|slot| slot.is_none()) else {
            return Err(self.record_rejection(MemoryLeaseError::TableFull));
        };
        let lease_id = match self.allocate_lease_id() {
            Ok(lease_id) => lease_id,
            Err(error) => return Err(self.record_rejection(error)),
        };
        let grant = MemGrant::new(lease_id, borrow.ptr(), borrow.len(), borrow.epoch(), rights);
        self.slots[slot_index] = Some(grant);
        Ok(grant)
    }

    fn allocate_lease_id(&mut self) -> Result<u8, MemoryLeaseError> {
        for candidate in 1..=u8::MAX {
            if candidate != MEM_LEASE_NONE
                && !self
                    .slots
                    .iter()
                    .flatten()
                    .any(|grant| grant.lease_id() == candidate)
            {
                return Ok(candidate);
            }
        }
        Err(MemoryLeaseError::TableFull)
    }

    fn get(&self, lease_id: u8) -> Result<MemGrant, MemoryLeaseError> {
        if lease_id == MEM_LEASE_NONE {
            return Err(self.record_rejection(MemoryLeaseError::InvalidLeaseId));
        }
        for grant in self.slots.iter().flatten() {
            if grant.lease_id() == lease_id {
                return Ok(*grant);
            }
        }
        Err(self.record_rejection(MemoryLeaseError::UnknownLease))
    }

    fn record_rejection(&self, error: MemoryLeaseError) -> MemoryLeaseError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }
}

pub struct Wasip1Module<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1Module<'a> {
    pub fn install(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        validate_module(bytes)?;
        validate_supported_import_set(bytes)?;
        Ok(Self { bytes })
    }

    pub fn install_with_handlers(
        bytes: &'a [u8],
        handlers: Wasip1HandlerSet,
    ) -> Result<Self, Wasip1Error> {
        validate_module(bytes)?;
        validate_supported_import_set(bytes)?;
        validate_imports_against_handlers(bytes, handlers)?;
        Ok(Self { bytes })
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wasip1ImportSummary {
    import_count: u16,
    preview1_import_count: u16,
}

impl Wasip1ImportSummary {
    pub fn parse_strict_preview1(bytes: &[u8]) -> Result<Self, Wasip1Error> {
        validate_module_header(bytes)?;
        parse_import_section(bytes)
    }

    pub const fn import_count(&self) -> u16 {
        self.import_count
    }

    pub const fn preview1_import_count(&self) -> u16 {
        self.preview1_import_count
    }
}

pub struct Wasip1FullSubsetModule<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1FullSubsetModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        require_import(bytes, IMPORT_FD_READ, Wasip1Error::MissingFdReadImport)?;
        require_import(
            bytes,
            IMPORT_FD_FDSTAT_GET,
            Wasip1Error::MissingFdStatImport,
        )?;
        require_import(bytes, IMPORT_FD_CLOSE, Wasip1Error::MissingFdCloseImport)?;
        require_import(
            bytes,
            IMPORT_CLOCK_RES_GET,
            Wasip1Error::MissingClockResGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_CLOCK_TIME_GET,
            Wasip1Error::MissingClockTimeGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_POLL_ONEOFF,
            Wasip1Error::MissingPollOneoffImport,
        )?;
        require_import(
            bytes,
            IMPORT_RANDOM_GET,
            Wasip1Error::MissingRandomGetImport,
        )?;
        require_import(bytes, IMPORT_PROC_EXIT, Wasip1Error::MissingProcExitImport)?;
        require_import(
            bytes,
            IMPORT_PROC_RAISE,
            Wasip1Error::MissingProcRaiseImport,
        )?;
        require_import(
            bytes,
            IMPORT_SCHED_YIELD,
            Wasip1Error::MissingSchedYieldImport,
        )?;
        require_import(
            bytes,
            IMPORT_ARGS_SIZES_GET,
            Wasip1Error::MissingArgsSizesGetImport,
        )?;
        require_import(bytes, IMPORT_ARGS_GET, Wasip1Error::MissingArgsGetImport)?;
        require_import(
            bytes,
            IMPORT_ENVIRON_SIZES_GET,
            Wasip1Error::MissingEnvironSizesGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_ENVIRON_GET,
            Wasip1Error::MissingEnvironGetImport,
        )?;
        require_path_minimal_imports(bytes)?;
        require_path_full_imports(bytes)?;
        require_socket_imports(bytes)?;
        Ok(Self { bytes })
    }

    pub fn parse_with_handlers(
        bytes: &'a [u8],
        handlers: Wasip1HandlerSet,
    ) -> Result<Self, Wasip1Error> {
        Wasip1Module::install_with_handlers(bytes, handlers)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        require_import(bytes, IMPORT_FD_READ, Wasip1Error::MissingFdReadImport)?;
        require_import(
            bytes,
            IMPORT_FD_FDSTAT_GET,
            Wasip1Error::MissingFdStatImport,
        )?;
        require_import(bytes, IMPORT_FD_CLOSE, Wasip1Error::MissingFdCloseImport)?;
        require_import(
            bytes,
            IMPORT_CLOCK_RES_GET,
            Wasip1Error::MissingClockResGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_CLOCK_TIME_GET,
            Wasip1Error::MissingClockTimeGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_POLL_ONEOFF,
            Wasip1Error::MissingPollOneoffImport,
        )?;
        require_import(
            bytes,
            IMPORT_RANDOM_GET,
            Wasip1Error::MissingRandomGetImport,
        )?;
        require_import(bytes, IMPORT_PROC_EXIT, Wasip1Error::MissingProcExitImport)?;
        require_import(
            bytes,
            IMPORT_PROC_RAISE,
            Wasip1Error::MissingProcRaiseImport,
        )?;
        require_import(
            bytes,
            IMPORT_SCHED_YIELD,
            Wasip1Error::MissingSchedYieldImport,
        )?;
        require_import(
            bytes,
            IMPORT_ARGS_SIZES_GET,
            Wasip1Error::MissingArgsSizesGetImport,
        )?;
        require_import(bytes, IMPORT_ARGS_GET, Wasip1Error::MissingArgsGetImport)?;
        require_import(
            bytes,
            IMPORT_ENVIRON_SIZES_GET,
            Wasip1Error::MissingEnvironSizesGetImport,
        )?;
        require_import(
            bytes,
            IMPORT_ENVIRON_GET,
            Wasip1Error::MissingEnvironGetImport,
        )?;
        require_path_minimal_imports(bytes)?;
        require_path_full_imports(bytes)?;
        require_socket_imports(bytes)?;
        Ok(Self { bytes })
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

pub struct Wasip1FdWriteModule<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1FdWriteModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        Ok(Self { bytes })
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

pub struct Wasip1LedBlinkModule<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1LedBlinkModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        require_import(
            bytes,
            IMPORT_POLL_ONEOFF,
            Wasip1Error::MissingPollOneoffImport,
        )?;
        Ok(Self { bytes })
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

pub struct Wasip1StdoutModule<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1StdoutModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        Ok(Self { bytes })
    }

    pub fn stdout_chunk_for(&self, marker: &[u8]) -> Result<StdoutChunk, Wasip1Error> {
        chunk_for(
            self.bytes,
            marker,
            Wasip1Error::StdoutNotFound,
            Wasip1Error::StdoutTooLarge,
        )
    }
}

pub struct Wasip1StderrModule<'a> {
    bytes: &'a [u8],
}

impl<'a> Wasip1StderrModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_WRITE, Wasip1Error::MissingFdWriteImport)?;
        Ok(Self { bytes })
    }

    pub fn stderr_chunk_for(&self, marker: &[u8]) -> Result<StderrChunk, Wasip1Error> {
        chunk_for(
            self.bytes,
            marker,
            Wasip1Error::StderrNotFound,
            Wasip1Error::StderrTooLarge,
        )
    }
}

pub struct Wasip1StdinModule<'a> {
    _bytes: &'a [u8],
}

impl<'a> Wasip1StdinModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_FD_READ, Wasip1Error::MissingFdReadImport)?;
        Ok(Self { _bytes: bytes })
    }

    pub fn stdin_request_for(&self, max_len: u8) -> Result<StdinRequest, Wasip1Error> {
        StdinRequest::new(max_len)
            .map_err(|error| map_chunk_error(error, Wasip1Error::StdinTooLarge))
    }

    pub fn stdin_chunk_for(&self, input: &[u8]) -> Result<StdinChunk, Wasip1Error> {
        StdinChunk::new(input).map_err(|error| map_chunk_error(error, Wasip1Error::StdinTooLarge))
    }
}

pub struct Wasip1ClockModule<'a> {
    _bytes: &'a [u8],
}

impl<'a> Wasip1ClockModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(
            bytes,
            IMPORT_CLOCK_TIME_GET,
            Wasip1Error::MissingClockTimeGetImport,
        )?;
        Ok(Self { _bytes: bytes })
    }

    pub const fn clock_now(&self, nanos: u64) -> ClockNow {
        ClockNow::new(nanos)
    }
}

pub struct Wasip1RandomModule<'a> {
    _bytes: &'a [u8],
}

impl<'a> Wasip1RandomModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(
            bytes,
            IMPORT_RANDOM_GET,
            Wasip1Error::MissingRandomGetImport,
        )?;
        Ok(Self { _bytes: bytes })
    }

    pub const fn random_seed(&self, lo: u64, hi: u64) -> RandomSeed {
        RandomSeed::new(lo, hi)
    }
}

pub struct Wasip1ExitModule<'a> {
    _bytes: &'a [u8],
}

impl<'a> Wasip1ExitModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        require_import(bytes, IMPORT_PROC_EXIT, Wasip1Error::MissingProcExitImport)?;
        Ok(Self { _bytes: bytes })
    }

    pub const fn exit_status(&self, code: u8) -> Wasip1ExitStatus {
        Wasip1ExitStatus::new(code)
    }
}

pub struct Wasip1EnvironmentModule<'a> {
    _bytes: &'a [u8],
    has_environ_get: bool,
    has_args_get: bool,
}

impl<'a> Wasip1EnvironmentModule<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Wasip1Error> {
        Wasip1Module::install(bytes)?;
        let has_environ_get = find_bytes(bytes, IMPORT_ENVIRON_GET).is_some();
        let has_args_get = find_bytes(bytes, IMPORT_ARGS_GET).is_some();
        if !has_environ_get && !has_args_get {
            return Err(Wasip1Error::MissingEnvironGetImport);
        }
        Ok(Self {
            _bytes: bytes,
            has_environ_get,
            has_args_get,
        })
    }

    pub const fn has_environ_get(&self) -> bool {
        self.has_environ_get
    }

    pub const fn has_args_get(&self) -> bool {
        self.has_args_get
    }

    pub const fn static_snapshot(&self) -> Wasip1StaticArgEnv<'static> {
        Wasip1StaticArgEnv::empty()
    }
}

fn validate_module(bytes: &[u8]) -> Result<(), Wasip1Error> {
    validate_module_header(bytes)?;
    require_import(bytes, IMPORT_MODULE, Wasip1Error::MissingWasiModule)?;
    Ok(())
}

fn validate_module_header(bytes: &[u8]) -> Result<(), Wasip1Error> {
    let header = bytes.get(..8).ok_or(Wasip1Error::Truncated)?;
    if header[..4] != WASM_MAGIC || header[4..8] != WASM_MODULE_VERSION {
        return Err(Wasip1Error::InvalidModule);
    }
    Ok(())
}

fn validate_supported_import_set(bytes: &[u8]) -> Result<(), Wasip1Error> {
    if find_bytes(bytes, DISALLOWED_WASI_PREFIX).is_some()
        || find_bytes(bytes, DISALLOWED_VERSION_SUFFIX).is_some()
    {
        return Err(Wasip1Error::UnsupportedImport);
    }
    Ok(())
}

fn validate_imports_against_handlers(
    bytes: &[u8],
    handlers: Wasip1HandlerSet,
) -> Result<(), Wasip1Error> {
    for import in WASIP1_PREVIEW1_IMPORTS {
        if find_bytes(bytes, import.name().as_bytes()).is_some()
            && !handlers.supports(import.syscall())
        {
            return Err(Wasip1Error::UnsupportedByProfile);
        }
    }
    Ok(())
}

fn require_path_minimal_imports(bytes: &[u8]) -> Result<(), Wasip1Error> {
    require_imports_for_syscall(
        bytes,
        Wasip1Syscall::PathMinimal,
        Wasip1Error::MissingPathMinimalImport,
    )
}

fn require_path_full_imports(bytes: &[u8]) -> Result<(), Wasip1Error> {
    require_imports_for_syscall(
        bytes,
        Wasip1Syscall::PathFull,
        Wasip1Error::MissingPathFullImport,
    )
}

fn require_socket_imports(bytes: &[u8]) -> Result<(), Wasip1Error> {
    require_imports_for_syscall(
        bytes,
        Wasip1Syscall::NetworkObject,
        Wasip1Error::MissingSocketImport,
    )
}

fn require_imports_for_syscall(
    bytes: &[u8],
    syscall: Wasip1Syscall,
    error: Wasip1Error,
) -> Result<(), Wasip1Error> {
    for import in WASIP1_PREVIEW1_IMPORTS {
        if import.syscall() == syscall {
            require_import(bytes, import.name().as_bytes(), error)?;
        }
    }
    Ok(())
}

fn parse_import_section(bytes: &[u8]) -> Result<Wasip1ImportSummary, Wasip1Error> {
    let mut reader = WasmByteReader::new(bytes);
    reader.skip_exact(8)?;
    while !reader.is_empty() {
        let section_id = reader.read_u8()?;
        let section_len = reader.read_var_u32()? as usize;
        let section = reader.read_bytes(section_len)?;
        if section_id == 2 {
            return parse_import_entries(section);
        }
    }
    Err(Wasip1Error::MissingWasiModule)
}

fn parse_import_entries(bytes: &[u8]) -> Result<Wasip1ImportSummary, Wasip1Error> {
    let mut reader = WasmByteReader::new(bytes);
    let import_count = reader.read_var_u32()?;
    if import_count > u16::MAX as u32 {
        return Err(Wasip1Error::UnsupportedImport);
    }

    let mut preview1_import_count = 0u16;
    for _ in 0..import_count {
        let module = reader.read_name()?;
        let _name = reader.read_name()?;
        let kind = reader.read_u8()?;
        match kind {
            0 => {
                reader.read_var_u32()?;
            }
            1 => {
                reader.skip_exact(1)?;
                skip_limits(&mut reader)?;
            }
            2 => skip_limits(&mut reader)?,
            3 => {
                reader.skip_exact(2)?;
            }
            _ => return Err(Wasip1Error::UnsupportedImport),
        }

        if module != IMPORT_MODULE {
            return Err(Wasip1Error::UnsupportedImport);
        }
        preview1_import_count = preview1_import_count
            .checked_add(1)
            .ok_or(Wasip1Error::UnsupportedImport)?;
    }
    if !reader.is_empty() {
        return Err(Wasip1Error::InvalidModule);
    }
    if preview1_import_count == 0 {
        return Err(Wasip1Error::MissingWasiModule);
    }
    Ok(Wasip1ImportSummary {
        import_count: import_count as u16,
        preview1_import_count,
    })
}

fn skip_limits(reader: &mut WasmByteReader<'_>) -> Result<(), Wasip1Error> {
    let flags = reader.read_u8()?;
    reader.read_var_u32()?;
    if flags & 0x01 != 0 {
        reader.read_var_u32()?;
    }
    if flags & !0x03 != 0 {
        return Err(Wasip1Error::UnsupportedImport);
    }
    Ok(())
}

fn require_import(bytes: &[u8], import: &[u8], missing: Wasip1Error) -> Result<(), Wasip1Error> {
    if find_bytes(bytes, import).is_none() {
        return Err(missing);
    }
    Ok(())
}

fn chunk_for(
    bytes: &[u8],
    marker: &[u8],
    not_found: Wasip1Error,
    too_large: Wasip1Error,
) -> Result<Wasip1StreamChunk, Wasip1Error> {
    let start = find_bytes(bytes, marker).ok_or(not_found)?;
    let end = start.checked_add(marker.len()).ok_or(too_large)?;
    let bytes = &bytes[start..end];
    Wasip1StreamChunk::new(bytes).map_err(|error| map_chunk_error(error, too_large))
}

fn map_chunk_error(error: CodecError, too_large: Wasip1Error) -> Wasip1Error {
    match error {
        CodecError::Invalid(_) => too_large,
        CodecError::Truncated => Wasip1Error::Truncated,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let last = haystack.len() - needle.len();
    let mut idx = 0usize;
    while idx <= last {
        if &haystack[idx..idx + needle.len()] == needle {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

struct WasmByteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> WasmByteReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos == self.bytes.len()
    }

    fn read_u8(&mut self) -> Result<u8, Wasip1Error> {
        let byte = *self.bytes.get(self.pos).ok_or(Wasip1Error::Truncated)?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], Wasip1Error> {
        let end = self.pos.checked_add(len).ok_or(Wasip1Error::Truncated)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(Wasip1Error::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn read_name(&mut self) -> Result<&'a [u8], Wasip1Error> {
        let len = self.read_var_u32()? as usize;
        self.read_bytes(len)
    }

    fn read_var_u32(&mut self) -> Result<u32, Wasip1Error> {
        let mut result = 0u32;
        let mut shift = 0u32;
        loop {
            if shift >= 35 {
                return Err(Wasip1Error::InvalidModule);
            }
            let byte = self.read_u8()?;
            result |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }

    fn skip_exact(&mut self, len: usize) -> Result<(), Wasip1Error> {
        self.read_bytes(len).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChoreoResourceKind, MemoryLeaseError, MemoryLeaseTable, PicoFdControl, PicoFdError,
        PicoFdRights, PicoFdRoute, PicoFdView, PicoFdViewSource, StaticArgEnvError,
        WASIP1_STATIC_ARG_BYTES_CAPACITY, WASIP1_STATIC_ENV_BYTES_CAPACITY, Wasip1ClockModule,
        Wasip1EnvironmentModule, Wasip1Error, Wasip1ExitModule, Wasip1FullSubsetModule,
        Wasip1ImportSummary, Wasip1Module, Wasip1RandomModule, Wasip1StaticArgEnv,
        Wasip1StderrModule, Wasip1StdinModule, Wasip1StdoutModule,
    };
    use crate::choreography::protocol::{
        MemBorrow, MemCommit, MemFence, MemFenceReason, MemRelease, MemRights, StdinRequest,
        StdoutChunk,
    };
    use crate::kernel::features::Wasip1HandlerSet;

    const TEST_STDOUT_TEXT: &[u8] = b"hibana wasip1 stdout\n";
    const TEST_STDERR_TEXT: &[u8] = b"hibana wasip1 stderr\n";
    const TEST_STDIN_INPUT: &[u8] = b"hibana stdin\n";
    const TEST_STDIN_MAX_LEN: u8 = 24;
    const TEST_CLOCK_NANOS: u64 = 123_456_789;
    const TEST_RANDOM_SEED_LO: u64 = 0x4849_4241_5241_4e44;
    const TEST_RANDOM_SEED_HI: u64 = 0x5345_4544_0000_0001;
    const TEST_EXIT_CODE: u8 = 7;

    static WASIP1_STDOUT_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stdout\n";
    static WASIP1_STDERR_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stderr\n";
    static WASIP1_STDIN_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_read environ_get hibana stdin\n";
    static WASIP1_CLOCK_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 clock_time_get environ_get";
    static WASIP1_RANDOM_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 random_get environ_get";
    static WASIP1_EXIT_GUEST: &[u8] =
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 proc_exit environ_get";
    static WASIP1_FULL_SUBSET_GUEST: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get fd_prestat_get fd_prestat_dir_name fd_filestat_get fd_readdir path_open path_filestat_get path_readlink path_create_directory path_remove_directory path_unlink_file path_rename fd_advise fd_allocate fd_datasync fd_fdstat_set_flags fd_fdstat_set_rights fd_filestat_set_size fd_filestat_set_times fd_pread fd_pwrite fd_renumber fd_seek fd_sync fd_tell path_filestat_set_times path_link path_symlink sock_accept sock_recv sock_send sock_shutdown";
    static STRICT_PREVIEW1_IMPORT_GUEST: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x02, 0x23, 0x01, 0x16, b'w', b'a', b's',
        b'i', b'_', b's', b'n', b'a', b'p', b's', b'h', b'o', b't', b'_', b'p', b'r', b'e', b'v',
        b'i', b'e', b'w', b'1', 0x08, b'f', b'd', b'_', b'w', b'r', b'i', b't', b'e', 0x00, 0x00,
    ];
    static STRICT_UNSUPPORTED_IMPORT_GUEST: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x02, 0x0c, 0x01, 0x03, b'e', b'n', b'v',
        0x04, b'h', b'o', b's', b't', 0x00, 0x00,
    ];

    #[test]
    fn fd_control_messages_materialize_restrict_and_revoke_view() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        let route = PicoFdRoute::new(
            2,
            0,
            22,
            crate::choreography::protocol::LABEL_NET_DATAGRAM_SEND,
            7,
            1,
        );
        let granted = fds
            .apply_control(PicoFdControl::cap_grant(
                30,
                PicoFdRights::ReadWrite,
                ChoreoResourceKind::NetworkDatagram,
                99,
                route,
            ))
            .expect("CapGrant materializes fd view");

        let view = granted.materialized_view();
        assert_eq!(view.fd(), 30);
        assert_eq!(view.source(), PicoFdViewSource::Grant);
        assert_eq!(view.rights(), PicoFdRights::ReadWrite);
        assert_eq!(
            view.choreo_resource_kind(),
            ChoreoResourceKind::NetworkDatagram
        );
        assert_eq!(view.choreo_object_id(), 99);
        assert_eq!(view.choreo_object_generation(), 7);
        assert_eq!(
            view.route().route_label(),
            crate::choreography::protocol::LABEL_NET_DATAGRAM_SEND
        );

        let restricted = fds
            .apply_control(PicoFdControl::CapRestrict {
                fd: 30,
                generation: granted.generation(),
                rights: PicoFdRights::Read,
            })
            .expect("CapRestrict narrows materialized view");
        assert_eq!(restricted.materialized_rights(), PicoFdRights::Read);
        assert_eq!(restricted.source(), PicoFdViewSource::Grant);
        assert_eq!(
            fds.resolve_current(30, PicoFdRights::Write, ChoreoResourceKind::NetworkDatagram),
            Err(PicoFdError::PermissionDenied)
        );
        assert!(
            fds.resolve_current(30, PicoFdRights::Read, ChoreoResourceKind::NetworkDatagram)
                .is_ok()
        );

        fds.apply_control(PicoFdControl::CapRevoke { fd: 30 })
            .expect("CapRevoke marks view revoked");
        assert_eq!(
            fds.resolve_current(30, PicoFdRights::Read, ChoreoResourceKind::NetworkDatagram),
            Err(PicoFdError::Revoked)
        );
    }

    #[test]
    fn pico_fd_view_rejects_invalid_stale_closed_and_wrong_rights() {
        let mut fds: PicoFdView<2> = PicoFdView::new();
        let stdin = fds
            .apply_local_cap_grant(0, PicoFdRights::Read, ChoreoResourceKind::Stdin, 1, 0, 0)
            .expect("grant stdin fd");
        assert_eq!(stdin.fd(), 0);
        assert_eq!(stdin.generation(), 1);
        assert_eq!(stdin.resource(), ChoreoResourceKind::Stdin);
        assert_eq!(stdin.rights(), PicoFdRights::Read);
        assert_eq!(stdin.lane(), 1);
        assert_eq!(stdin.route_label(), 0);
        assert_eq!(stdin.wait_or_subscription_id(), 0);
        assert!(fds.has_active());
        assert_eq!(fds.active_count(), 1);

        assert_eq!(
            fds.resolve_current(0, PicoFdRights::Read, ChoreoResourceKind::Stdin),
            Ok(stdin)
        );
        assert_eq!(
            fds.resolve_current(0, PicoFdRights::Write, ChoreoResourceKind::Stdin),
            Err(PicoFdError::PermissionDenied)
        );
        assert_eq!(
            fds.resolve_current(0, PicoFdRights::Read, ChoreoResourceKind::Stdout),
            Err(PicoFdError::WrongResource)
        );
        assert_eq!(
            fds.resolve(
                0,
                stdin.generation().wrapping_add(1),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin
            ),
            Err(PicoFdError::BadGeneration)
        );
        assert_eq!(
            fds.resolve_current(9, PicoFdRights::Read, ChoreoResourceKind::Stdin),
            Err(PicoFdError::BadFd)
        );

        let closed = fds.close_current(0).expect("close stdin");
        assert!(closed.is_revoked());
        assert!(!fds.has_active());
        assert_eq!(
            fds.resolve_current(0, PicoFdRights::Read, ChoreoResourceKind::Stdin),
            Err(PicoFdError::Revoked)
        );

        let reopened = fds
            .apply_local_cap_grant(0, PicoFdRights::Read, ChoreoResourceKind::Stdin, 1, 0, 0)
            .expect("reopen stdin with new generation");
        assert_ne!(stdin.generation(), reopened.generation());
        assert_eq!(
            fds.resolve(
                0,
                stdin.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin
            ),
            Err(PicoFdError::BadGeneration)
        );
        assert_eq!(
            fds.resolve(
                0,
                reopened.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin
            ),
            Ok(reopened)
        );
        let telemetry = fds.rejection_telemetry();
        assert_eq!(telemetry.bad_fd(), 1);
        assert_eq!(telemetry.bad_generation(), 2);
        assert_eq!(telemetry.permission_denied(), 1);
        assert_eq!(telemetry.wrong_resource(), 1);
        assert_eq!(telemetry.revoked(), 1);
        assert_eq!(telemetry.total(), 6);
    }

    #[test]
    fn pico_fd_view_tracks_interrupt_subscription_control_grant() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        let wait = fds
            .apply_local_cap_grant(
                60,
                PicoFdRights::Read,
                ChoreoResourceKind::InterruptSubscription,
                4,
                crate::choreography::protocol::LABEL_GPIO_WAIT_RET,
                7,
            )
            .expect("grant gpio wait fd");

        assert_eq!(wait.fd(), 60);
        assert_eq!(wait.wait_or_subscription_id(), 7);
        assert_eq!(
            wait.route_label(),
            crate::choreography::protocol::LABEL_GPIO_WAIT_RET
        );
        assert_eq!(
            fds.resolve_current(
                60,
                PicoFdRights::Read,
                ChoreoResourceKind::InterruptSubscription
            ),
            Ok(wait)
        );
        fds.revoke_fd(60).expect("revoke gpio wait fd");
        assert_eq!(
            fds.resolve_current(
                60,
                PicoFdRights::Read,
                ChoreoResourceKind::InterruptSubscription
            ),
            Err(PicoFdError::Revoked)
        );
    }

    #[test]
    fn pico_fd_view_tracks_gateway_route_metadata() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        let route = PicoFdRoute::new(
            4,
            crate::kernel::policy::NodeRole::Gateway.bit(),
            20,
            crate::choreography::protocol::LABEL_SWARM_TELEMETRY,
            7,
            1,
        );
        let gateway = fds
            .apply_cap_grant(
                40,
                PicoFdRights::ReadWrite,
                ChoreoResourceKind::Gateway,
                0,
                route,
            )
            .expect("grant gateway fd");

        assert_eq!(gateway.fd(), 40);
        assert_eq!(gateway.resource(), ChoreoResourceKind::Gateway);
        assert_eq!(gateway.lane(), 20);
        assert_eq!(
            gateway.route_label(),
            crate::choreography::protocol::LABEL_SWARM_TELEMETRY
        );
        assert_eq!(gateway.target_node(), 4);
        assert_eq!(
            gateway.target_role(),
            crate::kernel::policy::NodeRole::Gateway.bit()
        );
        assert_eq!(gateway.session_generation(), 7);
        assert_eq!(gateway.policy_slot(), 1);
        assert_eq!(
            fds.resolve_current(40, PicoFdRights::Write, ChoreoResourceKind::Gateway),
            Ok(gateway)
        );
        assert_eq!(
            fds.resolve_current(40, PicoFdRights::Read, ChoreoResourceKind::Gateway),
            Ok(gateway)
        );
        assert_eq!(
            fds.resolve_routed_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                gateway.route()
            ),
            Ok(gateway)
        );
        assert_eq!(
            fds.resolve_routed(
                40,
                gateway.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Gateway,
                gateway.route(),
            ),
            Ok(gateway)
        );
        let mut policy_slots: crate::kernel::policy::PolicySlotTable<1> =
            crate::kernel::policy::PolicySlotTable::new();
        assert_eq!(
            fds.resolve_routed_authorized_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                gateway.route(),
                &policy_slots,
            ),
            Err(PicoFdError::PolicyDenied)
        );
        policy_slots
            .allow(gateway.policy_slot())
            .expect("allow gateway policy slot");
        assert_eq!(
            fds.resolve_routed_authorized_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                gateway.route(),
                &policy_slots,
            ),
            Ok(gateway)
        );
        assert_eq!(
            fds.resolve_routed_authorized(
                40,
                gateway.generation(),
                PicoFdRights::Read,
                ChoreoResourceKind::Gateway,
                gateway.route(),
                &policy_slots,
            ),
            Ok(gateway)
        );
        policy_slots
            .deny(gateway.policy_slot())
            .expect("deny gateway policy slot");
        assert_eq!(
            fds.resolve_routed_authorized_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                gateway.route(),
                &policy_slots,
            ),
            Err(PicoFdError::PolicyDenied)
        );
        policy_slots
            .allow(gateway.policy_slot())
            .expect("re-allow gateway policy slot");
        assert_eq!(
            fds.resolve_current(40, PicoFdRights::Write, ChoreoResourceKind::Telemetry),
            Err(PicoFdError::WrongResource)
        );
        assert_eq!(
            fds.resolve_routed_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                PicoFdRoute::new(
                    gateway.target_node(),
                    gateway.target_role(),
                    gateway.lane(),
                    gateway.route_label(),
                    gateway.session_generation().wrapping_add(1),
                    gateway.policy_slot(),
                )
            ),
            Err(PicoFdError::BadSessionGeneration)
        );
        assert_eq!(
            fds.resolve_routed_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                PicoFdRoute::new(
                    gateway.target_node().wrapping_add(1),
                    gateway.target_role(),
                    gateway.lane(),
                    gateway.route_label(),
                    gateway.session_generation(),
                    gateway.policy_slot(),
                )
            ),
            Err(PicoFdError::BadRoute)
        );
        assert_eq!(
            fds.resolve_routed_current(
                40,
                PicoFdRights::Write,
                ChoreoResourceKind::Gateway,
                PicoFdRoute::new(
                    gateway.target_node(),
                    gateway.target_role(),
                    gateway.lane(),
                    gateway.route_label(),
                    gateway.session_generation(),
                    gateway.policy_slot().wrapping_add(1),
                )
            ),
            Err(PicoFdError::BadRoute)
        );
        let telemetry = fds.rejection_telemetry();
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.permission_denied(), 2);
        assert_eq!(telemetry.wrong_resource(), 1);
        assert_eq!(telemetry.bad_route(), 2);
        assert_eq!(telemetry.total(), 6);
    }

    #[test]
    fn strict_import_summary_accepts_preview1_import_section() {
        let summary = Wasip1ImportSummary::parse_strict_preview1(STRICT_PREVIEW1_IMPORT_GUEST)
            .expect("parse strict preview1 import section");
        assert_eq!(summary.import_count(), 1);
        assert_eq!(summary.preview1_import_count(), 1);
    }

    #[test]
    fn strict_import_summary_rejects_non_preview1_import_module() {
        assert_eq!(
            Wasip1ImportSummary::parse_strict_preview1(STRICT_UNSUPPORTED_IMPORT_GUEST),
            Err(Wasip1Error::UnsupportedImport)
        );
        assert_eq!(
            Wasip1ImportSummary::parse_strict_preview1(&STRICT_PREVIEW1_IMPORT_GUEST[..8]),
            Err(Wasip1Error::MissingWasiModule)
        );
    }

    #[test]
    fn rust_wasip1_stdout_module_is_accepted() {
        Wasip1Module::install(WASIP1_STDOUT_GUEST).expect("install module");
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse module");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);
    }

    #[test]
    fn rust_wasip1_stderr_module_is_accepted() {
        Wasip1Module::install(WASIP1_STDERR_GUEST).expect("install module");
        let module = Wasip1StderrModule::parse(WASIP1_STDERR_GUEST).expect("parse module");
        let chunk = module
            .stderr_chunk_for(TEST_STDERR_TEXT)
            .expect("stderr chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDERR_TEXT);
    }

    #[test]
    fn rust_wasip1_stdin_module_is_accepted() {
        Wasip1Module::install(WASIP1_STDIN_GUEST).expect("install module");
        let module = Wasip1StdinModule::parse(WASIP1_STDIN_GUEST).expect("parse module");
        let request = module
            .stdin_request_for(TEST_STDIN_MAX_LEN)
            .expect("stdin request");
        let chunk = module
            .stdin_chunk_for(TEST_STDIN_INPUT)
            .expect("stdin chunk");
        assert_eq!(request.max_len(), TEST_STDIN_MAX_LEN);
        assert_eq!(chunk.as_bytes(), TEST_STDIN_INPUT);
    }

    #[test]
    fn rust_wasip1_clock_module_is_accepted() {
        Wasip1Module::install(WASIP1_CLOCK_GUEST).expect("install module");
        let module = Wasip1ClockModule::parse(WASIP1_CLOCK_GUEST).expect("parse module");
        let now = module.clock_now(TEST_CLOCK_NANOS);
        assert_eq!(now.nanos(), TEST_CLOCK_NANOS);
    }

    #[test]
    fn rust_wasip1_random_module_is_accepted() {
        Wasip1Module::install(WASIP1_RANDOM_GUEST).expect("install module");
        let module = Wasip1RandomModule::parse(WASIP1_RANDOM_GUEST).expect("parse module");
        let seed = module.random_seed(TEST_RANDOM_SEED_LO, TEST_RANDOM_SEED_HI);
        assert_eq!(seed.lo(), TEST_RANDOM_SEED_LO);
        assert_eq!(seed.hi(), TEST_RANDOM_SEED_HI);
    }

    #[test]
    fn rust_wasip1_exit_module_is_accepted() {
        Wasip1Module::install(WASIP1_EXIT_GUEST).expect("install module");
        let module = Wasip1ExitModule::parse(WASIP1_EXIT_GUEST).expect("parse module");
        assert_eq!(module.exit_status(TEST_EXIT_CODE).code(), TEST_EXIT_CODE);
    }

    #[test]
    fn rust_wasip1_std_modules_use_static_empty_args_environment() {
        for bytes in [
            WASIP1_STDOUT_GUEST,
            WASIP1_STDERR_GUEST,
            WASIP1_STDIN_GUEST,
            WASIP1_CLOCK_GUEST,
            WASIP1_RANDOM_GUEST,
            WASIP1_EXIT_GUEST,
        ] {
            let module = Wasip1EnvironmentModule::parse(bytes).expect("parse environment module");
            assert!(module.has_environ_get());
            assert!(!module.has_args_get());

            let snapshot = module.static_snapshot();
            assert_eq!(snapshot.arg_count(), 0);
            assert_eq!(snapshot.environment_count(), 0);
            assert!(snapshot.args().is_empty());
            assert!(snapshot.environment().is_empty());
        }
    }

    #[test]
    fn static_args_environment_snapshot_is_bounded() {
        let args = [b"hibana".as_slice(), b"pico".as_slice()];
        let environment = [(b"MODE".as_slice(), b"test".as_slice())];
        let snapshot = Wasip1StaticArgEnv::new(&args, &environment).expect("static snapshot");
        assert_eq!(snapshot.args(), &args);
        assert_eq!(snapshot.environment(), &environment);

        let too_many_args = [
            b"a".as_slice(),
            b"b".as_slice(),
            b"c".as_slice(),
            b"d".as_slice(),
            b"e".as_slice(),
        ];
        assert_eq!(
            Wasip1StaticArgEnv::new(&too_many_args, &[]),
            Err(StaticArgEnvError::TooManyArgs)
        );

        let oversized_arg = [0u8; WASIP1_STATIC_ARG_BYTES_CAPACITY + 1];
        assert_eq!(
            Wasip1StaticArgEnv::new(&[oversized_arg.as_slice()], &[]),
            Err(StaticArgEnvError::ArgsTooLarge)
        );

        let oversized_env = [0u8; WASIP1_STATIC_ENV_BYTES_CAPACITY + 1];
        assert_eq!(
            Wasip1StaticArgEnv::new(&[], &[(b"BIG".as_slice(), oversized_env.as_slice())]),
            Err(StaticArgEnvError::EnvironmentTooLarge)
        );
    }

    #[test]
    fn environment_module_accepts_static_arguments_access() {
        let valid = b"\0asm\x01\0\0\0wasi_snapshot_preview1 args_get";
        let module = Wasip1EnvironmentModule::parse(valid).expect("parse arguments module");
        assert!(!module.has_environ_get());
        assert!(module.has_args_get());

        let missing_access = b"\0asm\x01\0\0\0wasi_snapshot_preview1";
        assert!(matches!(
            Wasip1EnvironmentModule::parse(missing_access),
            Err(Wasip1Error::MissingEnvironGetImport)
        ));
    }

    #[test]
    fn module_install_rejects_on_unsupported_import() {
        let invalid = &[
            0, 97, 115, 109, 1, 0, 0, 0, 119, 97, 115, 105, 95, 115, 110, 97, 112, 115, 104, 111,
            116, 95, 112, 114, 101, 118, 105, 101, 119, 49, 32, 119, 97, 115, 105, 58, 115, 111,
            99, 107, 101, 116, 115, 47, 110, 101, 116, 119, 111, 114, 107, 64, 48, 46, 50, 46, 48,
        ];
        assert!(matches!(
            Wasip1Module::install(invalid),
            Err(Wasip1Error::UnsupportedImport)
        ));
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_syscall_import() {
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };

        assert!(matches!(
            Wasip1Module::install_with_handlers(WASIP1_FULL_SUBSET_GUEST, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_path_minimal_import() {
        let path_guest = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write path_open";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(path_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            path_minimal: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(path_guest, handlers)
            .expect("path-minimal feature admits path imports for typed rejection");
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_path_full_import() {
        let path_guest = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_seek";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            path_minimal: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(path_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            path_minimal: true,
            path_full: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(path_guest, handlers)
            .expect("path-full feature admits path imports for typed rejection");
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_socket_import() {
        let sock_guest = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write sock_send";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(sock_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            network_object: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(sock_guest, handlers)
            .expect("Network object feature admits P1 socket imports for typed rejection");
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_args_env_sizes_imports() {
        let args_guest =
            b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write args_sizes_get environ_sizes_get";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(args_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            args_env: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(args_guest, handlers)
            .expect("args/env feature admits startup size imports");
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_sched_yield_import() {
        let yield_guest = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write sched_yield";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(yield_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            sched_yield: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(yield_guest, handlers)
            .expect("sched_yield feature admits typed yield syscall");
    }

    #[test]
    fn module_install_with_handler_set_rejects_disabled_clock_res_import() {
        let clock_res_guest = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write clock_res_get";
        let handlers = Wasip1HandlerSet {
            fd_write: true,
            ..Wasip1HandlerSet::EMPTY
        };
        assert!(matches!(
            Wasip1Module::install_with_handlers(clock_res_guest, handlers),
            Err(Wasip1Error::UnsupportedByProfile)
        ));

        let handlers = Wasip1HandlerSet {
            fd_write: true,
            clock_res_get: true,
            ..Wasip1HandlerSet::EMPTY
        };
        Wasip1Module::install_with_handlers(clock_res_guest, handlers)
            .expect("clock_res_get feature admits typed resolution syscall");
    }

    #[test]
    fn module_install_with_handler_set_accepts_profile_supported_imports() {
        let supported = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write poll_oneoff proc_exit";
        let module = Wasip1Module::install_with_handlers(supported, Wasip1HandlerSet::PICO_MIN)
            .expect("Pico minimum profile supports fd_write + poll_oneoff + proc_exit");

        assert_eq!(module.bytes(), supported);
    }

    #[test]
    fn full_subset_module_requires_all_p1_imports() {
        Wasip1FullSubsetModule::parse(WASIP1_FULL_SUBSET_GUEST).expect("full subset");
        let missing_clock_res = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_clock_res),
            Err(Wasip1Error::MissingClockResGetImport)
        ));
        let missing_poll = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_poll),
            Err(Wasip1Error::MissingPollOneoffImport)
        ));
        let missing_proc_raise = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit sched_yield args_sizes_get args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_proc_raise),
            Err(Wasip1Error::MissingProcRaiseImport)
        ));
        let missing_sched_yield = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise args_sizes_get args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_sched_yield),
            Err(Wasip1Error::MissingSchedYieldImport)
        ));
        let missing_args_sizes = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_args_sizes),
            Err(Wasip1Error::MissingArgsSizesGetImport)
        ));
        let missing_environ_sizes = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_environ_sizes),
            Err(Wasip1Error::MissingEnvironSizesGetImport)
        ));
        let missing_path = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_path),
            Err(Wasip1Error::MissingPathMinimalImport)
        ));
        let missing_path_full = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get fd_prestat_get fd_prestat_dir_name fd_filestat_get fd_readdir path_open path_filestat_get path_readlink path_create_directory path_remove_directory path_unlink_file path_rename";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_path_full),
            Err(Wasip1Error::MissingPathFullImport)
        ));
        let missing_socket = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write fd_read fd_fdstat_get fd_close clock_res_get clock_time_get poll_oneoff random_get proc_exit proc_raise sched_yield args_sizes_get args_get environ_sizes_get environ_get fd_prestat_get fd_prestat_dir_name fd_filestat_get fd_readdir path_open path_filestat_get path_readlink path_create_directory path_remove_directory path_unlink_file path_rename fd_advise fd_allocate fd_datasync fd_fdstat_set_flags fd_fdstat_set_rights fd_filestat_set_size fd_filestat_set_times fd_pread fd_pwrite fd_renumber fd_seek fd_sync fd_tell path_filestat_set_times path_link path_symlink";
        assert!(matches!(
            Wasip1FullSubsetModule::parse(missing_socket),
            Err(Wasip1Error::MissingSocketImport)
        ));
    }

    #[test]
    fn memory_lease_table_validates_read_stream_transfer() {
        let mut table: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 9);
        let borrow = MemBorrow::new(1024, 21, 9);
        let grant = table.grant_read(borrow).expect("grant read");
        assert_eq!(grant.rights(), MemRights::Read);

        let chunk = StdoutChunk::new_with_lease(grant.lease_id(), TEST_STDOUT_TEXT)
            .expect("leased stdout chunk");
        table.validate_read_chunk(&chunk).expect("valid read chunk");

        let oversized = StdoutChunk::new_with_lease(grant.lease_id(), b"0123456789012345678901")
            .expect("oversized chunk for lease");
        assert_eq!(
            table.validate_read_chunk(&oversized),
            Err(MemoryLeaseError::LengthExceeded)
        );

        let wrong_lease = chunk.with_lease(grant.lease_id() + 1);
        assert_eq!(
            table.validate_read_chunk(&wrong_lease),
            Err(MemoryLeaseError::UnknownLease)
        );

        let release = MemRelease::new(grant.lease_id());
        table.release(release).expect("release lease");
        assert_eq!(
            table.validate_read_chunk(&chunk),
            Err(MemoryLeaseError::UnknownLease)
        );

        let next = table
            .grant_read(MemBorrow::new(1024, 21, 9))
            .expect("grant next read");
        assert_eq!(next.lease_id(), grant.lease_id());
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.invalid_lease(), 2);
        assert_eq!(telemetry.length_exceeded(), 1);
        assert_eq!(telemetry.total(), 3);
    }

    #[test]
    fn memory_lease_table_validates_write_stream_transfer() {
        let mut table: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 4);
        let grant = table
            .grant_write(MemBorrow::new(2048, 24, 4))
            .expect("grant write");
        let request = StdinRequest::new_with_lease(grant.lease_id(), 24).expect("stdin request");
        table
            .validate_write_request(&request)
            .expect("valid write request");

        let chunk = crate::choreography::protocol::StdinChunk::new_with_lease(
            grant.lease_id(),
            TEST_STDIN_INPUT,
        )
        .expect("stdin chunk");
        table
            .validate_write_chunk(&chunk)
            .expect("valid write chunk");
        table
            .commit(MemCommit::new(grant.lease_id(), chunk.len() as u8))
            .expect("commit");

        let too_much = StdinRequest::new_with_lease(grant.lease_id(), 25).expect("request");
        assert_eq!(
            table.validate_write_request(&too_much),
            Err(MemoryLeaseError::LengthExceeded)
        );
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.length_exceeded(), 1);
        assert_eq!(telemetry.total(), 1);
    }

    #[test]
    fn memory_lease_table_rejects_invalid_borrows() {
        let mut table: MemoryLeaseTable<1> = MemoryLeaseTable::new(64, 1);
        assert_eq!(
            table.grant_read(MemBorrow::new(0, 0, 1)),
            Err(MemoryLeaseError::Empty)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(0, 4, 2)),
            Err(MemoryLeaseError::EpochMismatch)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(60, 8, 1)),
            Err(MemoryLeaseError::OutOfBounds)
        );

        let grant = table
            .grant_read(MemBorrow::new(0, 4, 1))
            .expect("grant read");
        assert_eq!(
            table.commit(MemCommit::new(grant.lease_id(), 1)),
            Err(MemoryLeaseError::RightsMismatch)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(8, 4, 1)),
            Err(MemoryLeaseError::TableFull)
        );
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.rights_mismatch(), 1);
        assert_eq!(telemetry.out_of_bounds(), 1);
        assert_eq!(telemetry.table_full(), 1);
        assert_eq!(telemetry.other(), 1);
        assert_eq!(telemetry.total(), 5);
    }

    #[test]
    fn memory_lease_table_fence_revokes_outstanding_leases_and_bumps_epoch() {
        let mut table: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 1);
        let grant = table
            .grant_read(MemBorrow::new(1024, 21, 1))
            .expect("grant read");
        let chunk =
            StdoutChunk::new_with_lease(grant.lease_id(), TEST_STDOUT_TEXT).expect("leased chunk");
        table
            .validate_read_chunk(&chunk)
            .expect("valid before fence");
        assert!(table.has_outstanding_leases());

        table.fence(MemFence::new(MemFenceReason::HotSwap, 2));
        assert_eq!(table.epoch(), 2);
        assert!(!table.has_outstanding_leases());
        assert_eq!(
            table.validate_read_chunk(&chunk),
            Err(MemoryLeaseError::UnknownLease)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(1024, 21, 1)),
            Err(MemoryLeaseError::EpochMismatch)
        );
        table
            .grant_read(MemBorrow::new(1024, 21, 2))
            .expect("grant after fence");
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.invalid_lease(), 1);
        assert_eq!(telemetry.total(), 2);
    }

    #[test]
    fn memory_lease_table_memory_grow_fence_rejects_stale_read_write_and_old_epoch() {
        let mut table: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 1);
        let read_grant = table
            .grant_read(MemBorrow::new(1024, 21, 1))
            .expect("grant read before memory grow");
        let write_grant = table
            .grant_write(MemBorrow::new(2048, 8, 1))
            .expect("grant write before memory grow");
        let read_chunk = StdoutChunk::new_with_lease(read_grant.lease_id(), TEST_STDOUT_TEXT)
            .expect("leased stdout before memory grow");
        table
            .validate_read_chunk(&read_chunk)
            .expect("read lease is valid before memory grow");

        table.fence(MemFence::new(MemFenceReason::MemoryGrow, 2));
        assert_eq!(table.epoch(), 2);
        assert!(!table.has_outstanding_leases());
        assert_eq!(
            table.validate_read_chunk(&read_chunk),
            Err(MemoryLeaseError::UnknownLease)
        );
        assert_eq!(
            table.commit(MemCommit::new(write_grant.lease_id(), 1)),
            Err(MemoryLeaseError::UnknownLease)
        );
        assert_eq!(
            table.release(MemRelease::new(read_grant.lease_id())),
            Err(MemoryLeaseError::UnknownLease)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(1024, 21, 1)),
            Err(MemoryLeaseError::EpochMismatch)
        );
        table
            .grant_read(MemBorrow::new(1024, 21, 2))
            .expect("new epoch read lease after memory grow");
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.invalid_lease(), 3);
        assert_eq!(telemetry.total(), 4);
    }

    #[test]
    fn wasm32_wasip1_stale_memory_lease_smoke_rejects() {
        let mut table: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 7);
        let grant = table
            .grant_read(MemBorrow::new(1024, 21, 7))
            .expect("grant read");
        let chunk =
            StdoutChunk::new_with_lease(grant.lease_id(), TEST_STDOUT_TEXT).expect("leased stdout");

        table
            .release(MemRelease::new(grant.lease_id()))
            .expect("release lease");
        assert_eq!(
            table.validate_read_chunk(&chunk),
            Err(MemoryLeaseError::UnknownLease)
        );

        let new_grant = table
            .grant_read(MemBorrow::new(1024, 21, 7))
            .expect("grant reused id");
        table.fence(MemFence::new(MemFenceReason::HotSwap, 8));
        assert_eq!(
            table.release(MemRelease::new(new_grant.lease_id())),
            Err(MemoryLeaseError::UnknownLease)
        );
        assert_eq!(
            table.grant_read(MemBorrow::new(1024, 21, 7)),
            Err(MemoryLeaseError::EpochMismatch)
        );
        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.invalid_lease(), 2);
        assert_eq!(telemetry.total(), 3);
    }

    #[test]
    fn module_validation_rejects_without_stdout_import() {
        let mut invalid = *b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_read";
        assert!(matches!(
            Wasip1StdoutModule::parse(&invalid),
            Err(Wasip1Error::MissingFdWriteImport)
        ));
        invalid[0] = 1;
        assert!(matches!(
            Wasip1StdoutModule::parse(&invalid),
            Err(Wasip1Error::InvalidModule)
        ));
    }

    #[test]
    fn module_validation_rejects_without_stderr_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_read";
        assert!(matches!(
            Wasip1StderrModule::parse(invalid),
            Err(Wasip1Error::MissingFdWriteImport)
        ));
    }

    #[test]
    fn module_validation_rejects_without_stdin_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write";
        assert!(matches!(
            Wasip1StdinModule::parse(invalid),
            Err(Wasip1Error::MissingFdReadImport)
        ));
    }

    #[test]
    fn module_validation_rejects_without_wall_clock_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1";
        assert!(matches!(
            Wasip1ClockModule::parse(invalid),
            Err(Wasip1Error::MissingClockTimeGetImport)
        ));
    }

    #[test]
    fn module_validation_rejects_without_random_seed_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1";
        assert!(matches!(
            Wasip1RandomModule::parse(invalid),
            Err(Wasip1Error::MissingRandomGetImport)
        ));
    }

    #[test]
    fn module_validation_rejects_without_exit_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1";
        assert!(matches!(
            Wasip1ExitModule::parse(invalid),
            Err(Wasip1Error::MissingProcExitImport)
        ));
    }

    #[test]
    fn module_validation_rejects_without_environment_import() {
        let invalid = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write";
        assert!(matches!(
            Wasip1EnvironmentModule::parse(invalid),
            Err(Wasip1Error::MissingEnvironGetImport)
        ));
    }
}
