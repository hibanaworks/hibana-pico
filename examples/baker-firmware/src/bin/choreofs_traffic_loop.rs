#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana::runtime::wire::WireEncode;
use hibana_pico::appkit;
use hibana_wasip1_runtime::choreofs;
use hibana_wasip1_runtime::protocol::{
    FdBinding, FdFdstatGetReqMsg, FdFdstatGetRetMsg, FdPrestat, FdPrestatDirNameDone,
    FdPrestatDirNameReqMsg, FdPrestatDirNameRetMsg, FdPrestatGetReqMsg, FdPrestatGetRetMsg, FdStat,
    FdStatRet, FdWrite, FdWriteObjectReqMsg, FdWriteObjectRetMsg, FdWriteRow,
    LABEL_WASI_FD_FDSTAT_GET, LABEL_WASI_FD_PRESTAT_DIR_NAME, LABEL_WASI_FD_PRESTAT_GET,
    LABEL_WASI_FD_WRITE_OBJECT, LABEL_WASI_PATH_OPEN, LABEL_WASI_POLL_ONEOFF, MemRights, PathOpen,
    PathOpenReqMsg, PathOpenRetMsg, PollOneoffReqMsg, PollOneoffRetMsg, PollReady, PollReadyRet,
};

const GREEN_LED_PIN: u8 = 22;
const YELLOW_LED_PIN: u8 = 21;
const RED_LED_PIN: u8 = 20;
const GREEN_LED_MASK: u32 = 1 << 0;
const YELLOW_LED_MASK: u32 = 1 << 1;
const RED_LED_MASK: u32 = 1 << 2;
const ROOT_PREOPEN_FD: u8 = 3;
const TRAFFIC_STATE_FD: u8 = 4;
const ROOT_PREOPEN_NAME: &[u8] = b"/";
const FD_WRITE_RIGHT: u64 = 1 << 6;
const COLOR_STEP_MS: u64 = 40;
const YELLOW_BLINK_STEP_MS: u64 = 20;
const FD_WRITE_COUNT_PER_CYCLE: u32 = 7;
const POLL_COUNT_PER_CYCLE: u32 = 7;
const ERRNO_BADF: u16 = 8;
const CHOREOFS_DRIVER_STARTED: u32 = 0x5741_0010;
const CHOREOFS_GPIO_READY: u32 = 0x5741_0020;
const REPLY_SEND_SAMPLE_REPLIES: u32 = 5 + FD_WRITE_COUNT_PER_CYCLE + POLL_COUNT_PER_CYCLE;
const REPLY_ENCODE_SCRATCH_BYTES: usize = 128;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_COUNT: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_LAST_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_TOTAL_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_MAX_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_ENCODE_COUNT: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_ENCODE_LAST_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_ENCODE_TOTAL_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_REPLY_ENCODE_MAX_US: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut REPLY_SEND_FUTURE_READY_COUNT: u32 = 0;

#[derive(Clone, Copy)]
struct LedObject {
    pin: u8,
    mask: u32,
}

impl LedObject {
    const fn new(pin: u8, mask: u32) -> Self {
        Self { pin, mask }
    }
}

#[derive(Clone, Copy)]
enum TrafficState {
    Green,
    Yellow,
    Dark,
    Red,
}

impl TrafficState {
    const fn payload(self) -> &'static [u8] {
        match self {
            Self::Green => b"G",
            Self::Yellow => b"Y",
            Self::Dark => b"0",
            Self::Red => b"R",
        }
    }

    const fn led_mask(self) -> u32 {
        match self {
            Self::Green => GREEN_LED_MASK,
            Self::Yellow => YELLOW_LED_MASK,
            Self::Dark => 0,
            Self::Red => RED_LED_MASK,
        }
    }
}

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_CHOREOFS_TRAFFIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-traffic-cycle.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_CHOREOFS_TRAFFIC: &[u8] = &[];

