use crate::{
    choreography::protocol::{
        FdRead, FdWrite, MemBorrow, MemFence, MemFenceReason, MemGrant, MemRelease, MemRights,
        PollOneoff, TimerSleepDone,
    },
    kernel::wasi::{
        ChoreoResourceKind, MemoryLeaseError, MemoryLeaseTable, PicoFdError, PicoFdRights,
        PicoFdRoute, PicoFdView, PicoFdViewEntry, PicoFdViewSource,
    },
};

pub const WASI_ERRNO_SUCCESS: u16 = 0;
pub const WASI_ERRNO_BADF: u16 = 8;
pub const WASI_ERRNO_FAULT: u16 = 21;
pub const WASI_ERRNO_INVAL: u16 = 28;
pub const WASI_ERRNO_MFILE: u16 = 33;
pub const WASI_ERRNO_NOMEM: u16 = 48;
pub const WASI_ERRNO_NOTCAPABLE: u16 = 76;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasiProfile {
    PicoMin,
    EmbeddedStd,
    HostFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestFdKind {
    Stdin,
    Stdout,
    Stderr,
    Gpio,
    Uart,
    Timer,
    LocalSensor,
    LocalActuator,
    Datagram,
    Stream,
    Telemetry,
    Management,
    Gateway,
    EventSubscription,
    PreopenRoot,
    ChoreoObject,
    DirectoryView,
    NetworkListener,
    RemoteObject,
    EphemeralPipe,
}

impl From<ChoreoResourceKind> for GuestFdKind {
    fn from(value: ChoreoResourceKind) -> Self {
        match value {
            ChoreoResourceKind::Stdin => Self::Stdin,
            ChoreoResourceKind::Stdout => Self::Stdout,
            ChoreoResourceKind::Stderr => Self::Stderr,
            ChoreoResourceKind::Gpio => Self::Gpio,
            ChoreoResourceKind::Uart => Self::Uart,
            ChoreoResourceKind::Timer => Self::Timer,
            ChoreoResourceKind::LocalSensor => Self::LocalSensor,
            ChoreoResourceKind::LocalActuator => Self::LocalActuator,
            ChoreoResourceKind::NetworkDatagram => Self::Datagram,
            ChoreoResourceKind::NetworkStream => Self::Stream,
            ChoreoResourceKind::Telemetry => Self::Telemetry,
            ChoreoResourceKind::Management => Self::Management,
            ChoreoResourceKind::Gateway => Self::Gateway,
            ChoreoResourceKind::InterruptSubscription => Self::EventSubscription,
            ChoreoResourceKind::PreopenRoot => Self::PreopenRoot,
            ChoreoResourceKind::ChoreoObject => Self::ChoreoObject,
            ChoreoResourceKind::DirectoryView => Self::DirectoryView,
            ChoreoResourceKind::NetworkListener => Self::NetworkListener,
            ChoreoResourceKind::RemoteObject => Self::RemoteObject,
            ChoreoResourceKind::EphemeralPipe => Self::EphemeralPipe,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestFd {
    fd: u8,
    generation: u16,
    source: PicoFdViewSource,
    kind: GuestFdKind,
    rights: PicoFdRights,
    route: PicoFdRoute,
    wait_or_subscription_id: u16,
    choreo_object_generation: u16,
}

impl GuestFd {
    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn source(&self) -> PicoFdViewSource {
        self.source
    }

    pub const fn kind(&self) -> GuestFdKind {
        self.kind
    }

    pub const fn rights(&self) -> PicoFdRights {
        self.rights
    }

    pub const fn route(&self) -> PicoFdRoute {
        self.route
    }

    pub const fn wait_or_subscription_id(&self) -> u16 {
        self.wait_or_subscription_id
    }

    pub const fn choreo_object_generation(&self) -> u16 {
        self.choreo_object_generation
    }
}

impl From<PicoFdViewEntry> for GuestFd {
    fn from(value: PicoFdViewEntry) -> Self {
        Self {
            fd: value.fd(),
            generation: value.generation(),
            source: value.source(),
            kind: value.resource().into(),
            rights: value.rights(),
            route: value.route(),
            wait_or_subscription_id: value.wait_or_subscription_id(),
            choreo_object_generation: value.choreo_object_generation(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingSyscallKind {
    FdRead,
    FdWrite,
    PollOneoff,
    ClockSleep,
    NetworkRecv,
    NetworkSend,
    ChoreoFsRead,
    ChoreoFsWrite,
    DirectoryRead,
    SockAccept,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingSyscallToken {
    id: u16,
    generation: u16,
    kind: PendingSyscallKind,
}

impl PendingSyscallToken {
    pub const fn new(id: u16, generation: u16, kind: PendingSyscallKind) -> Self {
        Self {
            id,
            generation,
            kind,
        }
    }

    pub const fn id(&self) -> u16 {
        self.id
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn kind(&self) -> PendingSyscallKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingSyscallSpec {
    kind: PendingSyscallKind,
    fd: Option<u8>,
    lease_id: Option<u8>,
    len: Option<u16>,
    tick: Option<u64>,
    fd_kind: Option<GuestFdKind>,
}

impl PendingSyscallSpec {
    pub const fn new(kind: PendingSyscallKind) -> Self {
        Self {
            kind,
            fd: None,
            lease_id: None,
            len: None,
            tick: None,
            fd_kind: None,
        }
    }

    pub const fn fd_io(
        kind: PendingSyscallKind,
        fd: u8,
        lease_id: u8,
        len: u16,
        fd_kind: GuestFdKind,
    ) -> Self {
        Self {
            kind,
            fd: Some(fd),
            lease_id: Some(lease_id),
            len: Some(len),
            tick: None,
            fd_kind: Some(fd_kind),
        }
    }

    pub const fn fd_event(kind: PendingSyscallKind, fd: u8, fd_kind: GuestFdKind) -> Self {
        Self {
            kind,
            fd: Some(fd),
            lease_id: None,
            len: None,
            tick: None,
            fd_kind: Some(fd_kind),
        }
    }

    pub const fn timer(kind: PendingSyscallKind, tick: u64) -> Self {
        Self {
            kind,
            fd: None,
            lease_id: None,
            len: None,
            tick: Some(tick),
            fd_kind: None,
        }
    }

    pub const fn kind(&self) -> PendingSyscallKind {
        self.kind
    }

    pub const fn fd(&self) -> Option<u8> {
        self.fd
    }

    pub const fn lease_id(&self) -> Option<u8> {
        self.lease_id
    }

    pub const fn len(&self) -> Option<u16> {
        self.len
    }

    pub const fn tick(&self) -> Option<u64> {
        self.tick
    }

    pub const fn fd_kind(&self) -> Option<GuestFdKind> {
        self.fd_kind
    }

    pub fn matches_completion(&self, completion: PendingSyscallCompletion) -> bool {
        if self.kind != completion.kind {
            return false;
        }
        if self.fd != completion.fd {
            return false;
        }
        if self.lease_id != completion.lease_id {
            return false;
        }
        if let Some(expected_len) = self.len {
            let Some(actual_len) = completion.len else {
                return false;
            };
            if actual_len > expected_len {
                return false;
            }
        } else if completion.len.is_some() {
            return false;
        }
        self.tick == completion.tick
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingSyscallCompletion {
    kind: PendingSyscallKind,
    fd: Option<u8>,
    lease_id: Option<u8>,
    len: Option<u16>,
    tick: Option<u64>,
}

impl PendingSyscallCompletion {
    pub const fn fd_io(kind: PendingSyscallKind, fd: u8, lease_id: u8, len: u16) -> Self {
        Self {
            kind,
            fd: Some(fd),
            lease_id: Some(lease_id),
            len: Some(len),
            tick: None,
        }
    }

    pub const fn fd_event(kind: PendingSyscallKind, fd: u8) -> Self {
        Self {
            kind,
            fd: Some(fd),
            lease_id: None,
            len: None,
            tick: None,
        }
    }

    pub const fn timer(kind: PendingSyscallKind, tick: u64) -> Self {
        Self {
            kind,
            fd: None,
            lease_id: None,
            len: None,
            tick: Some(tick),
        }
    }

    pub const fn kind(&self) -> PendingSyscallKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingSyscallEntry {
    token: PendingSyscallToken,
    spec: PendingSyscallSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingSyscallError {
    TableFull,
    NotFound,
    StaleGeneration,
    WrongKind,
    CompletionMismatch,
}

pub struct PendingSyscallTable<const N: usize> {
    slots: [Option<PendingSyscallEntry>; N],
    next_id: u16,
    next_generation: u16,
}

impl<const N: usize> PendingSyscallTable<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            next_id: 1,
            next_generation: 1,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn has_active(&self) -> bool {
        self.pending_count() != 0
    }

    pub fn begin(
        &mut self,
        spec: PendingSyscallSpec,
    ) -> Result<PendingSyscallToken, PendingSyscallError> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return Err(PendingSyscallError::TableFull);
        };
        let token = PendingSyscallToken::new(self.next_id, self.next_generation, spec.kind());
        *slot = Some(PendingSyscallEntry { token, spec });
        self.bump();
        Ok(token)
    }

    pub fn begin_poll_oneoff(
        &mut self,
        poll: PollOneoff,
    ) -> Result<PendingSyscallToken, PendingSyscallError> {
        self.begin(PendingSyscallSpec::timer(
            PendingSyscallKind::PollOneoff,
            poll.timeout_tick(),
        ))
    }

    pub fn complete(
        &mut self,
        token: PendingSyscallToken,
        completion: PendingSyscallCompletion,
    ) -> Result<(), PendingSyscallError> {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|entry| entry.token.id == token.id))
        else {
            return Err(PendingSyscallError::NotFound);
        };
        let entry = (*slot).expect("matched pending syscall must exist");
        if entry.token.generation != token.generation {
            return Err(PendingSyscallError::StaleGeneration);
        }
        if entry.token.kind != token.kind || entry.token.kind != completion.kind() {
            return Err(PendingSyscallError::WrongKind);
        }
        if !entry.spec.matches_completion(completion) {
            return Err(PendingSyscallError::CompletionMismatch);
        }
        *slot = None;
        Ok(())
    }

    pub fn complete_poll_oneoff(
        &mut self,
        token: PendingSyscallToken,
        done: TimerSleepDone,
    ) -> Result<(), PendingSyscallError> {
        self.complete(
            token,
            PendingSyscallCompletion::timer(PendingSyscallKind::PollOneoff, done.tick()),
        )
    }

    pub fn fence(&mut self) -> usize {
        let mut cleared = 0usize;
        for slot in &mut self.slots {
            if slot.take().is_some() {
                cleared += 1;
            }
        }
        cleared
    }

    fn bump(&mut self) {
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        if self.next_generation == 0 {
            self.next_generation = 1;
        }
    }
}

impl<const N: usize> Default for PendingSyscallTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestQuotaLimits {
    max_fds: u16,
    max_pending: u16,
}

impl GuestQuotaLimits {
    pub const fn new(max_fds: u16, max_pending: u16) -> Self {
        Self {
            max_fds,
            max_pending,
        }
    }

    pub const fn max_fds(&self) -> u16 {
        self.max_fds
    }

    pub const fn max_pending(&self) -> u16 {
        self.max_pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestQuotaError {
    FdLimit,
    PendingLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasiErrnoMap;

impl WasiErrnoMap {
    pub const fn new() -> Self {
        Self
    }

    pub const fn map_fd_error(self, error: PicoFdError) -> u16 {
        match error {
            PicoFdError::BadFd | PicoFdError::BadGeneration | PicoFdError::Revoked => {
                WASI_ERRNO_BADF
            }
            PicoFdError::PermissionDenied
            | PicoFdError::PolicyDenied
            | PicoFdError::WrongResource
            | PicoFdError::BadRoute
            | PicoFdError::BadSessionGeneration => WASI_ERRNO_NOTCAPABLE,
            PicoFdError::TableFull => WASI_ERRNO_MFILE,
        }
    }

    pub const fn map_memory_error(self, error: MemoryLeaseError) -> u16 {
        match error {
            MemoryLeaseError::OutOfBounds => WASI_ERRNO_FAULT,
            MemoryLeaseError::TableFull => WASI_ERRNO_NOMEM,
            MemoryLeaseError::EpochMismatch
            | MemoryLeaseError::InvalidLeaseId
            | MemoryLeaseError::UnknownLease
            | MemoryLeaseError::RightsMismatch
            | MemoryLeaseError::LeaseMismatch => WASI_ERRNO_NOTCAPABLE,
            MemoryLeaseError::Empty
            | MemoryLeaseError::TooLarge
            | MemoryLeaseError::LengthExceeded => WASI_ERRNO_INVAL,
        }
    }

    pub const fn map_pending_error(self, error: PendingSyscallError) -> u16 {
        match error {
            PendingSyscallError::TableFull => WASI_ERRNO_NOMEM,
            PendingSyscallError::NotFound
            | PendingSyscallError::StaleGeneration
            | PendingSyscallError::WrongKind
            | PendingSyscallError::CompletionMismatch => WASI_ERRNO_INVAL,
        }
    }

    pub const fn map_quota_error(self, error: GuestQuotaError) -> u16 {
        match error {
            GuestQuotaError::FdLimit => WASI_ERRNO_MFILE,
            GuestQuotaError::PendingLimit => WASI_ERRNO_NOMEM,
        }
    }
}

impl Default for WasiErrnoMap {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestLedgerError {
    Fd(PicoFdError),
    Memory(MemoryLeaseError),
    Pending(PendingSyscallError),
    Quota(GuestQuotaError),
    LeaseMismatch,
}

impl From<PicoFdError> for GuestLedgerError {
    fn from(value: PicoFdError) -> Self {
        Self::Fd(value)
    }
}

impl From<MemoryLeaseError> for GuestLedgerError {
    fn from(value: MemoryLeaseError) -> Self {
        Self::Memory(value)
    }
}

impl From<PendingSyscallError> for GuestLedgerError {
    fn from(value: PendingSyscallError) -> Self {
        Self::Pending(value)
    }
}

impl From<GuestQuotaError> for GuestLedgerError {
    fn from(value: GuestQuotaError) -> Self {
        Self::Quota(value)
    }
}

pub struct GuestLedger<const FDS: usize, const LEASES: usize, const PENDING: usize> {
    tier: WasiProfile,
    fds: PicoFdView<FDS>,
    leases: MemoryLeaseTable<LEASES>,
    pending: PendingSyscallTable<PENDING>,
    quotas: GuestQuotaLimits,
    errno: WasiErrnoMap,
}

impl<const FDS: usize, const LEASES: usize, const PENDING: usize>
    GuestLedger<FDS, LEASES, PENDING>
{
    pub const fn new(
        tier: WasiProfile,
        memory_len: u32,
        memory_epoch: u32,
        quotas: GuestQuotaLimits,
        errno: WasiErrnoMap,
    ) -> Self {
        Self {
            tier,
            fds: PicoFdView::new(),
            leases: MemoryLeaseTable::new(memory_len, memory_epoch),
            pending: PendingSyscallTable::new(),
            quotas,
            errno,
        }
    }

    pub const fn pico_min(memory_len: u32, memory_epoch: u32) -> Self {
        Self::new(
            WasiProfile::PicoMin,
            memory_len,
            memory_epoch,
            GuestQuotaLimits::new(FDS as u16, PENDING as u16),
            WasiErrnoMap::new(),
        )
    }

    pub const fn tier(&self) -> WasiProfile {
        self.tier
    }

    pub const fn quotas(&self) -> GuestQuotaLimits {
        self.quotas
    }

    pub const fn errno_contract(&self) -> WasiErrnoMap {
        self.errno
    }

    pub fn fd_view(&self) -> &PicoFdView<FDS> {
        &self.fds
    }

    pub fn lease_table(&self) -> &MemoryLeaseTable<LEASES> {
        &self.leases
    }

    pub fn pending_table(&self) -> &PendingSyscallTable<PENDING> {
        &self.pending
    }

    pub fn apply_fd_cap_grant(
        &mut self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
        lane: u8,
        route_label: u8,
        wait_or_subscription_id: u16,
        target_node: u8,
        target_role: u16,
        session_generation: u16,
        policy_slot: u8,
    ) -> Result<GuestFd, GuestLedgerError> {
        if self.fds.active_count() >= self.quotas.max_fds as usize {
            return Err(GuestQuotaError::FdLimit.into());
        }
        let route = PicoFdRoute::new(
            target_node,
            target_role,
            lane,
            route_label,
            session_generation,
            policy_slot,
        );
        Ok(self
            .fds
            .apply_cap_grant(fd, rights, resource, wait_or_subscription_id, route)?
            .into())
    }

    pub fn apply_abort_fence(&mut self, new_memory_epoch: u32) {
        self.leases
            .fence(MemFence::new(MemFenceReason::Trap, new_memory_epoch));
        self.pending.fence();
        self.fds.fence_all();
    }

    pub fn apply_fd_cap_mint(
        &mut self,
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
    ) -> Result<GuestFd, GuestLedgerError> {
        if self.fds.active_count() >= self.quotas.max_fds as usize {
            return Err(GuestQuotaError::FdLimit.into());
        }
        let route = PicoFdRoute::new(
            target_node,
            target_role,
            lane,
            route_label,
            session_generation,
            policy_slot,
        );
        Ok(self
            .fds
            .apply_cap_mint(
                fd,
                rights,
                resource,
                choreo_object_id,
                choreo_object_generation,
                route,
            )?
            .into())
    }

    pub fn close_fd_current(&mut self, fd: u8) -> Result<GuestFd, GuestLedgerError> {
        Ok(self.fds.close_current(fd)?.into())
    }

    pub fn resolve_fd(
        &self,
        fd: u8,
        rights: PicoFdRights,
        resource: ChoreoResourceKind,
    ) -> Result<GuestFd, GuestLedgerError> {
        Ok(self.fds.resolve_current(fd, rights, resource)?.into())
    }

    pub fn grant_read_lease(&mut self, borrow: MemBorrow) -> Result<MemGrant, GuestLedgerError> {
        Ok(self.leases.grant_read(borrow)?)
    }

    pub fn grant_write_lease(&mut self, borrow: MemBorrow) -> Result<MemGrant, GuestLedgerError> {
        Ok(self.leases.grant_write(borrow)?)
    }

    pub fn release_lease(&mut self, release: MemRelease) -> Result<MemGrant, GuestLedgerError> {
        Ok(self.leases.release(release)?)
    }

    pub fn validate_fd_write_lease(
        &self,
        write: &FdWrite,
        grant: MemGrant,
    ) -> Result<(), GuestLedgerError> {
        if write.lease_id() != grant.lease_id() || write.len() > grant.len() as usize {
            return Err(GuestLedgerError::LeaseMismatch);
        }
        Ok(())
    }

    pub fn validate_fd_read_lease(
        &self,
        read: &FdRead,
        grant: MemGrant,
    ) -> Result<(), GuestLedgerError> {
        if read.lease_id() != grant.lease_id()
            || read.max_len() > grant.len()
            || grant.rights() != MemRights::Write
        {
            return Err(GuestLedgerError::LeaseMismatch);
        }
        Ok(())
    }

    pub fn begin_fd_write(
        &mut self,
        write: &FdWrite,
        grant: MemGrant,
        resource: ChoreoResourceKind,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.validate_fd_write_lease(write, grant)?;
        let guest_fd = self.resolve_fd(write.fd(), PicoFdRights::Write, resource)?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::FdWrite,
            write.fd(),
            write.lease_id(),
            write.len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_fd_write(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        written: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::FdWrite, fd, lease_id, written),
        )?)
    }

    pub fn begin_fd_read(
        &mut self,
        read: &FdRead,
        grant: MemGrant,
        resource: ChoreoResourceKind,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.validate_fd_read_lease(read, grant)?;
        let guest_fd = self.resolve_fd(read.fd(), PicoFdRights::Read, resource)?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::FdRead,
            read.fd(),
            read.lease_id(),
            read.max_len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_fd_read(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        len: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::FdRead, fd, lease_id, len),
        )?)
    }

    pub fn begin_poll_oneoff(
        &mut self,
        poll: PollOneoff,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.begin_pending(PendingSyscallSpec::timer(
            PendingSyscallKind::PollOneoff,
            poll.timeout_tick(),
        ))
    }

    pub fn complete_poll_oneoff(
        &mut self,
        token: PendingSyscallToken,
        done: TimerSleepDone,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete_poll_oneoff(token, done)?)
    }

    pub fn begin_clock_sleep(
        &mut self,
        tick: u64,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.begin_pending(PendingSyscallSpec::timer(
            PendingSyscallKind::ClockSleep,
            tick,
        ))
    }

    pub fn complete_clock_sleep(
        &mut self,
        token: PendingSyscallToken,
        tick: u64,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::timer(PendingSyscallKind::ClockSleep, tick),
        )?)
    }

    pub fn begin_network_recv(
        &mut self,
        read: &FdRead,
        grant: MemGrant,
        resource: ChoreoResourceKind,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        ensure_network_resource(resource)?;
        self.validate_fd_read_lease(read, grant)?;
        let guest_fd = self.resolve_fd(read.fd(), PicoFdRights::Read, resource)?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::NetworkRecv,
            read.fd(),
            read.lease_id(),
            read.max_len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_network_recv(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        len: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::NetworkRecv, fd, lease_id, len),
        )?)
    }

    pub fn begin_network_send(
        &mut self,
        write: &FdWrite,
        grant: MemGrant,
        resource: ChoreoResourceKind,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        ensure_network_resource(resource)?;
        self.validate_fd_write_lease(write, grant)?;
        let guest_fd = self.resolve_fd(write.fd(), PicoFdRights::Write, resource)?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::NetworkSend,
            write.fd(),
            write.lease_id(),
            write.len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_network_send(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        written: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::NetworkSend, fd, lease_id, written),
        )?)
    }

    pub fn begin_choreofs_read(
        &mut self,
        read: &FdRead,
        grant: MemGrant,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.validate_fd_read_lease(read, grant)?;
        let guest_fd = self.resolve_fd(
            read.fd(),
            PicoFdRights::Read,
            ChoreoResourceKind::ChoreoObject,
        )?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::ChoreoFsRead,
            read.fd(),
            read.lease_id(),
            read.max_len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_choreofs_read(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        len: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::ChoreoFsRead, fd, lease_id, len),
        )?)
    }

    pub fn begin_choreofs_write(
        &mut self,
        write: &FdWrite,
        grant: MemGrant,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.validate_fd_write_lease(write, grant)?;
        let guest_fd = self.resolve_fd(
            write.fd(),
            PicoFdRights::Write,
            ChoreoResourceKind::ChoreoObject,
        )?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::ChoreoFsWrite,
            write.fd(),
            write.lease_id(),
            write.len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_choreofs_write(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        written: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(
                PendingSyscallKind::ChoreoFsWrite,
                fd,
                lease_id,
                written,
            ),
        )?)
    }

    pub fn begin_directory_read(
        &mut self,
        read: &FdRead,
        grant: MemGrant,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        self.validate_fd_read_lease(read, grant)?;
        let guest_fd = self.resolve_fd(
            read.fd(),
            PicoFdRights::Read,
            ChoreoResourceKind::DirectoryView,
        )?;
        self.begin_pending(PendingSyscallSpec::fd_io(
            PendingSyscallKind::DirectoryRead,
            read.fd(),
            read.lease_id(),
            read.max_len() as u16,
            guest_fd.kind(),
        ))
    }

    pub fn complete_directory_read(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
        lease_id: u8,
        len: u16,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_io(PendingSyscallKind::DirectoryRead, fd, lease_id, len),
        )?)
    }

    pub fn begin_sock_accept(&mut self, fd: u8) -> Result<PendingSyscallToken, GuestLedgerError> {
        let guest_fd =
            self.resolve_fd(fd, PicoFdRights::Read, ChoreoResourceKind::NetworkListener)?;
        self.begin_pending(PendingSyscallSpec::fd_event(
            PendingSyscallKind::SockAccept,
            fd,
            guest_fd.kind(),
        ))
    }

    pub fn complete_sock_accept(
        &mut self,
        token: PendingSyscallToken,
        fd: u8,
    ) -> Result<(), GuestLedgerError> {
        Ok(self.pending.complete(
            token,
            PendingSyscallCompletion::fd_event(PendingSyscallKind::SockAccept, fd),
        )?)
    }

    fn begin_pending(
        &mut self,
        spec: PendingSyscallSpec,
    ) -> Result<PendingSyscallToken, GuestLedgerError> {
        if self.pending.pending_count() >= self.quotas.max_pending as usize {
            return Err(GuestQuotaError::PendingLimit.into());
        }
        Ok(self.pending.begin(spec)?)
    }

    pub const fn errno(&self, error: GuestLedgerError) -> u16 {
        match error {
            GuestLedgerError::Fd(error) => self.errno.map_fd_error(error),
            GuestLedgerError::Memory(error) => self.errno.map_memory_error(error),
            GuestLedgerError::Pending(error) => self.errno.map_pending_error(error),
            GuestLedgerError::Quota(error) => self.errno.map_quota_error(error),
            GuestLedgerError::LeaseMismatch => WASI_ERRNO_NOTCAPABLE,
        }
    }
}

