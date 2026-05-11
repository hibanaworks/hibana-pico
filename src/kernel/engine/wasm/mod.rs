//! Public Wasm/WASI P1 engine facade.
//!
//! The engine has one public handle: [`Guest`].  The parser, interpreter,
//! import lowering, memory writeback, and pending slot live in the private
//! `vm` module.

mod vm;

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "wasm-engine-static-placement"
))]
use core::mem::MaybeUninit;

use crate::{
    choreography::protocol::{BudgetExpired, BudgetRun, ProcExitStatus},
    kernel::features::Wasip1HandlerSet,
};

pub use vm::{
    FdStat, FileStat, MemoryGrowEvent as MemoryGrow, PathBytes, WASIP1_FILETYPE_DIRECTORY,
    WASIP1_FILETYPE_REGULAR_FILE,
};

pub type Error = vm::WasmError;

pub struct Guest<'a> {
    vm: vm::Vm<'a>,
}

impl<'a> Guest<'a> {
    pub fn new(module: &'a [u8]) -> Result<Self, Error> {
        Ok(Self {
            vm: vm::Vm::new(module, Wasip1HandlerSet::active())?,
        })
    }

    #[cfg(all(
        target_arch = "arm",
        target_os = "none",
        feature = "wasm-engine-static-placement"
    ))]
    pub(crate) fn place_in_static_slot<'slot>(
        slot: &'slot mut MaybeUninit<Self>,
        module: &'a [u8],
    ) -> Result<&'slot mut Self, Error> {
        #[cfg(feature = "wasm-engine-wasip1-full")]
        {
            Ok(slot.write(Self::new(module)?))
        }
        #[cfg(not(feature = "wasm-engine-wasip1-full"))]
        {
            let guest = slot.as_mut_ptr();
            unsafe {
                let vm_slot =
                    &mut *core::ptr::addr_of_mut!((*guest).vm).cast::<MaybeUninit<vm::Vm<'a>>>();
                vm::Vm::initialize(vm_slot, module, Wasip1HandlerSet::active())?;
                Ok(&mut *guest)
            }
        }
    }

    pub fn resume<'guest>(&'guest mut self, budget: BudgetRun) -> Result<Event<'guest, 'a>, Error> {
        match self.vm.resume(budget) {
            Ok(vm::VmEvent::FdWrite(call)) => Ok(Event::Call(Call::FdWrite(Pending::new(
                self,
                FdWrite { call },
            )))),
            Ok(vm::VmEvent::FdRead(call)) => Ok(Event::Call(Call::FdRead(Pending::new(
                self,
                FdRead { call },
            )))),
            Ok(vm::VmEvent::FdFdstatGet(call)) => Ok(Event::Call(Call::FdFdstatGet(Pending::new(
                self,
                FdFdstatGet { call },
            )))),
            Ok(vm::VmEvent::FdClose(call)) => Ok(Event::Call(Call::FdClose(Pending::new(
                self,
                FdClose { call },
            )))),
            Ok(vm::VmEvent::ClockResGet(call)) => Ok(Event::Call(Call::ClockResGet(Pending::new(
                self,
                ClockResGet { call },
            )))),
            Ok(vm::VmEvent::ClockTimeGet(call)) => Ok(Event::Call(Call::ClockTimeGet(
                Pending::new(self, ClockTimeGet { call }),
            ))),
            Ok(vm::VmEvent::PollOneoff(call)) => Ok(Event::Call(Call::PollOneoff(Pending::new(
                self,
                PollOneoff { call },
            )))),
            Ok(vm::VmEvent::RandomGet(call)) => Ok(Event::Call(Call::RandomGet(Pending::new(
                self,
                RandomGet { call },
            )))),
            Ok(vm::VmEvent::SchedYield) => Ok(Event::Call(Call::SchedYield(Pending::new(
                self,
                SchedYield { _private: () },
            )))),
            Ok(vm::VmEvent::PathMinimal(call)) => Ok(Event::Call(Call::Path(Pending::new(
                self,
                Path { call, full: false },
            )))),
            Ok(vm::VmEvent::PathFull(call)) => Ok(Event::Call(Call::Path(Pending::new(
                self,
                Path { call, full: true },
            )))),
            Ok(vm::VmEvent::Socket(call)) => Ok(Event::Call(Call::Socket(Pending::new(
                self,
                Socket { call },
            )))),
            Ok(vm::VmEvent::ArgsSizesGet(call)) => Ok(Event::Call(Call::ArgsSizesGet(
                Pending::new(self, ArgsSizesGet { call }),
            ))),
            Ok(vm::VmEvent::ArgsGet(call)) => Ok(Event::Call(Call::ArgsGet(Pending::new(
                self,
                ArgsGet { call },
            )))),
            Ok(vm::VmEvent::EnvironSizesGet(call)) => Ok(Event::Call(Call::EnvironSizesGet(
                Pending::new(self, EnvironSizesGet { call }),
            ))),
            Ok(vm::VmEvent::EnvironGet(call)) => Ok(Event::Call(Call::EnvironGet(Pending::new(
                self,
                EnvironGet { call },
            )))),
            Ok(vm::VmEvent::ProcRaise(code)) => Ok(Event::Call(Call::ProcRaise(Pending::new(
                self,
                ProcRaise { code },
            )))),
            Ok(vm::VmEvent::MemoryGrow(event)) => Ok(Event::Call(Call::MemoryGrow(Pending::new(
                self,
                MemoryGrowCall { event },
            )))),
            Ok(vm::VmEvent::BudgetExpired(expired)) => Ok(Event::BudgetExpired(expired)),
            Ok(vm::VmEvent::ProcExit(status)) => Ok(Event::Exit(ProcExit::new(status))),
            Ok(vm::VmEvent::Done) => Ok(Event::Done),
            Err(error) => Err(error),
        }
    }
}