const TRAFFIC_STATE: choreofs::ChoreoFsObject = choreofs::ChoreoFsObject::writable(
    b"device/traffic/state",
    choreofs::ObjectId(1),
    choreofs::FdSpec::new(TRAFFIC_STATE_FD as u32, FD_WRITE_RIGHT, 1),
    FdBinding::write(FdWriteRow::Object),
);
static OBJECT_FACTS: choreofs::ChoreoFsObjectSet<1> =
    choreofs::ChoreoFsObjectSet::new([TRAFFIC_STATE]);
static LED_OBJECTS: [LedObject; 3] = [
    LedObject::new(GREEN_LED_PIN, GREEN_LED_MASK),
    LedObject::new(YELLOW_LED_PIN, YELLOW_LED_MASK),
    LedObject::new(RED_LED_PIN, RED_LED_MASK),
];

struct MeasuredReplySend<F> {
    future: F,
}

impl<F> MeasuredReplySend<F> {
    const fn new(future: F) -> Self {
        Self { future }
    }
}

impl<F> Future for MeasuredReplySend<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = unsafe { self.get_unchecked_mut() };
        let sample = reply_send_future_sample_active();
        let poll_start = if sample {
            baker_firmware::choreofs_measurement_start()
        } else {
            0
        };
        let result = unsafe { Pin::new_unchecked(&mut state.future) }.poll(cx);
        if sample {
            record_reply_send_future_poll_elapsed(poll_start);
            if matches!(result, Poll::Ready(_)) {
                record_reply_send_future_ready();
            }
        }
        result
    }
}

fn measured_reply_send<F>(future: F) -> MeasuredReplySend<F>
where
    F: Future,
{
    MeasuredReplySend::new(future)
}

fn measure_reply_payload_encode<P>(payload: &P) -> Result<(), ChoreoFsTrafficLoopError>
where
    P: WireEncode,
{
    if !reply_send_future_sample_active() {
        return Ok(());
    }
    let mut scratch = [0u8; REPLY_ENCODE_SCRATCH_BYTES];
    let encode_start = baker_firmware::choreofs_measurement_start();
    let encoded_len = payload
        .encode_into(&mut scratch)
        .map_err(|_| ChoreoFsTrafficLoopError::RuntimeViolation)?;
    record_reply_payload_encode_elapsed(encode_start);
    core::hint::black_box(encoded_len);
    core::hint::black_box(&scratch[..encoded_len]);
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn reply_send_future_sample_active() -> bool {
    unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(REPLY_SEND_FUTURE_READY_COUNT))
            < REPLY_SEND_SAMPLE_REPLIES
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn reply_send_future_sample_active() -> bool {
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_reply_send_future_ready() {
    unsafe {
        let next = core::ptr::read_volatile(core::ptr::addr_of!(REPLY_SEND_FUTURE_READY_COUNT))
            .saturating_add(1);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(REPLY_SEND_FUTURE_READY_COUNT), next);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn record_reply_send_future_ready() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_reply_payload_encode_elapsed(start_us: u32) {
    let elapsed = baker_firmware::choreofs_measurement_start().wrapping_sub(start_us);
    unsafe {
        let count =
            core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_CHOREOFS_REPLY_ENCODE_COUNT))
                .saturating_add(1);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_ENCODE_COUNT),
            count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_ENCODE_LAST_US),
            elapsed,
        );
        let total =
            core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_CHOREOFS_REPLY_ENCODE_TOTAL_US))
                .saturating_add(elapsed);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_ENCODE_TOTAL_US),
            total,
        );
        let max =
            core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_CHOREOFS_REPLY_ENCODE_MAX_US));
        if elapsed > max {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_ENCODE_MAX_US),
                elapsed,
            );
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn record_reply_payload_encode_elapsed(start_us: u32) {
    core::hint::black_box(start_us);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_reply_send_future_poll_elapsed(start_us: u32) {
    let elapsed = baker_firmware::choreofs_measurement_start().wrapping_sub(start_us);
    unsafe {
        let count = core::ptr::read_volatile(core::ptr::addr_of!(
            HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_COUNT
        ))
        .saturating_add(1);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_COUNT),
            count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_LAST_US),
            elapsed,
        );
        let total = core::ptr::read_volatile(core::ptr::addr_of!(
            HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_TOTAL_US
        ))
        .saturating_add(elapsed);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_TOTAL_US),
            total,
        );
        let max = core::ptr::read_volatile(core::ptr::addr_of!(
            HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_MAX_US
        ));
        if elapsed > max {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_REPLY_SEND_FUTURE_POLL_MAX_US),
                elapsed,
            );
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn record_reply_send_future_poll_elapsed(start_us: u32) {
    core::hint::black_box(start_us);
}

