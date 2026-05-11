use core::{cell::UnsafeCell, mem::MaybeUninit};

use hibana::{
    Endpoint,
    substrate::{SessionKit, runtime::CounterClock, tap::TapEvent},
};
use hibana_pico::{
    choreography::protocol::EngineLabelUniverse, machine::rp2040::sio::Rp2040SioBackend,
    port::transport::SioTransport,
};

#[cfg(feature = "baker-abort-safe-demo")]
pub(super) const SLAB_BYTES: usize = 200 * 1024;
#[cfg(not(feature = "baker-abort-safe-demo"))]
pub(super) const SLAB_BYTES: usize = 122 * 1024;

pub(super) const TEST_MEMORY_LEN: u32 = 64 * 1024;
pub(super) const TEST_MEMORY_EPOCH: u32 = 1;
pub(super) const TEST_LED_PTR: u32 = 128;
#[cfg(any(
    not(feature = "baker-abort-safe-demo"),
    feature = "baker-recoverable-abort-demo"
))]
pub(super) const BAKER_LINK_WASM_FUEL_PER_ACTIVATION: u32 = 250_000;

pub(super) type DemoTransport = SioTransport<Rp2040SioBackend>;
pub(super) type DemoKit = SessionKit<'static, DemoTransport, EngineLabelUniverse, CounterClock, 4>;
pub(super) type KernelEndpoint = Endpoint<'static, 0>;
pub(super) type EngineEndpoint = Endpoint<'static, 1>;
pub(super) type GpioEndpoint = Endpoint<'static, 2>;
#[cfg(not(feature = "baker-abort-safe-demo"))]
pub(super) type TimerEndpoint = Endpoint<'static, 3>;

#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
pub(super) type BakerLedger = hibana_pico::kernel::guest_ledger::GuestLedger<4, 1, 1>;
#[cfg(all(
    not(feature = "baker-abort-safe-demo"),
    not(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))
))]
pub(super) type BakerLedger = hibana_pico::kernel::guest_ledger::GuestLedger<3, 1, 1>;

pub(super) struct SharedRuntime {
    pub(super) clock: CounterClock,
    pub(super) tap: [TapEvent; 128],
    pub(super) slab: [u8; SLAB_BYTES],
    pub(super) session: MaybeUninit<DemoKit>,
    pub(super) core0_endpoint: MaybeUninit<KernelEndpoint>,
    pub(super) core1_endpoint: MaybeUninit<EngineEndpoint>,
    pub(super) core2_endpoint: MaybeUninit<GpioEndpoint>,
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    pub(super) core3_endpoint: MaybeUninit<TimerEndpoint>,
    #[cfg(not(feature = "baker-abort-safe-demo"))]
    pub(super) core1_guest: MaybeUninit<hibana_pico::kernel::engine::wasm::Guest<'static>>,
}

impl SharedRuntime {
    const fn new() -> Self {
        Self {
            clock: CounterClock::new(),
            tap: [TapEvent::zero(); 128],
            slab: [0; SLAB_BYTES],
            session: MaybeUninit::uninit(),
            core0_endpoint: MaybeUninit::uninit(),
            core1_endpoint: MaybeUninit::uninit(),
            core2_endpoint: MaybeUninit::uninit(),
            #[cfg(not(feature = "baker-abort-safe-demo"))]
            core3_endpoint: MaybeUninit::uninit(),
            #[cfg(not(feature = "baker-abort-safe-demo"))]
            core1_guest: MaybeUninit::uninit(),
        }
    }
}

struct SharedRuntimeCell(UnsafeCell<SharedRuntime>);

unsafe impl Sync for SharedRuntimeCell {}

static SHARED_RUNTIME: SharedRuntimeCell = SharedRuntimeCell(UnsafeCell::new(SharedRuntime::new()));

pub(super) fn shared_runtime_ptr() -> *mut SharedRuntime {
    SHARED_RUNTIME.0.get()
}

pub(super) unsafe fn shared_kernel_endpoint() -> &'static mut KernelEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core0_endpoint.as_mut_ptr() }
}

pub(super) unsafe fn shared_engine_endpoint() -> &'static mut EngineEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core1_endpoint.as_mut_ptr() }
}

pub(super) unsafe fn shared_gpio_endpoint() -> &'static mut GpioEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core2_endpoint.as_mut_ptr() }
}

#[cfg(not(feature = "baker-abort-safe-demo"))]
pub(super) unsafe fn shared_timer_endpoint() -> &'static mut TimerEndpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core3_endpoint.as_mut_ptr() }
}