pub enum Event<'guest, 'a> {
    Call(Call<'guest, 'a>),
    BudgetExpired(BudgetExpired),
    Done,
    Exit(ProcExit),
}

pub enum Call<'guest, 'a> {
    FdWrite(Pending<'guest, 'a, FdWrite>),
    FdRead(Pending<'guest, 'a, FdRead>),
    FdFdstatGet(Pending<'guest, 'a, FdFdstatGet>),
    FdClose(Pending<'guest, 'a, FdClose>),
    ClockResGet(Pending<'guest, 'a, ClockResGet>),
    ClockTimeGet(Pending<'guest, 'a, ClockTimeGet>),
    PollOneoff(Pending<'guest, 'a, PollOneoff>),
    RandomGet(Pending<'guest, 'a, RandomGet>),
    SchedYield(Pending<'guest, 'a, SchedYield>),
    Path(Pending<'guest, 'a, Path>),
    Socket(Pending<'guest, 'a, Socket>),
    ArgsSizesGet(Pending<'guest, 'a, ArgsSizesGet>),
    ArgsGet(Pending<'guest, 'a, ArgsGet>),
    EnvironSizesGet(Pending<'guest, 'a, EnvironSizesGet>),
    EnvironGet(Pending<'guest, 'a, EnvironGet>),
    ProcRaise(Pending<'guest, 'a, ProcRaise>),
    MemoryGrow(Pending<'guest, 'a, MemoryGrowCall>),
}

pub struct Pending<'guest, 'a, K> {
    guest: &'guest mut Guest<'a>,
    call: K,
}

impl<'guest, 'a, K> Pending<'guest, 'a, K> {
    fn new(guest: &'guest mut Guest<'a>, call: K) -> Self {
        Self { guest, call }
    }