struct ChoreoFsTrafficLoop;
struct ChoreoFsTrafficLoopLocal;

enum ChoreoFsTrafficLoopError {
    Endpoint(hibana::EndpointError),
    RuntimeViolation,
}

impl core::fmt::Debug for ChoreoFsTrafficLoopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Endpoint(error) => f.debug_tuple("Endpoint").field(error).finish(),
            Self::RuntimeViolation => f.write_str("RuntimeViolation"),
        }
    }
}

impl From<hibana::EndpointError> for ChoreoFsTrafficLoopError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl appkit::Capsule for ChoreoFsTrafficLoop {
    type Placement = BakerPlacement;
    type Localside = ChoreoFsTrafficLoopLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        let fd_prestat_get = || {
            g::seq(
                g::send::<1, 0, FdPrestatGetReqMsg>(),
                g::send::<0, 1, FdPrestatGetRetMsg>(),
            )
        };
        let fd_prestat_dir_name = || {
            g::seq(
                g::send::<1, 0, FdPrestatDirNameReqMsg>(),
                g::send::<0, 1, FdPrestatDirNameRetMsg>(),
            )
        };
        let fd_fdstat_get = || {
            g::seq(
                g::send::<1, 0, FdFdstatGetReqMsg>(),
                g::send::<0, 1, FdFdstatGetRetMsg>(),
            )
        };
        let path_open = || {
            g::seq(
                g::send::<1, 0, PathOpenReqMsg>(),
                g::send::<0, 1, PathOpenRetMsg>(),
            )
        };
        let fd_write_object = || {
            g::seq(
                g::send::<1, 0, FdWriteObjectReqMsg>(),
                g::send::<0, 1, FdWriteObjectRetMsg>(),
            )
        };
        let poll_oneoff = || {
            g::seq(
                g::send::<1, 0, PollOneoffReqMsg>(),
                g::send::<0, 1, PollOneoffRetMsg>(),
            )
        };
        let startup = g::seq(
            fd_prestat_get(),
            g::seq(
                fd_prestat_dir_name(),
                g::seq(fd_prestat_get(), g::seq(fd_fdstat_get(), path_open())),
            ),
        );
        let traffic_step = g::seq(fd_write_object(), poll_oneoff()).roll();

        g::seq(startup, traffic_step)
    }
}

impl BakerCapsuleFacts for ChoreoFsTrafficLoop {
    fn run_engine_image() {
        baker_firmware::run_engine_wasi::<Self>(appkit::WasiImage::from_bytes(
            WASM_CHOREOFS_TRAFFIC,
        ));
    }

    fn choreofs() -> choreofs::ChoreoFs<'static> {
        OBJECT_FACTS.choreofs()
    }
}

impl appkit::Localside<ChoreoFsTrafficLoop> for ChoreoFsTrafficLoopLocal {
    type Error = ChoreoFsTrafficLoopError;

