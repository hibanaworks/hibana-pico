#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
};
use core::{assert, assert_eq};
use hibana::{
    g,
    substrate::{
        cap::{
            GenericCapToken,
            advanced::{LoopBreakKind, LoopContinueKind},
        },
        program::Projectable,
    },
};
#[cfg(feature = "wasm-engine-core")]
use hibana_pico::choreography::protocol::BudgetRun;
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason, EngineReq, EngineRet, FdWrite, FdWriteDone,
        LABEL_MEM_FENCE, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
        LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET, MemFence,
        MemFenceReason, PathOpened, PollReady,
    },
    site,
};

pub struct BakerTraffic;
pub struct BakerChoreoFsTraffic;
pub struct BakerChoreoFsTrafficLoop;
pub struct BakerFailSafe;
pub struct BakerRecovery;
pub struct BakerManyReentry;
pub struct BakerPlacement;
pub struct BakerControlLocal;
pub struct BakerManyReentryLocal;
pub struct BakerChoreoFsLocal;
pub struct BakerArtifacts;

mod image {
    pub struct Driver;
    pub struct Engine;
}

type DriverImage = site::Local<image::Driver>;
type EngineImage = site::Local<image::Engine>;

mod rp2040_sio {
    use core::cell::Cell;

    use hibana_pico::appkit::CarrierKind;

    pub const SIO: CarrierKind = CarrierKind::new(2040);
    const SIO_FRAME_MAGIC: u32 = 0x4849_5301;
    const SIO_FRAME_BYTES: usize = 128;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SioTransport;

    impl SioTransport {
        pub const fn new() -> Self {
            Self
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct SioTx {
        local_role: u8,
        session_id: u32,
        sent_frames: u16,
    }

    #[derive(Debug)]
    pub struct SioRx {
        local_role: u8,
        session_id: u32,
        requeued: bool,
        delivered: bool,
        frame_label: Option<hibana::substrate::transport::FrameLabel>,
        hint_frame_label: Cell<Option<hibana::substrate::transport::FrameLabel>>,
        len: usize,
        bytes: [u8; SIO_FRAME_BYTES],
    }

    impl SioRx {
        const fn new(local_role: u8, session_id: u32) -> Self {
            Self {
                local_role,
                session_id,
                requeued: false,
                delivered: false,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; SIO_FRAME_BYTES],
            }
        }
    }

