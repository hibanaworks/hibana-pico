use crate::{
    choreography::protocol::FdRead,
    kernel::{
        guest_ledger::GuestFd,
        guest_ledger::{GuestFdKind, GuestLedger, GuestLedgerError},
        transaction::ObjectTransaction,
        wasi::{ChoreoResourceKind, PicoFdError, PicoFdRights},
    },
};

pub const CHOREOFS_WASI_ERRNO_NOENT: u16 = 44;
pub const CHOREOFS_WASI_ERRNO_NOSYS: u16 = 52;
pub const CHOREOFS_WASI_ERRNO_NOTDIR: u16 = 54;
pub const CHOREOFS_WASI_ERRNO_NOTCAPABLE: u16 = 76;

pub const WASIP1_RIGHT_FD_DATASYNC: u64 = 1 << 0;
pub const WASIP1_RIGHT_FD_READ: u64 = 1 << 1;
pub const WASIP1_RIGHT_FD_SEEK: u64 = 1 << 2;
pub const WASIP1_RIGHT_FD_SYNC: u64 = 1 << 4;
pub const WASIP1_RIGHT_FD_TELL: u64 = 1 << 5;
pub const WASIP1_RIGHT_FD_WRITE: u64 = 1 << 6;
pub const WASIP1_RIGHT_FD_ALLOCATE: u64 = 1 << 8;
pub const WASIP1_RIGHT_FD_READDIR: u64 = 1 << 14;
pub const WASIP1_RIGHT_FD_FILESTAT_GET: u64 = 1 << 21;
pub const WASIP1_RIGHT_FD_FILESTAT_SET_SIZE: u64 = 1 << 22;
pub const WASIP1_RIGHT_FD_FILESTAT_SET_TIMES: u64 = 1 << 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoreoFsObjectKind {
    StaticBlob,
    ConfigCell,
    AppendLog,
    ImageSlot,
    StateSnapshot,
    Directory,
    GpioDevice,
    TimerDevice,
    UartDevice,
    NetworkDatagram,
    NetworkStream,
    NetworkListener,
    RemoteObject,
    ManagementObject,
    TelemetryObject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoreoFsError {
    TableFull,
    PathTooLong,
    EmptyPath,
    AbsolutePath,
    InvalidComponent,
    NotFound,
    NotDirectory,
    IsDirectory,
    ReadOnly,
    PermissionDenied,
    BufferTooSmall,
    ObjectTooLarge,
    BadObjectId,
    WrongFdKind,
    BadOffset,
    Fd(PicoFdError),
    Ledger(GuestLedgerError),
}

impl From<PicoFdError> for ChoreoFsError {
    fn from(value: PicoFdError) -> Self {
        Self::Fd(value)
    }
}

impl From<GuestLedgerError> for ChoreoFsError {
    fn from(value: GuestLedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl ChoreoFsError {
    pub const fn wasi_errno(self) -> u16 {
        match self {
            Self::NotFound => CHOREOFS_WASI_ERRNO_NOENT,
            Self::NotDirectory => CHOREOFS_WASI_ERRNO_NOTDIR,
            Self::ReadOnly | Self::PermissionDenied | Self::WrongFdKind => {
                CHOREOFS_WASI_ERRNO_NOTCAPABLE
            }
            Self::Fd(error) => crate::kernel::guest_ledger::WasiErrnoMap::new().map_fd_error(error),
            Self::Ledger(error) => match error {
                GuestLedgerError::Fd(error) => {
                    crate::kernel::guest_ledger::WasiErrnoMap::new().map_fd_error(error)
                }
                GuestLedgerError::Memory(error) => {
                    crate::kernel::guest_ledger::WasiErrnoMap::new().map_memory_error(error)
                }
                GuestLedgerError::Pending(error) => {
                    crate::kernel::guest_ledger::WasiErrnoMap::new().map_pending_error(error)
                }
                GuestLedgerError::Quota(error) => {
                    crate::kernel::guest_ledger::WasiErrnoMap::new().map_quota_error(error)
                }
                GuestLedgerError::LeaseMismatch => CHOREOFS_WASI_ERRNO_NOTCAPABLE,
            },
            _ => CHOREOFS_WASI_ERRNO_NOSYS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizedPath<const PATH: usize> {
    bytes: [u8; PATH],
    len: usize,
}

impl<const PATH: usize> NormalizedPath<PATH> {
    pub fn new(path: &[u8]) -> Result<Self, ChoreoFsError> {
        normalize_path(path)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }

    pub const fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsObject<const PATH: usize, const DATA: usize> {
    kind: ChoreoFsObjectKind,
    path: [u8; PATH],
    path_len: usize,
    data: [u8; DATA],
    data_len: usize,
    generation: u16,
}

impl<const PATH: usize, const DATA: usize> ChoreoFsObject<PATH, DATA> {
    const EMPTY: Self = Self {
        kind: ChoreoFsObjectKind::StaticBlob,
        path: [0; PATH],
        path_len: 0,
        data: [0; DATA],
        data_len: 0,
        generation: 0,
    };

    pub const fn kind(&self) -> ChoreoFsObjectKind {
        self.kind
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub fn path(&self) -> &[u8] {
        self.path.split_at(self.path_len).0
    }

    pub fn data(&self) -> &[u8] {
        self.data.split_at(self.data_len).0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsOpened {
    object_id: u16,
    generation: u16,
    resource: ChoreoResourceKind,
}

impl ChoreoFsOpened {
    pub const fn object_id(self) -> u16 {
        self.object_id
    }

    pub const fn generation(self) -> u16 {
        self.generation
    }

    pub const fn resource(self) -> ChoreoResourceKind {
        self.resource
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsDirRead {
    written: usize,
    next_cursor: usize,
    done: bool,
}

impl ChoreoFsDirRead {
    pub const fn written(self) -> usize {
        self.written
    }

    pub const fn next_cursor(self) -> usize {
        self.next_cursor
    }

    pub const fn done(self) -> bool {
        self.done
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsStat {
    kind: ChoreoFsObjectKind,
    size: usize,
}

impl ChoreoFsStat {
    pub const fn new(kind: ChoreoFsObjectKind, size: usize) -> Self {
        Self { kind, size }
    }

    pub const fn kind(self) -> ChoreoFsObjectKind {
        self.kind
    }

    pub const fn size(self) -> usize {
        self.size
    }
}

pub struct ChoreoFsStore<const N: usize, const PATH: usize, const DATA: usize> {
    objects: [Option<ChoreoFsObject<PATH, DATA>>; N],
    next_generation: u16,
}

impl<const N: usize, const PATH: usize, const DATA: usize> ChoreoFsStore<N, PATH, DATA> {
    pub const fn new() -> Self {
        Self {
            objects: [None; N],
            next_generation: 1,
        }
    }

    pub fn install_static_blob(&mut self, path: &[u8], data: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::StaticBlob, path, data)
    }

    pub fn install_static_blob_in_tx(
        &mut self,
        path: &[u8],
        data: &[u8],
        tx: ObjectTransaction,
    ) -> Result<u16, ChoreoFsError> {
        self.require_object_tx(tx, self.next_generation)?;
        self.install_static_blob(path, data)
    }

    pub fn install_config_cell(&mut self, path: &[u8], data: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::ConfigCell, path, data)
    }

    pub fn install_append_log(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::AppendLog, path, &[])
    }

    pub fn install_image_slot(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::ImageSlot, path, &[])
    }

    pub fn install_state_snapshot(
        &mut self,
        path: &[u8],
        data: &[u8],
    ) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::StateSnapshot, path, data)
    }

    pub fn install_state_snapshot_in_tx(
        &mut self,
        path: &[u8],
        data: &[u8],
        tx: ObjectTransaction,
    ) -> Result<u16, ChoreoFsError> {
        self.require_object_tx(tx, self.next_generation)?;
        self.install_state_snapshot(path, data)
    }

    pub fn install_directory(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::Directory, path, &[])
    }

    pub fn install_gpio_device(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::GpioDevice, path, &[])
    }

    pub fn install_timer_device(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::TimerDevice, path, &[])
    }

    pub fn install_uart_device(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::UartDevice, path, &[])
    }

    pub fn install_network_datagram(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::NetworkDatagram, path, &[])
    }

    pub fn install_network_stream(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::NetworkStream, path, &[])
    }

    pub fn install_network_listener(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::NetworkListener, path, &[])
    }

    pub fn install_remote_object(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::RemoteObject, path, &[])
    }

    pub fn install_management_object(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::ManagementObject, path, &[])
    }

    pub fn install_telemetry_object(&mut self, path: &[u8]) -> Result<u16, ChoreoFsError> {
        self.install(ChoreoFsObjectKind::TelemetryObject, path, &[])
    }

    pub fn open(&self, path: &[u8], rights: PicoFdRights) -> Result<ChoreoFsOpened, ChoreoFsError> {
        if rights == PicoFdRights::None {
            return Err(ChoreoFsError::PermissionDenied);
        }
        let path = normalize_path::<PATH>(path)?;
        let object_id = self.lookup(path.as_bytes())?;
        let object = self.object(object_id)?;
        let resource = match object.kind {
            ChoreoFsObjectKind::Directory => {
                if !rights.allows(PicoFdRights::Read) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::DirectoryView
            }
            ChoreoFsObjectKind::StaticBlob => {
                if !rights.allows(PicoFdRights::Read) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::ChoreoObject
            }
            ChoreoFsObjectKind::ConfigCell
            | ChoreoFsObjectKind::AppendLog
            | ChoreoFsObjectKind::ImageSlot
            | ChoreoFsObjectKind::StateSnapshot => ChoreoResourceKind::ChoreoObject,
            ChoreoFsObjectKind::GpioDevice => {
                if !rights.allows(PicoFdRights::Write) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::Gpio
            }
            ChoreoFsObjectKind::TimerDevice => {
                if !rights.allows(PicoFdRights::Read) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::Timer
            }
            ChoreoFsObjectKind::UartDevice => {
                if !rights.allows(PicoFdRights::Write) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::Uart
            }
            ChoreoFsObjectKind::NetworkDatagram => ChoreoResourceKind::NetworkDatagram,
            ChoreoFsObjectKind::NetworkStream => ChoreoResourceKind::NetworkStream,
            ChoreoFsObjectKind::NetworkListener => {
                if !rights.allows(PicoFdRights::Read) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::NetworkListener
            }
            ChoreoFsObjectKind::RemoteObject => ChoreoResourceKind::RemoteObject,
            ChoreoFsObjectKind::ManagementObject => ChoreoResourceKind::Management,
            ChoreoFsObjectKind::TelemetryObject => {
                if !rights.allows(PicoFdRights::Write) {
                    return Err(ChoreoFsError::PermissionDenied);
                }
                ChoreoResourceKind::Telemetry
            }
        };
        Ok(ChoreoFsOpened {
            object_id: object_id as u16,
            generation: object.generation,
            resource,
        })
    }

    pub fn stat_fd(&self, fd: GuestFd) -> Result<ChoreoFsStat, ChoreoFsError> {
        match fd.kind() {
            GuestFdKind::ChoreoObject | GuestFdKind::DirectoryView => {
                let object = self.object_from_fd(fd)?;
                Ok(ChoreoFsStat::new(object.kind, object.data_len))
            }
            _ => Err(ChoreoFsError::WrongFdKind),
        }
    }

    pub fn stat_path(&self, path: &[u8]) -> Result<ChoreoFsStat, ChoreoFsError> {
        let path = normalize_path::<PATH>(path)?;
        let object = self.object(self.lookup(path.as_bytes())?)?;
        Ok(ChoreoFsStat::new(object.kind, object.data_len))
    }

    pub fn read(&self, fd: GuestFd, offset: usize, out: &mut [u8]) -> Result<usize, ChoreoFsError> {
        if fd.kind() != GuestFdKind::ChoreoObject {
            return Err(ChoreoFsError::WrongFdKind);
        }
        if !fd.rights().allows(PicoFdRights::Read) {
            return Err(ChoreoFsError::PermissionDenied);
        }
        let object = self.object_from_fd(fd)?;
        match object.kind {
            ChoreoFsObjectKind::Directory => return Err(ChoreoFsError::IsDirectory),
            ChoreoFsObjectKind::GpioDevice
            | ChoreoFsObjectKind::TimerDevice
            | ChoreoFsObjectKind::UartDevice
            | ChoreoFsObjectKind::NetworkDatagram
            | ChoreoFsObjectKind::NetworkStream
            | ChoreoFsObjectKind::NetworkListener
            | ChoreoFsObjectKind::RemoteObject
            | ChoreoFsObjectKind::ManagementObject
            | ChoreoFsObjectKind::TelemetryObject => return Err(ChoreoFsError::WrongFdKind),
            _ => {}
        }
        let data = object.data();
        if offset >= data.len() {
            return Ok(0);
        }
        let len = core::cmp::min(out.len(), data.len() - offset);
        out[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    pub fn write(
        &mut self,
        fd: GuestFd,
        offset: usize,
        bytes: &[u8],
    ) -> Result<usize, ChoreoFsError> {
        if fd.kind() != GuestFdKind::ChoreoObject {
            return Err(ChoreoFsError::WrongFdKind);
        }
        if !fd.rights().allows(PicoFdRights::Write) {
            return Err(ChoreoFsError::PermissionDenied);
        }
        let index = fd.wait_or_subscription_id() as usize;
        let object = self.object_mut(index)?;
        match object.kind {
            ChoreoFsObjectKind::StaticBlob => Err(ChoreoFsError::ReadOnly),
            ChoreoFsObjectKind::Directory => Err(ChoreoFsError::IsDirectory),
            ChoreoFsObjectKind::GpioDevice
            | ChoreoFsObjectKind::TimerDevice
            | ChoreoFsObjectKind::UartDevice
            | ChoreoFsObjectKind::NetworkDatagram
            | ChoreoFsObjectKind::NetworkStream
            | ChoreoFsObjectKind::NetworkListener
            | ChoreoFsObjectKind::RemoteObject
            | ChoreoFsObjectKind::ManagementObject
            | ChoreoFsObjectKind::TelemetryObject => Err(ChoreoFsError::WrongFdKind),
            ChoreoFsObjectKind::ConfigCell => {
                if offset != 0 {
                    return Err(ChoreoFsError::BadOffset);
                }
                if bytes.len() > DATA {
                    return Err(ChoreoFsError::ObjectTooLarge);
                }
                object.data[..bytes.len()].copy_from_slice(bytes);
                object.data_len = bytes.len();
                Ok(bytes.len())
            }
            ChoreoFsObjectKind::AppendLog => {
                if offset != object.data_len {
                    return Err(ChoreoFsError::BadOffset);
                }
                let end = object
                    .data_len
                    .checked_add(bytes.len())
                    .ok_or(ChoreoFsError::ObjectTooLarge)?;
                if end > DATA {
                    return Err(ChoreoFsError::ObjectTooLarge);
                }
                object.data[object.data_len..end].copy_from_slice(bytes);
                object.data_len = end;
                Ok(bytes.len())
            }
            ChoreoFsObjectKind::ImageSlot => {
                if offset > object.data_len {
                    return Err(ChoreoFsError::BadOffset);
                }
                let end = offset
                    .checked_add(bytes.len())
                    .ok_or(ChoreoFsError::ObjectTooLarge)?;
                if end > DATA {
                    return Err(ChoreoFsError::ObjectTooLarge);
                }
                object.data[offset..end].copy_from_slice(bytes);
                object.data_len = core::cmp::max(object.data_len, end);
                Ok(bytes.len())
            }
            ChoreoFsObjectKind::StateSnapshot => {
                if offset != 0 {
                    return Err(ChoreoFsError::BadOffset);
                }
                if bytes.len() > DATA {
                    return Err(ChoreoFsError::ObjectTooLarge);
                }
                object.data[..bytes.len()].copy_from_slice(bytes);
                object.data_len = bytes.len();
                Ok(bytes.len())
            }
        }
    }

    pub fn write_in_tx(
        &mut self,
        fd: GuestFd,
        offset: usize,
        bytes: &[u8],
        tx: ObjectTransaction,
    ) -> Result<usize, ChoreoFsError> {
        self.require_object_tx(tx, fd.generation())?;
        self.write(fd, offset, bytes)
    }

    fn require_object_tx(
        &self,
        tx: ObjectTransaction,
        generation: u16,
    ) -> Result<(), ChoreoFsError> {
        if tx.generation() != generation {
            return Err(ChoreoFsError::BadObjectId);
        }
        if !tx.is_commit() {
            return Err(ChoreoFsError::PermissionDenied);
        }
        Ok(())
    }

    pub fn read_directory(
        &self,
        fd: GuestFd,
        cursor: usize,
        out: &mut [u8],
    ) -> Result<ChoreoFsDirRead, ChoreoFsError> {
        if fd.kind() != GuestFdKind::DirectoryView {
            return Err(ChoreoFsError::WrongFdKind);
        }
        if !fd.rights().allows(PicoFdRights::Read) {
            return Err(ChoreoFsError::PermissionDenied);
        }
        let dir = self.object_from_fd(fd)?;
        if dir.kind != ChoreoFsObjectKind::Directory {
            return Err(ChoreoFsError::NotDirectory);
        }

        let mut written = 0usize;
        let mut index = cursor;
        while index < N {
            let Some(child) = self.objects[index] else {
                index += 1;
                continue;
            };
            if let Some(name) = direct_child_name(dir.path(), child.path()) {
                let needed = name.len().saturating_add(1);
                if written + needed > out.len() {
                    if written == 0 {
                        return Err(ChoreoFsError::BufferTooSmall);
                    }
                    break;
                }
                out[written..written + name.len()].copy_from_slice(name);
                written += name.len();
                out[written] = b'\n';
                written += 1;
            }
            index += 1;
        }
        Ok(ChoreoFsDirRead {
            written,
            next_cursor: index,
            done: index >= N,
        })
    }

    pub fn begin_directory_read<const FDS: usize, const LEASES: usize, const PENDING: usize>(
        &self,
        ledger: &mut GuestLedger<FDS, LEASES, PENDING>,
        read: &FdRead,
        grant: crate::choreography::protocol::MemGrant,
    ) -> Result<crate::kernel::guest_ledger::PendingSyscallToken, ChoreoFsError> {
        Ok(ledger.begin_directory_read(read, grant)?)
    }

    fn install(
        &mut self,
        kind: ChoreoFsObjectKind,
        path: &[u8],
        data: &[u8],
    ) -> Result<u16, ChoreoFsError> {
        let path = normalize_path::<PATH>(path)?;
        if data.len() > DATA {
            return Err(ChoreoFsError::ObjectTooLarge);
        }
        if self.lookup(path.as_bytes()).is_ok() {
            return Err(ChoreoFsError::PermissionDenied);
        }
        let Some((index, slot)) = self
            .objects
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.is_none())
        else {
            return Err(ChoreoFsError::TableFull);
        };
        let mut object = ChoreoFsObject::EMPTY;
        object.kind = kind;
        object.path[..path.len()].copy_from_slice(path.as_bytes());
        object.path_len = path.len();
        object.data[..data.len()].copy_from_slice(data);
        object.data_len = data.len();
        object.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        *slot = Some(object);
        Ok(index as u16)
    }

    fn lookup(&self, path: &[u8]) -> Result<usize, ChoreoFsError> {
        self.objects
            .iter()
            .enumerate()
            .find_map(|(index, object)| {
                object.filter(|object| object.path() == path).map(|_| index)
            })
            .ok_or(ChoreoFsError::NotFound)
    }

    fn object(&self, index: usize) -> Result<ChoreoFsObject<PATH, DATA>, ChoreoFsError> {
        self.objects
            .get(index)
            .and_then(|slot| *slot)
            .ok_or(ChoreoFsError::BadObjectId)
    }

    fn object_mut(
        &mut self,
        index: usize,
    ) -> Result<&mut ChoreoFsObject<PATH, DATA>, ChoreoFsError> {
        self.objects
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(ChoreoFsError::BadObjectId)
    }

    fn object_from_fd(&self, fd: GuestFd) -> Result<ChoreoFsObject<PATH, DATA>, ChoreoFsError> {
        let object = self.object(fd.wait_or_subscription_id() as usize)?;
        if object.generation != fd.choreo_object_generation() {
            return Err(ChoreoFsError::BadObjectId);
        }
        Ok(object)
    }
}