    fn engine<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn driver<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            let choreofs = OBJECT_FACTS.choreofs();
            if ROLE == 0 && !choreofs.facts().entries().is_empty() {
                baker_firmware::reset_choreofs_markers();
                baker_firmware::record_choreofs_engine_status(CHOREOFS_DRIVER_STARTED);
                init_led_outputs();
                baker_firmware::record_choreofs_engine_status(CHOREOFS_GPIO_READY);

                let mut completed_cycles = 0u32;
                drive_wasi_startup(&mut ctx, choreofs).await?;
                loop {
                    drive_traffic_cycle(&mut ctx, choreofs).await?;
                    completed_cycles = completed_cycles.saturating_add(1);
                    if completed_cycles == 1 {
                        baker_firmware::assert_choreofs_markers(
                            1,
                            FD_WRITE_COUNT_PER_CYCLE,
                            POLL_COUNT_PER_CYCLE,
                            RED_LED_MASK,
                            GREEN_LED_MASK | YELLOW_LED_MASK | RED_LED_MASK,
                        );
                        baker_firmware::freeze_choreofs_measurements();
                        baker_firmware::mark_runtime_ready();
                        baker_firmware::mark_success(
                            <ChoreoFsTrafficLoop as BakerCapsuleFacts>::SUCCESS_RESULT,
                        );
                        completed_cycles = completed_cycles.saturating_add(1);
                    }
                }
            }
            appkit::pending(ctx).await
        }
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }
}

fn enter_import(label: u8) {
    baker_firmware::record_choreofs_driver_import_enter();
    baker_firmware::record_choreofs_driver_trace(0x5754_0000 | u32::from(label));
}

fn exit_import() {
    baker_firmware::record_choreofs_driver_import_exit();
}

async fn drive_wasi_startup<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    handle_fd_prestat_get(ctx).await?;
    handle_fd_prestat_dir_name(ctx).await?;
    handle_fd_prestat_get(ctx).await?;
    handle_fd_fdstat_get(ctx, choreofs).await?;
    let request = recv_path_open(ctx).await?;
    handle_path_open(
        ctx,
        choreofs,
        request,
        TRAFFIC_STATE_FD,
        choreofs::ObjectId(1),
    )
    .await?;
    Ok(())
}

async fn drive_traffic_cycle<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    handle_next_state(ctx, choreofs, TrafficState::Green, COLOR_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Yellow, COLOR_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Dark, YELLOW_BLINK_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Yellow, YELLOW_BLINK_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Dark, YELLOW_BLINK_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Yellow, YELLOW_BLINK_STEP_MS).await?;
    handle_next_state(ctx, choreofs, TrafficState::Red, COLOR_STEP_MS).await?;
    Ok(())
}

async fn handle_next_state<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
    state: TrafficState,
    timeout_ms: u64,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_FD_WRITE_OBJECT);
    let request = observe_request_recv(recv_start, ctx.recv::<FdWriteObjectReqMsg>().await)?.0;
    handle_fd_write(ctx, choreofs, request, state).await?;

    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_POLL_ONEOFF);
    let request = observe_request_recv(recv_start, ctx.recv::<PollOneoffReqMsg>().await)?.0;
    handle_poll_oneoff(ctx, request, timeout_ms).await?;
    Ok(())
}

async fn handle_fd_prestat_get<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_FD_PRESTAT_GET);
    let request = observe_request_recv(recv_start, ctx.recv::<FdPrestatGetReqMsg>().await)?.0;
    let response = if request.fd() == ROOT_PREOPEN_FD {
        FdPrestat::new(request.fd(), ROOT_PREOPEN_NAME.len() as u8)
    } else {
        FdPrestat::new_with_errno(request.fd(), 0, ERRNO_BADF)
    };
    let response = hibana_wasip1_runtime::protocol::FdPrestatRet(response);
    measure_reply_payload_encode(&response)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_FD_PRESTAT_GET,
        send_start,
        measured_reply_send(ctx.send::<FdPrestatGetRetMsg>(&response)).await,
    )?;
    exit_import();
    Ok(())
}