    #[inline(always)]
    pub fn core_id() -> u32 {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            const SIO_CPUID: *const u32 = 0xd000_0000 as *const u32;
            unsafe { core::ptr::read_volatile(SIO_CPUID) & 1 }
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        {
            0
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    mod fifo {
        use core::ptr::{read_volatile, write_volatile};

        const SIO_BASE: usize = 0xd000_0000;
        const SIO_FIFO_ST: *const u32 = (SIO_BASE + 0x50) as *const u32;
        const SIO_FIFO_ST_WRITE: *mut u32 = (SIO_BASE + 0x50) as *mut u32;
        const SIO_FIFO_WR: *mut u32 = (SIO_BASE + 0x54) as *mut u32;
        const SIO_FIFO_RD: *const u32 = (SIO_BASE + 0x58) as *const u32;
        const FIFO_VLD: u32 = 1 << 0;
        const FIFO_RDY: u32 = 1 << 1;
        const FIFO_WOF: u32 = 1 << 2;
        const FIFO_ROE: u32 = 1 << 3;

        #[inline(always)]
        pub fn ready_to_recv() -> bool {
            unsafe { read_volatile(SIO_FIFO_ST) & FIFO_VLD != 0 }
        }

        #[inline(always)]
        pub fn clear_errors() {
            unsafe {
                write_volatile(SIO_FIFO_ST_WRITE, FIFO_WOF | FIFO_ROE);
            }
        }

        #[inline(always)]
        pub fn push_blocking(word: u32) {
            while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_RDY == 0 {
                core::hint::spin_loop();
            }
            unsafe {
                write_volatile(SIO_FIFO_WR, word);
            }
        }

        #[inline(always)]
        pub fn pop_blocking() -> u32 {
            while !ready_to_recv() {
                core::hint::spin_loop();
            }
            unsafe { read_volatile(SIO_FIFO_RD) }
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    mod fifo {
        #[inline(always)]
        pub fn ready_to_recv() -> bool {
            false
        }

        #[inline(always)]
        pub fn clear_errors() {}

        #[inline(always)]
        pub fn push_blocking(word: u32) {
            panic!("RP2040 SIO FIFO push is unavailable on this target: {word}");
        }

        #[inline(always)]
        pub fn pop_blocking() -> u32 {
            0
        }
    }

    fn encode_meta(
        sender_role: u8,
        peer_role: u8,
        frame_label: hibana::substrate::transport::FrameLabel,
        len: usize,
    ) -> u32 {
        ((frame_label.raw() as u32) << 24)
            | ((peer_role as u32) << 16)
            | ((sender_role as u32) << 8)
            | (len as u32)
    }

    fn decode_meta(word: u32) -> (u8, u8, hibana::substrate::transport::FrameLabel, usize) {
        let frame_label = hibana::substrate::transport::FrameLabel::new((word >> 24) as u8);
        let peer_role = ((word >> 16) & 0xff) as u8;
        let sender_role = ((word >> 8) & 0xff) as u8;
        let len = (word & 0xff) as usize;
        (sender_role, peer_role, frame_label, len)
    }

    fn pack_payload_word(bytes: &[u8], offset: usize) -> u32 {
        let mut word = 0u32;
        let mut idx = 0usize;
        while idx < 4 {
            let source = offset + idx;
            if source < bytes.len() {
                word |= (bytes[source] as u32) << (idx * 8);
            }
            idx += 1;
        }
        word
    }

    fn unpack_payload_word(word: u32, bytes: &mut [u8], offset: usize) {
        let mut idx = 0usize;
        while idx < 4 {
            let target = offset + idx;
            if target < bytes.len() {
                bytes[target] = ((word >> (idx * 8)) & 0xff) as u8;
            }
            idx += 1;
        }
    }

    impl hibana::substrate::Transport for SioTransport {
        type Error = hibana::substrate::transport::TransportError;
        type Tx<'a>
            = SioTx
        where
            Self: 'a;
        type Rx<'a>
            = SioRx
        where
            Self: 'a;
        type Metrics = ();

        fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
            fifo::clear_errors();
            (
                SioTx {
                    local_role,
                    session_id,
                    sent_frames: 0,
                },
                SioRx::new(local_role, session_id),
            )
        }

        fn poll_send<'a, 'f>(
            &'a self,
            tx: &'a mut Self::Tx<'a>,
            outgoing: hibana::substrate::transport::Outgoing<'f>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>>
        where
            'a: 'f,
        {
            let bytes = outgoing.payload().as_bytes();
            if bytes.len() > SIO_FRAME_BYTES {
                return core::task::Poll::Ready(Err(
                    hibana::substrate::transport::TransportError::Failed,
                ));
            }
            #[cfg(feature = "wasm-engine-core")]
            {
                let code = 0x5350_0000
                    | ((tx.local_role as u32) << 20)
                    | ((outgoing.peer() as u32) << 16)
                    | (((bytes.len() as u32) & 0xff) << 8)
                    | outgoing.frame_label().raw() as u32;
                super::record_choreofs_engine_error_code(code);
                super::record_choreofs_sio_trace(code);
            }
            fifo::push_blocking(SIO_FRAME_MAGIC);
            fifo::push_blocking(tx.session_id);
            fifo::push_blocking(encode_meta(
                tx.local_role,
                outgoing.peer(),
                outgoing.frame_label(),
                bytes.len(),
            ));
            let mut offset = 0usize;
            while offset < bytes.len() {
                fifo::push_blocking(pack_payload_word(bytes, offset));
                offset += 4;
            }
            tx.sent_frames = tx.sent_frames.saturating_add(1);
            cx.waker().wake_by_ref();
            core::task::Poll::Ready(Ok(()))
        }

        fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
            tx.sent_frames = 0;
        }

        fn poll_recv<'a>(
            &'a self,
            rx: &'a mut Self::Rx<'a>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<hibana::substrate::wire::Payload<'a>, Self::Error>> {
            if rx.frame_label.is_some() && (rx.requeued || !rx.delivered) {
                rx.requeued = false;
                rx.delivered = true;
                rx.hint_frame_label.set(None);
                #[cfg(feature = "wasm-engine-core")]
                {
                    let code = 0x5353_0000
                        | ((rx.local_role as u32) << 20)
                        | (((rx.len as u32) & 0xff) << 8)
                        | rx.frame_label.map(|label| label.raw() as u32).unwrap_or(0);
                    super::record_choreofs_sio_trace(code);
                }
                return core::task::Poll::Ready(Ok(hibana::substrate::wire::Payload::new(
                    &rx.bytes[..rx.len],
                )));
            }
            if rx.frame_label.is_some() {
                rx.frame_label = None;
                rx.hint_frame_label.set(None);
                rx.delivered = false;
                rx.len = 0;
            }
            if !fifo::ready_to_recv() {
                cx.waker().wake_by_ref();
                return core::task::Poll::Pending;
            }
            if fifo::pop_blocking() != SIO_FRAME_MAGIC {
                return core::task::Poll::Ready(Err(
                    hibana::substrate::transport::TransportError::Failed,
                ));
            }
            let session_id = fifo::pop_blocking();
            let (sender_role, peer_role, frame_label, len) = decode_meta(fifo::pop_blocking());
            if session_id != rx.session_id
                || peer_role != rx.local_role
                || sender_role == rx.local_role
                || len > SIO_FRAME_BYTES
            {
                return core::task::Poll::Ready(Err(
                    hibana::substrate::transport::TransportError::Failed,
                ));
            }
            rx.frame_label = Some(frame_label);
            rx.hint_frame_label.set(Some(frame_label));
            rx.len = len;
            rx.delivered = true;
            #[cfg(feature = "wasm-engine-core")]
            {
                let code = 0x5351_0000
                    | ((rx.local_role as u32) << 20)
                    | ((sender_role as u32) << 16)
                    | (((len as u32) & 0xff) << 8)
                    | frame_label.raw() as u32;
                super::record_choreofs_engine_error_code(code);
                super::record_choreofs_sio_trace(code);
            }
            let mut offset = 0usize;
            while offset < len {
                unpack_payload_word(fifo::pop_blocking(), &mut rx.bytes[..len], offset);
                offset += 4;
            }
            cx.waker().wake_by_ref();
            core::task::Poll::Ready(Ok(hibana::substrate::wire::Payload::new(
                &rx.bytes[..rx.len],
            )))
        }

        fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
            rx.requeued = rx.frame_label.is_some();
            if rx.requeued {
                rx.delivered = false;
            }
            #[cfg(feature = "wasm-engine-core")]
            {
                let code = 0x5352_0000
                    | ((rx.local_role as u32) << 20)
                    | (((rx.len as u32) & 0xff) << 8)
                    | rx.frame_label.map(|label| label.raw() as u32).unwrap_or(0);
                super::record_choreofs_sio_trace(code);
            }
        }

        fn drain_events(
            &self,
            _emit: &mut dyn FnMut(hibana::substrate::transport::advanced::TransportEvent),
        ) {
        }

        fn recv_frame_hint<'a>(
            &'a self,
            rx: &'a Self::Rx<'a>,
        ) -> Option<hibana::substrate::transport::FrameLabel> {
            let hint = rx.hint_frame_label.take();
            #[cfg(feature = "wasm-engine-core")]
            if let Some(frame_label) = hint {
                let code = 0x5354_0000
                    | ((rx.local_role as u32) << 20)
                    | (((rx.len as u32) & 0xff) << 8)
                    | frame_label.raw() as u32;
                super::record_choreofs_sio_trace(code);
            }
            hint
        }

        fn metrics(&self) -> Self::Metrics {}

        fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
            assert!(
                interval_us > 0 || burst_bytes == 0,
                "zero interval may only disable burst pacing"
            );
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
static BAKER_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_DRIVER_ATTACH_SLAB_BYTES: usize = 76 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_ENGINE_ATTACH_SLAB_BYTES: usize = 76 * 1024;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static BAKER_DRIVER_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<BAKER_DRIVER_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static BAKER_ENGINE_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<BAKER_ENGINE_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_driver_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    BAKER_DRIVER_ATTACH_STORAGE.lease()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_engine_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    BAKER_ENGINE_ATTACH_STORAGE.lease()
}

#[cfg(feature = "wasm-engine-core")]
fn baker_wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
    BAKER_WASI_GUEST_ARENA.storage()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(link_section = ".boot2")]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[cfg(all(target_arch = "arm", target_os = "none"))]
type Handler = unsafe extern "C" fn() -> !;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct VectorTable {
    initial_stack_pointer: *const u32,
    reset: Handler,
    exceptions: [Handler; 14],
    external_irqs: [Handler; 32],
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
global_asm!(
    r#"
    .global hard_fault_trampoline
    .type hard_fault_trampoline,%function
    .thumb_func
hard_fault_trampoline:
    mrs r0, msp
    ldr r1, 1f
    bx r1
    .align 2
1:
    .word hard_fault_handler_with_sp + 1
"#
);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    fn hard_fault_trampoline() -> !;
    fn baker_selected_run() -> !;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
    static __stack_limit: u32;
    static __data_load_start: u8;
    static mut __data_start: u8;
    static mut __data_end: u8;
    static mut __bss_start: u8;
    static mut __bss_end: u8;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(link_section = ".vector_table.reset_vector")]
static VECTOR_TABLE: VectorTable = VectorTable {
    initial_stack_pointer: core::ptr::addr_of!(__stack_top),
    reset: Reset,
    exceptions: [
        default_handler,
        hard_fault_trampoline,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
    ],
    external_irqs: [default_handler; 32],
};

const RESULT_SUCCESS: u32 = 0x4849_4f4b;
const RESULT_FAILURE: u32 = 0x4849_4641;
const RESULT_FAIL_SAFE_OK: u32 = 0x4849_4653;
const RESULT_RECOVERY_OK: u32 = 0x4849_5243;
const RESULT_MANY_REENTRY_OK: u32 = 0x4849_524d;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE0_START: u32 = 0x4849_0001;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_LAUNCHED: u32 = 0x4849_0002;
const STAGE_RUNTIME_BEGIN: u32 = 0x4849_0004;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_PROGRAM_READY: u32 = 0x4849_0006;
const STAGE_RUNTIME_READY: u32 = 0x4849_000a;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_ENGINE_RUNTIME_READY_SEEN: u32 = 0x4849_0033;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_HARD_PANIC: u32 = 0x4849_0f00;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_LAUNCH_ERR: u32 = 0x4849_0f01;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_START_TIMEOUT: u32 = 0x4849_0f02;
#[cfg(feature = "wasm-engine-core")]
const STAGE_WASI_ENGINE_ERROR: u32 = 0x4849_0f10;
const STAGE_CHOREOFS_DRIVER_ERROR: u32 = 0x4849_0f11;
const STAGE_CONTROL_FLOW_ERROR: u32 = 0x4849_0f12;

#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_STARTED: u32 = 0x5741_0001;
const CHOREOFS_DRIVER_STARTED: u32 = 0x5741_0010;
const CHOREOFS_GPIO_READY: u32 = 0x5741_0020;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_DRIVE_BEGIN: u32 = 0x5741_0201;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_PATH_OPEN_DONE: u32 = 0x5741_0202;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_LOOP_CONTINUE_BEGIN: u32 = 0x5741_0203;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_LOOP_CONTINUE_DONE: u32 = 0x5741_0204;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_CYCLE_DRIVE_BEGIN: u32 = 0x5741_0205;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_CYCLE_DRIVE_DONE: u32 = 0x5741_0206;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_DRIVE_DONE: u32 = 0x5741_0207;
#[cfg(feature = "wasm-engine-core")]
const CHOREOFS_ENGINE_OK: u32 = 0x5741_4f4b;
const CHOREOFS_ENGINE_ERROR: u32 = 0x5741_4641;
const CHOREOFS_LED_GREEN: u32 = 1 << 0;
const CHOREOFS_LED_ORANGE: u32 = 1 << 1;
const CHOREOFS_LED_RED: u32 = 1 << 2;

#[unsafe(no_mangle)]
static mut HIBANA_DEMO_RESULT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_FAILURE_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_PC: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_LR: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_ENGINE_STATUS: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_ENGINE_ERROR_CODE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_DRIVER_TRACE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE: [u32; 8] = [0; 8];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_PATH_OPEN_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_FD_WRITE_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_POLL_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LAST_OBJECT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LED_MASK: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SEEN_LED_MASK: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut CORE1_STARTED: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_BASE: usize = 0xd000_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST: *const u32 = (SIO_BASE + 0x50) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST_WRITE: *mut u32 = (SIO_BASE + 0x50) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_WR: *mut u32 = (SIO_BASE + 0x54) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_RD: *const u32 = (SIO_BASE + 0x58) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_VLD: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_RDY: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_WOF: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_ROE: u32 = 1 << 3;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_FRCE_OFF: *mut u32 = (0x4001_0000 + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_PROC1: u32 = 1 << 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CORE1_LAUNCH_RETRIES: u8 = 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const IO_BANK0_BASE: usize = 0x4001_4000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PADS_BANK0_BASE: usize = 0x4001_c000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_BASE: usize = 0x4000_c000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_CLR: *mut u32 = (RESETS_BASE + 0x3000) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_DONE: *const u32 = (RESETS_BASE + 0x08) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_IO_BANK0: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PADS_BANK0: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_SET: *mut u32 = (SIO_BASE + 0x14) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_CLR: *mut u32 = (SIO_BASE + 0x18) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OE_SET: *mut u32 = (SIO_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_SIO: u32 = 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_PAD_DEFAULT: u32 = 0x56;

const GREEN_LED_PIN: u8 = 22;
const ORANGE_LED_PIN: u8 = 21;
const RED_LED_PIN: u8 = 20;
const BAKER_LED_PREOPEN_FD: u8 = 9;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const CHOREOFS_REENTRY_CYCLES: u32 = 3;
const CHOREOFS_WRITES_PER_CYCLE: u32 = 3;
const CHOREOFS_EXPECTED_PATH_OPENS: u32 = 1;
const CHOREOFS_EXPECTED_FD_WRITES: u32 = CHOREOFS_REENTRY_CYCLES * CHOREOFS_WRITES_PER_CYCLE;
const CHOREOFS_EXPECTED_POLLS: u32 = CHOREOFS_EXPECTED_FD_WRITES;
const CHOREOFS_VISUAL_READY_CYCLES: u32 = 1;
#[cfg(feature = "wasm-engine-core")]
const BAKER_LINK_WASM_FUEL_PER_ACTIVATION: u32 = 250_000;
const LABEL_BAKER_CHOREOFS_ROUTE_CONTINUE: u8 = 120;
const LABEL_BAKER_CHOREOFS_ROUTE_BREAK: u8 = 121;

type BakerChoreoFsRouteContinue = g::Msg<
    { LABEL_BAKER_CHOREOFS_ROUTE_CONTINUE },
    GenericCapToken<LoopContinueKind>,
    LoopContinueKind,
>;
type BakerChoreoFsRouteBreak =
    g::Msg<{ LABEL_BAKER_CHOREOFS_ROUTE_BREAK }, GenericCapToken<LoopBreakKind>, LoopBreakKind>;

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_CHOREOFS_TRAFFIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-traffic-cycle.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_CHOREOFS_TRAFFIC: &[u8] = &[];

const TRAFFIC_DEVICE: appkit::ObjectSpec = appkit::ObjectSpec::new(
    b"device/traffic",
    appkit::ObjectId(1),
    appkit::FdSpec::new(3, FD_WRITE_RIGHT, 1),
);
static BAKER_OBJECT_FACTS: appkit::ObjectSpecSet<1> = appkit::ObjectSpecSet::new([TRAFFIC_DEVICE]);

trait BakerImageFacts {
    fn driver_facts() -> appkit::DriverFacts<'static> {
        appkit::DriverFacts::EMPTY
    }

    fn choreofs_visual_loop() -> bool {
        false
    }

    fn success_result() -> u32 {
        RESULT_SUCCESS
    }
}

impl BakerImageFacts for BakerTraffic {}
impl BakerImageFacts for BakerFailSafe {
    fn success_result() -> u32 {
        RESULT_FAIL_SAFE_OK
    }
}
impl BakerImageFacts for BakerRecovery {
    fn success_result() -> u32 {
        RESULT_RECOVERY_OK
    }
}
impl BakerImageFacts for BakerManyReentry {
    fn success_result() -> u32 {
        RESULT_MANY_REENTRY_OK
    }
}

impl BakerImageFacts for BakerChoreoFsTraffic {
    fn driver_facts() -> appkit::DriverFacts<'static> {
        BAKER_OBJECT_FACTS.driver_facts()
    }
}

impl BakerImageFacts for BakerChoreoFsTrafficLoop {
    fn driver_facts() -> appkit::DriverFacts<'static> {
        BAKER_OBJECT_FACTS.driver_facts()
    }

