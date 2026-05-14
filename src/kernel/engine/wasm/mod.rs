//! Private Wasm/WASI P1 engine facade.
//!
//! The engine boundary has one handle: [`Guest`]. The parser, interpreter,
//! import lowering, memory writeback, and pending slot live in `substrate`.

mod substrate;

use crate::{
    choreography::protocol::{BudgetExpired, BudgetRun, ProcExitStatus},
    kernel::features::Wasip1HandlerSet,
};

pub(crate) use substrate::{FdStat, PathBytes};

pub(crate) type Error = substrate::WasmError;

pub(crate) struct Guest<'a> {
    engine: substrate::Vm<'a>,
}

impl<'a> Guest<'a> {
    pub(crate) unsafe fn init_in_place(dst: *mut Self, module: &'a [u8]) -> Result<(), Error> {
        unsafe {
            substrate::Vm::init_in_place(
                core::ptr::addr_of_mut!((*dst).engine),
                module,
                Wasip1HandlerSet::active(),
            )?;
        }
        Ok(())
    }

    pub(crate) fn resume<'guest>(
        &'guest mut self,
        budget: BudgetRun,
    ) -> Result<Event<'guest, 'a>, Error> {
        match self.engine.resume(budget) {
            Ok(substrate::VmEvent::FdWrite(call)) => Ok(Event::Call(Call::FdWrite(Pending::new(
                self,
                FdWrite { call },
            )))),
            Ok(substrate::VmEvent::FdRead(call)) => Ok(Event::Call(Call::FdRead(Pending::new(
                self,
                FdRead { call },
            )))),
            Ok(substrate::VmEvent::FdFdstatGet(call)) => Ok(Event::Call(Call::FdFdstatGet(
                Pending::new(self, FdFdstatGet { call }),
            ))),
            Ok(substrate::VmEvent::FdClose(call)) => Ok(Event::Call(Call::FdClose(Pending::new(
                self,
                FdClose { call },
            )))),
            Ok(substrate::VmEvent::ClockResGet(call)) => Ok(Event::Call(Call::ClockResGet(
                Pending::new(self, ClockResGet { call }),
            ))),
            Ok(substrate::VmEvent::ClockTimeGet(call)) => Ok(Event::Call(Call::ClockTimeGet(
                Pending::new(self, ClockTimeGet { call }),
            ))),
            Ok(substrate::VmEvent::PollOneoff(call)) => Ok(Event::Call(Call::PollOneoff(
                Pending::new(self, PollOneoff { call }),
            ))),
            Ok(substrate::VmEvent::RandomGet(call)) => Ok(Event::Call(Call::RandomGet(
                Pending::new(self, RandomGet { call }),
            ))),
            Ok(substrate::VmEvent::FdReaddir(call)) => Ok(Event::Call(Call::FdReaddir(
                Pending::new(self, FdReaddir { call }),
            ))),
            Ok(substrate::VmEvent::PathOpen(call)) => Ok(Event::Call(Call::PathOpen(
                Pending::new(self, PathOpen { call }),
            ))),
            Ok(substrate::VmEvent::ArgsSizesGet(call)) => Ok(Event::Call(Call::ArgsSizesGet(
                Pending::new(self, ArgsSizesGet { call }),
            ))),
            Ok(substrate::VmEvent::ArgsGet(call)) => Ok(Event::Call(Call::ArgsGet(Pending::new(
                self,
                ArgsGet { call },
            )))),
            Ok(substrate::VmEvent::EnvironSizesGet(call)) => Ok(Event::Call(
                Call::EnvironSizesGet(Pending::new(self, EnvironSizesGet { call })),
            )),
            Ok(substrate::VmEvent::EnvironGet(call)) => Ok(Event::Call(Call::EnvironGet(
                Pending::new(self, EnvironGet { call }),
            ))),
            Ok(substrate::VmEvent::MemoryGrow(event)) => Ok(Event::MemoryFence(Pending::new(
                self,
                MemoryFence { event },
            ))),
            Ok(substrate::VmEvent::BudgetExpired(expired)) => Ok(Event::BudgetExpired(expired)),
            Ok(substrate::VmEvent::ProcExit(status)) => Ok(Event::Exit(ProcExit::new(status))),
            Ok(substrate::VmEvent::Done) => Ok(Event::Done),
            Err(error) => Err(error),
        }
    }
}

