use hibana_pico::{
    choreography::protocol::{FdRead, FdWrite, MemBorrow, MemRelease},
    kernel::{
        choreofs::{
            ChoreoFsError, ChoreoFsObjectKind, ChoreoFsStore, WASIP1_RIGHT_FD_READ,
            WASIP1_RIGHT_FD_READDIR, WASIP1_RIGHT_FD_WRITE, pico_rights_from_wasip1_base,
        },
        guest_ledger::{
            GuestFd, GuestFdKind, GuestLedger, GuestLedgerError, GuestQuotaLimits, WasiErrnoMap,
            WasiProfile,
        },
        wasi::PicoFdError,
        wasi::{ChoreoResourceKind, PicoFdRights, PicoFdViewSource},
    },
};

type TestLedger = GuestLedger<16, 4, 4>;
type TestStore = ChoreoFsStore<8, 64, 64>;

fn ledger() -> TestLedger {
    GuestLedger::new(
        WasiProfile::HostFull,
        4096,
        1,
        GuestQuotaLimits::new(16, 4),
        WasiErrnoMap::new(),
    )
}

const TEST_CHOREOFS_PREOPEN_LANE: u8 = 7;
const TEST_CHOREOFS_PREOPEN_ROUTE_LABEL: u8 = 0;
const TEST_CHOREOFS_OBJECT_LANE: u8 = 8;
const TEST_CHOREOFS_OBJECT_ROUTE_LABEL: u8 = 1;
const TEST_CHOREOFS_DIRECTORY_ROUTE_LABEL: u8 = 2;

fn grant_preopen(ledger: &mut TestLedger, fd: u8) -> Result<GuestFd, ChoreoFsError> {
    Ok(ledger.apply_fd_cap_grant(
        fd,
        PicoFdRights::Read,
        ChoreoResourceKind::PreopenRoot,
        TEST_CHOREOFS_PREOPEN_LANE,
        TEST_CHOREOFS_PREOPEN_ROUTE_LABEL,
        0,
        0,
        0,
        0,
        0,
    )?)
}

fn route_label_for(resource: ChoreoResourceKind) -> u8 {
    match resource {
        ChoreoResourceKind::DirectoryView => TEST_CHOREOFS_DIRECTORY_ROUTE_LABEL,
        _ => TEST_CHOREOFS_OBJECT_ROUTE_LABEL,
    }
}

fn open_path_with_ledger(
    store: &TestStore,
    ledger: &mut TestLedger,
    preopen_fd: u8,
    new_fd: u8,
    path: &[u8],
    rights: PicoFdRights,
) -> Result<GuestFd, ChoreoFsError> {
    ledger.resolve_fd(
        preopen_fd,
        PicoFdRights::Read,
        ChoreoResourceKind::PreopenRoot,
    )?;
    let opened = store.open(path, rights)?;
    Ok(ledger.apply_fd_cap_mint(
        new_fd,
        rights,
        opened.resource(),
        TEST_CHOREOFS_OBJECT_LANE,
        route_label_for(opened.resource()),
        opened.object_id(),
        0,
        opened.object_id(),
        0,
        opened.generation(),
        0,
    )?)
}

fn mint_fd_after_choreofs_open_route(
    store: &TestStore,
    ledger: &mut TestLedger,
    preopen_fd: u8,
    new_fd: u8,
    path: &[u8],
    rights_base: u64,
) -> Result<GuestFd, ChoreoFsError> {
    open_path_with_ledger(
        store,
        ledger,
        preopen_fd,
        new_fd,
        path,
        pico_rights_from_wasip1_base(rights_base),
    )
}

#[test]
fn choreofs_path_open_mints_object_fd_and_uses_lease_backed_read() {
    let mut store = TestStore::new();
    store
        .install_directory(b"app")
        .expect("install app directory");
    store
        .install_static_blob(b"app/config", b"mode=demo")
        .expect("install config object");

    let mut ledger = ledger();
    let preopen = grant_preopen(&mut ledger, 3).expect("grant preopen root");
    assert_eq!(preopen.fd(), 3);
    assert_eq!(preopen.kind(), GuestFdKind::PreopenRoot);
    assert_eq!(preopen.source(), PicoFdViewSource::Grant);

    let object_fd = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"app/config",
        WASIP1_RIGHT_FD_READ,
    )
    .expect("open config object through manifest");
    assert_eq!(object_fd.kind(), GuestFdKind::ChoreoObject);
    assert_eq!(object_fd.source(), PicoFdViewSource::Mint);
    let stat = store.stat_fd(object_fd).expect("stat opened object");
    assert_eq!(stat.kind(), ChoreoFsObjectKind::StaticBlob);
    assert_eq!(stat.size(), b"mode=demo".len());

    let grant = ledger
        .grant_write_lease(MemBorrow::new(128, 16, 1))
        .expect("grant write lease for fd_read destination");
    let read = FdRead::new_with_lease(object_fd.fd(), grant.lease_id(), 16).expect("fd_read");
    let token = ledger
        .begin_choreofs_read(&read, grant)
        .expect("begin ChoreoFS read pending token");

    let mut out = [0u8; 16];
    let len = store
        .read(object_fd, 0, &mut out)
        .expect("read static config object");
    assert_eq!(&out[..len], b"mode=demo");

    ledger
        .complete_choreofs_read(token, object_fd.fd(), grant.lease_id(), len as u16)
        .expect("complete ChoreoFS read pending token");
    ledger
        .release_lease(MemRelease::new(grant.lease_id()))
        .expect("release read destination lease");
}