    fn choreofs_visual_loop() -> bool {
        true
    }
}

impl appkit::Capsule for BakerTraffic {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerControlLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                ),
            ),
        )
    }
}

impl appkit::Capsule for BakerChoreoFsTraffic {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerChoreoFsLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        let path_open = g::seq(
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(),
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 1>(),
        );
        let write_wait = || {
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(
                    ),
                    g::seq(
                        g::send::<
                            g::Role<1>,
                            g::Role<0>,
                            g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>,
                            1,
                        >(),
                        g::send::<
                            g::Role<0>,
                            g::Role<1>,
                            g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>,
                            1,
                        >(),
                    ),
                ),
            )
        };
        let admitted_cycle = || {
            g::route(
                g::seq(
                    g::send::<g::Role<1>, g::Role<1>, BakerChoreoFsRouteContinue, 1>(),
                    write_wait(),
                ),
                g::send::<g::Role<1>, g::Role<1>, BakerChoreoFsRouteBreak, 1>(),
            )
        };
        g::seq(path_open, admitted_cycle())
    }
}

impl appkit::Capsule for BakerChoreoFsTrafficLoop {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerChoreoFsLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        <BakerChoreoFsTraffic as appkit::Capsule>::choreography()
    }
}

impl appkit::Capsule for BakerFailSafe {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerControlLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                ),
            ),
        )
    }
}

impl appkit::Capsule for BakerRecovery {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerControlLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                ),
            ),
        )
    }
}