impl<const N: usize, const PATH: usize, const DATA: usize> Default
    for ChoreoFsStore<N, PATH, DATA>
{
    fn default() -> Self {
        Self::new()
    }
}

pub const fn pico_rights_from_wasip1_base(rights_base: u64) -> PicoFdRights {
    let reads = rights_base
        & (WASIP1_RIGHT_FD_READ
            | WASIP1_RIGHT_FD_SEEK
            | WASIP1_RIGHT_FD_TELL
            | WASIP1_RIGHT_FD_READDIR
            | WASIP1_RIGHT_FD_FILESTAT_GET)
        != 0;
    let writes = rights_base
        & (WASIP1_RIGHT_FD_DATASYNC
            | WASIP1_RIGHT_FD_SYNC
            | WASIP1_RIGHT_FD_WRITE
            | WASIP1_RIGHT_FD_ALLOCATE
            | WASIP1_RIGHT_FD_FILESTAT_SET_SIZE
            | WASIP1_RIGHT_FD_FILESTAT_SET_TIMES)
        != 0;
    match (reads, writes) {
        (true, true) => PicoFdRights::ReadWrite,
        (false, true) => PicoFdRights::Write,
        (true, false) => PicoFdRights::Read,
        (false, false) => PicoFdRights::None,
    }
}