#[test]
fn choreofs_config_and_append_log_are_bounded_minted_objects() {
    let mut store = TestStore::new();
    store.install_directory(b"app").expect("install app dir");
    store
        .install_config_cell(b"app/state", b"v1")
        .expect("install config cell");
    store
        .install_append_log(b"app/events")
        .expect("install append log");

    let mut ledger = ledger();
    grant_preopen(&mut ledger, 3).expect("grant preopen root");
    let state_fd = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"app/state",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open config cell");
    let log_fd = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        5,
        b"app/events",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open append log");

    let grant = ledger
        .grant_read_lease(MemBorrow::new(64, 2, 1))
        .expect("grant source lease");
    let write = FdWrite::new_with_lease(state_fd.fd(), grant.lease_id(), b"v2").expect("fd_write");
    let token = ledger
        .begin_choreofs_write(&write, grant)
        .expect("begin config write");
    let written = store.write(state_fd, 0, b"v2").expect("write config cell");
    ledger
        .complete_choreofs_write(token, state_fd.fd(), grant.lease_id(), written as u16)
        .expect("complete config write");
    ledger
        .release_lease(MemRelease::new(grant.lease_id()))
        .expect("release source lease");

    let mut out = [0u8; 8];
    let len = store.read(state_fd, 0, &mut out).expect("read new config");
    assert_eq!(&out[..len], b"v2");

    assert_eq!(
        store.write(log_fd, 1, b"bad"),
        Err(ChoreoFsError::BadOffset),
        "append log must not allow random writes"
    );
    assert_eq!(store.write(log_fd, 0, b"a"), Ok(1));
    assert_eq!(store.write(log_fd, 1, b"b"), Ok(1));
    let len = store.read(log_fd, 0, &mut out).expect("read append log");
    assert_eq!(&out[..len], b"ab");
}

#[test]
fn choreofs_gpio_device_object_mints_gpio_fd_without_data_path() {
    let mut store = TestStore::new();
    store
        .install_gpio_device(b"device/led/green")
        .expect("install GPIO device object");

    let mut ledger = ledger();
    grant_preopen(&mut ledger, 3).expect("grant preopen root");
    let fd = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"device/led/green",
        WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open GPIO device object");

    assert_eq!(fd.kind(), GuestFdKind::Gpio);
    assert_eq!(fd.source(), PicoFdViewSource::Mint);
    assert_eq!(fd.choreo_object_generation(), 1);
    assert_eq!(
        ledger.resolve_fd(4, PicoFdRights::Write, ChoreoResourceKind::Gpio),
        Ok(fd)
    );

    let mut out = [0u8; 4];
    assert_eq!(store.read(fd, 0, &mut out), Err(ChoreoFsError::WrongFdKind));
    assert_eq!(store.write(fd, 0, b"1"), Err(ChoreoFsError::WrongFdKind));
}