impl appkit::Capsule for BakerManyReentry {
    type Universe = appkit::BuiltInUniverse;
    type Placement = BakerPlacement;
    type Local = BakerManyReentryLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, EngineAbortFenceControl, 0>(),
                    g::seq(
                        g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 0>(),
                        g::seq(
                            g::send::<g::Role<1>, g::Role<0>, EngineAbortBeginControl, 1>(),
                            g::seq(
                                g::send::<g::Role<1>, g::Role<0>, EngineAbortMsg, 1>(),
                                g::seq(
                                    g::send::<
                                        g::Role<0>,
                                        g::Role<1>,
                                        g::Msg<LABEL_MEM_FENCE, MemFence>,
                                        1,
                                    >(),
                                    g::send::<g::Role<1>, g::Role<0>, EngineAbortAckControl, 1>(),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
    }
}

impl<C> appkit::Placement<C> for BakerPlacement
where
    C: appkit::Capsule<Placement = BakerPlacement>,
{
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            1 => appkit::RoleKind::Engine,
            0 => appkit::RoleKind::Driver,
            2 => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

impl<C> appkit::Localside<C> for BakerControlLocal
where
    C: appkit::Capsule<Local = BakerControlLocal> + BakerImageFacts,
{
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            if ROLE == 1 {
                return baker_control_engine_one_cycle(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            if ROLE == 0 {
                return baker_control_driver_one_cycle(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl<C> appkit::Localside<C> for BakerManyReentryLocal
where
    C: appkit::Capsule<Local = BakerManyReentryLocal> + BakerImageFacts,
{
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            if ROLE == 1 {
                return baker_many_reentry_engine(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            if ROLE == 0 {
                return baker_many_reentry_driver(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

async fn baker_control_engine_one_cycle<C, const ROLE: u8>(
    mut ctx: appkit::EngineCtx<'_, '_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerControlLocal> + BakerImageFacts,
{
    baker_engine_send_abort_begin(&mut ctx).await;
    baker_engine_send_abort_reason(&mut ctx, 1).await;
    baker_engine_recv_abort_fence(&mut ctx).await;
    baker_engine_send_abort_ack(&mut ctx).await;
    mark_stage(STAGE_RUNTIME_READY);
    core::future::pending::<core::convert::Infallible>().await
}

async fn baker_control_driver_one_cycle<C, const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerControlLocal> + BakerImageFacts,
{
    baker_driver_recv_abort_begin(&mut ctx).await;
    baker_driver_recv_abort_reason(&mut ctx).await;
    mark_safe_state();
    baker_driver_send_abort_fence(&mut ctx).await;
    baker_driver_recv_abort_ack(&mut ctx).await;
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(C::success_result());
    core::future::pending::<core::convert::Infallible>().await
}

async fn baker_many_reentry_engine<C, const ROLE: u8>(
    mut ctx: appkit::EngineCtx<'_, '_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerManyReentryLocal> + BakerImageFacts,
{
    baker_engine_send_abort_begin(&mut ctx).await;
    baker_engine_send_abort_reason(&mut ctx, 1).await;
    baker_engine_recv_abort_fence(&mut ctx).await;
    baker_engine_send_abort_ack(&mut ctx).await;
    baker_engine_send_abort_begin(&mut ctx).await;
    baker_engine_send_abort_reason(&mut ctx, 2).await;
    baker_engine_recv_mem_fence(&mut ctx).await;
    baker_engine_send_abort_ack(&mut ctx).await;
    mark_stage(STAGE_RUNTIME_READY);
    core::future::pending::<core::convert::Infallible>().await
}

async fn baker_many_reentry_driver<C, const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerManyReentryLocal> + BakerImageFacts,
{
    baker_driver_recv_abort_begin(&mut ctx).await;
    baker_driver_recv_abort_reason(&mut ctx).await;
    mark_safe_state();
    baker_driver_send_abort_fence(&mut ctx).await;
    baker_driver_recv_abort_ack(&mut ctx).await;
    baker_driver_recv_abort_begin(&mut ctx).await;
    baker_driver_recv_abort_reason(&mut ctx).await;
    mark_safe_state();
    baker_driver_send_mem_fence(&mut ctx).await;
    baker_driver_recv_abort_ack(&mut ctx).await;
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(C::success_result());
    core::future::pending::<core::convert::Infallible>().await
}

async fn baker_engine_send_abort_begin<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
) where
    C: appkit::Capsule,
{
    let flow = match ctx.endpoint().flow::<EngineAbortBeginControl>() {
        Ok(flow) => flow,
        Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    };
    if flow.send(()).await.is_err() {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_engine_send_abort_reason<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
    code: u16,
) where
    C: appkit::Capsule,
{
    let abort = EngineAbort::new(EngineAbortReason::FuelExhausted, code);
    let flow = match ctx.endpoint().flow::<EngineAbortMsg>() {
        Ok(flow) => flow,
        Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    };
    if flow.send(&abort).await.is_err() {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_engine_recv_abort_fence<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
) where
    C: appkit::Capsule,
{
    if ctx
        .endpoint()
        .recv::<EngineAbortFenceControl>()
        .await
        .is_err()
    {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_engine_recv_mem_fence<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
) where
    C: appkit::Capsule,
{
    if ctx
        .endpoint()
        .recv::<g::Msg<LABEL_MEM_FENCE, MemFence>>()
        .await
        .is_err()
    {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_engine_send_abort_ack<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
) where
    C: appkit::Capsule,
{
    let flow = match ctx.endpoint().flow::<EngineAbortAckControl>() {
        Ok(flow) => flow,
        Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    };
    if flow.send(()).await.is_err() {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_driver_recv_abort_begin<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule,
{
    if ctx
        .endpoint()
        .recv::<EngineAbortBeginControl>()
        .await
        .is_err()
    {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_driver_recv_abort_reason<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule,
{
    match ctx.endpoint().recv::<EngineAbortMsg>().await {
        Ok(abort) if abort.reason() == EngineAbortReason::FuelExhausted => {}
        Ok(_) | Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    }
}

async fn baker_driver_send_abort_fence<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule,
{
    let flow = match ctx.endpoint().flow::<EngineAbortFenceControl>() {
        Ok(flow) => flow,
        Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    };
    if flow.send(()).await.is_err() {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_driver_send_mem_fence<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule,
{
    let fence = MemFence::new(MemFenceReason::HotSwap, 2);
    let flow = match ctx.endpoint().flow::<g::Msg<LABEL_MEM_FENCE, MemFence>>() {
        Ok(flow) => flow,
        Err(_) => runtime_fail(STAGE_CONTROL_FLOW_ERROR),
    };
    if flow.send(&fence).await.is_err() {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

async fn baker_driver_recv_abort_ack<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule,
{
    if ctx
        .endpoint()
        .recv::<EngineAbortAckControl>()
        .await
        .is_err()
    {
        runtime_fail(STAGE_CONTROL_FLOW_ERROR);
    }
}

impl<C> appkit::Localside<C> for BakerChoreoFsLocal
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            #[cfg(feature = "wasm-engine-core")]
            {
                if ROLE == 1 && ctx.artifact_len() != 0 {
                    return baker_drive_wasi_engine(ctx).await;
                }
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            if ROLE == 0 && !ctx.choreofs().entries().is_empty() {
                return baker_choreofs_driver(ctx).await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

#[cfg(feature = "wasm-engine-core")]
async fn baker_drive_wasi_engine<C, const ROLE: u8>(
    mut ctx: appkit::EngineCtx<'_, '_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    record_choreofs_engine_status(CHOREOFS_ENGINE_STARTED);
    record_choreofs_engine_status(CHOREOFS_ENGINE_DRIVE_BEGIN);
    let status = ctx
        .drive_wasi_guest_imports(
            BudgetRun::new(1, 0, BAKER_LINK_WASM_FUEL_PER_ACTIVATION, 0),
            1,
        )
        .await;
    match status {
        Ok(appkit::WasiGuestStatus::ImportLimitReached(1)) => {}
        Ok(other) => {
            core::hint::black_box(other);
            runtime_fail(STAGE_WASI_ENGINE_ERROR);
        }
        Err(error) => {
            record_choreofs_engine_error_code(error.diagnostic_code());
            runtime_fail(STAGE_WASI_ENGINE_ERROR);
        }
    }
    record_choreofs_engine_status(CHOREOFS_ENGINE_PATH_OPEN_DONE);

    let visual_loop = C::choreofs_visual_loop();
    let mut cycle = 0u32;
    loop {
        let mut write_index = 0u32;
        while write_index < CHOREOFS_WRITES_PER_CYCLE {
            baker_engine_resolve_choreofs_loop(&mut ctx).await;
            record_choreofs_engine_status(CHOREOFS_ENGINE_CYCLE_DRIVE_BEGIN);
            let status = ctx
                .drive_wasi_guest_imports(
                    BudgetRun::new(1, 0, BAKER_LINK_WASM_FUEL_PER_ACTIVATION, 0),
                    2,
                )
                .await;
            match status {
                Ok(appkit::WasiGuestStatus::ImportLimitReached(2)) => {}
                Ok(other) => {
                    core::hint::black_box(other);
                    runtime_fail(STAGE_WASI_ENGINE_ERROR);
                }
                Err(error) => {
                    record_choreofs_engine_error_code(error.diagnostic_code());
                    runtime_fail(STAGE_WASI_ENGINE_ERROR);
                }
            }
            record_choreofs_engine_status(CHOREOFS_ENGINE_CYCLE_DRIVE_DONE);
            write_index += 1;
        }
        cycle += 1;
        if visual_loop && cycle == CHOREOFS_VISUAL_READY_CYCLES {
            record_choreofs_engine_status(CHOREOFS_ENGINE_OK);
            mark_stage(STAGE_RUNTIME_READY);
        }
        if !visual_loop && cycle >= CHOREOFS_REENTRY_CYCLES {
            break;
        }
    }

    record_choreofs_engine_status(CHOREOFS_ENGINE_DRIVE_DONE);
    record_choreofs_engine_status(CHOREOFS_ENGINE_OK);
    mark_stage(STAGE_RUNTIME_READY);
    core::future::pending::<core::convert::Infallible>().await
}

#[cfg(feature = "wasm-engine-core")]
async fn baker_engine_resolve_choreofs_loop<C, const ROLE: u8>(
    ctx: &mut appkit::EngineCtx<'_, '_, C, ROLE>,
) where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    record_choreofs_engine_status(CHOREOFS_ENGINE_LOOP_CONTINUE_BEGIN);
    let flow = match ctx.endpoint().flow::<BakerChoreoFsRouteContinue>() {
        Ok(flow) => flow,
        Err(error) => {
            core::hint::black_box(error);
            record_choreofs_engine_error_code(0x5745_6000);
            runtime_fail(STAGE_WASI_ENGINE_ERROR);
        }
    };
    if let Err(error) = flow.send(()).await {
        core::hint::black_box(error);
        record_choreofs_engine_error_code(0x5745_7000);
        runtime_fail(STAGE_WASI_ENGINE_ERROR);
    };
    record_choreofs_engine_status(CHOREOFS_ENGINE_LOOP_CONTINUE_DONE);
}

async fn baker_choreofs_driver<C, const ROLE: u8>(
    mut ctx: appkit::DriverCtx<'_, C, ROLE>,
) -> core::convert::Infallible
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    reset_choreofs_markers();
    record_choreofs_engine_status(CHOREOFS_DRIVER_STARTED);
    init_baker_led_outputs();
    record_choreofs_engine_status(CHOREOFS_GPIO_READY);

    baker_driver_path_open(&mut ctx).await;
    let expected = [
        (3u8, b"1".as_slice()),
        (3u8, b"2".as_slice()),
        (3u8, b"4".as_slice()),
    ];
    let visual_loop = C::choreofs_visual_loop();
    let mut cycle = 0u32;
    loop {
        let mut index = 0usize;
        while index < expected.len() {
            let (expected_fd, expected_payload) = expected[index];
            baker_driver_fd_write(&mut ctx, expected_fd, expected_payload).await;
            baker_driver_poll_oneoff(&mut ctx).await;
            index += 1;
        }
        cycle += 1;
        if visual_loop && cycle == CHOREOFS_VISUAL_READY_CYCLES {
            record_stack_high_water();
            mark_stage(STAGE_RUNTIME_READY);
            mark_result(C::success_result());
        }
        if !visual_loop && cycle >= CHOREOFS_REENTRY_CYCLES {
            break;
        }
    }

    baker_driver_assert_choreofs_counts();
    record_stack_high_water();
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(C::success_result());
    core::future::pending::<core::convert::Infallible>().await
}

async fn baker_recv_engine_req<C, const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
) -> EngineReq
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    match ctx.endpoint().recv::<g::Msg<LABEL, EngineReq>>().await {
        Ok(request) => request,
        Err(error) => {
            #[cfg(feature = "wasm-engine-core")]
            record_choreofs_engine_error_code(choreofs_recv_error_code(&error));
            core::hint::black_box(error);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    }
}

async fn baker_offer_engine_req<C, const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
) -> EngineReq
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    record_choreofs_driver_trace(0x5745_c000 | LABEL as u32);
    let branch = match ctx.endpoint().offer().await {
        Ok(branch) => branch,
        Err(error) => {
            #[cfg(feature = "wasm-engine-core")]
            record_choreofs_engine_error_code(choreofs_recv_error_code(&error));
            core::hint::black_box(error);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    record_choreofs_driver_trace(0x5745_c100 | branch.label() as u32);
    if branch.label() != LABEL {
        #[cfg(feature = "wasm-engine-core")]
        record_choreofs_engine_error_code(0x5745_c000 | branch.label() as u32);
        core::hint::black_box(branch.label());
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    let request = match branch.decode::<g::Msg<LABEL, EngineReq>>().await {
        Ok(request) => request,
        Err(error) => {
            #[cfg(feature = "wasm-engine-core")]
            record_choreofs_engine_error_code(choreofs_recv_error_code(&error));
            core::hint::black_box(error);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    record_choreofs_driver_trace(0x5745_c200 | LABEL as u32);
    request
}

async fn baker_send_engine_ret<C, const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
    reply: EngineRet,
) where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    let flow = match ctx.endpoint().flow::<g::Msg<LABEL, EngineRet>>() {
        Ok(flow) => flow,
        Err(error) => {
            core::hint::black_box(error);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    if let Err(error) = flow.send(&reply).await {
        core::hint::black_box(error);
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
}

async fn baker_driver_path_open<C, const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
) -> appkit::LedgerFdFact
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    let request = match baker_recv_engine_req::<C, ROLE, LABEL_WASI_PATH_OPEN>(ctx).await {
        EngineReq::PathOpen(request) => request,
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    if request.preopen_fd() != BAKER_LED_PREOPEN_FD || request.rights_base() != FD_WRITE_RIGHT {
        core::hint::black_box(request);
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    let object = match ctx.choreofs().resolve(request.path()) {
        Some(object) => object,
        None => {
            core::hint::black_box(request);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    let fact = match baker_find_ledger_fd(ctx.ledger(), object, request.rights_base()) {
        Some(fact) => fact,
        None => {
            core::hint::black_box((object, request.rights_base()));
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    if fact.fd() > u8::MAX as u32 {
        core::hint::black_box(fact);
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    record_choreofs_path_open(object);
    baker_send_engine_ret::<C, ROLE, LABEL_WASI_PATH_OPEN_RET>(
        ctx,
        EngineRet::PathOpened(PathOpened::new(fact.fd() as u8, 0)),
    )
    .await;
    fact
}

async fn baker_driver_fd_write<C, const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
    expected_fd: u8,
    expected_payload: &[u8],
) where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    let request = match baker_offer_engine_req::<C, ROLE, LABEL_WASI_FD_WRITE>(ctx).await {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    baker_handle_fd_write(ctx, request, expected_fd, expected_payload).await;
}

async fn baker_handle_fd_write<C, const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, C, ROLE>,
    request: FdWrite,
    expected_fd: u8,
    expected_payload: &[u8],
) where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    if request.fd() != expected_fd || request.as_bytes() != expected_payload {
        core::hint::black_box((request, expected_fd));
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    let fact = match ctx.ledger().fd(request.fd() as u32) {
        Some(fact) if fact.rights() == FD_WRITE_RIGHT => fact,
        Some(fact) => {
            core::hint::black_box(fact);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
        None => {
            core::hint::black_box(request);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    let mask = match request.as_bytes() {
        b"1" => CHOREOFS_LED_GREEN,
        b"2" => CHOREOFS_LED_ORANGE,
        b"4" => CHOREOFS_LED_RED,
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    if fact.object() != appkit::ObjectId(1) {
        core::hint::black_box(fact);
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    write_baker_traffic_mask(mask);
    record_choreofs_fd_write(fact.object());
    baker_send_engine_ret::<C, ROLE, LABEL_WASI_FD_WRITE_RET>(
        ctx,
        EngineRet::FdWriteDone(FdWriteDone::new(request.fd(), request.len() as u8)),
    )
    .await;
}

async fn baker_driver_poll_oneoff<C, const ROLE: u8>(ctx: &mut appkit::DriverCtx<'_, C, ROLE>)
where
    C: appkit::Capsule<Local = BakerChoreoFsLocal> + BakerImageFacts,
{
    let request = match baker_recv_engine_req::<C, ROLE, LABEL_WASI_POLL_ONEOFF>(ctx).await {
        EngineReq::PollOneoff(request) => request,
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    };
    baker_poll_delay(request.timeout_tick());
    record_choreofs_poll();
    baker_send_engine_ret::<C, ROLE, LABEL_WASI_POLL_ONEOFF_RET>(
        ctx,
        EngineRet::PollReady(PollReady::new(1)),
    )
    .await;
}

fn baker_find_ledger_fd(
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
        index += 1;
    }
    None
}

fn baker_poll_delay(timeout_tick: u64) {
    let capped = core::cmp::min(timeout_tick / 50_000, 80_000);
    let mut spin = 0u64;
    while spin < capped {
        core::hint::black_box(spin);
        core::hint::spin_loop();
        spin += 1;
    }
}

fn baker_driver_assert_choreofs_counts() {
    let path_opens = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_PATH_OPEN_COUNT));
    let writes = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_FD_WRITE_COUNT));
    let polls = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_POLL_COUNT));
    let led_mask = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_LED_MASK));
    let seen_led_mask = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_SEEN_LED_MASK));
    if path_opens != CHOREOFS_EXPECTED_PATH_OPENS
        || writes != CHOREOFS_EXPECTED_FD_WRITES
        || polls != CHOREOFS_EXPECTED_POLLS
        || led_mask != CHOREOFS_LED_RED
        || seen_led_mask != (CHOREOFS_LED_GREEN | CHOREOFS_LED_ORANGE | CHOREOFS_LED_RED)
    {
        core::hint::black_box((path_opens, writes, polls, led_mask, seen_led_mask));
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
}

impl<I> appkit::ArtifactForImage<BakerTraffic, I> for BakerArtifacts
where
    I: appkit::LogicalImage<BakerTraffic, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl appkit::ArtifactForImage<BakerChoreoFsTraffic, DriverImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::NoWasi {
        appkit::NoWasi
    }
}

impl appkit::ArtifactForImage<BakerChoreoFsTraffic, EngineImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_CHOREOFS_TRAFFIC)
    }
}

impl appkit::ArtifactForImage<BakerChoreoFsTrafficLoop, DriverImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::NoWasi {
        appkit::NoWasi
    }
}

impl appkit::ArtifactForImage<BakerChoreoFsTrafficLoop, EngineImage> for BakerArtifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_CHOREOFS_TRAFFIC)
    }
}

impl<I> appkit::ArtifactForImage<BakerFailSafe, I> for BakerArtifacts
where
    I: appkit::LogicalImage<BakerFailSafe, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl<I> appkit::ArtifactForImage<BakerRecovery, I> for BakerArtifacts
where
    I: appkit::LogicalImage<BakerRecovery, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl<I> appkit::ArtifactForImage<BakerManyReentry, I> for BakerArtifacts
where
    I: appkit::LogicalImage<BakerManyReentry, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

fn park() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn default_handler() -> ! {
    record_failure_stage(STAGE_HARD_PANIC);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
unsafe extern "C" fn hard_fault_handler_with_sp(sp: *const u32) -> ! {
    record_hard_fault_frame(sp);
    record_failure_stage(STAGE_HARD_PANIC);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_hard_fault_frame(sp: *const u32) {
    unsafe {
        let stacked_lr = core::ptr::read_volatile(sp.add(5));
        let stacked_pc = core::ptr::read_volatile(sp.add(6));
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_LR),
            stacked_lr,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_PC),
            stacked_pc,
        );
    }
}

fn marker_core_id() -> u32 {
    rp2040_sio::core_id()
}

fn marker_stage_slot() -> *mut u32 {
    if marker_core_id() == 0 {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STAGE)
    } else {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STAGE)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn marker_stack_slot() -> *mut u32 {
    if marker_core_id() == 0 {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES)
    } else {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_stack_high_water() {
    let sp: u32;
    unsafe {
        asm!("mov {0}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
    }
    let (top, limit) = if marker_core_id() == 0 {
        (
            core::ptr::addr_of!(__stack_top) as u32,
            core::ptr::addr_of!(__core1_stack_top) as u32,
        )
    } else {
        (
            core::ptr::addr_of!(__core1_stack_top) as u32,
            core::ptr::addr_of!(__stack_limit) as u32,
        )
    };
    if sp < limit || sp > top {
        return;
    }
    let used = top.saturating_sub(sp);
    let slot = marker_stack_slot();
    unsafe {
        let current = read_volatile(slot);
        if used > current {
            write_volatile(slot, used);
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn record_stack_high_water() {}

fn mark_stage(stage: u32) {
    record_stack_high_water();
    unsafe {
        core::ptr::write_volatile(marker_stage_slot(), stage);
    }
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    event();
}

fn mark_result(result: u32) {
    record_stack_high_water();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), result);
    }
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_failure_stage(stage: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_FAILURE_STAGE), stage);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn record_failure_stage(stage: u32) {
    core::hint::black_box(stage);
}

fn write_marker(slot: *mut u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(slot, value);
    }
}

#[cfg(feature = "wasm-engine-core")]
fn record_choreofs_sio_trace(code: u32) {
    unsafe {
        let count = core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_CHOREOFS_SIO_TRACE_COUNT));
        let index = (count as usize) & 7;
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE[index]),
            code,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_COUNT),
            count.wrapping_add(1),
        );
    }
}

fn read_marker(slot: *const u32) -> u32 {
    unsafe { core::ptr::read_volatile(slot) }
}

fn increment_marker(slot: *mut u32) -> u32 {
    let next = read_marker(slot).saturating_add(1);
    write_marker(slot, next);
    next
}

fn reset_choreofs_markers() {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_STATUS), 0);
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_ERROR_CODE),
        0,
    );
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_DRIVER_TRACE), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_COUNT), 0);
    let mut trace_index = 0usize;
    while trace_index < 8 {
        unsafe {
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE[trace_index]),
                0,
            );
        }
        trace_index += 1;
    }
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_PATH_OPEN_COUNT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_FD_WRITE_COUNT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_POLL_COUNT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LED_MASK), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SEEN_LED_MASK), 0);
}

fn record_choreofs_engine_status(status: u32) {
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_STATUS),
        status,
    );
}

#[cfg(feature = "wasm-engine-core")]
fn record_choreofs_engine_error_code(code: u32) {
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_ERROR_CODE),
        code,
    );
}