fn ensure_network_resource(resource: ChoreoResourceKind) -> Result<(), GuestLedgerError> {
    match resource {
        ChoreoResourceKind::NetworkDatagram | ChoreoResourceKind::NetworkStream => Ok(()),
        _ => Err(GuestLedgerError::Fd(PicoFdError::WrongResource)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GuestFdKind, GuestLedger, GuestLedgerError, GuestQuotaError, GuestQuotaLimits,
        PendingSyscallCompletion, PendingSyscallError, PendingSyscallKind, PendingSyscallSpec,
        PendingSyscallTable, WASI_ERRNO_INVAL, WASI_ERRNO_MFILE, WasiErrnoMap,
    };
    use crate::{
        choreography::protocol::{
            FdRead, FdWrite, LABEL_GPIO_SET, MemBorrow, MemRelease, PollOneoff, TimerSleepDone,
        },
        kernel::wasi::{ChoreoResourceKind, PicoFdError, PicoFdRights},
    };

    #[test]
    fn pico_min_guest_ledger_groups_fd_lease_pending_and_errno() {
        let mut ledger: GuestLedger<2, 1, 1> = GuestLedger::pico_min(4096, 7);
        let guest_fd = ledger
            .apply_fd_cap_grant(
                3,
                PicoFdRights::Write,
                ChoreoResourceKind::Gpio,
                3,
                LABEL_GPIO_SET,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant gpio fd");
        assert_eq!(guest_fd.fd(), 3);
        assert_eq!(guest_fd.kind(), GuestFdKind::Gpio);

        let grant = ledger
            .grant_read_lease(MemBorrow::new(128, 1, 7))
            .expect("grant read lease");
        ledger
            .validate_fd_write_lease(
                &FdWrite::new_with_lease(3, grant.lease_id(), b"1").expect("write"),
                grant,
            )
            .expect("fd_write lease matches");
        ledger
            .release_lease(MemRelease::new(grant.lease_id()))
            .expect("release lease");

        let poll = PollOneoff::new(42);
        let token = ledger.begin_poll_oneoff(poll).expect("begin poll");
        ledger
            .complete_poll_oneoff(token, TimerSleepDone::new(42))
            .expect("complete poll");
    }

    #[test]
    fn pending_syscall_generation_and_completion_are_reject() {
        let mut ledger: GuestLedger<1, 1, 1> = GuestLedger::pico_min(1024, 1);
        let token = ledger
            .begin_poll_oneoff(PollOneoff::new(10))
            .expect("begin poll");

        assert_eq!(
            ledger.complete_poll_oneoff(token, TimerSleepDone::new(11)),
            Err(GuestLedgerError::Pending(
                PendingSyscallError::CompletionMismatch
            ))
        );
        assert_eq!(
            ledger.begin_poll_oneoff(PollOneoff::new(12)),
            Err(GuestLedgerError::Quota(GuestQuotaError::PendingLimit))
        );
        assert_eq!(
            ledger.errno(GuestLedgerError::Pending(
                PendingSyscallError::CompletionMismatch
            )),
            WASI_ERRNO_INVAL
        );
    }

    #[test]
    fn pending_table_tracks_multiple_syscall_kinds_as_linear_tokens() {
        let mut pending: PendingSyscallTable<2> = PendingSyscallTable::new();
        let fd_read = pending
            .begin(PendingSyscallSpec::fd_io(
                PendingSyscallKind::FdRead,
                7,
                2,
                16,
                GuestFdKind::Stream,
            ))
            .expect("begin fd_read");
        assert_eq!(fd_read.kind(), PendingSyscallKind::FdRead);

        assert_eq!(
            pending.complete(
                fd_read,
                PendingSyscallCompletion::fd_io(PendingSyscallKind::NetworkRecv, 7, 2, 4)
            ),
            Err(PendingSyscallError::WrongKind)
        );
        assert_eq!(
            pending.complete(
                fd_read,
                PendingSyscallCompletion::fd_io(PendingSyscallKind::FdRead, 7, 2, 17)
            ),
            Err(PendingSyscallError::CompletionMismatch)
        );
        pending
            .complete(
                fd_read,
                PendingSyscallCompletion::fd_io(PendingSyscallKind::FdRead, 7, 2, 8),
            )
            .expect("complete fd_read");
        assert!(!pending.has_active());

        let dir = pending
            .begin(PendingSyscallSpec::fd_io(
                PendingSyscallKind::DirectoryRead,
                9,
                3,
                64,
                GuestFdKind::DirectoryView,
            ))
            .expect("begin directory read");
        pending
            .complete(
                dir,
                PendingSyscallCompletion::fd_io(PendingSyscallKind::DirectoryRead, 9, 3, 64),
            )
            .expect("complete directory read");
    }

    #[test]
    fn guest_ledger_begins_and_completes_fd_read_write_pending_tokens() {
        let mut ledger: GuestLedger<2, 4, 2> = GuestLedger::pico_min(4096, 1);
        ledger
            .apply_fd_cap_grant(
                0,
                PicoFdRights::Read,
                ChoreoResourceKind::Stdin,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant stdin");
        ledger
            .apply_fd_cap_grant(
                1,
                PicoFdRights::Write,
                ChoreoResourceKind::Stdout,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant stdout");

        let read_grant = ledger
            .grant_write_lease(MemBorrow::new(256, 8, 1))
            .expect("grant write lease for fd_read");
        let read = FdRead::new_with_lease(0, read_grant.lease_id(), 8).expect("fd_read");
        let read_token = ledger
            .begin_fd_read(&read, read_grant, ChoreoResourceKind::Stdin)
            .expect("begin fd_read pending");
        assert_eq!(read_token.kind(), PendingSyscallKind::FdRead);
        ledger
            .complete_fd_read(read_token, 0, read_grant.lease_id(), 5)
            .expect("complete fd_read pending");

        let write_grant = ledger
            .grant_read_lease(MemBorrow::new(512, 4, 1))
            .expect("grant read lease for fd_write");
        let write = FdWrite::new_with_lease(1, write_grant.lease_id(), b"pong").expect("fd_write");
        let write_token = ledger
            .begin_fd_write(&write, write_grant, ChoreoResourceKind::Stdout)
            .expect("begin fd_write pending");
        assert_eq!(
            ledger.complete_fd_write(write_token, 1, write_grant.lease_id(), 5),
            Err(GuestLedgerError::Pending(
                PendingSyscallError::CompletionMismatch
            ))
        );
        ledger
            .complete_fd_write(write_token, 1, write_grant.lease_id(), 4)
            .expect("complete fd_write pending");
    }

    #[test]
    fn guest_ledger_pending_covers_network_and_choreofs_resources() {
        let mut ledger: GuestLedger<4, 4, 2> = GuestLedger::pico_min(4096, 1);
        ledger
            .apply_fd_cap_grant(
                30,
                PicoFdRights::ReadWrite,
                ChoreoResourceKind::NetworkDatagram,
                22,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant datagram fd");
        ledger
            .apply_fd_cap_grant(
                40,
                PicoFdRights::ReadWrite,
                ChoreoResourceKind::ChoreoObject,
                8,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant choreofs object fd");
        ledger
            .apply_fd_cap_grant(
                41,
                PicoFdRights::Read,
                ChoreoResourceKind::DirectoryView,
                8,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant choreofs directory view fd");
        ledger
            .apply_fd_cap_grant(
                50,
                PicoFdRights::Read,
                ChoreoResourceKind::NetworkListener,
                23,
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .expect("grant network listener fd");

        let net_grant = ledger
            .grant_write_lease(MemBorrow::new(1024, 8, 1))
            .expect("grant recv lease");
        let net_read = FdRead::new_with_lease(30, net_grant.lease_id(), 8).expect("network read");
        assert_eq!(
            ledger.begin_network_recv(&net_read, net_grant, ChoreoResourceKind::Gpio),
            Err(GuestLedgerError::Fd(PicoFdError::WrongResource))
        );
        let net_token = ledger
            .begin_network_recv(&net_read, net_grant, ChoreoResourceKind::NetworkDatagram)
            .expect("begin network recv");
        assert_eq!(
            ledger.complete_fd_read(net_token, 30, net_grant.lease_id(), 4),
            Err(GuestLedgerError::Pending(PendingSyscallError::WrongKind))
        );
        ledger
            .complete_network_recv(net_token, 30, net_grant.lease_id(), 4)
            .expect("complete network recv");

        let fs_grant = ledger
            .grant_read_lease(MemBorrow::new(2048, 4, 1))
            .expect("grant choreofs write lease");
        let fs_write =
            FdWrite::new_with_lease(40, fs_grant.lease_id(), b"cfg!").expect("choreofs write");
        let fs_token = ledger
            .begin_choreofs_write(&fs_write, fs_grant)
            .expect("begin choreofs write");
        ledger
            .complete_choreofs_write(fs_token, 40, fs_grant.lease_id(), 4)
            .expect("complete choreofs write");

        let dir_grant = ledger
            .grant_write_lease(MemBorrow::new(3072, 16, 1))
            .expect("grant directory read lease");
        let dir_read =
            FdRead::new_with_lease(41, dir_grant.lease_id(), 16).expect("directory read");
        let dir_token = ledger
            .begin_directory_read(&dir_read, dir_grant)
            .expect("begin directory read");
        ledger
            .complete_directory_read(dir_token, 41, dir_grant.lease_id(), 12)
            .expect("complete directory read");

        let accept_token = ledger.begin_sock_accept(50).expect("begin sock accept");
        assert_eq!(
            ledger.complete_sock_accept(accept_token, 51),
            Err(GuestLedgerError::Pending(
                PendingSyscallError::CompletionMismatch
            ))
        );
        ledger
            .complete_sock_accept(accept_token, 50)
            .expect("complete sock accept");
    }

    #[test]
    fn fd_quota_maps_to_errno_map() {
        let mut ledger: GuestLedger<1, 1, 1> = GuestLedger::new(
            super::WasiProfile::PicoMin,
            1024,
            1,
            GuestQuotaLimits::new(0, 1),
            WasiErrnoMap::new(),
        );
        let err = ledger
            .apply_fd_cap_grant(
                3,
                PicoFdRights::Write,
                ChoreoResourceKind::Gpio,
                3,
                LABEL_GPIO_SET,
                0,
                0,
                0,
                0,
                0,
            )
            .expect_err("quota rejects fd");
        assert_eq!(err, GuestLedgerError::Quota(GuestQuotaError::FdLimit));
        assert_eq!(ledger.errno(err), WASI_ERRNO_MFILE);
    }
}
