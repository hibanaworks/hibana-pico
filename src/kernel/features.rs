#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wasip1Syscall {
    ArgsEnv,
    FdWrite,
    FdRead,
    FdFdstatGet,
    FdClose,
    ClockResGet,
    ClockTimeGet,
    PollOneoff,
    RandomGet,
    ProcExit,
    ProcRaise,
    SchedYield,
    PathMinimal,
    PathFull,
    NetworkObject,
}

pub(crate) const WASIP1_PREVIEW1_MODULE: &str = "wasi_snapshot_preview1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wasip1ImportName {
    ArgsGet,
    ArgsSizesGet,
    ClockResGet,
    ClockTimeGet,
    EnvironGet,
    EnvironSizesGet,
    FdAdvise,
    FdAllocate,
    FdClose,
    FdDatasync,
    FdFdstatGet,
    FdFdstatSetFlags,
    FdFdstatSetRights,
    FdFilestatGet,
    FdFilestatSetSize,
    FdFilestatSetTimes,
    FdPread,
    FdPrestatGet,
    FdPrestatDirName,
    FdPwrite,
    FdRead,
    FdReaddir,
    FdRenumber,
    FdSeek,
    FdSync,
    FdTell,
    FdWrite,
    PathCreateDirectory,
    PathFilestatGet,
    PathFilestatSetTimes,
    PathLink,
    PathOpen,
    PathReadlink,
    PathRemoveDirectory,
    PathRename,
    PathSymlink,
    PathUnlinkFile,
    PollOneoff,
    ProcExit,
    ProcRaise,
    RandomGet,
    SchedYield,
    SockAccept,
    SockRecv,
    SockSend,
    SockShutdown,
}

impl Wasip1ImportName {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::ArgsGet => "args_get",
            Self::ArgsSizesGet => "args_sizes_get",
            Self::ClockResGet => "clock_res_get",
            Self::ClockTimeGet => "clock_time_get",
            Self::EnvironGet => "environ_get",
            Self::EnvironSizesGet => "environ_sizes_get",
            Self::FdAdvise => "fd_advise",
            Self::FdAllocate => "fd_allocate",
            Self::FdClose => "fd_close",
            Self::FdDatasync => "fd_datasync",
            Self::FdFdstatGet => "fd_fdstat_get",
            Self::FdFdstatSetFlags => "fd_fdstat_set_flags",
            Self::FdFdstatSetRights => "fd_fdstat_set_rights",
            Self::FdFilestatGet => "fd_filestat_get",
            Self::FdFilestatSetSize => "fd_filestat_set_size",
            Self::FdFilestatSetTimes => "fd_filestat_set_times",
            Self::FdPread => "fd_pread",
            Self::FdPrestatGet => "fd_prestat_get",
            Self::FdPrestatDirName => "fd_prestat_dir_name",
            Self::FdPwrite => "fd_pwrite",
            Self::FdRead => "fd_read",
            Self::FdReaddir => "fd_readdir",
            Self::FdRenumber => "fd_renumber",
            Self::FdSeek => "fd_seek",
            Self::FdSync => "fd_sync",
            Self::FdTell => "fd_tell",
            Self::FdWrite => "fd_write",
            Self::PathCreateDirectory => "path_create_directory",
            Self::PathFilestatGet => "path_filestat_get",
            Self::PathFilestatSetTimes => "path_filestat_set_times",
            Self::PathLink => "path_link",
            Self::PathOpen => "path_open",
            Self::PathReadlink => "path_readlink",
            Self::PathRemoveDirectory => "path_remove_directory",
            Self::PathRename => "path_rename",
            Self::PathSymlink => "path_symlink",
            Self::PathUnlinkFile => "path_unlink_file",
            Self::PollOneoff => "poll_oneoff",
            Self::ProcExit => "proc_exit",
            Self::ProcRaise => "proc_raise",
            Self::RandomGet => "random_get",
            Self::SchedYield => "sched_yield",
            Self::SockAccept => "sock_accept",
            Self::SockRecv => "sock_recv",
            Self::SockSend => "sock_send",
            Self::SockShutdown => "sock_shutdown",
        }
    }

    pub(crate) const fn syscall(self) -> Wasip1Syscall {
        match self {
            Self::ArgsGet | Self::ArgsSizesGet | Self::EnvironGet | Self::EnvironSizesGet => {
                Wasip1Syscall::ArgsEnv
            }
            Self::ClockResGet => Wasip1Syscall::ClockResGet,
            Self::ClockTimeGet => Wasip1Syscall::ClockTimeGet,
            Self::FdClose => Wasip1Syscall::FdClose,
            Self::FdFdstatGet => Wasip1Syscall::FdFdstatGet,
            Self::FdRead => Wasip1Syscall::FdRead,
            Self::FdWrite => Wasip1Syscall::FdWrite,
            Self::FdPrestatGet
            | Self::FdPrestatDirName
            | Self::FdFilestatGet
            | Self::FdReaddir
            | Self::PathCreateDirectory
            | Self::PathFilestatGet
            | Self::PathOpen
            | Self::PathReadlink
            | Self::PathRemoveDirectory
            | Self::PathRename
            | Self::PathUnlinkFile => Wasip1Syscall::PathMinimal,
            Self::FdAdvise
            | Self::FdAllocate
            | Self::FdDatasync
            | Self::FdFdstatSetFlags
            | Self::FdFdstatSetRights
            | Self::FdFilestatSetSize
            | Self::FdFilestatSetTimes
            | Self::FdPread
            | Self::FdPwrite
            | Self::FdRenumber
            | Self::FdSeek
            | Self::FdSync
            | Self::FdTell
            | Self::PathFilestatSetTimes
            | Self::PathLink
            | Self::PathSymlink => Wasip1Syscall::PathFull,
            Self::PollOneoff => Wasip1Syscall::PollOneoff,
            Self::ProcExit => Wasip1Syscall::ProcExit,
            Self::ProcRaise => Wasip1Syscall::ProcRaise,
            Self::RandomGet => Wasip1Syscall::RandomGet,
            Self::SchedYield => Wasip1Syscall::SchedYield,
            Self::SockAccept | Self::SockRecv | Self::SockSend | Self::SockShutdown => {
                Wasip1Syscall::NetworkObject
            }
        }
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        WASIP1_PREVIEW1_IMPORTS
            .iter()
            .copied()
            .find(|import| import.name().as_bytes() == bytes)
    }
}

