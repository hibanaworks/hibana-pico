#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmEngineProfile {
    None,
    Tiny,
    /// Core Wasm execution capacity only. This does not imply any WASI P1
    /// syscall handler; syscall imports are selected by `wasip1-sys-*`.
    Core,
    Wasip1StdProfile,
    Wasip1Full,
}

impl WasmEngineProfile {
    pub const fn active() -> Self {
        if cfg!(feature = "wasm-engine-wasip1-full") {
            Self::Wasip1Full
        } else if cfg!(feature = "wasm-engine-wasip1-std-profile") {
            Self::Wasip1StdProfile
        } else if cfg!(feature = "wasm-engine-core") {
            Self::Core
        } else if cfg!(feature = "wasm-engine-tiny") {
            Self::Tiny
        } else {
            Self::None
        }
    }

    pub const fn can_run_ordinary_wasip1_std(self) -> bool {
        matches!(self, Self::Wasip1Full)
    }

    pub const fn can_run_core_wasip1(self) -> bool {
        matches!(self, Self::Core | Self::Wasip1StdProfile | Self::Wasip1Full)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wasip1Syscall {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wasip1ImportDisposition {
    Supported,
    TypedEnosys,
    TypedReject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wasip1ImportEffectiveDisposition {
    Supported,
    TypedEnosys,
    TypedReject,
    UnsupportedByProfile,
}

pub const WASIP1_PREVIEW1_MODULE: &str = "wasi_snapshot_preview1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wasip1ImportName {
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
    FdPrestatDirName,
    FdPrestatGet,
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
    pub const fn name(self) -> &'static str {
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
            Self::FdPrestatDirName => "fd_prestat_dir_name",
            Self::FdPrestatGet => "fd_prestat_get",
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

    pub const fn syscall(self) -> Wasip1Syscall {
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

    pub const fn disposition(self) -> Wasip1ImportDisposition {
        match self {
            Self::FdAdvise
            | Self::FdAllocate
            | Self::FdDatasync
            | Self::FdFdstatSetFlags
            | Self::FdFdstatSetRights
            | Self::FdFilestatSetSize
            | Self::FdFilestatSetTimes
            | Self::FdPwrite
            | Self::FdRenumber
            | Self::FdSync
            | Self::PathCreateDirectory
            | Self::PathFilestatSetTimes
            | Self::PathLink
            | Self::PathReadlink
            | Self::PathRemoveDirectory
            | Self::PathRename
            | Self::PathSymlink
            | Self::PathUnlinkFile => Wasip1ImportDisposition::TypedEnosys,
            Self::ProcRaise | Self::SockAccept => Wasip1ImportDisposition::TypedReject,
            _ => Wasip1ImportDisposition::Supported,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        WASIP1_PREVIEW1_IMPORTS
            .iter()
            .copied()
            .find(|import| import.name().as_bytes() == bytes)
    }
}

pub const WASIP1_PREVIEW1_IMPORTS: [Wasip1ImportName; 46] = [
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wasip1ImportCoverage {
    pub kind: Wasip1ImportName,
    pub import: &'static str,
    pub syscall: Wasip1Syscall,
    pub disposition: Wasip1ImportDisposition,
}

impl Wasip1ImportCoverage {
    pub const fn from_import(kind: Wasip1ImportName) -> Self {
        Self {
            kind,
            import: kind.name(),
            syscall: kind.syscall(),
            disposition: kind.disposition(),
        }
    }

    pub const fn effective(self, handlers: Wasip1HandlerSet) -> Wasip1ImportEffectiveDisposition {
        if !handlers.supports(self.syscall) {
            return Wasip1ImportEffectiveDisposition::UnsupportedByProfile;
        }
        match self.disposition {
            Wasip1ImportDisposition::Supported => Wasip1ImportEffectiveDisposition::Supported,
            Wasip1ImportDisposition::TypedEnosys => Wasip1ImportEffectiveDisposition::TypedEnosys,
            Wasip1ImportDisposition::TypedReject => Wasip1ImportEffectiveDisposition::TypedReject,
        }
    }
}

pub const WASIP1_PREVIEW1_IMPORT_COVERAGE: [Wasip1ImportCoverage; 46] = [
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ArgsGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ArgsSizesGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ClockResGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ClockTimeGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::EnvironGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::EnvironSizesGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdAdvise),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdAllocate),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdClose),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdDatasync),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFdstatGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFdstatSetFlags),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFdstatSetRights),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFilestatGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFilestatSetSize),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdFilestatSetTimes),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdPread),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdPrestatGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdPrestatDirName),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdPwrite),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdRead),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdReaddir),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdRenumber),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdSeek),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdSync),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdTell),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::FdWrite),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathCreateDirectory),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathFilestatGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathFilestatSetTimes),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathLink),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathOpen),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathReadlink),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathRemoveDirectory),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathRename),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathSymlink),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PathUnlinkFile),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::PollOneoff),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ProcExit),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::ProcRaise),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::RandomGet),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::SchedYield),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::SockAccept),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::SockRecv),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::SockSend),
    Wasip1ImportCoverage::from_import(Wasip1ImportName::SockShutdown),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wasip1HandlerSet {
    pub args_env: bool,
    pub fd_write: bool,
    pub fd_read: bool,
    pub fd_fdstat_get: bool,
    pub fd_close: bool,
    pub clock_res_get: bool,
    pub clock_time_get: bool,
    pub poll_oneoff: bool,
    pub random_get: bool,
    pub proc_exit: bool,
    pub proc_raise: bool,
    pub sched_yield: bool,
    pub path_minimal: bool,
    pub path_full: bool,
    pub network_object: bool,
}