pub(crate) enum Event<'guest, 'a> {
    Call(Call<'guest, 'a>),
    MemoryFence(Pending<'guest, 'a, MemoryFence>),
    BudgetExpired(BudgetExpired),
    Done,
    Exit(ProcExit),
}

pub(crate) enum Call<'guest, 'a> {
    FdWrite(Pending<'guest, 'a, FdWrite>),
    FdRead(Pending<'guest, 'a, FdRead>),
    FdFdstatGet(Pending<'guest, 'a, FdFdstatGet>),
    FdClose(Pending<'guest, 'a, FdClose>),
    ClockResGet(Pending<'guest, 'a, ClockResGet>),
    ClockTimeGet(Pending<'guest, 'a, ClockTimeGet>),
    PollOneoff(Pending<'guest, 'a, PollOneoff>),
    RandomGet(Pending<'guest, 'a, RandomGet>),
    FdReaddir(Pending<'guest, 'a, FdReaddir>),
    PathOpen(Pending<'guest, 'a, PathOpen>),
    ArgsSizesGet(Pending<'guest, 'a, ArgsSizesGet>),
    ArgsGet(Pending<'guest, 'a, ArgsGet>),
    EnvironSizesGet(Pending<'guest, 'a, EnvironSizesGet>),
    EnvironGet(Pending<'guest, 'a, EnvironGet>),
}

pub(crate) struct Pending<'guest, 'a, K> {
    guest: &'guest mut Guest<'a>,
    call: K,
}

impl<'guest, 'a, K> Pending<'guest, 'a, K> {
    fn new(guest: &'guest mut Guest<'a>, call: K) -> Self {
        Self { guest, call }
    }

    fn engine(&self) -> &substrate::Vm<'a> {
        &self.guest.engine
    }

    fn complete_with<R>(self, f: impl FnOnce(&mut substrate::Vm<'a>, K) -> R) -> R {
        let Self { guest, call } = self;
        f(&mut guest.engine, call)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcExit {
    status: u32,
}

impl ProcExit {
    const fn new(status: u32) -> Self {
        Self { status }
    }

    pub(crate) const fn as_protocol_status(self) -> Option<ProcExitStatus> {
        if self.status <= u8::MAX as u32 {
            Some(ProcExitStatus::new(self.status as u8))
        } else {
            None
        }
    }
}

pub(crate) struct Payload {
    raw: substrate::InlinePayload,
}

impl Payload {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.raw.as_bytes()
    }
}

pub(crate) struct FdWrite {
    call: substrate::FdWriteCall,
}

impl Pending<'_, '_, FdWrite> {
    pub(crate) const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub(crate) fn payload(&self) -> Result<Payload, Error> {
        Ok(Payload {
            raw: self.engine().fd_write_payload(self.call.call)?,
        })
    }

    pub(crate) fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_fd_write(call.call, errno))
    }
}

pub(crate) struct FdRead {
    call: substrate::FdReadCall,
}

impl Pending<'_, '_, FdRead> {
    pub(crate) const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub(crate) fn max_len(&self) -> Result<usize, Error> {
        let (_, max_len) = self.engine().fd_read_iovec(self.call.call)?;
        Ok(max_len as usize)
    }

    pub(crate) fn complete(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_fd_read(call.call, bytes, errno))
    }
}

pub(crate) struct FdFdstatGet {
    call: substrate::FdRequestCall,
}

impl Pending<'_, '_, FdFdstatGet> {
    pub(crate) const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub(crate) fn complete(self, stat: FdStat, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_fd_fdstat_get(call.call, stat, errno))
    }
}

pub(crate) struct FdClose {
    call: substrate::FdRequestCall,
}

impl Pending<'_, '_, FdClose> {
    pub(crate) const fn fd(&self) -> u8 {
        self.call.call.fd()
    }

    pub(crate) fn complete(self, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, _| vm.finish_host_call(errno))
    }
}

pub(crate) struct ClockResGet {
    call: substrate::ClockResGetCall,
}

impl Pending<'_, '_, ClockResGet> {
    pub(crate) const fn clock_id(&self) -> u32 {
        self.call.call.clock_id()
    }

    pub(crate) fn complete(self, resolution_nanos: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_clock_res_get(call.call, resolution_nanos, errno))
    }
}

pub(crate) struct ClockTimeGet {
    call: substrate::ClockTimeGetCall,
}

impl Pending<'_, '_, ClockTimeGet> {
    pub(crate) const fn clock_id(&self) -> u32 {
        self.call.call.clock_id()
    }