pub(crate) const WASIP1_PREVIEW1_IMPORTS: [Wasip1ImportName; 46] = [
    Wasip1ImportName::ArgsGet,
    Wasip1ImportName::ArgsSizesGet,
    Wasip1ImportName::ClockResGet,
    Wasip1ImportName::ClockTimeGet,
    Wasip1ImportName::EnvironGet,
    Wasip1ImportName::EnvironSizesGet,
    Wasip1ImportName::FdAdvise,
    Wasip1ImportName::FdAllocate,
    Wasip1ImportName::FdClose,
    Wasip1ImportName::FdDatasync,
    Wasip1ImportName::FdFdstatGet,
    Wasip1ImportName::FdFdstatSetFlags,
    Wasip1ImportName::FdFdstatSetRights,
    Wasip1ImportName::FdFilestatGet,
    Wasip1ImportName::FdFilestatSetSize,
    Wasip1ImportName::FdFilestatSetTimes,
    Wasip1ImportName::FdPread,
    Wasip1ImportName::FdPrestatGet,
    Wasip1ImportName::FdPrestatDirName,
    Wasip1ImportName::FdPwrite,
    Wasip1ImportName::FdRead,
    Wasip1ImportName::FdReaddir,
    Wasip1ImportName::FdRenumber,
    Wasip1ImportName::FdSeek,
    Wasip1ImportName::FdSync,
    Wasip1ImportName::FdTell,
    Wasip1ImportName::FdWrite,
    Wasip1ImportName::PathCreateDirectory,
    Wasip1ImportName::PathFilestatGet,
    Wasip1ImportName::PathFilestatSetTimes,
    Wasip1ImportName::PathLink,
    Wasip1ImportName::PathOpen,
    Wasip1ImportName::PathReadlink,
    Wasip1ImportName::PathRemoveDirectory,
    Wasip1ImportName::PathRename,
    Wasip1ImportName::PathSymlink,
    Wasip1ImportName::PathUnlinkFile,
    Wasip1ImportName::PollOneoff,
    Wasip1ImportName::ProcExit,
    Wasip1ImportName::ProcRaise,
    Wasip1ImportName::RandomGet,
    Wasip1ImportName::SchedYield,
    Wasip1ImportName::SockAccept,
    Wasip1ImportName::SockRecv,
    Wasip1ImportName::SockSend,
    Wasip1ImportName::SockShutdown,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Wasip1HandlerSet {
    pub(crate) args_env: bool,
    pub(crate) fd_write: bool,
    pub(crate) fd_read: bool,
    pub(crate) fd_fdstat_get: bool,
    pub(crate) fd_close: bool,
    pub(crate) clock_res_get: bool,
    pub(crate) clock_time_get: bool,
    pub(crate) poll_oneoff: bool,
    pub(crate) random_get: bool,
    pub(crate) proc_exit: bool,
    pub(crate) proc_raise: bool,
    pub(crate) sched_yield: bool,
    pub(crate) path_minimal: bool,
    pub(crate) path_full: bool,
    pub(crate) network_object: bool,
}

impl Wasip1HandlerSet {
    #[cfg(test)]
    pub(crate) const EMPTY: Self = Self {
        args_env: false,
        fd_write: false,
        fd_read: false,
        fd_fdstat_get: false,
        fd_close: false,
        clock_res_get: false,
        clock_time_get: false,
        poll_oneoff: false,
        random_get: false,
        proc_exit: false,
        proc_raise: false,
        sched_yield: false,
        path_minimal: false,
        path_full: false,
        network_object: false,
    };

    #[cfg(test)]
    pub(crate) const PICO_MIN: Self = Self {
        args_env: false,
        fd_write: true,
        fd_read: false,
        fd_fdstat_get: false,
        fd_close: false,
        clock_res_get: false,
        clock_time_get: false,
        poll_oneoff: true,
        random_get: false,
        proc_exit: true,
        proc_raise: false,
        sched_yield: false,
        path_minimal: false,
        path_full: false,
        network_object: false,
    };

    #[cfg(test)]
    pub(crate) const FULL: Self = Self {
        args_env: true,
        fd_write: true,
        fd_read: true,
        fd_fdstat_get: true,
        fd_close: true,
        clock_res_get: true,
        clock_time_get: true,
        poll_oneoff: true,
        random_get: true,
        proc_exit: true,
        proc_raise: true,
        sched_yield: true,
        path_minimal: true,
        path_full: true,
        network_object: true,
    };

    pub(crate) const fn active() -> Self {
        Self {
            args_env: cfg!(feature = "wasip1-sys-args-env"),
            fd_write: cfg!(feature = "wasip1-sys-fd-write"),
            fd_read: cfg!(feature = "wasip1-sys-fd-read"),
            fd_fdstat_get: cfg!(feature = "wasip1-sys-fd-fdstat-get"),
            fd_close: cfg!(feature = "wasip1-sys-fd-close"),
            clock_res_get: cfg!(feature = "wasip1-sys-clock-res-get"),
            clock_time_get: cfg!(feature = "wasip1-sys-clock-time-get"),
            poll_oneoff: cfg!(feature = "wasip1-sys-poll-oneoff"),
            random_get: cfg!(feature = "wasip1-sys-random-get"),
            proc_exit: cfg!(feature = "wasip1-sys-proc-exit"),
            proc_raise: cfg!(feature = "wasip1-sys-proc-raise"),
            sched_yield: cfg!(feature = "wasip1-sys-sched-yield"),
            path_minimal: cfg!(feature = "wasip1-sys-path-open"),
            path_full: false,
            network_object: cfg!(feature = "wasip1-sys-sock"),
        }
    }

    pub(crate) const fn supports(self, syscall: Wasip1Syscall) -> bool {
        match syscall {
            Wasip1Syscall::ArgsEnv => self.args_env,
            Wasip1Syscall::FdWrite => self.fd_write,
            Wasip1Syscall::FdRead => self.fd_read,
            Wasip1Syscall::FdFdstatGet => self.fd_fdstat_get,
            Wasip1Syscall::FdClose => self.fd_close,
            Wasip1Syscall::ClockResGet => self.clock_res_get,
            Wasip1Syscall::ClockTimeGet => self.clock_time_get,
            Wasip1Syscall::PollOneoff => self.poll_oneoff,
            Wasip1Syscall::RandomGet => self.random_get,
            Wasip1Syscall::ProcExit => self.proc_exit,
            Wasip1Syscall::ProcRaise => self.proc_raise,
            Wasip1Syscall::SchedYield => self.sched_yield,
            Wasip1Syscall::PathMinimal => self.path_minimal,
            Wasip1Syscall::PathFull => self.path_full,
            Wasip1Syscall::NetworkObject => self.network_object,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Wasip1HandlerSet, Wasip1Syscall};

    #[test]
    fn minimal_handler_set_is_small() {
        let handlers = Wasip1HandlerSet::PICO_MIN;

        assert!(handlers.supports(Wasip1Syscall::FdWrite));
        assert!(handlers.supports(Wasip1Syscall::PollOneoff));
        assert!(handlers.supports(Wasip1Syscall::ProcExit));
        assert!(!handlers.supports(Wasip1Syscall::ProcRaise));
        assert!(!handlers.supports(Wasip1Syscall::FdRead));
        assert!(!handlers.supports(Wasip1Syscall::RandomGet));
    }
}