fn record_choreofs_driver_trace(trace: u32) {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_DRIVER_TRACE), trace);
}

#[cfg(feature = "wasm-engine-core")]
fn choreofs_recv_error_code(error: &hibana::RecvError) -> u32 {
    match error {
        hibana::RecvError::Transport(_) => 0x5745_6101,
        hibana::RecvError::Binding(_) => 0x5745_6102,
        hibana::RecvError::Codec(_) => 0x5745_6103,
        hibana::RecvError::PhaseInvariant => 0x5745_6104,
        hibana::RecvError::LabelMismatch { .. } => 0x5745_6105,
        hibana::RecvError::PeerMismatch { .. } => 0x5745_6106,
        hibana::RecvError::SessionMismatch { .. } => 0x5745_6107,
        hibana::RecvError::PolicyAbort { reason } => 0x5745_7000 | (*reason as u32),
    }
}

fn record_choreofs_path_open(object: appkit::ObjectId) {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_PATH_OPEN_COUNT));
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT),
        object.0,
    );
}

fn record_choreofs_fd_write(object: appkit::ObjectId) {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_FD_WRITE_COUNT));
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT),
        object.0,
    );
}

fn record_choreofs_poll() {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_POLL_COUNT));
}

fn record_choreofs_led_mask(object: appkit::ObjectId, high: bool) {
    let bit = match object.0 {
        1 => CHOREOFS_LED_GREEN,
        2 => CHOREOFS_LED_ORANGE,
        3 => CHOREOFS_LED_RED,
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR)
        }
    };
    let slot = core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LED_MASK);
    let current = read_marker(slot);
    let next = if high { current | bit } else { current & !bit };
    write_marker(slot, next);
}