fn normalize_path<const PATH: usize>(path: &[u8]) -> Result<NormalizedPath<PATH>, ChoreoFsError> {
    if path.is_empty() {
        return Err(ChoreoFsError::EmptyPath);
    }
    if path[0] == b'/' {
        return Err(ChoreoFsError::AbsolutePath);
    }
    if path.len() > PATH {
        return Err(ChoreoFsError::PathTooLong);
    }

    let mut out = [0u8; PATH];
    let mut out_len = 0usize;
    let mut component_start = 0usize;
    let mut index = 0usize;
    while index <= path.len() {
        if index == path.len() || path[index] == b'/' {
            let component = &path[component_start..index];
            if component.is_empty() || component == b"." || component == b".." {
                return Err(ChoreoFsError::InvalidComponent);
            }
            for byte in component {
                if *byte == 0 {
                    return Err(ChoreoFsError::InvalidComponent);
                }
            }
            if out_len != 0 {
                if out_len >= PATH {
                    return Err(ChoreoFsError::PathTooLong);
                }
                out[out_len] = b'/';
                out_len += 1;
            }
            if out_len + component.len() > PATH {
                return Err(ChoreoFsError::PathTooLong);
            }
            out[out_len..out_len + component.len()].copy_from_slice(component);
            out_len += component.len();
            component_start = index.saturating_add(1);
        }
        index += 1;
    }

    Ok(NormalizedPath {
        bytes: out,
        len: out_len,
    })
}

fn direct_child_name<'a>(dir: &[u8], child: &'a [u8]) -> Option<&'a [u8]> {
    if dir == child {
        return None;
    }
    let rest = if dir.is_empty() {
        child
    } else {
        child.strip_prefix(dir)?.strip_prefix(b"/")?
    };
    if rest.is_empty() || rest.contains(&b'/') {
        None
    } else {
        Some(rest)
    }
}