impl Wasip1HandlerSet {
    pub const EMPTY: Self = Self {
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

    pub const PICO_MIN: Self = Self {
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

    pub const PICO_STD_START: Self = Self {
        args_env: true,
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

    pub const PICO_STD_CHOREOFS: Self = Self {
        args_env: true,
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
        path_minimal: true,
        path_full: false,
        network_object: false,
    };

    pub const FULL: Self = Self {
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

    pub const fn active() -> Self {
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
            path_minimal: cfg!(feature = "wasip1-sys-path-minimal"),
            path_full: cfg!(feature = "wasip1-sys-path-full"),
            network_object: cfg!(feature = "wasip1-sys-sock"),
        }
    }

    pub const fn supports(self, syscall: Wasip1Syscall) -> bool {
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

    pub const fn implemented_count(self) -> usize {
        self.args_env as usize
            + self.fd_write as usize
            + self.fd_read as usize
            + self.fd_fdstat_get as usize
            + self.fd_close as usize
            + self.clock_res_get as usize
            + self.clock_time_get as usize
            + self.poll_oneoff as usize
            + self.random_get as usize
            + self.proc_exit as usize
            + self.proc_raise as usize
            + self.sched_yield as usize
            + self.path_minimal as usize
            + self.path_full as usize
            + self.network_object as usize
    }

    pub const fn is_fullish(self) -> bool {
        self.args_env
            && self.fd_write
            && self.fd_read
            && self.fd_fdstat_get
            && self.fd_close
            && self.clock_res_get
            && self.clock_time_get
            && self.poll_oneoff
            && self.random_get
            && self.proc_exit
            && self.proc_raise
            && self.sched_yield
            && self.path_minimal
            && self.path_full
            && self.network_object
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wasip1ControlCapacity {
    pub fd_view: bool,
    pub memory_lease: bool,
    pub errno: bool,
    pub import_validation: bool,
    pub unsupported_reject: bool,
    pub memory_grow_fence: bool,
}

impl Wasip1ControlCapacity {
    pub const FULL: Self = Self {
        fd_view: true,
        memory_lease: true,
        errno: true,
        import_validation: true,
        unsupported_reject: true,
        memory_grow_fence: true,
    };

    pub const fn active() -> Self {
        Self {
            fd_view: cfg!(feature = "wasip1-ctrl-fd-view"),
            memory_lease: cfg!(feature = "wasip1-ctrl-memory-lease"),
            errno: cfg!(feature = "wasip1-ctrl-errno"),
            import_validation: cfg!(feature = "wasip1-ctrl-import-validation"),
            unsupported_reject: cfg!(feature = "wasip1-ctrl-unsupported-reject"),
            memory_grow_fence: cfg!(feature = "wasip1-ctrl-memory-grow-fence"),
        }
    }

    pub const fn is_complete_for_wasip1(self) -> bool {
        self.fd_view
            && self.memory_lease
            && self.errno
            && self.import_validation
            && self.unsupported_reject
            && self.memory_grow_fence
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureProfiles {
    pub rp2040_pico_min: bool,
    pub rp2040_picow_swarm_min: bool,
    pub rp2350_pico2w_swarm_min: bool,
    pub host_qemu_swarm: bool,
    pub host_linux_wasip1_full: bool,
}

impl FeatureProfiles {
    pub const fn active() -> Self {
        Self {
            rp2040_pico_min: cfg!(feature = "profile-rp2040-pico-min"),
            rp2040_picow_swarm_min: cfg!(feature = "profile-rp2040-picow-swarm-min"),
            rp2350_pico2w_swarm_min: cfg!(feature = "profile-rp2350-pico2w-swarm-min"),
            host_qemu_swarm: cfg!(feature = "profile-host-qemu-swarm"),
            host_linux_wasip1_full: cfg!(feature = "profile-host-linux-wasip1-full"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureMatrix {
    pub profiles: FeatureProfiles,
    pub engine: WasmEngineProfile,
    pub wasip1_handlers: Wasip1HandlerSet,
    pub wasip1_control: Wasip1ControlCapacity,
}

impl FeatureMatrix {
    pub const fn active() -> Self {
        Self {
            profiles: FeatureProfiles::active(),
            engine: WasmEngineProfile::active(),
            wasip1_handlers: Wasip1HandlerSet::active(),
            wasip1_control: Wasip1ControlCapacity::active(),
        }
    }

    pub const fn can_claim_wasip1_profile(self) -> bool {
        self.engine.can_run_core_wasip1()
            && self.wasip1_control.is_complete_for_wasip1()
            && self.wasip1_handlers.implemented_count() > 0
    }

    pub const fn can_claim_full_ordinary_std(self) -> bool {
        self.engine.can_run_ordinary_wasip1_std()
            && self.wasip1_handlers.is_fullish()
            && self.wasip1_control.is_complete_for_wasip1()
    }
}

pub const ACTIVE_FEATURE_MATRIX: FeatureMatrix = FeatureMatrix::active();

#[cfg(test)]
mod tests {
    use super::{
        FeatureMatrix, Wasip1ControlCapacity, Wasip1HandlerSet, Wasip1Syscall, WasmEngineProfile,
    };

    #[test]
    fn pico_min_profile_has_only_the_small_wasi_surface() {
        let handlers = Wasip1HandlerSet::PICO_MIN;

        assert!(handlers.supports(Wasip1Syscall::FdWrite));
        assert!(handlers.supports(Wasip1Syscall::PollOneoff));
        assert!(handlers.supports(Wasip1Syscall::ProcExit));
        assert!(!handlers.supports(Wasip1Syscall::ProcRaise));
        assert!(!handlers.supports(Wasip1Syscall::FdRead));
        assert!(!handlers.supports(Wasip1Syscall::RandomGet));
        assert!(!handlers.is_fullish());
    }

    #[test]
    fn core_wasm_engine_profile_does_not_imply_wasi_syscalls() {
        let matrix = FeatureMatrix {
            profiles: Default::default(),
            engine: WasmEngineProfile::Core,
            wasip1_handlers: Wasip1HandlerSet::EMPTY,
            wasip1_control: Wasip1ControlCapacity::FULL,
        };

        assert!(matrix.engine.can_run_core_wasip1());
        assert!(!matrix.wasip1_handlers.supports(Wasip1Syscall::ProcExit));
        assert!(!matrix.wasip1_handlers.supports(Wasip1Syscall::FdWrite));
        assert!(!matrix.can_claim_wasip1_profile());
    }

    #[test]
    fn full_profile_requires_engine_handlers_and_common_control_capacity() {
        let matrix = FeatureMatrix {
            profiles: Default::default(),
            engine: WasmEngineProfile::Wasip1Full,
            wasip1_handlers: Wasip1HandlerSet::FULL,
            wasip1_control: Wasip1ControlCapacity::FULL,
        };

        assert!(matrix.can_claim_wasip1_profile());
        assert!(matrix.can_claim_full_ordinary_std());
    }
}