async fn handle_fd_prestat_dir_name<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_FD_PRESTAT_DIR_NAME);
    let request = observe_request_recv(recv_start, ctx.recv::<FdPrestatDirNameReqMsg>().await)?.0;
    let response = if request.fd() == ROOT_PREOPEN_FD {
        FdPrestatDirNameDone::new(request.fd(), ROOT_PREOPEN_NAME, 0)
    } else {
        FdPrestatDirNameDone::new(request.fd(), b"", ERRNO_BADF)
    }
    .map_err(|_| ChoreoFsTrafficLoopError::RuntimeViolation)?;
    let response = hibana_wasip1_runtime::protocol::FdPrestatDirNameRet(response);
    measure_reply_payload_encode(&response)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_FD_PRESTAT_DIR_NAME,
        send_start,
        measured_reply_send(ctx.send::<FdPrestatDirNameRetMsg>(&response)).await,
    )?;
    exit_import();
    Ok(())
}

async fn handle_fd_fdstat_get<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_FD_FDSTAT_GET);
    let request = observe_request_recv(recv_start, ctx.recv::<FdFdstatGetReqMsg>().await)?.0;
    let response = fd_stat_response(choreofs, request)?;
    measure_reply_payload_encode(&response)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_FD_FDSTAT_GET,
        send_start,
        measured_reply_send(ctx.send::<FdFdstatGetRetMsg>(&response)).await,
    )?;
    exit_import();
    Ok(())
}

async fn recv_path_open<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<PathOpen, ChoreoFsTrafficLoopError> {
    let recv_start = baker_firmware::choreofs_measurement_start();
    enter_import(LABEL_WASI_PATH_OPEN);
    observe_request_recv(recv_start, ctx.recv::<PathOpenReqMsg>().await).map(|request| request.0)
}

fn observe_request_recv<T, E>(
    start_us: u32,
    result: Result<T, E>,
) -> Result<T, ChoreoFsTrafficLoopError>
where
    ChoreoFsTrafficLoopError: From<E>,
{
    baker_firmware::record_choreofs_request_recv_elapsed(start_us);
    result.map_err(Into::into)
}

fn observe_reply_send<E>(
    label: u8,
    start_us: u32,
    result: Result<(), E>,
) -> Result<(), ChoreoFsTrafficLoopError>
where
    ChoreoFsTrafficLoopError: From<E>,
{
    baker_firmware::record_choreofs_reply_send_elapsed(start_us, label);
    result.map_err(Into::into)
}

fn fd_stat_response(
    choreofs: choreofs::ChoreoFs<'static>,
    request: hibana_wasip1_runtime::protocol::FdRequest,
) -> Result<FdStatRet, ChoreoFsTrafficLoopError> {
    match request.fd() {
        0 | ROOT_PREOPEN_FD => Ok(FdStatRet(FdStat::new(request.fd(), MemRights::Read))),
        1 | 2 => Ok(FdStatRet(FdStat::new(request.fd(), MemRights::Write))),
        TRAFFIC_STATE_FD => Ok(choreofs.fd_fdstat_get(request)),
        other => {
            baker_firmware::record_choreofs_driver_trace(0x5754_5000 | u32::from(other));
            Err(ChoreoFsTrafficLoopError::RuntimeViolation)
        }
    }
}

fn normalize_path(path: &[u8]) -> &[u8] {
    match path {
        [b'/', rest @ ..] => rest,
        _ => path,
    }
}

