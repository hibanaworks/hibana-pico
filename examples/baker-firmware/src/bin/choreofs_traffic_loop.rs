#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana_pico::appkit;
use hibana_wasip1_runtime::protocol::{
    EngineReq, EngineRet, FdWrite, FdWriteDone, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET,
    LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF,
    LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, PathOpen, PathOpened, PollReady,
};

const GREEN_LED_PIN: u8 = 22;
const YELLOW_LED_PIN: u8 = 21;
const RED_LED_PIN: u8 = 20;
const GREEN_LED_MASK: u32 = 1 << 0;
const YELLOW_LED_MASK: u32 = 1 << 1;
const RED_LED_MASK: u32 = 1 << 2;
const LED_PREOPEN_FD: u8 = 9;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const EXPECTED_POLL_TIMEOUT_MS: u64 = 80;
const CHOREOFS_DRIVER_STARTED: u32 = 0x5741_0010;
const CHOREOFS_GPIO_READY: u32 = 0x5741_0020;

#[derive(Clone, Copy)]
struct LedObject {
    object: appkit::ObjectId,
    pin: u8,
    mask: u32,
}

impl LedObject {
    const fn new(object: appkit::ObjectId, pin: u8, mask: u32) -> Self {
        Self { object, pin, mask }
    }
}

#[derive(Clone, Copy)]
struct PathOpenStep {
    fd: u8,
    object: appkit::ObjectId,
}

impl PathOpenStep {
    const fn new(fd: u8, object: appkit::ObjectId) -> Self {
        Self { fd, object }
    }
}

#[derive(Clone, Copy)]
struct FdWriteStep {
    fd: u8,
    payload: &'static [u8],
}

impl FdWriteStep {
    const fn new(fd: u8, payload: &'static [u8]) -> Self {
        Self { fd, payload }
    }
}

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_CHOREOFS_TRAFFIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-traffic-cycle.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_CHOREOFS_TRAFFIC: &[u8] = &[];

const GREEN_LED: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/led/green",
    appkit::ObjectId(1),
    appkit::FdSpec::new(3, FD_WRITE_RIGHT, 1),
);
const YELLOW_LED: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/led/yellow",
    appkit::ObjectId(2),
    appkit::FdSpec::new(4, FD_WRITE_RIGHT, 1),
);
const RED_LED: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/led/red",
    appkit::ObjectId(3),
    appkit::FdSpec::new(5, FD_WRITE_RIGHT, 1),
);
static OBJECT_FACTS: appkit::ChoreoFsObjectSet<3> =
    appkit::ChoreoFsObjectSet::new([GREEN_LED, YELLOW_LED, RED_LED]);
static LED_OBJECTS: [LedObject; 3] = [
    LedObject::new(appkit::ObjectId(1), GREEN_LED_PIN, GREEN_LED_MASK),
    LedObject::new(appkit::ObjectId(2), YELLOW_LED_PIN, YELLOW_LED_MASK),
    LedObject::new(appkit::ObjectId(3), RED_LED_PIN, RED_LED_MASK),
];
static PATH_OPEN_STEPS: [PathOpenStep; 3] = [
    PathOpenStep::new(3, appkit::ObjectId(1)),
    PathOpenStep::new(4, appkit::ObjectId(2)),
    PathOpenStep::new(5, appkit::ObjectId(3)),
];
static FD_WRITE_CYCLE: [FdWriteStep; 13] = [
    FdWriteStep::new(3, b"1"),
    FdWriteStep::new(4, b"0"),
    FdWriteStep::new(5, b"0"),
    FdWriteStep::new(3, b"0"),
    FdWriteStep::new(4, b"1"),
    FdWriteStep::new(5, b"0"),
    FdWriteStep::new(4, b"0"),
    FdWriteStep::new(4, b"1"),
    FdWriteStep::new(4, b"0"),
    FdWriteStep::new(4, b"1"),
    FdWriteStep::new(3, b"0"),
    FdWriteStep::new(4, b"0"),
    FdWriteStep::new(5, b"1"),
];

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
    type Local = ChoreoFsTrafficLoopLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        let path_open = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>(),
            )
        };
        let open_leds = || g::seq(path_open(), g::seq(path_open(), path_open()));
        let write_wait = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>(),
                g::seq(
                    g::send::<0, 1, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>(),
                    g::seq(
                        g::send::<1, 0, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>(),
                        g::send::<0, 1, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>(),
                    ),
                ),
            )
        };
        let cycle = g::route(
            write_wait(),
            g::send::<1, 0, g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>>(),
        )
        .roll();
        g::seq(open_leds(), cycle)
    }
}