#[cold]
fn runtime_fail(stage: u32) -> ! {
    record_failure_stage(stage);
    record_choreofs_engine_status(CHOREOFS_ENGINE_ERROR);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn gpio_ctrl(pin: u8) -> *mut u32 {
    (IO_BANK0_BASE + 0x04 + pin as usize * 8) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn gpio_pad(pin: u8) -> *mut u32 {
    (PADS_BANK0_BASE + 0x04 + pin as usize * 4) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_gpio_bank_init() {
    unsafe {
        write_volatile(RESETS_RESET_CLR, RESETS_IO_BANK0 | RESETS_PADS_BANK0);
    }
    while unsafe { read_volatile(RESETS_RESET_DONE) } & (RESETS_IO_BANK0 | RESETS_PADS_BANK0)
        != (RESETS_IO_BANK0 | RESETS_PADS_BANK0)
    {
        core::hint::spin_loop();
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn baker_gpio_bank_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_gpio_init_output(pin: u8) {
    baker_gpio_bank_init();
    unsafe {
        write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
        write_volatile(GPIO_OE_SET, 1u32 << pin);
        write_volatile(GPIO_OUT_CLR, 1u32 << pin);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn baker_gpio_init_output(pin: u8) {
    baker_gpio_bank_init();
    core::hint::black_box(pin);
}

fn init_baker_led_outputs() {
    baker_gpio_init_output(GREEN_LED_PIN);
    baker_gpio_init_output(ORANGE_LED_PIN);
    baker_gpio_init_output(RED_LED_PIN);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_gpio_write(pin: u8, high: bool) {
    let bit = 1u32 << pin;
    unsafe {
        if high {
            write_volatile(GPIO_OUT_SET, bit);
        } else {
            write_volatile(GPIO_OUT_CLR, bit);
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn baker_gpio_write(pin: u8, high: bool) {
    core::hint::black_box((pin, high));
}

fn write_baker_led_object(object: appkit::ObjectId, high: bool) {
    match object.0 {
        1 => baker_gpio_write(GREEN_LED_PIN, high),
        2 => baker_gpio_write(ORANGE_LED_PIN, high),
        3 => baker_gpio_write(RED_LED_PIN, high),
        other => {
            core::hint::black_box(other);
            runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
        }
    }
    record_choreofs_led_mask(object, high);
}

fn write_baker_traffic_mask(mask: u32) {
    if mask & !(CHOREOFS_LED_GREEN | CHOREOFS_LED_ORANGE | CHOREOFS_LED_RED) != 0 {
        core::hint::black_box(mask);
        runtime_fail(STAGE_CHOREOFS_DRIVER_ERROR);
    }
    baker_gpio_write(GREEN_LED_PIN, mask & CHOREOFS_LED_GREEN != 0);
    baker_gpio_write(ORANGE_LED_PIN, mask & CHOREOFS_LED_ORANGE != 0);
    baker_gpio_write(RED_LED_PIN, mask & CHOREOFS_LED_RED != 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LED_MASK), mask);
    let seen_slot = core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SEEN_LED_MASK);
    write_marker(seen_slot, read_marker(seen_slot) | mask);
}

fn mark_safe_state() {
    init_baker_led_outputs();
    write_baker_led_object(appkit::ObjectId(1), false);
    write_baker_led_object(appkit::ObjectId(2), false);
    write_baker_led_object(appkit::ObjectId(3), false);
    record_stack_high_water();
}

fn check_report<R, I>(report: &appkit::RunReport<R, I>, required_role: u8) {
    assert!(report.projected_roles().contains(required_role));
    assert_eq!(
        report.attached_endpoint_count(),
        report.validated_role_count()
    );
}

impl appkit::LogicalImage<BakerTraffic> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(0);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(1));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        <BakerTraffic as BakerImageFacts>::driver_facts()
    }
}

impl appkit::LogicalImage<BakerTraffic> for EngineImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(1);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(0));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerChoreoFsTraffic> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(10);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(11));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        <BakerChoreoFsTraffic as BakerImageFacts>::driver_facts()
    }
}

impl appkit::LogicalImage<BakerChoreoFsTraffic> for EngineImage {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(11);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(10));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerChoreoFsTrafficLoop> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(12);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(13));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        <BakerChoreoFsTrafficLoop as BakerImageFacts>::driver_facts()
    }
}

