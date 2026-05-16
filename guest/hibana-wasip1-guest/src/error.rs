#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidPath,
    PollNotReady { ready: usize },
    ShortWrite { expected: usize, actual: usize },
    UnexpectedEvent { event_type: u8 },
    Wasi { syscall: Syscall, errno: u16 },
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Syscall {
    FdWrite,
    PathOpen,
    PollOneoff,
}