impl BakerCapsuleFacts for ChoreoFsTrafficLoop {
    fn run_engine_image() {
        baker_firmware::run_engine_wasi::<Self>(appkit::WasiImage::from_static(
            WASM_CHOREOFS_TRAFFIC,
        ));
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        OBJECT_FACTS.driver_facts()
    }
}

impl appkit::Localside<ChoreoFsTrafficLoop> for ChoreoFsTrafficLoopLocal {
    type Error = ChoreoFsTrafficLoopError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 && !ctx.choreofs().entries().is_empty() {
                baker_firmware::reset_choreofs_markers();
                baker_firmware::record_choreofs_engine_status(CHOREOFS_DRIVER_STARTED);
                init_led_outputs();
                baker_firmware::record_choreofs_engine_status(CHOREOFS_GPIO_READY);

                let mut path_index = 0usize;
                while path_index < PATH_OPEN_STEPS.len() {
                    let step = PATH_OPEN_STEPS[path_index];
                    driver_path_open(&mut ctx, step.fd, step.object).await?;
                    path_index += 1usize;
                }

                let mut completed_cycles = 0u32;
                loop {
                    let mut index = 0usize;
                    while index < FD_WRITE_CYCLE.len() {
                        let step = FD_WRITE_CYCLE[index];
                        driver_fd_write(&mut ctx, step.fd, step.payload).await?;
                        driver_poll_oneoff(&mut ctx).await?;
                        index += 1usize;
                    }
                    completed_cycles = completed_cycles.saturating_add(1);
                    if completed_cycles == 1 {
                        baker_firmware::mark_runtime_ready();
                        baker_firmware::mark_success(
                            <ChoreoFsTrafficLoop as BakerCapsuleFacts>::SUCCESS_RESULT,
                        );
                    }
                }
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ChoreoFsTrafficLoop, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

async fn recv_engine_req<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
) -> Result<EngineReq, ChoreoFsTrafficLoopError> {
    Ok(ctx.endpoint().recv::<g::Msg<LABEL, EngineReq>>().await?)
}

async fn send_engine_ret<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
    reply: EngineRet,
) -> Result<(), ChoreoFsTrafficLoopError> {
    ctx.endpoint()
        .send::<g::Msg<LABEL, EngineRet>>(&reply)
        .await?;
    Ok(())
}