#[test]
fn choreofs_plan_object_vocabulary_mints_resource_fds_without_data_authority() {
    let mut store = TestStore::new();
    store
        .install_timer_device(b"device/timer0")
        .expect("install timer object");
    store
        .install_uart_device(b"device/uart0")
        .expect("install uart object");
    store
        .install_network_datagram(b"net/datagram0")
        .expect("install datagram object");
    store
        .install_network_stream(b"net/stream0")
        .expect("install stream object");
    store
        .install_network_listener(b"net/listener0")
        .expect("install listener object");
    store
        .install_remote_object(b"remote/sensor0")
        .expect("install remote object");
    store
        .install_management_object(b"mgmt/update")
        .expect("install management object");
    store
        .install_telemetry_object(b"telemetry/log")
        .expect("install telemetry object");

    let mut ledger = ledger();
    grant_preopen(&mut ledger, 3).expect("grant preopen root");

    let timer = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"device/timer0",
        WASIP1_RIGHT_FD_READ,
    )
    .expect("open timer");
    assert_eq!(timer.kind(), GuestFdKind::Timer);

    let uart = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        5,
        b"device/uart0",
        WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open uart");
    assert_eq!(uart.kind(), GuestFdKind::Uart);

    let datagram = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        6,
        b"net/datagram0",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open datagram");
    assert_eq!(datagram.kind(), GuestFdKind::Datagram);

    let stream = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        7,
        b"net/stream0",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open stream");
    assert_eq!(stream.kind(), GuestFdKind::Stream);

    let listener = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        8,
        b"net/listener0",
        WASIP1_RIGHT_FD_READ,
    )
    .expect("open listener");
    assert_eq!(listener.kind(), GuestFdKind::NetworkListener);

    let remote = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        9,
        b"remote/sensor0",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open remote object");
    assert_eq!(remote.kind(), GuestFdKind::RemoteObject);

    let management = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        10,
        b"mgmt/update",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open management object");
    assert_eq!(management.kind(), GuestFdKind::Management);

    let telemetry = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        11,
        b"telemetry/log",
        WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open telemetry object");
    assert_eq!(telemetry.kind(), GuestFdKind::Telemetry);

    let mut out = [0u8; 4];
    for fd in [
        timer, uart, datagram, stream, listener, remote, management, telemetry,
    ] {
        assert_eq!(store.read(fd, 0, &mut out), Err(ChoreoFsError::WrongFdKind));
        assert_eq!(store.write(fd, 0, b"x"), Err(ChoreoFsError::WrongFdKind));
    }
}

#[test]
fn choreofs_image_slot_and_state_snapshot_are_bounded_storage_objects() {
    let mut store = TestStore::new();
    store
        .install_image_slot(b"image/app0")
        .expect("install image slot");
    store
        .install_state_snapshot(b"state/last", b"old")
        .expect("install state snapshot");

    let mut ledger = ledger();
    grant_preopen(&mut ledger, 3).expect("grant preopen root");
    let image = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"image/app0",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open image slot");
    let state = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        5,
        b"state/last",
        WASIP1_RIGHT_FD_READ | WASIP1_RIGHT_FD_WRITE,
    )
    .expect("open state snapshot");

    assert_eq!(store.write(image, 0, b"ab"), Ok(2));
    assert_eq!(store.write(image, 2, b"cd"), Ok(2));
    let mut out = [0u8; 8];
    let len = store.read(image, 0, &mut out).expect("read image slot");
    assert_eq!(&out[..len], b"abcd");

    assert_eq!(store.write(state, 0, b"new"), Ok(3));
    assert_eq!(store.write(state, 1, b"bad"), Err(ChoreoFsError::BadOffset));
    let len = store.read(state, 0, &mut out).expect("read state snapshot");
    assert_eq!(&out[..len], b"new");
}

#[test]
fn choreofs_directory_view_and_path_normalization_reject() {
    let mut store = TestStore::new();
    store.install_directory(b"app").expect("install app dir");
    store
        .install_static_blob(b"app/config", b"cfg")
        .expect("install config");
    store
        .install_static_blob(b"app/state", b"state")
        .expect("install state");
    store
        .install_static_blob(b"other/root", b"ignored")
        .expect("install non-child object");

    let mut ledger = ledger();
    grant_preopen(&mut ledger, 3).expect("grant preopen root");
    let dir_fd = mint_fd_after_choreofs_open_route(
        &store,
        &mut ledger,
        3,
        4,
        b"app",
        WASIP1_RIGHT_FD_READDIR,
    )
    .expect("open directory view");
    assert_eq!(dir_fd.kind(), GuestFdKind::DirectoryView);
    assert_eq!(
        store.stat_path(b"app/config").expect("stat path").size(),
        b"cfg".len()
    );

    let mut out = [0u8; 32];
    let read = store
        .read_directory(dir_fd, 0, &mut out)
        .expect("read manifest directory view");
    assert!(read.done());
    assert_eq!(&out[..read.written()], b"config\nstate\n");

    assert_eq!(
        store.open(b"../secret", PicoFdRights::Read),
        Err(ChoreoFsError::InvalidComponent)
    );
    assert_eq!(
        store.open(b"/app/config", PicoFdRights::Read),
        Err(ChoreoFsError::AbsolutePath)
    );
    assert_eq!(pico_rights_from_wasip1_base(0), PicoFdRights::None);
    assert_eq!(
        mint_fd_after_choreofs_open_route(&store, &mut ledger, 3, 6, b"app/config", 0),
        Err(ChoreoFsError::PermissionDenied),
        "empty WASI rights must not silently become read authority"
    );
    assert_eq!(
        open_path_with_ledger(&store, &mut ledger, 3, 6, b"missing", PicoFdRights::Read),
        Err(ChoreoFsError::NotFound)
    );
    assert_eq!(
        ledger.resolve_fd(4, PicoFdRights::Read, ChoreoResourceKind::ChoreoObject),
        Err(GuestLedgerError::Fd(PicoFdError::WrongResource)),
        "directory fd must not be usable as an object fd"
    );
}