impl appkit::LogicalImage<BakerChoreoFsTrafficLoop> for EngineImage {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(13);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(12));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerFailSafe> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(20);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(21));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerFailSafe> for EngineImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(21);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(20));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerRecovery> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(30);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(31));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerRecovery> for EngineImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(31);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(30));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerManyReentry> for DriverImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(40);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(41));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<BakerManyReentry> for EngineImage {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = rp2040_sio::SioTransport;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(41);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(appkit::ImageId(40));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a> {
        rp2040_sio::SioTransport::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        baker_wasi_guest_storage::<ROLE>()
    }
}

static ARTIFACTS: BakerArtifacts = BakerArtifacts;

pub fn run_traffic() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let mut report =
            appkit::run::<DriverImage, BakerTraffic>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerTraffic,
            >>::for_image::<DriverImage>(
                &ARTIFACTS
            ));
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<BakerTraffic>>::safe_state(report.image_mut());
    } else {
        let mut report =
            appkit::run::<EngineImage, BakerTraffic>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerTraffic,
            >>::for_image::<EngineImage>(
                &ARTIFACTS
            ));
        check_report(&report, 1);
        <EngineImage as appkit::LogicalImage<BakerTraffic>>::safe_state(report.image_mut());
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_SUCCESS);
    park()
}

