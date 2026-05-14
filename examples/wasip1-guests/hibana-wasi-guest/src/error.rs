#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidPath,
    InvalidPayload,
    PayloadTooLarge { max: usize, actual: usize },
    PollNotReady { ready: usize },
    ShortWrite { expected: usize, actual: usize },
    UnexpectedEvent { event_type: u8 },
    UnexpectedSocketFlags { flags: u32 },
    UnexpectedSocketLength { max: usize, actual: usize },
    Wasi { syscall: Syscall, errno: u16 },
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Syscall {
    FdWrite,
    PathOpen,
    PollOneoff,
    SockSend,
    SockRecv,
    SockShutdown,
    SockAccept,
}
