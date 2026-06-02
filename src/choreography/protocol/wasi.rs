use super::*;

pub type BudgetRunMsg = Msg<LABEL_ENGINE_RUN, BudgetRun>;
pub type BudgetExpiredMsg = Msg<LABEL_ENGINE_BUDGET_EXPIRED, BudgetExpired>;
pub type BudgetSuspendMsg = Msg<LABEL_ENGINE_SUSPEND, BudgetSuspend>;
pub type BudgetRestartMsg = Msg<LABEL_ENGINE_RESTART, BudgetRestart>;

pub type MemReadGrantControl = Msg<LABEL_MEM_GRANT_READ_CONTROL, ()>;
pub type MemWriteGrantControl = Msg<LABEL_MEM_GRANT_WRITE_CONTROL, ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemRights {
    Read,
    Write,
}

impl MemRights {
    pub const fn tag(self) -> u8 {
        match self {
            Self::Read => 1,
            Self::Write => 2,
        }
    }

    fn decode(tag: u8) -> Result<Self, CodecError> {
        match tag {
            1 => Ok(Self::Read),
            2 => Ok(Self::Write),
            _ => Err(CodecError::Invalid("unknown memory lease rights")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemBorrow {
    ptr: u32,
    len: u8,
    epoch: u32,
}

impl MemBorrow {
    pub const fn new(ptr: u32, len: u8, epoch: u32) -> Self {
        Self { ptr, len, epoch }
    }

    pub const fn ptr(&self) -> u32 {
        self.ptr
    }

    pub const fn len(&self) -> u8 {
        self.len
    }

    pub const fn epoch(&self) -> u32 {
        self.epoch
    }
}

impl WireEncode for MemBorrow {
    fn encoded_len(&self) -> Option<usize> {
        Some(9)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 9 {
            return Err(CodecError::Truncated);
        }
        out[..4].copy_from_slice(&self.ptr.to_be_bytes());
        out[4] = self.len;
        out[5..9].copy_from_slice(&self.epoch.to_be_bytes());
        Ok(9)
    }
}

impl WirePayload for MemBorrow {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 9 {
            return Err(CodecError::Invalid("memory borrow carries nine bytes"));
        }
        let mut ptr = [0u8; 4];
        let mut epoch = [0u8; 4];
        ptr.copy_from_slice(&bytes[..4]);
        epoch.copy_from_slice(&bytes[5..9]);
        Ok(Self::new(
            u32::from_be_bytes(ptr),
            bytes[4],
            u32::from_be_bytes(epoch),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemGrant {
    lease_id: u8,
    ptr: u32,
    len: u8,
    epoch: u32,
    rights: MemRights,
}

impl MemGrant {
    pub const fn new(lease_id: u8, ptr: u32, len: u8, epoch: u32, rights: MemRights) -> Self {
        Self {
            lease_id,
            ptr,
            len,
            epoch,
            rights,
        }
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn ptr(&self) -> u32 {
        self.ptr
    }

    pub const fn len(&self) -> u8 {
        self.len
    }

    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    pub const fn rights(&self) -> MemRights {
        self.rights
    }
}

impl WireEncode for MemGrant {
    fn encoded_len(&self) -> Option<usize> {
        Some(11)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 11 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.lease_id;
        out[1..5].copy_from_slice(&self.ptr.to_be_bytes());
        out[5] = self.len;
        out[6..10].copy_from_slice(&self.epoch.to_be_bytes());
        out[10] = self.rights.tag();
        Ok(11)
    }
}

impl WirePayload for MemGrant {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 11 {
            return Err(CodecError::Invalid("memory grant carries eleven bytes"));
        }
        let mut ptr = [0u8; 4];
        let mut epoch = [0u8; 4];
        ptr.copy_from_slice(&bytes[1..5]);
        epoch.copy_from_slice(&bytes[6..10]);
        Ok(Self::new(
            bytes[0],
            u32::from_be_bytes(ptr),
            bytes[5],
            u32::from_be_bytes(epoch),
            MemRights::decode(bytes[10])?,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemRelease {
    lease_id: u8,
}

impl MemRelease {
    pub const fn new(lease_id: u8) -> Self {
        Self { lease_id }
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }
}

impl WireEncode for MemRelease {
    fn encoded_len(&self) -> Option<usize> {
        Some(1)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let Some(first) = out.first_mut() else {
            return Err(CodecError::Truncated);
        };
        *first = self.lease_id;
        Ok(1)
    }
}

impl WirePayload for MemRelease {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("memory release carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemCommit {
    lease_id: u8,
    written: u8,
}

impl MemCommit {
    pub const fn new(lease_id: u8, written: u8) -> Self {
        Self { lease_id, written }
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn written(&self) -> u8 {
        self.written
    }
}

impl WireEncode for MemCommit {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.lease_id;
        out[1] = self.written;
        Ok(2)
    }
}

impl WirePayload for MemCommit {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("memory commit carries two bytes"));
        }
        Ok(Self::new(bytes[0], bytes[1]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemFenceReason {
    MemoryGrow,
    Trap,
    Suspend,
    Kill,
    HotSwap,
}

impl MemFenceReason {
    pub const fn tag(self) -> u8 {
        match self {
            Self::MemoryGrow => 1,
            Self::Trap => 2,
            Self::Suspend => 3,
            Self::Kill => 4,
            Self::HotSwap => 5,
        }
    }

    fn decode(tag: u8) -> Result<Self, CodecError> {
        match tag {
            1 => Ok(Self::MemoryGrow),
            2 => Ok(Self::Trap),
            3 => Ok(Self::Suspend),
            4 => Ok(Self::Kill),
            5 => Ok(Self::HotSwap),
            _ => Err(CodecError::Invalid("unknown memory fence reason")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemFence {
    reason: MemFenceReason,
    new_epoch: u32,
}

impl MemFence {
    pub const fn new(reason: MemFenceReason, new_epoch: u32) -> Self {
        Self { reason, new_epoch }
    }

    pub const fn reason(&self) -> MemFenceReason {
        self.reason
    }

    pub const fn new_epoch(&self) -> u32 {
        self.new_epoch
    }
}

impl WireEncode for MemFence {
    fn encoded_len(&self) -> Option<usize> {
        Some(5)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 5 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.reason.tag();
        out[1..5].copy_from_slice(&self.new_epoch.to_be_bytes());
        Ok(5)
    }
}

impl WirePayload for MemFence {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 5 {
            return Err(CodecError::Invalid("memory fence carries five bytes"));
        }
        let mut new_epoch = [0u8; 4];
        new_epoch.copy_from_slice(&bytes[1..5]);
        Ok(Self::new(
            MemFenceReason::decode(bytes[0])?,
            u32::from_be_bytes(new_epoch),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetRun {
    run_id: u16,
    generation: u16,
    fuel: u32,
    deadline_tick: u64,
}

impl BudgetRun {
    pub const fn new(run_id: u16, generation: u16, fuel: u32, deadline_tick: u64) -> Self {
        Self {
            run_id,
            generation,
            fuel,
            deadline_tick,
        }
    }

    pub const fn run_id(&self) -> u16 {
        self.run_id
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn fuel(&self) -> u32 {
        self.fuel
    }

    pub const fn deadline_tick(&self) -> u64 {
        self.deadline_tick
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 16 {
            return Err(CodecError::Invalid("budget run carries sixteen bytes"));
        }
        Ok(Self::new(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            u64::from_be_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]),
        ))
    }
}

impl WireEncode for BudgetRun {
    fn encoded_len(&self) -> Option<usize> {
        Some(16)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 16 {
            return Err(CodecError::Truncated);
        }
        out[0..2].copy_from_slice(&self.run_id.to_be_bytes());
        out[2..4].copy_from_slice(&self.generation.to_be_bytes());
        out[4..8].copy_from_slice(&self.fuel.to_be_bytes());
        out[8..16].copy_from_slice(&self.deadline_tick.to_be_bytes());
        Ok(16)
    }
}

impl WirePayload for BudgetRun {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        Self::decode(input.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetExpired {
    run_id: u16,
    generation: u16,
}

impl BudgetExpired {
    pub const fn new(run_id: u16, generation: u16) -> Self {
        Self { run_id, generation }
    }

    pub const fn run_id(&self) -> u16 {
        self.run_id
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 4 {
            return Err(CodecError::Invalid("budget event carries four bytes"));
        }
        Ok(Self::new(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            u16::from_be_bytes([bytes[2], bytes[3]]),
        ))
    }
}

impl WireEncode for BudgetExpired {
    fn encoded_len(&self) -> Option<usize> {
        Some(4)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 4 {
            return Err(CodecError::Truncated);
        }
        out[0..2].copy_from_slice(&self.run_id.to_be_bytes());
        out[2..4].copy_from_slice(&self.generation.to_be_bytes());
        Ok(4)
    }
}

impl WirePayload for BudgetExpired {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        Self::decode(input.as_bytes())
    }
}

pub type BudgetSuspend = BudgetExpired;
pub type BudgetRestart = BudgetRun;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineReq {
    LogU32(u32),
    Wasip1Stdout(StdoutChunk),
    Wasip1Stderr(StderrChunk),
    Wasip1Stdin(StdinRequest),
    Wasip1ClockNow,
    Wasip1RandomSeed,
    Wasip1Exit(Wasip1ExitStatus),
    FdWrite(FdWrite),
    FdRead(FdRead),
    FdReaddir(FdReaddir),
    FdFdstatGet(FdRequest),
    FdClose(FdRequest),
    ClockResGet(ClockResGet),
    ClockTimeGet(ClockTimeGet),
    PollOneoff(PollOneoff),
    RandomGet(RandomGet),
    ProcExit(ProcExitStatus),
    ArgsSizesGet(ArgsSizesGet),
    ArgsGet(ArgsGet),
    EnvironSizesGet(EnvironSizesGet),
    EnvironGet(EnvironGet),
    PathOpen(PathOpen),
}

impl WireEncode for EngineReq {
    fn encoded_len(&self) -> Option<usize> {
        Some(match self {
            Self::LogU32(_) => 5,
            Self::Wasip1Stdout(chunk) => 3 + chunk.len(),
            Self::Wasip1Stderr(chunk) => 3 + chunk.len(),
            Self::Wasip1Stdin(_) => 3,
            Self::Wasip1ClockNow => 1,
            Self::Wasip1RandomSeed => 1,
            Self::Wasip1Exit(_) => 2,
            Self::FdWrite(write) => 4 + write.len(),
            Self::FdRead(_) => 4,
            Self::FdReaddir(_) => 12,
            Self::FdFdstatGet(_) => 2,
            Self::FdClose(_) => 2,
            Self::ClockResGet(_) => 2,
            Self::ClockTimeGet(_) => 10,
            Self::PollOneoff(_) => 9,
            Self::RandomGet(_) => 3,
            Self::ProcExit(_) => 2,
            Self::ArgsSizesGet(_) => 1,
            Self::ArgsGet(_) => 3,
            Self::EnvironSizesGet(_) => 1,
            Self::EnvironGet(_) => 3,
            Self::PathOpen(open) => 12 + open.len(),
        })
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        match *self {
            Self::LogU32(value) => {
                if out.len() < 5 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_LOG_U32;
                out[1..5].copy_from_slice(&value.to_be_bytes());
                Ok(5)
            }
            Self::Wasip1Stdout(chunk) => {
                let len = chunk.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_STDOUT;
                out[1] = chunk.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(chunk.as_bytes());
                Ok(3 + len)
            }
            Self::Wasip1Stderr(chunk) => {
                let len = chunk.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_STDERR;
                out[1] = chunk.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(chunk.as_bytes());
                Ok(3 + len)
            }
            Self::Wasip1Stdin(request) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_STDIN;
                out[1] = request.lease_id();
                out[2] = request.max_len();
                Ok(3)
            }
            Self::Wasip1ClockNow => {
                if out.is_empty() {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_CLOCK_NOW;
                Ok(1)
            }
            Self::Wasip1RandomSeed => {
                if out.is_empty() {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_RANDOM_SEED;
                Ok(1)
            }
            Self::Wasip1Exit(status) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASIP1_EXIT;
                out[1] = status.code();
                Ok(2)
            }
            Self::FdWrite(write) => {
                let len = write.len();
                if out.len() < 4 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_FD_WRITE;
                out[1] = write.fd();
                out[2] = write.lease_id();
                out[3] = len as u8;
                out[4..4 + len].copy_from_slice(write.as_bytes());
                Ok(4 + len)
            }
            Self::FdRead(read) => {
                if out.len() < 4 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_FD_READ;
                out[1] = read.fd();
                out[2] = read.lease_id();
                out[3] = read.max_len();
                Ok(4)
            }
            Self::FdReaddir(read) => {
                if out.len() < 12 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_FD_READDIR;
                out[1] = read.fd();
                out[2] = read.lease_id();
                out[3..11].copy_from_slice(&read.cookie().to_be_bytes());
                out[11] = read.max_len();
                Ok(12)
            }
            Self::FdFdstatGet(request) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_FD_FDSTAT_GET;
                out[1] = request.fd();
                Ok(2)
            }
            Self::FdClose(request) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_FD_CLOSE;
                out[1] = request.fd();
                Ok(2)
            }
            Self::ClockResGet(request) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_CLOCK_RES_GET;
                out[1] = request.clock_id();
                Ok(2)
            }
            Self::ClockTimeGet(request) => {
                if out.len() < 10 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_CLOCK_TIME_GET;
                out[1] = request.clock_id();
                out[2..10].copy_from_slice(&request.precision().to_be_bytes());
                Ok(10)
            }
            Self::PollOneoff(request) => {
                if out.len() < 9 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_POLL_ONEOFF;
                out[1..9].copy_from_slice(&request.timeout_tick().to_be_bytes());
                Ok(9)
            }
            Self::RandomGet(request) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_RANDOM_GET;
                out[1] = request.lease_id();
                out[2] = request.max_len();
                Ok(3)
            }
            Self::ProcExit(status) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_PROC_EXIT;
                out[1] = status.code();
                Ok(2)
            }
            Self::ArgsSizesGet(_) => {
                if out.is_empty() {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_ARGS_SIZES_GET;
                Ok(1)
            }
            Self::ArgsGet(request) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_ARGS_GET;
                out[1] = request.lease_id();
                out[2] = request.max_len();
                Ok(3)
            }
            Self::EnvironSizesGet(_) => {
                if out.is_empty() {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_ENVIRON_SIZES_GET;
                Ok(1)
            }
            Self::EnvironGet(request) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_ENVIRON_GET;
                out[1] = request.lease_id();
                out[2] = request.max_len();
                Ok(3)
            }
            Self::PathOpen(open) => {
                let len = open.len();
                if out.len() < 12 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_REQ_WASI_PATH_OPEN;
                out[1] = open.preopen_fd();
                out[2] = open.lease_id();
                out[3..11].copy_from_slice(&open.rights_base().to_be_bytes());
                out[11] = len as u8;
                out[12..12 + len].copy_from_slice(open.path());
                Ok(12 + len)
            }
        }
    }
}

impl WirePayload for EngineReq {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        let Some((&tag, rest)) = bytes.split_first() else {
            return Err(CodecError::Truncated);
        };
        match tag {
            TAG_REQ_LOG_U32 => Ok(Self::LogU32(decode_u32_payload(rest)?)),
            TAG_REQ_WASIP1_STDOUT => Ok(Self::Wasip1Stdout(StdoutChunk::decode(rest)?)),
            TAG_REQ_WASIP1_STDERR => Ok(Self::Wasip1Stderr(StderrChunk::decode(rest)?)),
            TAG_REQ_WASIP1_STDIN => Ok(Self::Wasip1Stdin(StdinRequest::decode(rest)?)),
            TAG_REQ_WASIP1_CLOCK_NOW => {
                if !rest.is_empty() {
                    return Err(CodecError::Invalid("clock request carries no payload"));
                }
                Ok(Self::Wasip1ClockNow)
            }
            TAG_REQ_WASIP1_RANDOM_SEED => {
                if !rest.is_empty() {
                    return Err(CodecError::Invalid("random request carries no payload"));
                }
                Ok(Self::Wasip1RandomSeed)
            }
            TAG_REQ_WASIP1_EXIT => Ok(Self::Wasip1Exit(Wasip1ExitStatus::decode(rest)?)),
            TAG_REQ_WASI_FD_WRITE => Ok(Self::FdWrite(FdWrite::decode(rest)?)),
            TAG_REQ_WASI_FD_READ => Ok(Self::FdRead(FdRead::decode(rest)?)),
            TAG_REQ_WASI_FD_READDIR => Ok(Self::FdReaddir(FdReaddir::decode(rest)?)),
            TAG_REQ_WASI_FD_FDSTAT_GET => Ok(Self::FdFdstatGet(FdRequest::decode(rest)?)),
            TAG_REQ_WASI_FD_CLOSE => Ok(Self::FdClose(FdRequest::decode(rest)?)),
            TAG_REQ_WASI_CLOCK_RES_GET => Ok(Self::ClockResGet(ClockResGet::decode(rest)?)),
            TAG_REQ_WASI_CLOCK_TIME_GET => Ok(Self::ClockTimeGet(ClockTimeGet::decode(rest)?)),
            TAG_REQ_WASI_POLL_ONEOFF => Ok(Self::PollOneoff(PollOneoff::decode(rest)?)),
            TAG_REQ_WASI_RANDOM_GET => Ok(Self::RandomGet(RandomGet::decode(rest)?)),
            TAG_REQ_WASI_PROC_EXIT => Ok(Self::ProcExit(ProcExitStatus::decode(rest)?)),
            TAG_REQ_WASI_ARGS_SIZES_GET => Ok(Self::ArgsSizesGet(ArgsSizesGet::decode(rest)?)),
            TAG_REQ_WASI_ARGS_GET => Ok(Self::ArgsGet(ArgsGet::decode(rest)?)),
            TAG_REQ_WASI_ENVIRON_SIZES_GET => {
                Ok(Self::EnvironSizesGet(EnvironSizesGet::decode(rest)?))
            }
            TAG_REQ_WASI_ENVIRON_GET => Ok(Self::EnvironGet(EnvironGet::decode(rest)?)),
            TAG_REQ_WASI_PATH_OPEN => Ok(Self::PathOpen(PathOpen::decode(rest)?)),
            _ => Err(CodecError::Invalid("unknown engine request tag")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineRet {
    Logged(u32),
    Wasip1StdoutWritten(u8),
    Wasip1StderrWritten(u8),
    Wasip1StdinRead(StdinChunk),
    Wasip1ClockNow(ClockNow),
    Wasip1RandomSeed(RandomSeed),
    FdWriteDone(FdWriteDone),
    FdReadDone(FdReadDone),
    FdReaddirDone(FdReaddirDone),
    FdStat(FdStat),
    FdClosed(FdClosed),
    ClockResolution(ClockResolution),
    ClockTime(ClockNow),
    PollReady(PollReady),
    RandomDone(RandomDone),
    ArgsSizes(ArgsSizes),
    ArgsDone(ArgsDone),
    EnvironSizes(EnvironSizes),
    EnvironDone(EnvironDone),
    PathOpened(PathOpened),
}

impl WireEncode for EngineRet {
    fn encoded_len(&self) -> Option<usize> {
        Some(match self {
            Self::Logged(_) => 5,
            Self::Wasip1StdoutWritten(_) => 2,
            Self::Wasip1StderrWritten(_) => 2,
            Self::Wasip1StdinRead(chunk) => 3 + chunk.len(),
            Self::Wasip1ClockNow(_) => 9,
            Self::Wasip1RandomSeed(_) => 17,
            Self::FdWriteDone(_) => 5,
            Self::FdReadDone(done) => 4 + done.len(),
            Self::FdReaddirDone(done) => 6 + done.len(),
            Self::FdStat(_) => 3,
            Self::FdClosed(_) => 2,
            Self::ClockResolution(_) => 9,
            Self::ClockTime(_) => 9,
            Self::PollReady(_) => 2,
            Self::RandomDone(done) => 3 + done.len(),
            Self::ArgsSizes(_) => 3,
            Self::ArgsDone(done) => 3 + done.len(),
            Self::EnvironSizes(_) => 3,
            Self::EnvironDone(done) => 3 + done.len(),
            Self::PathOpened(_) => 4,
        })
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        match *self {
            Self::Logged(value) => {
                if out.len() < 5 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_LOGGED;
                out[1..5].copy_from_slice(&value.to_be_bytes());
                Ok(5)
            }
            Self::Wasip1StdoutWritten(written) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASIP1_STDOUT_WRITTEN;
                out[1] = written;
                Ok(2)
            }
            Self::Wasip1StderrWritten(written) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASIP1_STDERR_WRITTEN;
                out[1] = written;
                Ok(2)
            }
            Self::Wasip1StdinRead(chunk) => {
                let len = chunk.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASIP1_STDIN_READ;
                out[1] = chunk.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(chunk.as_bytes());
                Ok(3 + len)
            }
            Self::Wasip1ClockNow(now) => {
                if out.len() < 9 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASIP1_CLOCK_NOW;
                out[1..9].copy_from_slice(&now.nanos().to_be_bytes());
                Ok(9)
            }
            Self::Wasip1RandomSeed(seed) => {
                if out.len() < 17 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASIP1_RANDOM_SEED;
                out[1..9].copy_from_slice(&seed.lo().to_be_bytes());
                out[9..17].copy_from_slice(&seed.hi().to_be_bytes());
                Ok(17)
            }
            Self::FdWriteDone(done) => {
                if out.len() < 5 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_FD_WRITE_DONE;
                out[1] = done.fd();
                out[2] = done.written();
                out[3..5].copy_from_slice(&done.errno().to_be_bytes());
                Ok(5)
            }
            Self::FdReadDone(done) => {
                let len = done.len();
                if out.len() < 4 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_FD_READ_DONE;
                out[1] = done.fd();
                out[2] = done.lease_id();
                out[3] = len as u8;
                out[4..4 + len].copy_from_slice(done.as_bytes());
                Ok(4 + len)
            }
            Self::FdReaddirDone(done) => {
                let len = done.len();
                if out.len() < 6 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_FD_READDIR_DONE;
                out[1] = done.fd();
                out[2] = done.lease_id();
                out[3..5].copy_from_slice(&done.errno().to_be_bytes());
                out[5] = len as u8;
                out[6..6 + len].copy_from_slice(done.as_bytes());
                Ok(6 + len)
            }
            Self::FdStat(stat) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_FD_FDSTAT;
                out[1] = stat.fd();
                out[2] = stat.rights().tag();
                Ok(3)
            }
            Self::FdClosed(closed) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_FD_CLOSED;
                out[1] = closed.fd();
                Ok(2)
            }
            Self::ClockResolution(resolution) => {
                if out.len() < 9 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_CLOCK_RESOLUTION;
                out[1..9].copy_from_slice(&resolution.nanos().to_be_bytes());
                Ok(9)
            }
            Self::ClockTime(now) => {
                if out.len() < 9 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_CLOCK_TIME;
                out[1..9].copy_from_slice(&now.nanos().to_be_bytes());
                Ok(9)
            }
            Self::PollReady(ready) => {
                if out.len() < 2 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_POLL_READY;
                out[1] = ready.ready();
                Ok(2)
            }
            Self::RandomDone(done) => {
                let len = done.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_RANDOM_DONE;
                out[1] = done.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(done.as_bytes());
                Ok(3 + len)
            }
            Self::ArgsSizes(sizes) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_ARGS_SIZES;
                out[1] = sizes.count();
                out[2] = sizes.buf_size();
                Ok(3)
            }
            Self::ArgsDone(done) => {
                let len = done.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_ARGS_DONE;
                out[1] = done.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(done.as_bytes());
                Ok(3 + len)
            }
            Self::EnvironSizes(sizes) => {
                if out.len() < 3 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_ENVIRON_SIZES;
                out[1] = sizes.count();
                out[2] = sizes.buf_size();
                Ok(3)
            }
            Self::EnvironDone(done) => {
                let len = done.len();
                if out.len() < 3 + len {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_ENVIRON_DONE;
                out[1] = done.lease_id();
                out[2] = len as u8;
                out[3..3 + len].copy_from_slice(done.as_bytes());
                Ok(3 + len)
            }
            Self::PathOpened(opened) => {
                if out.len() < 4 {
                    return Err(CodecError::Truncated);
                }
                out[0] = TAG_RET_WASI_PATH_OPENED;
                out[1] = opened.fd();
                out[2..4].copy_from_slice(&opened.errno().to_be_bytes());
                Ok(4)
            }
        }
    }
}

impl WirePayload for EngineRet {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        let Some((&tag, rest)) = bytes.split_first() else {
            return Err(CodecError::Truncated);
        };
        match tag {
            TAG_RET_LOGGED => Ok(Self::Logged(decode_u32_payload(rest)?)),
            TAG_RET_WASIP1_STDOUT_WRITTEN => {
                if rest.len() != 1 {
                    return Err(CodecError::Invalid("stdout reply carries one byte"));
                }
                Ok(Self::Wasip1StdoutWritten(rest[0]))
            }
            TAG_RET_WASIP1_STDERR_WRITTEN => {
                if rest.len() != 1 {
                    return Err(CodecError::Invalid("stderr reply carries one byte"));
                }
                Ok(Self::Wasip1StderrWritten(rest[0]))
            }
            TAG_RET_WASIP1_STDIN_READ => Ok(Self::Wasip1StdinRead(StdinChunk::decode(rest)?)),
            TAG_RET_WASIP1_CLOCK_NOW => Ok(Self::Wasip1ClockNow(ClockNow::decode(rest)?)),
            TAG_RET_WASIP1_RANDOM_SEED => Ok(Self::Wasip1RandomSeed(RandomSeed::decode(rest)?)),
            TAG_RET_WASI_FD_WRITE_DONE => Ok(Self::FdWriteDone(FdWriteDone::decode(rest)?)),
            TAG_RET_WASI_FD_READ_DONE => Ok(Self::FdReadDone(FdReadDone::decode(rest)?)),
            TAG_RET_WASI_FD_READDIR_DONE => Ok(Self::FdReaddirDone(FdReaddirDone::decode(rest)?)),
            TAG_RET_WASI_FD_FDSTAT => Ok(Self::FdStat(FdStat::decode(rest)?)),
            TAG_RET_WASI_FD_CLOSED => Ok(Self::FdClosed(FdClosed::decode(rest)?)),
            TAG_RET_WASI_CLOCK_RESOLUTION => {
                Ok(Self::ClockResolution(ClockResolution::decode(rest)?))
            }
            TAG_RET_WASI_CLOCK_TIME => Ok(Self::ClockTime(ClockNow::decode(rest)?)),
            TAG_RET_WASI_POLL_READY => Ok(Self::PollReady(PollReady::decode(rest)?)),
            TAG_RET_WASI_RANDOM_DONE => Ok(Self::RandomDone(RandomDone::decode(rest)?)),
            TAG_RET_WASI_ARGS_SIZES => Ok(Self::ArgsSizes(ArgsSizes::decode(rest)?)),
            TAG_RET_WASI_ARGS_DONE => Ok(Self::ArgsDone(ArgsDone::decode(rest)?)),
            TAG_RET_WASI_ENVIRON_SIZES => Ok(Self::EnvironSizes(EnvironSizes::decode(rest)?)),
            TAG_RET_WASI_ENVIRON_DONE => Ok(Self::EnvironDone(EnvironDone::decode(rest)?)),
            TAG_RET_WASI_PATH_OPENED => Ok(Self::PathOpened(PathOpened::decode(rest)?)),
            _ => Err(CodecError::Invalid("unknown engine reply tag")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockNow {
    nanos: u64,
}

impl ClockNow {
    pub const fn new(nanos: u64) -> Self {
        Self { nanos }
    }

    pub const fn nanos(&self) -> u64 {
        self.nanos
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 8 {
            return Err(CodecError::Invalid("clock reply carries eight bytes"));
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        Ok(Self::new(u64::from_be_bytes(buf)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wasip1ExitStatus {
    code: u8,
}

impl Wasip1ExitStatus {
    pub const fn new(code: u8) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> u8 {
        self.code
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("exit request carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomSeed {
    lo: u64,
    hi: u64,
}

impl RandomSeed {
    pub const fn new(lo: u64, hi: u64) -> Self {
        Self { lo, hi }
    }

    pub const fn lo(&self) -> u64 {
        self.lo
    }

    pub const fn hi(&self) -> u64 {
        self.hi
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 16 {
            return Err(CodecError::Invalid(
                "random seed reply carries sixteen bytes",
            ));
        }
        let mut lo = [0u8; 8];
        let mut hi = [0u8; 8];
        lo.copy_from_slice(&bytes[..8]);
        hi.copy_from_slice(&bytes[8..16]);
        Ok(Self::new(u64::from_be_bytes(lo), u64::from_be_bytes(hi)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdinRequest {
    lease_id: u8,
    max_len: u8,
}

impl StdinRequest {
    pub fn new(max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(MEM_LEASE_NONE, max_len)
    }

    pub fn new_with_lease(lease_id: u8, max_len: u8) -> Result<Self, CodecError> {
        if max_len as usize > STDIN_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("stdin request exceeds fixed capacity"));
        }
        Ok(Self { lease_id, max_len })
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("stdin request carries two bytes"));
        }
        Self::new_with_lease(bytes[0], bytes[1])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wasip1StreamChunk {
    lease_id: u8,
    len: u8,
    bytes: [u8; WASIP1_STREAM_CHUNK_CAPACITY],
}

pub type StdoutChunk = Wasip1StreamChunk;
pub type StderrChunk = Wasip1StreamChunk;
pub type StdinChunk = Wasip1StreamChunk;

impl Wasip1StreamChunk {
    pub fn new(bytes: &[u8]) -> Result<Self, CodecError> {
        Self::new_with_lease(MEM_LEASE_NONE, bytes)
    }

    pub fn new_with_lease(lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("stream chunk exceeds fixed capacity"));
        }
        let mut out = [0u8; WASIP1_STREAM_CHUNK_CAPACITY];
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            lease_id,
            len: bytes.len() as u8,
            bytes: out,
        })
    }

    pub fn with_lease(&self, lease_id: u8) -> Self {
        Self {
            lease_id,
            len: self.len,
            bytes: self.bytes,
        }
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < 2 {
            return Err(CodecError::Truncated);
        };
        let lease_id = bytes[0];
        let len = bytes[1] as usize;
        let payload = &bytes[2..];
        if len > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("stream chunk length exceeds capacity"));
        }
        if payload.len() != len {
            return Err(CodecError::Invalid("stream chunk length mismatch"));
        }
        Self::new_with_lease(lease_id, payload)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdWrite {
    fd: u8,
    chunk: Wasip1StreamChunk,
}

impl FdWrite {
    pub fn new(fd: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Self::new_with_lease(fd, MEM_LEASE_NONE, bytes)
    }

    pub fn new_with_lease(fd: u8, lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self {
            fd,
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
        })
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < 3 {
            return Err(CodecError::Truncated);
        }
        let fd = bytes[0];
        Self::new_with_lease(fd, bytes[1], &bytes[3..]).and_then(|write| {
            if write.len() != bytes[2] as usize {
                return Err(CodecError::Invalid("fd_write length mismatch"));
            }
            Ok(write)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdRead {
    fd: u8,
    lease_id: u8,
    max_len: u8,
}

impl FdRead {
    pub fn new(fd: u8, max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(fd, MEM_LEASE_NONE, max_len)
    }

    pub fn new_with_lease(fd: u8, lease_id: u8, max_len: u8) -> Result<Self, CodecError> {
        if max_len as usize > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid(
                "fd_read request exceeds fixed capacity",
            ));
        }
        Ok(Self {
            fd,
            lease_id,
            max_len,
        })
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 3 {
            return Err(CodecError::Invalid("fd_read request carries three bytes"));
        }
        Self::new_with_lease(bytes[0], bytes[1], bytes[2])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdReaddir {
    fd: u8,
    lease_id: u8,
    cookie: u64,
    max_len: u8,
}

impl FdReaddir {
    pub fn new(fd: u8, cookie: u64, max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(fd, MEM_LEASE_NONE, cookie, max_len)
    }

    pub fn new_with_lease(
        fd: u8,
        lease_id: u8,
        cookie: u64,
        max_len: u8,
    ) -> Result<Self, CodecError> {
        if max_len as usize > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid(
                "fd_readdir request exceeds fixed capacity",
            ));
        }
        Ok(Self {
            fd,
            lease_id,
            cookie,
            max_len,
        })
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn cookie(&self) -> u64 {
        self.cookie
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 11 {
            return Err(CodecError::Invalid(
                "fd_readdir request carries eleven bytes",
            ));
        }
        let mut cookie = [0u8; 8];
        cookie.copy_from_slice(&bytes[2..10]);
        Self::new_with_lease(bytes[0], bytes[1], u64::from_be_bytes(cookie), bytes[10])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdRequest {
    fd: u8,
}

impl FdRequest {
    pub const fn new(fd: u8) -> Self {
        Self { fd }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("fd request carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdError {
    fd: u8,
    errno: u16,
}

impl FdError {
    pub const fn new(fd: u8, errno: u16) -> Self {
        Self { fd, errno }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn errno(&self) -> u16 {
        self.errno
    }
}

impl WireEncode for FdError {
    fn encoded_len(&self) -> Option<usize> {
        Some(3)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 3 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.errno.to_be_bytes());
        Ok(3)
    }
}

impl WirePayload for FdError {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 3 {
            return Err(CodecError::Invalid("fd error carries three bytes"));
        }
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
        ))
    }
}

pub type FdErrorMsg = Msg<LABEL_WASI_FD_ERROR, FdError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PathOpen {
    rights_base: u64,
    path: [u8; WASIP1_PATH_CHUNK_CAPACITY],
    preopen_fd: u8,
    lease_id: u8,
    len: u8,
}

impl PathOpen {
    pub fn new(
        preopen_fd: u8,
        lease_id: u8,
        rights_base: u64,
        path: &[u8],
    ) -> Result<Self, CodecError> {
        if path.len() > WASIP1_PATH_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("path_open path exceeds capacity"));
        }
        let mut out = [0u8; WASIP1_PATH_CHUNK_CAPACITY];
        out[..path.len()].copy_from_slice(path);
        Ok(Self {
            rights_base,
            path: out,
            preopen_fd,
            lease_id,
            len: path.len() as u8,
        })
    }

    pub const fn preopen_fd(&self) -> u8 {
        self.preopen_fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn rights_base(&self) -> u64 {
        self.rights_base
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub fn path(&self) -> &[u8] {
        self.path.split_at(self.len()).0
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < 11 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[10] as usize;
        if bytes.len() != 11 + len {
            return Err(CodecError::Invalid("path_open length mismatch"));
        }
        let mut rights = [0u8; 8];
        rights.copy_from_slice(&bytes[2..10]);
        Self::new(bytes[0], bytes[1], u64::from_be_bytes(rights), &bytes[11..])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathOpened {
    fd: u8,
    errno: u16,
}

impl PathOpened {
    pub const fn new(fd: u8, errno: u16) -> Self {
        Self { fd, errno }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn errno(&self) -> u16 {
        self.errno
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 3 {
            return Err(CodecError::Invalid("path_open reply carries three bytes"));
        }
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockResGet {
    clock_id: u8,
}

impl ClockResGet {
    pub const fn new(clock_id: u8) -> Self {
        Self { clock_id }
    }

    pub const fn clock_id(&self) -> u8 {
        self.clock_id
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("clock_res_get carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTimeGet {
    clock_id: u8,
    precision: u64,
}

impl ClockTimeGet {
    pub const fn new(clock_id: u8, precision: u64) -> Self {
        Self {
            clock_id,
            precision,
        }
    }

    pub const fn clock_id(&self) -> u8 {
        self.clock_id
    }

    pub const fn precision(&self) -> u64 {
        self.precision
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 9 {
            return Err(CodecError::Invalid("clock_time_get carries nine bytes"));
        }
        let mut precision = [0u8; 8];
        precision.copy_from_slice(&bytes[1..9]);
        Ok(Self::new(bytes[0], u64::from_be_bytes(precision)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockResolution {
    nanos: u64,
}

impl ClockResolution {
    pub const fn new(nanos: u64) -> Self {
        Self { nanos }
    }

    pub const fn nanos(&self) -> u64 {
        self.nanos
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 8 {
            return Err(CodecError::Invalid("clock resolution carries eight bytes"));
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        Ok(Self::new(u64::from_be_bytes(buf)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollOneoff {
    timeout_tick: u64,
}

impl PollOneoff {
    pub const fn new(timeout_tick: u64) -> Self {
        Self { timeout_tick }
    }

    pub const fn timeout_tick(&self) -> u64 {
        self.timeout_tick
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 8 {
            return Err(CodecError::Invalid("poll_oneoff carries eight bytes"));
        }
        let mut timeout = [0u8; 8];
        timeout.copy_from_slice(bytes);
        Ok(Self::new(u64::from_be_bytes(timeout)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomGet {
    lease_id: u8,
    max_len: u8,
}

impl RandomGet {
    pub fn new(max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(MEM_LEASE_NONE, max_len)
    }

    pub fn new_with_lease(lease_id: u8, max_len: u8) -> Result<Self, CodecError> {
        if max_len as usize > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid(
                "random_get request exceeds fixed capacity",
            ));
        }
        Ok(Self { lease_id, max_len })
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("random_get carries two bytes"));
        }
        Self::new_with_lease(bytes[0], bytes[1])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcExitStatus {
    code: u8,
}

impl ProcExitStatus {
    pub const fn new(code: u8) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> u8 {
        self.code
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("proc_exit carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgsSizesGet;

impl ArgsSizesGet {
    pub const fn new() -> Self {
        Self
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if !bytes.is_empty() {
            return Err(CodecError::Invalid("args_sizes_get carries no payload"));
        }
        Ok(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgsGet {
    lease_id: u8,
    max_len: u8,
}

impl ArgsGet {
    pub fn new(max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(MEM_LEASE_NONE, max_len)
    }

    pub fn new_with_lease(lease_id: u8, max_len: u8) -> Result<Self, CodecError> {
        if max_len as usize > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid(
                "args_get request exceeds fixed capacity",
            ));
        }
        Ok(Self { lease_id, max_len })
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("args_get carries two bytes"));
        }
        Self::new_with_lease(bytes[0], bytes[1])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironSizesGet;

impl EnvironSizesGet {
    pub const fn new() -> Self {
        Self
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if !bytes.is_empty() {
            return Err(CodecError::Invalid("environ_sizes_get carries no payload"));
        }
        Ok(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironGet {
    lease_id: u8,
    max_len: u8,
}

impl EnvironGet {
    pub fn new(max_len: u8) -> Result<Self, CodecError> {
        Self::new_with_lease(MEM_LEASE_NONE, max_len)
    }

    pub fn new_with_lease(lease_id: u8, max_len: u8) -> Result<Self, CodecError> {
        if max_len as usize > WASIP1_STREAM_CHUNK_CAPACITY {
            return Err(CodecError::Invalid(
                "environ_get request exceeds fixed capacity",
            ));
        }
        Ok(Self { lease_id, max_len })
    }

    pub const fn lease_id(&self) -> u8 {
        self.lease_id
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("environ_get carries two bytes"));
        }
        Self::new_with_lease(bytes[0], bytes[1])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdWriteDone {
    fd: u8,
    written: u8,
    errno: u16,
}

impl FdWriteDone {
    pub const fn new(fd: u8, written: u8) -> Self {
        Self {
            fd,
            written,
            errno: 0,
        }
    }

    pub const fn new_with_errno(fd: u8, written: u8, errno: u16) -> Self {
        Self { fd, written, errno }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn written(&self) -> u8 {
        self.written
    }

    pub const fn errno(&self) -> u16 {
        self.errno
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 4 {
            return Err(CodecError::Invalid("fd_write reply carries four bytes"));
        }
        Ok(Self::new_with_errno(
            bytes[0],
            bytes[1],
            u16::from_be_bytes([bytes[2], bytes[3]]),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdReadDone {
    fd: u8,
    chunk: Wasip1StreamChunk,
}

impl FdReadDone {
    pub fn new_with_lease(fd: u8, lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self {
            fd,
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
        })
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < 3 {
            return Err(CodecError::Truncated);
        }
        let fd = bytes[0];
        Self::new_with_lease(fd, bytes[1], &bytes[3..]).and_then(|read| {
            if read.len() != bytes[2] as usize {
                return Err(CodecError::Invalid("fd_read reply length mismatch"));
            }
            Ok(read)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdReaddirDone {
    fd: u8,
    chunk: Wasip1StreamChunk,
    errno: u16,
}

impl FdReaddirDone {
    pub fn new_with_lease(
        fd: u8,
        lease_id: u8,
        bytes: &[u8],
        errno: u16,
    ) -> Result<Self, CodecError> {
        Ok(Self {
            fd,
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
            errno,
        })
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub const fn errno(&self) -> u16 {
        self.errno
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < 5 {
            return Err(CodecError::Truncated);
        }
        let fd = bytes[0];
        let lease_id = bytes[1];
        let errno = u16::from_be_bytes([bytes[2], bytes[3]]);
        let len = bytes[4] as usize;
        let payload = &bytes[5..];
        if payload.len() != len {
            return Err(CodecError::Invalid("fd_readdir reply length mismatch"));
        }
        Self::new_with_lease(fd, lease_id, payload, errno)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdStat {
    fd: u8,
    rights: MemRights,
}

impl FdStat {
    pub const fn new(fd: u8, rights: MemRights) -> Self {
        Self { fd, rights }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn rights(&self) -> MemRights {
        self.rights
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("fd_fdstat_get reply carries two bytes"));
        }
        Ok(Self::new(bytes[0], MemRights::decode(bytes[1])?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdClosed {
    fd: u8,
}

impl FdClosed {
    pub const fn new(fd: u8) -> Self {
        Self { fd }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("fd_close reply carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollReady {
    ready: u8,
}

impl PollReady {
    pub const fn new(ready: u8) -> Self {
        Self { ready }
    }

    pub const fn ready(&self) -> u8 {
        self.ready
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("poll_oneoff reply carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomDone {
    chunk: Wasip1StreamChunk,
}

impl RandomDone {
    pub fn new_with_lease(lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self {
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
        })
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        Wasip1StreamChunk::decode(bytes).map(|chunk| Self { chunk })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgsSizes {
    count: u8,
    buf_size: u8,
}

impl ArgsSizes {
    pub const fn new(count: u8, buf_size: u8) -> Self {
        Self { count, buf_size }
    }

    pub const fn count(&self) -> u8 {
        self.count
    }

    pub const fn buf_size(&self) -> u8 {
        self.buf_size
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("args sizes reply carries two bytes"));
        }
        Ok(Self::new(bytes[0], bytes[1]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgsDone {
    chunk: Wasip1StreamChunk,
}

impl ArgsDone {
    pub fn new_with_lease(lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self {
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
        })
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        Wasip1StreamChunk::decode(bytes).map(|chunk| Self { chunk })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironSizes {
    count: u8,
    buf_size: u8,
}

impl EnvironSizes {
    pub const fn new(count: u8, buf_size: u8) -> Self {
        Self { count, buf_size }
    }

    pub const fn count(&self) -> u8 {
        self.count
    }

    pub const fn buf_size(&self) -> u8 {
        self.buf_size
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("environ sizes reply carries two bytes"));
        }
        Ok(Self::new(bytes[0], bytes[1]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironDone {
    chunk: Wasip1StreamChunk,
}

impl EnvironDone {
    pub fn new_with_lease(lease_id: u8, bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self {
            chunk: Wasip1StreamChunk::new_with_lease(lease_id, bytes)?,
        })
    }

    pub const fn lease_id(&self) -> u8 {
        self.chunk.lease_id()
    }

    pub const fn len(&self) -> usize {
        self.chunk.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        Wasip1StreamChunk::decode(bytes).map(|chunk| Self { chunk })
    }
}

fn decode_u32_payload(bytes: &[u8]) -> Result<u32, CodecError> {
    if bytes.len() < 4 {
        return Err(CodecError::Truncated);
    }
    if bytes.len() > 4 {
        return Err(CodecError::Invalid("unexpected trailing payload bytes"));
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(bytes);
    Ok(u32::from_be_bytes(buf))
}