    fn vm(&self) -> &vm::Vm<'a> {
        &self.guest.vm
    }

    fn complete_with<R>(self, f: impl FnOnce(&mut vm::Vm<'a>, K) -> R) -> R {
        let Self { guest, call } = self;
        f(&mut guest.vm, call)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcExit {
    status: u32,
}

impl ProcExit {
    const fn new(status: u32) -> Self {
        Self { status }
    }

    pub const fn status(self) -> u32 {
        self.status
    }

    pub const fn as_protocol_status(self) -> Option<ProcExitStatus> {
        if self.status <= u8::MAX as u32 {
            Some(ProcExitStatus::new(self.status as u8))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    FdPrestatGet,
    FdPrestatDirName,
    FdFilestatGet,
    FdReaddir,
    FdAdvise,
    FdAllocate,
    FdDatasync,
    FdFdstatSetFlags,
    FdFdstatSetRights,
    FdFilestatSetSize,
    FdFilestatSetTimes,
    FdPread,
    FdPwrite,
    FdRenumber,
    FdSeek,
    FdSync,
    FdTell,
    PathOpen,
    PathFilestatGet,
    PathReadlink,
    PathCreateDirectory,
    PathRemoveDirectory,
    PathUnlinkFile,
    PathRename,
    PathFilestatSetTimes,
    PathLink,
    PathSymlink,
}

impl From<vm::PathOp> for PathKind {
    fn from(value: vm::PathOp) -> Self {
        match value {
            vm::PathOp::FdPrestatGet => Self::FdPrestatGet,
            vm::PathOp::FdPrestatDirName => Self::FdPrestatDirName,
            vm::PathOp::FdFilestatGet => Self::FdFilestatGet,
            vm::PathOp::FdReaddir => Self::FdReaddir,
            vm::PathOp::FdAdvise => Self::FdAdvise,
            vm::PathOp::FdAllocate => Self::FdAllocate,
            vm::PathOp::FdDatasync => Self::FdDatasync,
            vm::PathOp::FdFdstatSetFlags => Self::FdFdstatSetFlags,
            vm::PathOp::FdFdstatSetRights => Self::FdFdstatSetRights,
            vm::PathOp::FdFilestatSetSize => Self::FdFilestatSetSize,
            vm::PathOp::FdFilestatSetTimes => Self::FdFilestatSetTimes,
            vm::PathOp::FdPread => Self::FdPread,
            vm::PathOp::FdPwrite => Self::FdPwrite,
            vm::PathOp::FdRenumber => Self::FdRenumber,
            vm::PathOp::FdSeek => Self::FdSeek,
            vm::PathOp::FdSync => Self::FdSync,
            vm::PathOp::FdTell => Self::FdTell,
            vm::PathOp::PathOpen => Self::PathOpen,
            vm::PathOp::PathFilestatGet => Self::PathFilestatGet,
            vm::PathOp::PathReadlink => Self::PathReadlink,
            vm::PathOp::PathCreateDirectory => Self::PathCreateDirectory,
            vm::PathOp::PathRemoveDirectory => Self::PathRemoveDirectory,
            vm::PathOp::PathUnlinkFile => Self::PathUnlinkFile,
            vm::PathOp::PathRename => Self::PathRename,
            vm::PathOp::PathFilestatSetTimes => Self::PathFilestatSetTimes,
            vm::PathOp::PathLink => Self::PathLink,
            vm::PathOp::PathSymlink => Self::PathSymlink,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketKind {
    SockAccept,
    SockRecv,
    SockSend,
    SockShutdown,
}

impl From<vm::SocketOp> for SocketKind {
    fn from(value: vm::SocketOp) -> Self {
        match value {
            vm::SocketOp::SockAccept => Self::SockAccept,
            vm::SocketOp::SockRecv => Self::SockRecv,
            vm::SocketOp::SockSend => Self::SockSend,
            vm::SocketOp::SockShutdown => Self::SockShutdown,
        }
    }
}

pub struct Payload {
    raw: vm::InlinePayload,
}

impl Payload {
    pub fn as_bytes(&self) -> &[u8] {
        self.raw.as_bytes()
    }
}

pub struct FdWrite {
    call: vm::FdWriteCall,
}

impl Pending<'_, '_, FdWrite> {
    pub const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub fn payload_len(&self) -> Result<usize, Error> {
        Ok(self.vm().fd_write_total_len(self.call.call)? as usize)
    }

    pub fn copy_payload_into(&self, out: &mut [u8]) -> Result<usize, Error> {
        self.vm().copy_fd_write_payload(self.call.call, out)
    }

    pub fn payload(&self) -> Result<Payload, Error> {
        Ok(Payload {
            raw: self.vm().fd_write_payload(self.call.call)?,
        })
    }

    pub fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_write(call.call, errno))
    }
}

pub struct FdRead {
    call: vm::FdReadCall,
}

impl Pending<'_, '_, FdRead> {
    pub const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub fn max_len(&self) -> Result<usize, Error> {
        let (_, max_len) = self.vm().fd_read_iovec(self.call.call)?;
        Ok(max_len as usize)
    }

    pub fn complete(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_read(call.call, bytes, errno))
    }
}

pub struct FdFdstatGet {
    call: vm::FdRequestCall,
}

impl Pending<'_, '_, FdFdstatGet> {
    pub const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub fn complete(self, stat: FdStat, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_fdstat_get(call.call, stat, errno))
    }
}

pub struct FdClose {
    call: vm::FdRequestCall,
}

impl Pending<'_, '_, FdClose> {
    pub const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, _| vm.complete_host_call(errno))
    }
}

pub struct ClockResGet {
    call: vm::ClockResGetCall,
}

impl Pending<'_, '_, ClockResGet> {
    pub const fn clock_id(&self) -> u32 {
        self.call.call.clock_id()
    }

    pub fn complete(self, resolution_nanos: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_clock_res_get(call.call, resolution_nanos, errno))
    }
}