async fn handle_path_open<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
    request: PathOpen,
    expected_fd: u8,
    expected_object: choreofs::ObjectId,
) -> Result<(), ChoreoFsTrafficLoopError> {
    if request.preopen_fd() != ROOT_PREOPEN_FD {
        baker_firmware::record_choreofs_driver_trace(
            0x5754_6000
                | (u32::from(request.preopen_fd()) << 8)
                | (request.rights_base() as u32 & 0xff),
        );
        core::hint::black_box(request);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let normalized = PathOpen::new(
        request.preopen_fd(),
        request.rights_base(),
        normalize_path(request.path()),
    )
    .map_err(|_| ChoreoFsTrafficLoopError::RuntimeViolation)?;
    let open = choreofs.path_open(normalized);
    if open.fd() != Some(expected_fd) || open.object() != Some(expected_object) {
        if let Some(fd) = open.fd() {
            baker_firmware::record_choreofs_driver_trace(0x5754_6100 | u32::from(fd));
        } else {
            baker_firmware::record_choreofs_driver_trace(0x5754_61ff);
        }
        core::hint::black_box(open);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    baker_firmware::record_choreofs_path_open(expected_object);
    let opened = open.opened_ret();
    measure_reply_payload_encode(&opened)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_PATH_OPEN,
        send_start,
        measured_reply_send(ctx.send::<PathOpenRetMsg>(&opened)).await,
    )?;
    exit_import();
    Ok(())
}

async fn handle_fd_write<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    choreofs: choreofs::ChoreoFs<'static>,
    request: FdWrite,
    expected_state: TrafficState,
) -> Result<(), ChoreoFsTrafficLoopError> {
    if request.fd() != TRAFFIC_STATE_FD || request.as_bytes() != expected_state.payload() {
        baker_firmware::record_choreofs_driver_trace(0x5754_7000 | u32::from(request.fd()));
        core::hint::black_box(request);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let write = choreofs.fd_write(request);
    if !write.is_writable() {
        baker_firmware::record_choreofs_driver_trace(0x5754_7100 | u32::from(request.fd()));
        core::hint::black_box(write);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let object = write
        .object()
        .ok_or(ChoreoFsTrafficLoopError::RuntimeViolation)?;
    if object != choreofs::ObjectId(1) {
        core::hint::black_box(object);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    write_traffic_state(expected_state);
    baker_firmware::record_choreofs_fd_write(object);
    let written = write.written();
    measure_reply_payload_encode(&written)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_FD_WRITE_OBJECT,
        send_start,
        measured_reply_send(ctx.send::<FdWriteObjectRetMsg>(&written)).await,
    )?;
    exit_import();
    Ok(())
}

async fn handle_poll_oneoff<const ROLE: u8>(
    ctx: &mut hibana::Endpoint<'_, ROLE>,
    request: hibana_wasip1_runtime::protocol::PollOneoff,
    expected_timeout_ms: u64,
) -> Result<(), ChoreoFsTrafficLoopError> {
    baker_firmware::record_choreofs_poll_timeout(request.timeout_tick());
    if request.timeout_tick() != expected_timeout_ms {
        #[cfg(feature = "wasm-engine-core")]
        baker_firmware::record_choreofs_engine_error_code(0x5745_d000);
        baker_firmware::record_choreofs_driver_trace(0x5754_8000 | request.timeout_tick() as u32);
        core::hint::black_box(request);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    baker_firmware::baker_poll_delay(request.timeout_tick());
    baker_firmware::record_choreofs_poll();
    let ready = PollReadyRet(PollReady::new(1));
    measure_reply_payload_encode(&ready)?;
    let send_start = baker_firmware::choreofs_measurement_start();
    observe_reply_send(
        LABEL_WASI_POLL_ONEOFF,
        send_start,
        measured_reply_send(ctx.send::<PollOneoffRetMsg>(&ready)).await,
    )?;
    exit_import();
    Ok(())
}

fn init_led_outputs() {
    let mut index = 0usize;
    while index < LED_OBJECTS.len() {
        baker_firmware::baker_gpio_init_output(LED_OBJECTS[index].pin);
        index += 1usize;
    }
}

fn write_traffic_state(state: TrafficState) {
    let state_mask = state.led_mask();
    let mut index = 0usize;
    while index < LED_OBJECTS.len() {
        let led = LED_OBJECTS[index];
        let high = state_mask & led.mask != 0;
        baker_firmware::baker_gpio_write(led.pin, high);
        baker_firmware::record_choreofs_led_mask(led.mask, high);
        index += 1usize;
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn baker_selected_run() -> ! {
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}