pub fn run_choreofs_traffic() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let report = appkit::run::<DriverImage, BakerChoreoFsTraffic>(appkit::NoWasi);
        check_report(&report, 0);
    } else {
        let report = appkit::run::<EngineImage, BakerChoreoFsTraffic>(
            <BakerArtifacts as appkit::ArtifactBundle<BakerChoreoFsTraffic>>::for_image::<
                EngineImage,
            >(&ARTIFACTS),
        );
        check_report(&report, 1);
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_SUCCESS);
    park()
}

pub fn run_choreofs_traffic_loop() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let report = appkit::run::<DriverImage, BakerChoreoFsTrafficLoop>(appkit::NoWasi);
        check_report(&report, 0);
    } else {
        let report = appkit::run::<EngineImage, BakerChoreoFsTrafficLoop>(
            <BakerArtifacts as appkit::ArtifactBundle<BakerChoreoFsTrafficLoop>>::for_image::<
                EngineImage,
            >(&ARTIFACTS),
        );
        check_report(&report, 1);
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_SUCCESS);
    park()
}

pub fn run_fail_safe() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let mut report =
            appkit::run::<DriverImage, BakerFailSafe>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerFailSafe,
            >>::for_image::<DriverImage>(
                &ARTIFACTS
            ));
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<BakerFailSafe>>::safe_state(report.image_mut());
    } else {
        let mut report =
            appkit::run::<EngineImage, BakerFailSafe>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerFailSafe,
            >>::for_image::<EngineImage>(
                &ARTIFACTS
            ));
        check_report(&report, 1);
        <EngineImage as appkit::LogicalImage<BakerFailSafe>>::safe_state(report.image_mut());
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_FAIL_SAFE_OK);
    park()
}

pub fn run_recovery() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let mut report =
            appkit::run::<DriverImage, BakerRecovery>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerRecovery,
            >>::for_image::<DriverImage>(
                &ARTIFACTS
            ));
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<BakerRecovery>>::safe_state(report.image_mut());
    } else {
        let mut report =
            appkit::run::<EngineImage, BakerRecovery>(<BakerArtifacts as appkit::ArtifactBundle<
                BakerRecovery,
            >>::for_image::<EngineImage>(
                &ARTIFACTS
            ));
        check_report(&report, 1);
        <EngineImage as appkit::LogicalImage<BakerRecovery>>::safe_state(report.image_mut());
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_RECOVERY_OK);
    park()
}

pub fn run_many_reentry() -> ! {
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let mut report = appkit::run::<DriverImage, BakerManyReentry>(
            <BakerArtifacts as appkit::ArtifactBundle<BakerManyReentry>>::for_image::<DriverImage>(
                &ARTIFACTS,
            ),
        );
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<BakerManyReentry>>::safe_state(report.image_mut());
    } else {
        let mut report = appkit::run::<EngineImage, BakerManyReentry>(
            <BakerArtifacts as appkit::ArtifactBundle<BakerManyReentry>>::for_image::<EngineImage>(
                &ARTIFACTS,
            ),
        );
        check_report(&report, 1);
        <EngineImage as appkit::LogicalImage<BakerManyReentry>>::safe_state(report.image_mut());
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(RESULT_MANY_REENTRY_OK);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let stage = info
        .location()
        .map(|location| 0x4c00_0000 | (location.line() & 0x0000_ffff))
        .unwrap_or(STAGE_HARD_PANIC);
    record_failure_stage(stage);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_ram() {
    unsafe {
        let data_src = core::ptr::addr_of!(__data_load_start);
        let data_start = core::ptr::addr_of_mut!(__data_start);
        let data_end = core::ptr::addr_of_mut!(__data_end);
        let data_len = data_end as usize - data_start as usize;
        core::ptr::copy_nonoverlapping(data_src, data_start, data_len);

        let bss_start = core::ptr::addr_of_mut!(__bss_start);
        let bss_end = core::ptr::addr_of_mut!(__bss_end);
        let bss_len = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, bss_len);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn event() {
    unsafe {
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_drain() {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD != 0 {
        unsafe {
            read_volatile(SIO_FIFO_RD);
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_clear_errors() {
    unsafe {
        write_volatile(SIO_FIFO_ST_WRITE, FIFO_WOF | FIFO_ROE);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn reset_core1_to_bootrom() {
    let force_off = unsafe { read_volatile(PSM_FRCE_OFF) };
    unsafe {
        write_volatile(PSM_FRCE_OFF, force_off | PSM_PROC1);
    }
    for spin in 0..32 {
        core::hint::black_box(spin);
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(PSM_FRCE_OFF, force_off & !PSM_PROC1);
    }
    for spin in 0..32 {
        core::hint::black_box(spin);
        core::hint::spin_loop();
    }
    fifo_drain();
    fifo_clear_errors();
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_push_blocking(word: u32) {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_RDY == 0 {
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(SIO_FIFO_WR, word);
    }
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_pop_blocking() -> u32 {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD == 0 {
        core::hint::spin_loop();
    }
    unsafe { read_volatile(SIO_FIFO_RD) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn launch_core1(vector_table: u32, stack_top: u32, entry: u32) -> bool {
    reset_core1_to_bootrom();

    let sequence = [0, 0, 1, vector_table, stack_top, entry];
    let mut index = 0usize;
    let mut failures = 0u8;
    while index < sequence.len() {
        let word = sequence[index];
        if word == 0 {
            fifo_drain();
            fifo_clear_errors();
            event();
        }
        fifo_push_blocking(word);
        if fifo_pop_blocking() == word {
            index += 1;
            continue;
        }
        index = 0;
        failures = failures.saturating_add(1);
        if failures > CORE1_LAUNCH_RETRIES {
            return false;
        }
    }
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn mark_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 1);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn clear_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 0);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core1_started() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(CORE1_STARTED)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn ensure_core1_launched() {
    clear_core1_started();
    let launched = launch_core1(
        core::ptr::addr_of!(VECTOR_TABLE) as u32,
        core::ptr::addr_of!(__core1_stack_top) as u32,
        core1_entry as *const () as usize as u32,
    );
    if !launched {
        record_failure_stage(STAGE_CORE1_LAUNCH_ERR);
        mark_result(RESULT_FAILURE);
        park();
    }
    for spin in 0..100_000 {
        core::hint::black_box(spin);
        if core1_started() {
            mark_stage(STAGE_CORE1_LAUNCHED);
            return;
        }
        core::hint::spin_loop();
    }
    record_failure_stage(STAGE_CORE1_START_TIMEOUT);
    mark_result(RESULT_FAILURE);
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn core1_entry() -> ! {
    fifo_drain();
    mark_core1_started();
    event();
    mark_stage(STAGE_ENGINE_RUNTIME_READY_SEEN);
    unsafe { baker_selected_run() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    init_ram();
    mark_stage(STAGE_CORE0_START);
    ensure_core1_launched();
    mark_stage(STAGE_PROGRAM_READY);
    unsafe { baker_selected_run() }
}