async fn driver_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
    expected_fd: u8,
    expected_object: appkit::ObjectId,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_PATH_OPEN>(ctx).await? {
        EngineReq::PathOpen(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    handle_path_open(ctx, request, expected_fd, expected_object).await
}

async fn handle_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
    request: PathOpen,
    expected_fd: u8,
    expected_object: appkit::ObjectId,
) -> Result<(), ChoreoFsTrafficLoopError> {
    if request.preopen_fd() != LED_PREOPEN_FD || request.rights_base() != FD_WRITE_RIGHT {
        core::hint::black_box(request);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let object = match ctx.choreofs().resolve(request.path()) {
        Some(object) => object,
        None => {
            core::hint::black_box(request);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    let fact = match find_ledger_fd(ctx.ledger(), object, request.rights_base()) {
        Some(fact) => fact,
        None => {
            core::hint::black_box((object, request.rights_base()));
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    if fact.fd() != expected_fd as u32 || fact.object() != expected_object {
        core::hint::black_box(fact);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    baker_firmware::record_choreofs_path_open(object);
    send_engine_ret::<ROLE, LABEL_WASI_PATH_OPEN_RET>(
        ctx,
        EngineRet::PathOpened(PathOpened::new(fact.fd() as u8, 0)),
    )
    .await
}

async fn driver_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
    expected_fd: u8,
    expected_payload: &[u8],
) -> Result<(), ChoreoFsTrafficLoopError> {
    let branch = ctx.endpoint().offer().await?;
    if branch.label() != LABEL_WASI_FD_WRITE {
        core::hint::black_box(branch.label());
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let request = match branch
        .recv::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .await?
    {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    handle_fd_write(ctx, request, expected_fd, expected_payload).await
}

async fn handle_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
    request: FdWrite,
    expected_fd: u8,
    expected_payload: &[u8],
) -> Result<(), ChoreoFsTrafficLoopError> {
    if request.fd() != expected_fd || request.as_bytes() != expected_payload {
        core::hint::black_box((request, expected_fd));
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    let fact = match ctx.ledger().fd(request.fd() as u32) {
        Some(fact) if fact.rights() == FD_WRITE_RIGHT => fact,
        Some(fact) => {
            core::hint::black_box(fact);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
        None => {
            core::hint::black_box(request);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    let high = match request.as_bytes() {
        b"1" => true,
        b"0" => false,
        other => {
            core::hint::black_box(other);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    write_led_object(fact.object(), high)?;
    baker_firmware::record_choreofs_fd_write(fact.object());
    send_engine_ret::<ROLE, LABEL_WASI_FD_WRITE_RET>(
        ctx,
        EngineRet::FdWriteDone(FdWriteDone::new(request.fd(), request.len() as u8)),
    )
    .await
}

async fn driver_poll_oneoff<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, ChoreoFsTrafficLoop, ROLE>,
) -> Result<(), ChoreoFsTrafficLoopError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_POLL_ONEOFF>(ctx).await? {
        EngineReq::PollOneoff(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    baker_firmware::record_choreofs_poll_timeout(request.timeout_tick());
    if request.timeout_tick() != EXPECTED_POLL_TIMEOUT_MS {
        #[cfg(feature = "wasm-engine-core")]
        baker_firmware::record_choreofs_engine_error_code(0x5745_d000);
        core::hint::black_box(request);
        return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
    }
    baker_firmware::baker_poll_delay(request.timeout_tick());
    baker_firmware::record_choreofs_poll();
    send_engine_ret::<ROLE, LABEL_WASI_POLL_ONEOFF_RET>(
        ctx,
        EngineRet::PollReady(PollReady::new(1)),
    )
    .await
}

fn find_ledger_fd(
    ledger: appkit::LedgerFacts<'_>,
    object: appkit::ObjectId,
    rights: u64,
) -> Option<appkit::LedgerFdFact> {
    let facts = ledger.fds();
    let mut index = 0usize;
    while index < facts.len() {
        let fact = facts[index];
        if fact.object() == object && fact.rights() == rights {
            return Some(fact);
        }
        index += 1usize;
    }
    None
}

fn init_led_outputs() {
    let mut index = 0usize;
    while index < LED_OBJECTS.len() {
        baker_firmware::baker_gpio_init_output(LED_OBJECTS[index].pin);
        index += 1usize;
    }
}

fn led_for_object(object: appkit::ObjectId) -> Option<LedObject> {
    let mut index = 0usize;
    while index < LED_OBJECTS.len() {
        let led = LED_OBJECTS[index];
        if led.object == object {
            return Some(led);
        }
        index += 1usize;
    }
    None
}

fn write_led_object(object: appkit::ObjectId, high: bool) -> Result<(), ChoreoFsTrafficLoopError> {
    let led = match led_for_object(object) {
        Some(led) => led,
        None => {
            core::hint::black_box(object);
            return Err(ChoreoFsTrafficLoopError::RuntimeViolation);
        }
    };
    baker_firmware::baker_gpio_write(led.pin, high);
    baker_firmware::record_choreofs_led_mask(led.mask, high);
    Ok(())
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
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<ChoreoFsTrafficLoop>()
}