pub struct ClockTimeGet {
    call: vm::ClockTimeGetCall,
}

impl Pending<'_, '_, ClockTimeGet> {
    pub const fn clock_id(&self) -> u32 {
        self.call.call.clock_id()
    }

    pub fn complete(self, nanos: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_clock_time_get(call.call, nanos, errno))
    }
}

pub struct PollOneoff {
    call: vm::PollOneoffCall,
}

impl Pending<'_, '_, PollOneoff> {
    pub fn delay_ticks(&self) -> Result<u64, Error> {
        self.vm().poll_oneoff_delay_ticks(self.call.call)
    }

    pub fn complete(self, ready: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_poll_oneoff(call.call, ready, errno))
    }
}

pub struct RandomGet {
    call: vm::RandomGetCall,
}

impl Pending<'_, '_, RandomGet> {
    pub const fn len(&self) -> u32 {
        self.call.call.buf_len()
    }

    pub const fn buf_len(&self) -> u32 {
        self.call.call.buf_len()
    }

    pub fn complete(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_random_get(call.call, bytes, errno))
    }
}

pub struct SchedYield {
    _private: (),
}

impl Pending<'_, '_, SchedYield> {
    pub fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, _| vm.complete_sched_yield(errno))
    }
}

pub struct Path {
    call: vm::PathCall,
    full: bool,
}

impl Pending<'_, '_, Path> {
    pub const fn is_full(&self) -> bool {
        self.call.full
    }

    pub fn kind(&self) -> PathKind {
        self.call.call.kind().into()
    }

    pub fn fd(&self) -> Result<u8, Error> {
        self.call.call.fd()
    }

    pub fn arg_i32(&self, index: usize) -> Result<u32, Error> {
        self.call.call.arg_i32(index)
    }

    pub fn arg_i64(&self, index: usize) -> Result<u64, Error> {
        self.call.call.arg_i64(index)
    }

    pub fn path_bytes(&self) -> Result<PathBytes, Error> {
        self.vm().path_bytes(self.call.call)
    }