    pub(crate) const fn precision(&self) -> u64 {
        self.call.call.precision()
    }

    pub(crate) fn complete(self, nanos: u64, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_clock_time_get(call.call, nanos, errno))
    }
}

pub(crate) struct PollOneoff {
    call: substrate::PollOneoffCall,
}

impl Pending<'_, '_, PollOneoff> {
    pub(crate) fn delay_ticks(&self) -> Result<u64, Error> {
        self.engine().poll_oneoff_delay_ticks(self.call.call)
    }

    pub(crate) fn complete(self, ready: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_poll_oneoff(call.call, ready, errno))
    }
}

pub(crate) struct RandomGet {
    call: substrate::RandomGetCall,
}

impl Pending<'_, '_, RandomGet> {
    pub(crate) const fn buf_len(&self) -> u32 {
        self.call.call.buf_len()
    }

    pub(crate) fn complete(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_random_get(call.call, bytes, errno))
    }
}

pub(crate) struct FdReaddir {
    call: substrate::PathCall,
}

impl Pending<'_, '_, FdReaddir> {
    pub(crate) fn fd(&self) -> Result<u8, Error> {
        self.call.call.fd()
    }

    pub(crate) fn cookie(&self) -> Result<u64, Error> {
        self.call.call.arg_i64(3)
    }

    pub(crate) fn max_len(&self) -> Result<usize, Error> {
        Ok(self.call.call.arg_i32(2)? as usize)
    }

    pub(crate) fn complete(self, bytes: &[u8], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_fd_readdir(call.call, bytes, errno))
    }
}

pub(crate) struct PathOpen {
    call: substrate::PathCall,
}

impl Pending<'_, '_, PathOpen> {
    pub(crate) fn fd(&self) -> Result<u8, Error> {
        self.call.call.fd()
    }

    pub(crate) fn rights_base(&self) -> Result<u64, Error> {
        self.call.call.arg_i64(5)
    }

    pub(crate) fn path_bytes(&self) -> Result<PathBytes, Error> {
        self.engine().path_bytes(self.call.call)
    }

    pub(crate) fn complete(self, opened_fd: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_path_open(call.call, opened_fd, errno))
    }
}

pub(crate) struct ArgsSizesGet {
    call: substrate::ArgsSizesGetCall,
}

impl Pending<'_, '_, ArgsSizesGet> {
    pub(crate) fn complete(self, argc: u32, argv_buf_size: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| {
            vm.finish_args_sizes_get(call.call, argc, argv_buf_size, errno)
        })
    }
}

pub(crate) struct ArgsGet {
    call: substrate::ArgsGetCall,
}

impl Pending<'_, '_, ArgsGet> {
    pub(crate) const fn max_len(&self) -> u8 {
        u8::MAX
    }

    pub(crate) fn complete(self, args: &[&[u8]], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_args_get(call.call, args, errno))
    }
}

pub(crate) struct EnvironSizesGet {
    call: substrate::EnvironSizesGetCall,
}

impl Pending<'_, '_, EnvironSizesGet> {
    pub(crate) fn complete(self, count: u32, buf_size: u32, errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| {
            vm.finish_environ_sizes_get(call.call, count, buf_size, errno)
        })
    }
}

pub(crate) struct EnvironGet {
    call: substrate::EnvironGetCall,
}

impl Pending<'_, '_, EnvironGet> {
    pub(crate) const fn max_len(&self) -> u8 {
        u8::MAX
    }

    pub(crate) fn complete(self, environ: &[(&[u8], &[u8])], errno: u32) -> Result<(), Error> {
        self.complete_with(|vm, call| vm.finish_environ_get(call.call, environ, errno))
    }
}

pub(crate) struct MemoryFence {
    event: substrate::MemoryGrowEvent,
}

impl Pending<'_, '_, MemoryFence> {
    pub(crate) const fn previous_pages(&self) -> u32 {
        self.call.event.previous_pages
    }

    pub(crate) const fn new_pages(&self) -> Option<u32> {
        self.call.event.new_pages
    }

    pub(crate) const fn fence_epoch(&self) -> u32 {
        match self.new_pages() {
            Some(pages) => pages,
            None => self.previous_pages(),
        }
    }

    pub(crate) fn complete(self) -> Result<(), Error> {
        self.complete_with(|vm, _| vm.finish_memory_grow_event().map(|_| ()))
    }
}