    pub fn complete_errno(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| {
            if call.full {
                vm.complete_path_full(call.call, errno)
            } else {
                vm.complete_path_minimal(call.call, errno)
            }
        })
    }

    pub fn complete_path_open(self, opened_fd: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_path_open(call.call, opened_fd, errno))
    }

    pub fn complete_fd_prestat_get(self, name_len: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_prestat_get(call.call, name_len, errno))
    }

    pub fn complete_fd_prestat_dir_name(self, name: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_prestat_dir_name(call.call, name, errno))
    }

    pub fn complete_fd_filestat_get(self, stat: FileStat, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_filestat_get(call.call, stat, errno))
    }

    pub fn complete_path_filestat_get(self, stat: FileStat, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_path_filestat_get(call.call, stat, errno))
    }

    pub fn complete_path_readlink(self, target: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_path_readlink(call.call, target, errno))
    }

    pub fn complete_fd_readdir(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_readdir(call.call, bytes, errno))
    }

    pub fn complete_fd_pread(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_pread(call.call, bytes, errno))
    }

    pub fn complete_fd_pwrite(self, written: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_pwrite(call.call, written, errno))
    }

    pub fn complete_fd_seek(self, offset: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_seek(call.call, offset, errno))
    }

    pub fn complete_fd_tell(self, offset: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_fd_tell(call.call, offset, errno))
    }
}

pub struct Socket {
    call: vm::SocketCall,
}

impl Pending<'_, '_, Socket> {
    pub fn kind(&self) -> SocketKind {
        self.call.call.kind().into()
    }

    pub fn fd(&self) -> Result<u8, Error> {
        self.call.call.fd()
    }

    pub fn payload(&self) -> Result<Payload, Error> {
        Ok(Payload {
            raw: self.vm().sock_send_payload(self.call.call)?,
        })
    }

    pub fn payload_len(&self) -> Result<usize, Error> {
        Ok(self.vm().sock_send_total_len(self.call.call)? as usize)
    }

    pub fn copy_payload_into(&self, out: &mut [u8]) -> Result<usize, Error> {
        self.vm().copy_sock_send_payload(self.call.call, out)
    }

    pub fn max_recv_len(&self) -> Result<usize, Error> {
        let (_, max_len) = self.vm().sock_recv_iovec(self.call.call)?;
        Ok(max_len as usize)
    }

    pub fn complete_errno(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_socket(call.call, errno))
    }

    pub fn complete_sock_send(self, written: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_sock_send(call.call, written, errno))
    }

    pub fn complete_sock_recv(self, bytes: &[u8], ro_flags: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_sock_recv(call.call, bytes, ro_flags, errno))
    }

    pub fn complete_sock_shutdown(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_sock_shutdown(call.call, errno))
    }

    pub fn complete_sock_accept(self, accepted_fd: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_sock_accept(call.call, accepted_fd, errno))
    }
}

pub struct ArgsSizesGet {
    call: vm::ArgsSizesGetCall,
}

impl Pending<'_, '_, ArgsSizesGet> {
    pub fn complete(self, argc: u32, argv_buf_size: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| {
            vm.complete_args_sizes_get(call.call, argc, argv_buf_size, errno)
        })
    }
}

pub struct ArgsGet {
    call: vm::ArgsGetCall,
}

impl Pending<'_, '_, ArgsGet> {
    pub fn complete(self, args: &[&[u8]], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_args_get(call.call, args, errno))
    }
}

pub struct EnvironSizesGet {
    call: vm::EnvironSizesGetCall,
}

impl Pending<'_, '_, EnvironSizesGet> {
    pub fn complete(self, count: u32, buf_size: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| {
            vm.complete_environ_sizes_get(call.call, count, buf_size, errno)
        })
    }
}

pub struct EnvironGet {
    call: vm::EnvironGetCall,
}

impl Pending<'_, '_, EnvironGet> {
    pub fn complete(self, environ: &[(&[u8], &[u8])], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.complete_environ_get(call.call, environ, errno))
    }
}

pub struct ProcRaise {
    code: u32,
}

impl Pending<'_, '_, ProcRaise> {
    pub const fn code(&self) -> u32 {
        self.call.code
    }

    pub fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, _| vm.complete_proc_raise(errno))
    }
}

pub struct MemoryGrowCall {
    event: MemoryGrow,
}

impl Pending<'_, '_, MemoryGrowCall> {
    pub const fn event(&self) -> MemoryGrow {
        self.call.event
    }

    pub fn complete(self) -> Result<MemoryGrow, Error> {
        self.complete_with(|vm, _| vm.complete_memory_grow_event())
    }
}
