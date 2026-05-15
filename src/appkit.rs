//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose kernel internals,
//! or complete WASI P1 imports outside projected endpoint/carrier progress.

use core::{
    convert::Infallible,
    fmt::Debug,
    marker::PhantomData,
    mem::{MaybeUninit, align_of, size_of},
    pin::Pin,
    slice,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(any(test, not(target_os = "none")))]
use core::future::Future;

#[cfg(any(feature = "wasm-engine-core", all(not(test), target_os = "none")))]
use core::cell::UnsafeCell;

#[cfg(feature = "wasm-engine-core")]
use core::sync::atomic::{AtomicBool, Ordering};

use crate::choreography::protocol::{
    EngineReq, EngineRet, LABEL_WASI_ARGS_GET, LABEL_WASI_ARGS_GET_RET, LABEL_WASI_ARGS_SIZES_GET,
    LABEL_WASI_ARGS_SIZES_GET_RET, LABEL_WASI_CLOCK_RES_GET, LABEL_WASI_CLOCK_RES_GET_RET,
    LABEL_WASI_CLOCK_TIME_GET, LABEL_WASI_CLOCK_TIME_GET_RET, LABEL_WASI_ENVIRON_GET,
    LABEL_WASI_ENVIRON_GET_RET, LABEL_WASI_ENVIRON_SIZES_GET, LABEL_WASI_ENVIRON_SIZES_GET_RET,
    LABEL_WASI_FD_CLOSE, LABEL_WASI_FD_CLOSE_RET, LABEL_WASI_FD_FDSTAT_GET,
    LABEL_WASI_FD_FDSTAT_GET_RET, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET,
    LABEL_WASI_FD_READDIR, LABEL_WASI_FD_READDIR_RET, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET,
    LABEL_WASI_IMPORT_LOOP_BREAK_CONTROL, LABEL_WASI_IMPORT_LOOP_CONTINUE_CONTROL,
    LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF,
    LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, LABEL_WASI_RANDOM_GET,
    LABEL_WASI_RANDOM_GET_RET, LABEL_WASIP1_CLOCK_NOW, LABEL_WASIP1_CLOCK_NOW_RET,
    LABEL_WASIP1_EXIT, LABEL_WASIP1_RANDOM_SEED, LABEL_WASIP1_RANDOM_SEED_RET, LABEL_WASIP1_STDERR,
    LABEL_WASIP1_STDERR_RET, LABEL_WASIP1_STDIN, LABEL_WASIP1_STDIN_RET, LABEL_WASIP1_STDOUT,
    LABEL_WASIP1_STDOUT_RET,
};

pub use crate::choreography::protocol::BuiltInLabelUniverse as BuiltInUniverse;

#[cfg(feature = "wasm-engine-core")]
use crate::choreography::protocol::{
    ArgsGet, ArgsSizesGet, BudgetExpired, BudgetRun, ClockResGet, ClockTimeGet, EnvironGet,
    EnvironSizesGet, FdRead, FdReaddir, FdRequest, FdWrite, LABEL_MEM_FENCE, MemFence,
    MemFenceReason, MemRights, PathOpen, PollOneoff, ProcExitStatus, RandomGet,
    WasiImportLoopBreak, WasiImportLoopContinue,
};

#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ATTACH_TAP_EVENTS: usize = 128;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_ATTACH_TAP_EVENTS: usize = 128;

#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ATTACH_SLAB_BYTES: usize = 262_144;
#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ROLE_FUTURE_ALIGN: usize = 16;
#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ROLE_FUTURE_BYTES: usize = 8 * 1024;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_FUTURE_ALIGN: usize = 16;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES: usize = 2 * 1024;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES: usize = 8 * 1024;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_WASI_GUEST_ARENA_ALIGN: usize = 16;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_WASI_GUEST_BYTES: usize = size_of::<crate::kernel::engine::wasm::Guest<'static>>();
#[cfg(feature = "wasm-engine-core")]
const APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION: u32 = 1_000_000;
// A logical image owns one carrier rendezvous. Larger rendezvous sets are a
// different site/artifact composition, not implicit appkit capacity.
const APPKIT_SESSION_RV_SLOTS: usize = 1;
/// Current typed hibana role domain: `Role<0>` through `Role<15>`.
///
/// Raising this is a hibana representation change, not an appkit knob. The
/// carrier materialization deliberately follows the typed projection domain so
/// one logical image cannot request roles appkit cannot project.
pub const HIBANA_TYPED_ROLE_DOMAIN_SIZE: u8 = 16;
#[cfg(any(test, not(target_os = "none")))]
const APPKIT_CARRIER_ROLES: usize = HIBANA_TYPED_ROLE_DOMAIN_SIZE as usize;

/// Result shape for a localside role task.
///
/// Role tasks normally run forever. Returning `Err` is a top-level image
/// failure; appkit turns it into one panic at the scheduler boundary instead of
/// forcing example localsides to scatter panic sites through protocol code.
pub type RoleResult<E> = Result<Infallible, E>;

struct PendingRole<T, E> {
    context: T,
    error: PhantomData<fn() -> E>,
}

impl<T, E> PendingRole<T, E> {
    const fn new(context: T) -> Self {
        Self {
            context,
            error: PhantomData,
        }
    }
}

impl<T, E> Unpin for PendingRole<T, E> {}

impl<T, E> core::future::Future for PendingRole<T, E> {
    type Output = RoleResult<E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.get_mut();
        core::hint::black_box(&state.context);
        core::hint::black_box(cx.waker());
        Poll::Pending
    }
}

enum RoleTaskError<E> {
    Local(E),
    #[cfg(feature = "wasm-engine-core")]
    Wasi(WasiGuestError),
}

impl<E> Debug for RoleTaskError<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Local(error) => formatter.debug_tuple("Local").field(error).finish(),
            #[cfg(feature = "wasm-engine-core")]
            Self::Wasi(error) => formatter.debug_tuple("Wasi").field(error).finish(),
        }
    }
}

async fn local_role_task<F, E>(future: F) -> RoleResult<RoleTaskError<E>>
where
    F: core::future::Future<Output = RoleResult<E>>,
{
    match future.await {
        Ok(done) => Ok(done),
        Err(error) => Err(RoleTaskError::Local(error)),
    }
}

#[cfg(feature = "wasm-engine-core")]
async fn wasi_role_task<F, E>(future: F) -> RoleResult<RoleTaskError<E>>
where
    F: core::future::Future<Output = RoleResult<WasiGuestError>>,
{
    match future.await {
        Ok(done) => Ok(done),
        Err(error) => Err(RoleTaskError::Wasi(error)),
    }
}

#[cfg(all(not(test), target_os = "none"))]
#[repr(C, align(16))]
pub struct EmbeddedAttachStorage<const SLAB_BYTES: usize> {
    tap: UnsafeCell<[hibana::integration::tap::TapEvent; APPKIT_ATTACH_TAP_EVENTS]>,
    slab: UnsafeCell<[u8; SLAB_BYTES]>,
}

#[cfg(all(not(test), target_os = "none"))]
pub struct EmbeddedAttachStorageRef<'a> {
    tap: &'a mut [hibana::integration::tap::TapEvent; APPKIT_ATTACH_TAP_EVENTS],
    slab: &'a mut [u8],
}

#[cfg(all(not(test), target_os = "none"))]
impl<const SLAB_BYTES: usize> EmbeddedAttachStorage<SLAB_BYTES> {
    pub const fn empty() -> Self {
        Self {
            tap: UnsafeCell::new(
                [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS],
            ),
            slab: UnsafeCell::new([0; SLAB_BYTES]),
        }
    }

    pub fn lease(&'static self) -> EmbeddedAttachStorageRef<'static> {
        let tap = unsafe { &mut *self.tap.get() };
        let slab = unsafe { &mut *self.slab.get() };
        tap.fill(hibana::integration::tap::TapEvent::zero());
        slab.fill(0);
        EmbeddedAttachStorageRef { tap, slab }
    }
}

#[cfg(all(not(test), target_os = "none"))]
unsafe impl<const SLAB_BYTES: usize> Sync for EmbeddedAttachStorage<SLAB_BYTES> {}

fn align_storage_up(value: usize, align: usize) -> usize {
    let mask = align.saturating_sub(1);
    (value + mask) & !mask
}

fn carve_session_kit_storage<'a, TransportTy, UniverseTy, ClockTy, const MAX_RV: usize>(
    slab: &'a mut [u8],
) -> (
    &'a mut MaybeUninit<
        hibana::integration::SessionKit<'a, TransportTy, UniverseTy, ClockTy, MAX_RV>,
    >,
    &'a mut [u8],
)
where
    TransportTy: hibana::integration::Transport + 'a,
    UniverseTy: hibana::integration::runtime::LabelUniverse + 'a,
    ClockTy: hibana::integration::runtime::Clock + 'a,
{
    type Kit<'a, TransportTy, UniverseTy, ClockTy, const MAX_RV: usize> =
        hibana::integration::SessionKit<'a, TransportTy, UniverseTy, ClockTy, MAX_RV>;

    let base = slab.as_mut_ptr() as usize;
    let len = slab.len();
    let end = base
        .checked_add(len)
        .expect("appkit attach slab address range overflow");
    let kit_start = align_storage_up(
        base,
        align_of::<Kit<'a, TransportTy, UniverseTy, ClockTy, MAX_RV>>(),
    );
    let kit_end = kit_start
        .checked_add(size_of::<Kit<'a, TransportTy, UniverseTy, ClockTy, MAX_RV>>())
        .expect("appkit SessionKit storage address range overflow");
    assert!(
        kit_end <= end,
        "appkit attach slab cannot fit in-place SessionKit"
    );
    let kit_offset = kit_start - base;
    let rest_offset = kit_end - base;
    unsafe {
        let kit_storage = &mut *slab
            .as_mut_ptr()
            .add(kit_offset)
            .cast::<MaybeUninit<Kit<'a, TransportTy, UniverseTy, ClockTy, MAX_RV>>>();
        let rest = slice::from_raw_parts_mut(slab.as_mut_ptr().add(rest_offset), len - rest_offset);
        (kit_storage, rest)
    }
}

#[cfg(all(not(test), target_os = "none"))]
#[repr(C, align(16))]
struct EmbeddedFutureArena<const N: usize> {
    bytes: UnsafeCell<[u8; N]>,
}

#[repr(C, align(16))]
#[cfg(feature = "wasm-engine-core")]
pub struct WasiGuestArena {
    bytes: UnsafeCell<[u8; APPKIT_WASI_GUEST_BYTES]>,
    occupied: AtomicBool,
}

#[cfg(all(not(test), target_os = "none"))]
impl<const N: usize> EmbeddedFutureArena<N> {
    const EMPTY: Self = Self {
        bytes: UnsafeCell::new([0; N]),
    };

    fn as_mut_ptr(&self) -> *mut u8 {
        unsafe { (*self.bytes.get()).as_mut_ptr() }
    }
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestArena {
    const EMPTY: Self = Self {
        bytes: UnsafeCell::new([0; APPKIT_WASI_GUEST_BYTES]),
        occupied: AtomicBool::new(false),
    };

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    pub fn storage<'guest>(&'static self) -> WasiGuestStorage<'guest> {
        assert!(
            align_of::<crate::kernel::engine::wasm::Guest<'static>>()
                <= APPKIT_WASI_GUEST_ARENA_ALIGN,
            "WASI guest arena alignment is too small"
        );
        #[cfg(target_has_atomic = "ptr")]
        while self
            .occupied
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            assert!(
                !self.occupied.load(Ordering::Acquire),
                "WASI guest arena is already leased"
            );
            self.occupied.store(true, Ordering::Release);
        }
        WasiGuestStorage {
            occupied: &self.occupied,
            ptr: unsafe { (*self.bytes.get()).as_mut_ptr().cast() },
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
unsafe impl<const N: usize> Sync for EmbeddedFutureArena<N> {}

#[cfg(feature = "wasm-engine-core")]
unsafe impl Sync for WasiGuestArena {}

#[cfg(feature = "wasm-engine-core")]
pub struct WasiGuestStorage<'guest> {
    occupied: &'static AtomicBool,
    ptr: *mut crate::kernel::engine::wasm::Guest<'guest>,
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestStorage<'guest> {
    fn guest_ptr(&mut self) -> *mut crate::kernel::engine::wasm::Guest<'guest> {
        self.ptr
    }
}

#[cfg(feature = "wasm-engine-core")]
impl Drop for WasiGuestStorage<'_> {
    fn drop(&mut self) {
        self.occupied.store(false, Ordering::Release);
    }
}

#[cfg(all(not(test), target_os = "none"))]
static EMBEDDED_ROLE0_FUTURE_ARENA: EmbeddedFutureArena<APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES> =
    EmbeddedFutureArena::EMPTY;

#[cfg(all(not(test), target_os = "none"))]
static EMBEDDED_ROLE1_FUTURE_ARENA: EmbeddedFutureArena<APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES> =
    EmbeddedFutureArena::EMPTY;

#[cfg(all(not(test), target_os = "none"))]
unsafe fn noop_waker_clone(data: *const ()) -> RawWaker {
    assert!(data.is_null(), "appkit noop waker data must be null");
    RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)
}

#[cfg(all(not(test), target_os = "none"))]
unsafe fn noop_waker_wake(data: *const ()) {
    assert!(data.is_null(), "appkit noop waker data must be null");
}

#[cfg(all(not(test), target_os = "none"))]
unsafe fn noop_waker_wake_by_ref(data: *const ()) {
    assert!(data.is_null(), "appkit noop waker data must be null");
}

#[cfg(all(not(test), target_os = "none"))]
unsafe fn noop_waker_drop(data: *const ()) {
    assert!(data.is_null(), "appkit noop waker data must be null");
}

#[cfg(all(not(test), target_os = "none"))]
static NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    noop_waker_clone,
    noop_waker_wake,
    noop_waker_wake_by_ref,
    noop_waker_drop,
);

#[cfg(all(not(test), target_os = "none"))]
fn poll_localside_once<F, E>(future: F)
where
    F: core::future::Future<Output = RoleResult<E>>,
    E: Debug,
{
    let raw = RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE);
    let waker = unsafe {
        // SAFETY: The vtable never dereferences the data pointer and all
        // operations are no-ops suitable for a single immediate poll.
        Waker::from_raw(raw)
    };
    let mut cx = Context::from_waker(&waker);
    let mut pinned = core::pin::pin!(future);
    match pinned.as_mut().poll(&mut cx) {
        Poll::Pending => {}
        Poll::Ready(Ok(done)) => match done {},
        Poll::Ready(Err(error)) => {
            panic!("appkit role task failed: {error:?}");
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
fn embedded_future_arena_for_role<const ROLE: u8>() -> (*mut u8, usize) {
    if ROLE == 0 {
        (
            EMBEDDED_ROLE0_FUTURE_ARENA.as_mut_ptr(),
            APPKIT_EMBEDDED_ROLE0_FUTURE_BYTES,
        )
    } else {
        (
            EMBEDDED_ROLE1_FUTURE_ARENA.as_mut_ptr(),
            APPKIT_EMBEDDED_ROLE1_FUTURE_BYTES,
        )
    }
}

#[cfg(all(not(test), target_os = "none"))]
fn poll_localside_forever<const ROLE: u8, F, E>(future: F) -> !
where
    F: core::future::Future<Output = RoleResult<E>>,
    E: Debug,
{
    let raw_waker = RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE);
    let waker = unsafe {
        // SAFETY: The vtable never dereferences the data pointer and all
        // operations are no-ops suitable for cooperative embedded polling.
        Waker::from_raw(raw_waker)
    };
    let mut task_context = Context::from_waker(&waker);
    let (future_arena, future_arena_bytes) = embedded_future_arena_for_role::<ROLE>();

    assert!(
        size_of::<F>() <= future_arena_bytes,
        "appkit role future exceeds embedded future arena"
    );
    assert!(
        align_of::<F>() <= APPKIT_EMBEDDED_FUTURE_ALIGN,
        "appkit role future alignment exceeds embedded future arena"
    );

    unsafe {
        let future_ptr = future_arena.cast::<F>();
        future_ptr.write(future);
        let mut pinned = Pin::new_unchecked(&mut *future_ptr);
        loop {
            match pinned.as_mut().poll(&mut task_context) {
                Poll::Pending => core::hint::spin_loop(),
                Poll::Ready(Ok(done)) => match done {},
                Poll::Ready(Err(error)) => {
                    panic!("appkit embedded role {ROLE} task failed: {error:?}")
                }
            }
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
fn poll_embedded_role_future<const ROLE: u8, F, E>(requested_roles: RoleSet, future: F)
where
    F: core::future::Future<Output = RoleResult<E>>,
    E: Debug,
{
    if requested_roles.count() == 1 {
        poll_localside_forever::<ROLE, F, E>(future);
    }
    poll_localside_once::<F, E>(future);
}

/// Compact logical image identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageId(pub u16);

/// Compact site identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiteId(pub u16);

/// Bounded peer logical images expected to attach to the same choreography.
///
/// This is build/attach metadata only. It does not authorize protocol
/// progress and it does not instantiate a carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerImageSet {
    ids: [ImageId; 8],
    len: u8,
}

impl PeerImageSet {
    pub const EMPTY: Self = Self {
        ids: [ImageId(0); 8],
        len: 0,
    };

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    pub const fn single(image: ImageId) -> Self {
        Self {
            ids: [
                image,
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
            ],
            len: 1,
        }
    }

    pub const fn pair(first: ImageId, second: ImageId) -> Self {
        Self {
            ids: [
                first,
                second,
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
                ImageId(0),
            ],
            len: 2,
        }
    }

    pub const fn ids(self) -> [ImageId; 8] {
        self.ids
    }

    pub const fn len(self) -> u8 {
        self.len
    }

    pub const fn contains(self, image: ImageId) -> bool {
        let mut idx = 0usize;
        while idx < self.len as usize {
            if self.ids[idx].0 == image.0 {
                return true;
            }
            idx += 1;
        }
        false
    }
}

/// Opaque carrier family identifier used by a logical image.
///
/// Appkit treats this as attach/build metadata only. Site-specific carrier
/// names live in examples or user crates, not in appkit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CarrierKind(u16);

impl CarrierKind {
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u16 {
        self.0
    }
}

/// Localside context family assigned to one projected role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleKind {
    Engine,
    Driver,
    Boundary,
    Link,
    Supervisor,
}

/// Count of attached projected roles by localside context family.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoleKindCounts {
    pub engine: u8,
    pub driver: u8,
    pub boundary: u8,
    pub link: u8,
    pub supervisor: u8,
}

impl RoleKindCounts {
    pub const fn total(self) -> u8 {
        self.engine
            .saturating_add(self.driver)
            .saturating_add(self.boundary)
            .saturating_add(self.link)
            .saturating_add(self.supervisor)
    }

    fn record(&mut self, kind: RoleKind) {
        match kind {
            RoleKind::Engine => self.engine = self.engine.saturating_add(1),
            RoleKind::Driver => self.driver = self.driver.saturating_add(1),
            RoleKind::Boundary => self.boundary = self.boundary.saturating_add(1),
            RoleKind::Link => self.link = self.link.saturating_add(1),
            RoleKind::Supervisor => self.supervisor = self.supervisor.saturating_add(1),
        }
    }
}

/// Requested projection slice for a logical image.
///
/// This is not protocol authority. The requested roles must be validated
/// against the capsule placement and hibana projection metadata before a
/// logical image is attached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoleSet {
    words: [u64; 4],
}

impl RoleSet {
    pub const EMPTY: Self = Self { words: [0; 4] };

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    pub const fn single(role: u8) -> Self {
        let word = (role / 64) as usize;
        let bit = 1u64 << (role % 64);
        Self {
            words: [
                if word == 0 { bit } else { 0 },
                if word == 1 { bit } else { 0 },
                if word == 2 { bit } else { 0 },
                if word == 3 { bit } else { 0 },
            ],
        }
    }

    pub const fn from_bits(bits: u128) -> Self {
        Self {
            words: [bits as u64, (bits >> 64) as u64, 0, 0],
        }
    }

    pub const fn from_words(words: [u64; 4]) -> Self {
        Self { words }
    }

    pub const fn bits(self) -> u128 {
        self.words[0] as u128 | ((self.words[1] as u128) << 64)
    }

    pub const fn words(self) -> [u64; 4] {
        self.words
    }

    pub const fn count(self) -> u8 {
        (self.words[0].count_ones()
            + self.words[1].count_ones()
            + self.words[2].count_ones()
            + self.words[3].count_ones()) as u8
    }

    pub const fn contains(self, role: u8) -> bool {
        let word = (role / 64) as usize;
        let bit = 1u64 << (role % 64);
        (self.words[word] & bit) != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            words: [
                self.words[0] | other.words[0],
                self.words[1] | other.words[1],
                self.words[2] | other.words[2],
                self.words[3] | other.words[3],
            ],
        }
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        ((self.words[0] & !other.words[0])
            | (self.words[1] & !other.words[1])
            | (self.words[2] & !other.words[2])
            | (self.words[3] & !other.words[3]))
            == 0
    }
}

/// Roles currently accepted by typed hibana projection.
pub const HIBANA_TYPED_ROLE_DOMAIN: RoleSet = RoleSet::from_bits(0xffff);

/// Projected lane set derived from hibana metadata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaneSet {
    words: [u64; 4],
}

impl LaneSet {
    pub const EMPTY: Self = Self { words: [0; 4] };

    pub const fn single(lane: u8) -> Self {
        let word = (lane / 64) as usize;
        let bit = 1u64 << (lane % 64);
        Self {
            words: [
                if word == 0 { bit } else { 0 },
                if word == 1 { bit } else { 0 },
                if word == 2 { bit } else { 0 },
                if word == 3 { bit } else { 0 },
            ],
        }
    }

    pub const fn words(self) -> [u64; 4] {
        self.words
    }

    pub const fn contains(self, lane: u8) -> bool {
        let word = (lane / 64) as usize;
        let bit = 1u64 << (lane % 64);
        (self.words[word] & bit) != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            words: [
                self.words[0] | other.words[0],
                self.words[1] | other.words[1],
                self.words[2] | other.words[2],
                self.words[3] | other.words[3],
            ],
        }
    }

    fn configured_range_end(self) -> u16 {
        let mut lane = 0u16;
        let mut end = 1u16;
        while lane < 256 {
            if self.contains(lane as u8) {
                end = lane + 1;
            }
            lane += 1;
        }
        end
    }
}

/// Borrowed WASI artifact supplied to a capsule run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasiImage<'a> {
    bytes: &'a [u8],
}

impl<'a> WasiImage<'a> {
    pub const fn from_static(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Marker for capsules whose selected logical image embeds no WASI P1 guest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWasi;

/// WASI Preview 1 imports required by a projected choreography.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WasiImports {
    bits: u32,
}

impl WasiImports {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const FD_WRITE: Self = Self { bits: 1 << 0 };
    pub const FD_READ: Self = Self { bits: 1 << 1 };
    pub const FD_FDSTAT_GET: Self = Self { bits: 1 << 2 };
    pub const FD_CLOSE: Self = Self { bits: 1 << 3 };
    pub const CLOCK_RES_GET: Self = Self { bits: 1 << 4 };
    pub const CLOCK_TIME_GET: Self = Self { bits: 1 << 5 };
    pub const POLL_ONEOFF: Self = Self { bits: 1 << 6 };
    pub const RANDOM_GET: Self = Self { bits: 1 << 7 };
    pub const PROC_EXIT: Self = Self { bits: 1 << 8 };
    pub const ARGS_SIZES_GET: Self = Self { bits: 1 << 9 };
    pub const ARGS_GET: Self = Self { bits: 1 << 10 };
    pub const ENVIRON_SIZES_GET: Self = Self { bits: 1 << 11 };
    pub const ENVIRON_GET: Self = Self { bits: 1 << 12 };
    pub const PATH_OPEN: Self = Self { bits: 1 << 13 };
    pub const FD_READDIR: Self = Self { bits: 1 << 14 };

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn contains(self, import: Self) -> bool {
        (self.bits & import.bits) == import.bits
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        (self.bits & !other.bits) == 0
    }
}

/// Runtime placement facts for a capsule.
///
/// Placement decides location, not protocol legality.
pub trait Placement<C: Capsule> {
    fn requested_roles<I>() -> RoleSet
    where
        I: LogicalImage<C>,
    {
        I::REQUESTED_ROLES
    }

    fn role_kind(role: u8) -> RoleKind;
}

/// Resolver registration surface for Capsule-local hibana policy points.
pub trait ResolverRegistry<'cfg, C: Capsule> {
    fn policy<const POLICY: u16, const ROLE: u8>(
        &mut self,
        resolver: hibana::integration::policy::ResolverRef<'cfg>,
    );
}

/// A projectable raw hibana choreography plus its placement and localside code.
pub trait Capsule: Sized {
    type Universe: hibana::integration::runtime::LabelUniverse + Default;
    type Placement: Placement<Self>;
    type Local: Localside<Self>;
    type Report;

    fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe>;

    fn register_resolvers<'cfg, R>(_: &mut R)
    where
        R: ResolverRegistry<'cfg, Self>,
    {
    }
}

/// Runtime artifact bundle. This is a value, not capsule meaning.
pub trait ArtifactBundle<C: Capsule>: Sized {
    fn for_image<I>(&self) -> I::Artifact
    where
        I: LogicalImage<C>,
        Self: ArtifactForImage<C, I>,
    {
        <Self as ArtifactForImage<C, I>>::artifact_for_image(self)
    }
}

impl<C, T> ArtifactBundle<C> for T
where
    C: Capsule,
    T: Sized,
{
}

/// Per-logical-image artifact selection for a runtime bundle.
pub trait ArtifactForImage<C: Capsule, I: LogicalImage<C>> {
    fn artifact_for_image(&self) -> I::Artifact;
}

/// One projection-derived logical site image.
pub trait LogicalImage<C: Capsule>: Sized {
    type Artifact;
    type Exit<R>: FromRunReport<R, Self>;
    type Carrier<'a>: hibana::integration::Transport + 'a
    where
        Self: 'a;

    const IMAGE_ID: ImageId;
    const SITE_ID: SiteId;
    const REQUESTED_ROLES: RoleSet;
    const CARRIER: CarrierKind;
    const PEER_IMAGES: PeerImageSet = PeerImageSet::EMPTY;
    /// Runtime-owned wait fuse for this logical image.
    ///
    /// `0` keeps the fuse disabled. This is an image/attach fact, not
    /// a public timeout or protocol-branch API: endpoint methods still expose only
    /// `flow`/`send`/`recv`/`offer`/`decode`.
    const OPERATIONAL_DEADLINE_TICKS: u32 = 0;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>;
    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> EmbeddedAttachStorageRef<'static>;
    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> WasiGuestStorage<'guest>;
    #[cfg(feature = "wasm-engine-core")]
    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        core::hint::black_box(ROLE);
        BudgetRun::new(1, 0, APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION, 0)
    }
    fn driver_facts() -> DriverFacts<'static> {
        DriverFacts::EMPTY
    }
}

/// Requested roles that were materialized as hibana RoleProgram values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProjectedRoles {
    roles: RoleSet,
    count: u8,
}

impl ProjectedRoles {
    const fn new() -> Self {
        Self {
            roles: RoleSet::EMPTY,
            count: 0,
        }
    }

    const fn roles(self) -> RoleSet {
        self.roles
    }

    const fn count(self) -> u8 {
        self.count
    }
}

/// Consumer of projected hibana role programs for one logical image.
trait ProjectedRoleVisitor<C: Capsule> {
    fn visit<const ROLE: u8>(&mut self, program: hibana::integration::program::RoleProgram<ROLE>);
}

impl<C> ProjectedRoleVisitor<C> for ProjectedRoles
where
    C: Capsule,
{
    fn visit<const ROLE: u8>(&mut self, program: hibana::integration::program::RoleProgram<ROLE>) {
        let role_program_size = core::mem::size_of_val(&program);
        assert!(
            role_program_size > 0,
            "projected RoleProgram witness must be materialized"
        );
        self.roles = self.roles.union(RoleSet::single(ROLE));
        self.count = self.count.saturating_add(1);
    }
}

fn visit_projected_role<C, V, const ROLE: u8>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
    visitor: &mut V,
) where
    C: Capsule,
    V: ProjectedRoleVisitor<C>,
{
    visitor.visit::<ROLE>(program.project::<ROLE>());
}

fn visit_requested_projected_roles<C, V>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
    requested_roles: RoleSet,
    visitor: &mut V,
) where
    C: Capsule,
    V: ProjectedRoleVisitor<C>,
{
    if requested_roles.contains(0) {
        visit_projected_role::<C, V, 0>(program, visitor);
    }
    if requested_roles.contains(1) {
        visit_projected_role::<C, V, 1>(program, visitor);
    }
    if requested_roles.contains(2) {
        visit_projected_role::<C, V, 2>(program, visitor);
    }
    if requested_roles.contains(3) {
        visit_projected_role::<C, V, 3>(program, visitor);
    }
    if requested_roles.contains(4) {
        visit_projected_role::<C, V, 4>(program, visitor);
    }
    if requested_roles.contains(5) {
        visit_projected_role::<C, V, 5>(program, visitor);
    }
    if requested_roles.contains(6) {
        visit_projected_role::<C, V, 6>(program, visitor);
    }
    if requested_roles.contains(7) {
        visit_projected_role::<C, V, 7>(program, visitor);
    }
    if requested_roles.contains(8) {
        visit_projected_role::<C, V, 8>(program, visitor);
    }
    if requested_roles.contains(9) {
        visit_projected_role::<C, V, 9>(program, visitor);
    }
    if requested_roles.contains(10) {
        visit_projected_role::<C, V, 10>(program, visitor);
    }
    if requested_roles.contains(11) {
        visit_projected_role::<C, V, 11>(program, visitor);
    }
    if requested_roles.contains(12) {
        visit_projected_role::<C, V, 12>(program, visitor);
    }
    if requested_roles.contains(13) {
        visit_projected_role::<C, V, 13>(program, visitor);
    }
    if requested_roles.contains(14) {
        visit_projected_role::<C, V, 14>(program, visitor);
    }
    if requested_roles.contains(15) {
        visit_projected_role::<C, V, 15>(program, visitor);
    }
}
fn collect_projected_roles<C, I>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
) -> ProjectedRoles
where
    C: Capsule,
    I: LogicalImage<C>,
{
    let mut projected = ProjectedRoles::new();
    visit_requested_projected_roles::<C, _>(program, I::REQUESTED_ROLES, &mut projected);
    projected
}

/// Conversion from the canonical appkit run report into a logical image exit.
pub trait FromRunReport<R, I> {
    fn from_run_report(report: RunReport<R, I>) -> Self;
}

impl<R, I> FromRunReport<R, I> for RunReport<R, I> {
    fn from_run_report(report: RunReport<R, I>) -> Self {
        report
    }
}

mod artifact_seal {
    pub trait Sealed {}
}

/// Internal artifact evidence consumed by [`run`] while validating a logical image.
///
/// This trait is public only as a sealed generic bound for wrappers that are
/// themselves generic over a capsule. User code should select `WasiImage` or
/// `NoWasi`; it cannot implement new artifact authority.
#[doc(hidden)]
pub trait ArtifactEvidence: artifact_seal::Sealed {
    fn byte_len(&self) -> usize;
    fn wasi_bytes(&self) -> Option<&[u8]>;
    fn validate(&self, required: WasiImports) -> bool;
}

/// Endpoint/carrier facts validated for one logical image run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointCarrierFacts {
    image_id: ImageId,
    site_id: SiteId,
    session_id: u32,
    requested_roles: RoleSet,
    projected_roles: RoleSet,
    lanes: LaneSet,
    carrier: CarrierKind,
    wasi_imports: WasiImports,
    wasi_completion_pair_count: u8,
    has_wasi_import_loop_control: bool,
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasiGuestStatus {
    Done,
    Exit(ProcExitStatus),
    BudgetExpired(BudgetExpired),
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Debug)]
pub enum WasiGuestError {
    NoWasiArtifact,
    GuestRejected(crate::kernel::engine::wasm::Error),
    EndpointRejected(u32),
    Endpoint {
        code: u32,
        source: hibana::EndpointError,
    },
    ProtocolRejected(hibana::integration::wire::CodecError),
    UnexpectedReply,
    UnsupportedGuestEvent,
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestError {
    pub const fn diagnostic_code(&self) -> u32 {
        match self {
            Self::NoWasiArtifact => 0x5745_0001,
            Self::GuestRejected(error) => error.diagnostic_code(),
            Self::EndpointRejected(code) => *code,
            Self::Endpoint { code, .. } => *code,
            Self::ProtocolRejected(_) => 0x5745_0003,
            Self::UnexpectedReply => 0x5745_0004,
            Self::UnsupportedGuestEvent => 0x5745_0005,
        }
    }

    fn endpoint(code: u32, source: hibana::EndpointError) -> Self {
        Self::Endpoint { code, source }
    }
}

#[cfg(feature = "wasm-engine-core")]
impl From<crate::kernel::engine::wasm::Error> for WasiGuestError {
    fn from(error: crate::kernel::engine::wasm::Error) -> Self {
        Self::GuestRejected(error)
    }
}

#[cfg(feature = "wasm-engine-core")]
impl From<hibana::integration::wire::CodecError> for WasiGuestError {
    fn from(error: hibana::integration::wire::CodecError) -> Self {
        Self::ProtocolRejected(error)
    }
}

impl EndpointCarrierFacts {
    const fn new(
        image_id: ImageId,
        site_id: SiteId,
        requested_roles: RoleSet,
        carrier: CarrierKind,
        projection: ProjectionCaps,
    ) -> Self {
        Self {
            image_id,
            site_id,
            session_id: session_id_from_projection(projection),
            requested_roles,
            projected_roles: projection.roles,
            lanes: projection.lanes,
            carrier,
            wasi_imports: projection.wasi_imports,
            wasi_completion_pair_count: projection.wasi_completion_pair_count,
            has_wasi_import_loop_control: projection_has_label(
                projection,
                LABEL_WASI_IMPORT_LOOP_CONTINUE_CONTROL,
            ) && projection_has_label(
                projection,
                LABEL_WASI_IMPORT_LOOP_BREAK_CONTROL,
            ),
        }
    }

    pub const fn image_id(self) -> ImageId {
        self.image_id
    }

    pub const fn site_id(self) -> SiteId {
        self.site_id
    }

    pub const fn session_id(self) -> u32 {
        self.session_id
    }

    pub const fn requested_roles(self) -> RoleSet {
        self.requested_roles
    }

    pub const fn projected_roles(self) -> RoleSet {
        self.projected_roles
    }

    pub const fn lanes(self) -> LaneSet {
        self.lanes
    }

    pub const fn carrier(self) -> CarrierKind {
        self.carrier
    }

    pub const fn wasi_imports(self) -> WasiImports {
        self.wasi_imports
    }

    pub const fn wasi_completion_pair_count(self) -> u8 {
        self.wasi_completion_pair_count
    }
}

const fn projection_has_label(projection: ProjectionCaps, label: u8) -> bool {
    let mut idx = 0usize;
    while idx < projection.label_count as usize {
        if projection.labels[idx] == label {
            return true;
        }
        idx += 1;
    }
    false
}

const fn session_id_from_projection(projection: ProjectionCaps) -> u32 {
    let mixed = projection.fingerprint[0] ^ projection.fingerprint[1].rotate_left(17);
    let folded = (mixed as u32) ^ ((mixed >> 32) as u32);
    if folded == 0 { 1 } else { folded }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactError {
    BadWasmMagic,
    MalformedWasm,
    UnsupportedWasiImport,
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    let mut idx = 0usize;
    while idx + needle.len() <= haystack.len() {
        if &haystack[idx..idx + needle.len()] == needle {
            return true;
        }
        idx += 1;
    }
    false
}

fn wasi_import_for_engine_req_label(label: u8) -> Option<WasiImports> {
    match label {
        LABEL_WASIP1_STDOUT | LABEL_WASIP1_STDERR | LABEL_WASI_FD_WRITE => {
            Some(WasiImports::FD_WRITE)
        }
        LABEL_WASIP1_STDIN | LABEL_WASI_FD_READ => Some(WasiImports::FD_READ),
        LABEL_WASI_FD_FDSTAT_GET => Some(WasiImports::FD_FDSTAT_GET),
        LABEL_WASI_FD_CLOSE => Some(WasiImports::FD_CLOSE),
        LABEL_WASI_CLOCK_RES_GET => Some(WasiImports::CLOCK_RES_GET),
        LABEL_WASIP1_CLOCK_NOW | LABEL_WASI_CLOCK_TIME_GET => Some(WasiImports::CLOCK_TIME_GET),
        LABEL_WASI_POLL_ONEOFF => Some(WasiImports::POLL_ONEOFF),
        LABEL_WASIP1_RANDOM_SEED | LABEL_WASI_RANDOM_GET => Some(WasiImports::RANDOM_GET),
        LABEL_WASIP1_EXIT | LABEL_WASI_PROC_EXIT => Some(WasiImports::PROC_EXIT),
        LABEL_WASI_ARGS_SIZES_GET => Some(WasiImports::ARGS_SIZES_GET),
        LABEL_WASI_ARGS_GET => Some(WasiImports::ARGS_GET),
        LABEL_WASI_ENVIRON_SIZES_GET => Some(WasiImports::ENVIRON_SIZES_GET),
        LABEL_WASI_ENVIRON_GET => Some(WasiImports::ENVIRON_GET),
        LABEL_WASI_PATH_OPEN => Some(WasiImports::PATH_OPEN),
        LABEL_WASI_FD_READDIR => Some(WasiImports::FD_READDIR),
        _ => None,
    }
}

fn wasi_completion_label_for_engine_req_label(label: u8) -> Option<u8> {
    match label {
        LABEL_WASIP1_STDOUT => Some(LABEL_WASIP1_STDOUT_RET),
        LABEL_WASIP1_STDERR => Some(LABEL_WASIP1_STDERR_RET),
        LABEL_WASIP1_STDIN => Some(LABEL_WASIP1_STDIN_RET),
        LABEL_WASIP1_CLOCK_NOW => Some(LABEL_WASIP1_CLOCK_NOW_RET),
        LABEL_WASIP1_RANDOM_SEED => Some(LABEL_WASIP1_RANDOM_SEED_RET),
        LABEL_WASI_FD_WRITE => Some(LABEL_WASI_FD_WRITE_RET),
        LABEL_WASI_FD_READ => Some(LABEL_WASI_FD_READ_RET),
        LABEL_WASI_FD_FDSTAT_GET => Some(LABEL_WASI_FD_FDSTAT_GET_RET),
        LABEL_WASI_FD_CLOSE => Some(LABEL_WASI_FD_CLOSE_RET),
        LABEL_WASI_CLOCK_RES_GET => Some(LABEL_WASI_CLOCK_RES_GET_RET),
        LABEL_WASI_CLOCK_TIME_GET => Some(LABEL_WASI_CLOCK_TIME_GET_RET),
        LABEL_WASI_POLL_ONEOFF => Some(LABEL_WASI_POLL_ONEOFF_RET),
        LABEL_WASI_RANDOM_GET => Some(LABEL_WASI_RANDOM_GET_RET),
        LABEL_WASI_ARGS_SIZES_GET => Some(LABEL_WASI_ARGS_SIZES_GET_RET),
        LABEL_WASI_ARGS_GET => Some(LABEL_WASI_ARGS_GET_RET),
        LABEL_WASI_ENVIRON_SIZES_GET => Some(LABEL_WASI_ENVIRON_SIZES_GET_RET),
        LABEL_WASI_ENVIRON_GET => Some(LABEL_WASI_ENVIRON_GET_RET),
        LABEL_WASI_PATH_OPEN => Some(LABEL_WASI_PATH_OPEN_RET),
        LABEL_WASI_FD_READDIR => Some(LABEL_WASI_FD_READDIR_RET),
        LABEL_WASIP1_EXIT | LABEL_WASI_PROC_EXIT => None,
        _ => None,
    }
}

fn import_name_to_wasi(name: &[u8]) -> Option<WasiImports> {
    match name {
        b"fd_write" => Some(WasiImports::FD_WRITE),
        b"fd_read" => Some(WasiImports::FD_READ),
        b"fd_fdstat_get" => Some(WasiImports::FD_FDSTAT_GET),
        b"fd_close" => Some(WasiImports::FD_CLOSE),
        b"clock_res_get" => Some(WasiImports::CLOCK_RES_GET),
        b"clock_time_get" => Some(WasiImports::CLOCK_TIME_GET),
        b"poll_oneoff" => Some(WasiImports::POLL_ONEOFF),
        b"random_get" => Some(WasiImports::RANDOM_GET),
        b"proc_exit" => Some(WasiImports::PROC_EXIT),
        b"args_sizes_get" => Some(WasiImports::ARGS_SIZES_GET),
        b"args_get" => Some(WasiImports::ARGS_GET),
        b"environ_sizes_get" => Some(WasiImports::ENVIRON_SIZES_GET),
        b"environ_get" => Some(WasiImports::ENVIRON_GET),
        b"path_open" => Some(WasiImports::PATH_OPEN),
        b"fd_readdir" => Some(WasiImports::FD_READDIR),
        _ => None,
    }
}

fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, ArtifactError> {
    if *cursor >= bytes.len() {
        return Err(ArtifactError::MalformedWasm);
    }
    let value = bytes[*cursor];
    *cursor += 1;
    Ok(value)
}

fn read_leb_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ArtifactError> {
    let mut result = 0u32;
    let mut shift = 0u32;
    let mut count = 0u8;
    while count < 5 {
        let byte = read_byte(bytes, cursor)?;
        result |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        count += 1;
    }
    Err(ArtifactError::MalformedWasm)
}

fn read_name<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], ArtifactError> {
    let len = read_leb_u32(bytes, cursor)? as usize;
    let end = cursor
        .checked_add(len)
        .ok_or(ArtifactError::MalformedWasm)?;
    if end > bytes.len() {
        return Err(ArtifactError::MalformedWasm);
    }
    let name = &bytes[*cursor..end];
    *cursor = end;
    Ok(name)
}

fn skip_import_desc(bytes: &[u8], cursor: &mut usize, kind: u8) -> Result<(), ArtifactError> {
    match kind {
        0x00 => {
            read_leb_u32(bytes, cursor)?;
        }
        0x01 => {
            read_byte(bytes, cursor)?;
            read_leb_u32(bytes, cursor)?;
            let flags = read_leb_u32(bytes, cursor)?;
            read_leb_u32(bytes, cursor)?;
            if flags & 0x01 != 0 {
                read_leb_u32(bytes, cursor)?;
            }
        }
        0x02 => {
            let flags = read_leb_u32(bytes, cursor)?;
            read_leb_u32(bytes, cursor)?;
            if flags & 0x01 != 0 {
                read_leb_u32(bytes, cursor)?;
            }
        }
        0x03 => {
            read_byte(bytes, cursor)?;
            read_byte(bytes, cursor)?;
        }
        _ => return Err(ArtifactError::MalformedWasm),
    }
    Ok(())
}

fn parse_wasip1_imports(bytes: &[u8]) -> Result<WasiImports, ArtifactError> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" || &bytes[4..8] != b"\x01\0\0\0" {
        return Err(ArtifactError::BadWasmMagic);
    }

    let mut cursor = 8usize;
    let mut imports = WasiImports::EMPTY;
    while cursor < bytes.len() {
        let section_id = read_byte(bytes, &mut cursor)?;
        let section_size = read_leb_u32(bytes, &mut cursor)? as usize;
        let section_end = cursor
            .checked_add(section_size)
            .ok_or(ArtifactError::MalformedWasm)?;
        if section_end > bytes.len() {
            return Err(ArtifactError::MalformedWasm);
        }
        if section_id == 2 {
            let mut section_cursor = cursor;
            let count = read_leb_u32(bytes, &mut section_cursor)?;
            let mut item = 0u32;
            while item < count {
                let module = read_name(bytes, &mut section_cursor)?;
                let name = read_name(bytes, &mut section_cursor)?;
                let kind = read_byte(bytes, &mut section_cursor)?;
                if module == b"wasi_snapshot_preview1" {
                    if kind != 0 {
                        return Err(ArtifactError::UnsupportedWasiImport);
                    }
                    let import =
                        import_name_to_wasi(name).ok_or(ArtifactError::UnsupportedWasiImport)?;
                    imports = imports.union(import);
                }
                skip_import_desc(bytes, &mut section_cursor, kind)?;
                item += 1;
            }
            if section_cursor != section_end {
                return Err(ArtifactError::MalformedWasm);
            }
        }
        cursor = section_end;
    }
    Ok(imports)
}

impl ArtifactEvidence for WasiImage<'_> {
    fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    fn wasi_bytes(&self) -> Option<&[u8]> {
        Some(self.bytes)
    }

    fn validate(&self, required: WasiImports) -> bool {
        if self.bytes.is_empty() {
            return false;
        }
        if !self.bytes.starts_with(b"\0asm") {
            return false;
        }
        if !contains_bytes(self.bytes, b"wasi_snapshot_preview1") {
            return false;
        }
        for forbidden in [
            b"wasi_snapshot_preview2".as_slice(),
            b"wasm32-wasip2".as_slice(),
            b"wit-bindgen".as_slice(),
            b"wit_component".as_slice(),
            b"component-model".as_slice(),
        ] {
            if contains_bytes(self.bytes, forbidden) {
                return false;
            }
        }
        let Ok(observed) = parse_wasip1_imports(self.bytes) else {
            return false;
        };
        if observed.is_empty() {
            return false;
        }
        if !required.is_subset_of(observed) {
            return false;
        }
        true
    }
}

impl artifact_seal::Sealed for WasiImage<'_> {}

impl ArtifactEvidence for NoWasi {
    fn byte_len(&self) -> usize {
        0
    }

    fn wasi_bytes(&self) -> Option<&[u8]> {
        None
    }

    fn validate(&self, required: WasiImports) -> bool {
        required.is_empty()
    }
}

impl artifact_seal::Sealed for NoWasi {}

/// Metadata-derived capacity facts for a capsule program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectionCaps {
    pub roles: RoleSet,
    pub lanes: LaneSet,
    pub labels: [u8; 32],
    pub label_count: u8,
    pub policies: [u16; 16],
    pub control_ops: [u8; 16],
    pub control_tap_ids: [u16; 16],
    pub control_count: u16,
    pub wasi_imports: WasiImports,
    pub wasi_completion_pair_count: u8,
    pub role_count: u8,
    pub eff_count: u16,
    pub scope_count: u16,
    pub route_scope_count: u16,
    pub fingerprint: [u64; 2],
    pub policy_count: u16,
    pub has_parallel: bool,
    pub has_policy: bool,
    pub has_control: bool,
}

struct ProjectionCapsVisitor {
    caps: ProjectionCaps,
    import_roles: Option<RoleSet>,
    engine_req_labels: [u8; 32],
    engine_req_label_count: u8,
    engine_ret_labels: [u8; 32],
    engine_ret_label_count: u8,
}

impl ProjectionCapsVisitor {
    const fn new() -> Self {
        Self {
            caps: ProjectionCaps {
                roles: RoleSet::EMPTY,
                lanes: LaneSet::EMPTY,
                labels: [0; 32],
                label_count: 0,
                policies: [0; 16],
                control_ops: [0; 16],
                control_tap_ids: [0; 16],
                control_count: 0,
                wasi_imports: WasiImports::EMPTY,
                wasi_completion_pair_count: 0,
                role_count: 0,
                eff_count: 0,
                scope_count: 0,
                route_scope_count: 0,
                fingerprint: [0; 2],
                policy_count: 0,
                has_parallel: false,
                has_policy: false,
                has_control: false,
            },
            import_roles: None,
            engine_req_labels: [0; 32],
            engine_req_label_count: 0,
            engine_ret_labels: [0; 32],
            engine_ret_label_count: 0,
        }
    }

    const fn for_import_roles(import_roles: RoleSet) -> Self {
        Self {
            caps: ProjectionCaps {
                roles: RoleSet::EMPTY,
                lanes: LaneSet::EMPTY,
                labels: [0; 32],
                label_count: 0,
                policies: [0; 16],
                control_ops: [0; 16],
                control_tap_ids: [0; 16],
                control_count: 0,
                wasi_imports: WasiImports::EMPTY,
                wasi_completion_pair_count: 0,
                role_count: 0,
                eff_count: 0,
                scope_count: 0,
                route_scope_count: 0,
                fingerprint: [0; 2],
                policy_count: 0,
                has_parallel: false,
                has_policy: false,
                has_control: false,
            },
            import_roles: Some(import_roles),
            engine_req_labels: [0; 32],
            engine_req_label_count: 0,
            engine_ret_labels: [0; 32],
            engine_ret_label_count: 0,
        }
    }

    fn push_label(&mut self, label: u8) {
        let mut idx = 0usize;
        while idx < self.caps.label_count as usize {
            if self.caps.labels[idx] == label {
                return;
            }
            idx += 1;
        }
        if idx < self.caps.labels.len() {
            self.caps.labels[idx] = label;
            self.caps.label_count += 1;
        }
    }

    fn push_policy(&mut self, policy_id: u16) {
        let mut idx = 0usize;
        while idx < self.caps.policy_count as usize {
            if self.caps.policies[idx] == policy_id {
                return;
            }
            idx += 1;
        }
        if idx < self.caps.policies.len() {
            self.caps.policies[idx] = policy_id;
            self.caps.policy_count += 1;
        }
    }

    fn push_control(&mut self, op: u8, tap_id: u16) {
        let mut idx = 0usize;
        while idx < self.caps.control_count as usize {
            if self.caps.control_ops[idx] == op && self.caps.control_tap_ids[idx] == tap_id {
                return;
            }
            idx += 1;
        }
        if idx < self.caps.control_ops.len() {
            self.caps.control_ops[idx] = op;
            self.caps.control_tap_ids[idx] = tap_id;
            self.caps.control_count += 1;
        }
    }

    fn push_engine_req_label(&mut self, label: u8) {
        let mut idx = 0usize;
        while idx < self.engine_req_label_count as usize {
            if self.engine_req_labels[idx] == label {
                return;
            }
            idx += 1;
        }
        if idx < self.engine_req_labels.len() {
            self.engine_req_labels[idx] = label;
            self.engine_req_label_count += 1;
        }
    }

    fn push_engine_ret_label(&mut self, label: u8) {
        let mut idx = 0usize;
        while idx < self.engine_ret_label_count as usize {
            if self.engine_ret_labels[idx] == label {
                return;
            }
            idx += 1;
        }
        if idx < self.engine_ret_labels.len() {
            self.engine_ret_labels[idx] = label;
            self.engine_ret_label_count += 1;
        }
    }

    fn has_engine_ret_label(&self, label: u8) -> bool {
        let mut idx = 0usize;
        while idx < self.engine_ret_label_count as usize {
            if self.engine_ret_labels[idx] == label {
                return true;
            }
            idx += 1;
        }
        false
    }

    fn wasi_completion_pair_count(&self) -> u8 {
        let mut count = 0u8;
        let mut idx = 0usize;
        while idx < self.engine_req_label_count as usize {
            let label = self.engine_req_labels[idx];
            if wasi_import_for_engine_req_label(label).is_some() {
                if let Some(reply_label) = wasi_completion_label_for_engine_req_label(label) {
                    assert!(
                        self.has_engine_ret_label(reply_label),
                        "WASI P1 import request label must have a projected typed EngineRet completion"
                    );
                    count = count.saturating_add(1);
                }
            }
            idx += 1;
        }
        count
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AttachSummary {
    endpoint_count: u8,
    role_kinds: RoleKindCounts,
}

#[cfg(all(not(test), target_os = "none"))]
#[cold]
fn panic_appkit_attach_role_error<const ROLE: u8>(error: hibana::integration::AttachError) -> ! {
    panic!("appkit embedded role {ROLE} attach error: {error:?}")
}

#[cfg(any(test, not(target_os = "none")))]
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct ScheduledTaskArena {
    bytes: [u8; APPKIT_ROLE_FUTURE_BYTES],
}

#[cfg(any(test, not(target_os = "none")))]
impl ScheduledTaskArena {
    const EMPTY: Self = Self {
        bytes: [0; APPKIT_ROLE_FUTURE_BYTES],
    };

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }
}

#[cfg(any(test, not(target_os = "none")))]
type ScheduledTaskPoll<E> = unsafe fn(*mut u8, &mut Context<'_>) -> Poll<RoleResult<E>>;

#[cfg(any(test, not(target_os = "none")))]
type ScheduledTaskDrop = unsafe fn(*mut u8);

#[cfg(any(test, not(target_os = "none")))]
unsafe fn poll_scheduled_task<F, E>(ptr: *mut u8, cx: &mut Context<'_>) -> Poll<RoleResult<E>>
where
    F: Future<Output = RoleResult<E>>,
{
    let future = unsafe {
        // SAFETY: `ScheduledTaskSlot::push` wrote an initialized `F` into this
        // aligned arena and keeps it alive while the slot is active.
        &mut *ptr.cast::<F>()
    };
    let mut pinned = unsafe {
        // SAFETY: The future remains in the fixed arena until it is dropped by
        // the slot, so pinning it in place is valid.
        Pin::new_unchecked(future)
    };
    pinned.as_mut().poll(cx)
}

#[cfg(any(test, not(target_os = "none")))]
unsafe fn drop_scheduled_task<F>(ptr: *mut u8)
where
    F: Future,
{
    unsafe {
        // SAFETY: `ScheduledTaskSlot::push` initialized this arena with an `F`
        // and active slots are dropped exactly once by `ScheduledTasks`.
        core::ptr::drop_in_place(ptr.cast::<F>());
    }
}

#[cfg(any(test, not(target_os = "none")))]
unsafe fn wake_flag_clone(data: *const ()) -> RawWaker {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    RawWaker::new(data, &WAKE_FLAG_WAKER_VTABLE)
}

#[cfg(any(test, not(target_os = "none")))]
unsafe fn wake_flag_wake(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    unsafe {
        *data.cast_mut().cast::<bool>() = true;
    }
}

#[cfg(any(test, not(target_os = "none")))]
unsafe fn wake_flag_wake_by_ref(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    unsafe {
        *data.cast_mut().cast::<bool>() = true;
    }
}

#[cfg(any(test, not(target_os = "none")))]
unsafe fn wake_flag_drop(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
}

#[cfg(any(test, not(target_os = "none")))]
static WAKE_FLAG_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    wake_flag_clone,
    wake_flag_wake,
    wake_flag_wake_by_ref,
    wake_flag_drop,
);

#[cfg(any(test, not(target_os = "none")))]
#[derive(Clone, Copy)]
struct ScheduledTaskSlot<'task, E> {
    arena: ScheduledTaskArena,
    poll: Option<ScheduledTaskPoll<E>>,
    drop_task: Option<ScheduledTaskDrop>,
    active: bool,
    lifetime: PhantomData<&'task mut ()>,
}

#[cfg(any(test, not(target_os = "none")))]
impl<'task, E> ScheduledTaskSlot<'task, E> {
    const EMPTY: Self = Self {
        arena: ScheduledTaskArena::EMPTY,
        poll: None,
        drop_task: None,
        active: false,
        lifetime: PhantomData,
    };

    fn push<F>(&mut self, future: F)
    where
        F: Future<Output = RoleResult<E>> + 'task,
    {
        assert!(
            !self.active,
            "appkit fixed scheduler slot must be empty before use"
        );
        assert!(
            size_of::<F>() <= APPKIT_ROLE_FUTURE_BYTES,
            "appkit role future exceeds fixed scheduler arena"
        );
        assert!(
            align_of::<F>() <= APPKIT_ROLE_FUTURE_ALIGN,
            "appkit role future alignment exceeds fixed scheduler arena"
        );
        unsafe {
            // SAFETY: The fixed arena is aligned to APPKIT_ROLE_FUTURE_ALIGN,
            // the size/alignment checks above prove it can hold F, and the
            // slot records the matching poll/drop vtable immediately after.
            self.arena.as_mut_ptr().cast::<F>().write(future);
        }
        self.poll = Some(poll_scheduled_task::<F, E>);
        self.drop_task = Some(drop_scheduled_task::<F>);
        self.active = true;
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<RoleResult<E>> {
        assert!(self.active, "appkit scheduler polled an inactive slot");
        let poll = self
            .poll
            .expect("appkit scheduler active slot must have a poll function");
        unsafe {
            // SAFETY: Active slots contain the future type associated with
            // their stored poll function.
            poll(self.arena.as_mut_ptr(), cx)
        }
    }

    fn drop_active(&mut self) {
        if !self.active {
            return;
        }
        let drop_task = self
            .drop_task
            .expect("appkit scheduler active slot must have a drop function");
        unsafe {
            // SAFETY: Active slots contain the future type associated with
            // their stored drop function and are deactivated immediately.
            drop_task(self.arena.as_mut_ptr());
        }
        self.poll = None;
        self.drop_task = None;
        self.active = false;
    }
}

#[cfg(any(test, not(target_os = "none")))]
struct ScheduledTasks<'task, E> {
    slots: [ScheduledTaskSlot<'task, E>; APPKIT_CARRIER_ROLES],
    len: usize,
}

#[cfg(any(test, not(target_os = "none")))]
impl<'task, E> ScheduledTasks<'task, E>
where
    E: Debug,
{
    fn new() -> Self {
        Self {
            slots: [ScheduledTaskSlot::EMPTY; APPKIT_CARRIER_ROLES],
            len: 0,
        }
    }

    fn push<F>(&mut self, future: F)
    where
        F: Future<Output = RoleResult<E>> + 'task,
    {
        assert!(
            self.len < self.slots.len(),
            "appkit fixed scheduler has no free role slot"
        );
        self.slots[self.len].push(future);
        self.len += 1;
    }

    fn poll_until_quiescent(&mut self) {
        let mut woke = true;
        while woke {
            woke = false;
            let raw_waker = RawWaker::new(
                core::ptr::addr_of_mut!(woke).cast::<()>(),
                &WAKE_FLAG_WAKER_VTABLE,
            );
            let waker = unsafe {
                // SAFETY: The raw waker points to `woke`, which lives for this
                // poll pass. The vtable only writes `true` to that flag.
                Waker::from_raw(raw_waker)
            };
            let mut task_context = Context::from_waker(&waker);
            let mut task_idx = 0usize;
            while task_idx < self.len {
                match self.slots[task_idx].poll(&mut task_context) {
                    Poll::Pending => {}
                    Poll::Ready(Ok(done)) => match done {},
                    Poll::Ready(Err(error)) => {
                        panic!("appkit role task failed: {error:?}");
                    }
                }
                task_idx += 1;
            }
        }
    }
}

#[cfg(any(test, not(target_os = "none")))]
impl<E> Drop for ScheduledTasks<'_, E> {
    fn drop(&mut self) {
        let mut task_idx = 0usize;
        while task_idx < self.len {
            self.slots[task_idx].drop_active();
            task_idx += 1;
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
#[cold]
fn panic_appkit_resolver_error<const POLICY: u16, const ROLE: u8>(
    error: hibana::integration::policy::ResolverError,
) -> ! {
    panic!("appkit resolver registration failed: policy={POLICY} role={ROLE} error={error:?}")
}

struct AttachResolverRegistry<
    'kit,
    'prog,
    'cfg,
    C,
    ProgramTy,
    TransportTy,
    UniverseTy,
    ClockTy,
    const MAX_RV: usize,
> where
    C: Capsule,
    ProgramTy: hibana::integration::program::Projectable<C::Universe> + ?Sized,
    TransportTy: hibana::integration::Transport + 'cfg,
    UniverseTy: hibana::integration::runtime::LabelUniverse + 'cfg,
    ClockTy: hibana::integration::runtime::Clock + 'cfg,
{
    kit: &'kit hibana::integration::SessionKit<'cfg, TransportTy, UniverseTy, ClockTy, MAX_RV>,
    rendezvous: hibana::integration::ids::RendezvousId,
    program: &'prog ProgramTy,
    requested_roles: RoleSet,
    capsule: PhantomData<C>,
}

impl<'kit, 'prog, 'cfg, C, ProgramTy, TransportTy, UniverseTy, ClockTy, const MAX_RV: usize>
    ResolverRegistry<'cfg, C>
    for AttachResolverRegistry<
        'kit,
        'prog,
        'cfg,
        C,
        ProgramTy,
        TransportTy,
        UniverseTy,
        ClockTy,
        MAX_RV,
    >
where
    C: Capsule,
    ProgramTy: hibana::integration::program::Projectable<C::Universe> + ?Sized,
    TransportTy: hibana::integration::Transport + 'cfg,
    UniverseTy: hibana::integration::runtime::LabelUniverse + 'cfg,
    ClockTy: hibana::integration::runtime::Clock + 'cfg,
{
    fn policy<const POLICY: u16, const ROLE: u8>(
        &mut self,
        resolver: hibana::integration::policy::ResolverRef<'cfg>,
    ) {
        if !self.requested_roles.contains(ROLE) {
            return;
        }
        let role_program = self.program.project::<ROLE>();
        if let Err(error) =
            self.kit
                .set_resolver::<POLICY, ROLE>(self.rendezvous, &role_program, resolver)
        {
            #[cfg(any(test, not(target_os = "none")))]
            panic!(
                "appkit resolver registration failed: policy={POLICY} role={ROLE} error={error:?}"
            );
            #[cfg(all(not(test), target_os = "none"))]
            panic_appkit_resolver_error::<POLICY, ROLE>(error);
        }
    }
}

struct AttachProjectedRoles<
    'kit,
    'tasks,
    'cfg,
    'guest,
    C,
    ImageTy,
    TransportTy,
    UniverseTy,
    ClockTy,
    const MAX_RV: usize,
> where
    C: Capsule,
    TransportTy: hibana::integration::Transport + 'cfg,
    UniverseTy: hibana::integration::runtime::LabelUniverse + 'cfg,
    ClockTy: hibana::integration::runtime::Clock + 'cfg,
{
    kit: &'kit hibana::integration::SessionKit<'cfg, TransportTy, UniverseTy, ClockTy, MAX_RV>,
    rendezvous: hibana::integration::ids::RendezvousId,
    session: hibana::integration::ids::SessionId,
    endpoint_carrier: EndpointCarrierFacts,
    wasi_guest_bytes: Option<&'guest [u8]>,
    driver_facts: DriverFacts<'static>,
    count: u8,
    role_kinds: RoleKindCounts,
    tasks_lifetime: PhantomData<&'tasks mut ()>,
    capsule_lifetime: PhantomData<C>,
    image_lifetime: PhantomData<ImageTy>,
    #[cfg(any(test, not(target_os = "none")))]
    tasks: &'tasks mut ScheduledTasks<'kit, RoleTaskError<<C::Local as Localside<C>>::Error>>,
}

impl<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, TransportTy, UniverseTy, ClockTy, const MAX_RV: usize>
    ProjectedRoleVisitor<C>
    for AttachProjectedRoles<
        'kit,
        'tasks,
        'cfg,
        'guest,
        C,
        ImageTy,
        TransportTy,
        UniverseTy,
        ClockTy,
        MAX_RV,
    >
where
    C: Capsule + 'kit,
    C::Local: 'kit,
    ImageTy: LogicalImage<C> + 'kit,
    TransportTy: hibana::integration::Transport + 'cfg,
    UniverseTy: hibana::integration::runtime::LabelUniverse + 'cfg,
    ClockTy: hibana::integration::runtime::Clock + 'cfg,
    'cfg: 'kit,
    'guest: 'kit,
{
    fn visit<const ROLE: u8>(&mut self, program: hibana::integration::program::RoleProgram<ROLE>) {
        let endpoint = match self.kit.enter::<ROLE, _>(
            self.rendezvous,
            self.session,
            &program,
            hibana::integration::binding::NoBinding,
        ) {
            Ok(endpoint) => endpoint,
            #[cfg(any(test, not(target_os = "none")))]
            Err(error) => panic!("projected role must attach through SessionKit: {error:?}"),
            #[cfg(all(not(test), target_os = "none"))]
            Err(error) => panic_appkit_attach_role_error::<ROLE>(error),
        };
        let endpoint_ctx = RoleEndpointCtx::<C, ROLE>::new(endpoint);
        assert_eq!(
            endpoint_ctx.role(),
            ROLE,
            "attached endpoint context role mismatch"
        );
        let role_kind = C::Placement::role_kind(ROLE);
        self.role_kinds.record(role_kind);
        match role_kind {
            RoleKind::Engine => {
                #[cfg(feature = "wasm-engine-core")]
                let ctx = EngineCtx::new(
                    endpoint_ctx,
                    self.endpoint_carrier,
                    self.wasi_guest_bytes,
                    ImageTy::wasi_guest_storage::<ROLE>(),
                );
                #[cfg(not(feature = "wasm-engine-core"))]
                let ctx =
                    EngineCtx::new(endpoint_ctx, self.endpoint_carrier, self.wasi_guest_bytes);
                #[cfg(feature = "wasm-engine-core")]
                {
                    if self.wasi_guest_bytes.is_some() {
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks
                            .push(wasi_role_task::<_, <C::Local as Localside<C>>::Error>(
                                drive_canonical_wasi_engine::<C, ImageTy, ROLE>(ctx),
                            ));
                        #[cfg(all(not(test), target_os = "none"))]
                        poll_embedded_role_future::<
                            ROLE,
                            _,
                            RoleTaskError<<C::Local as Localside<C>>::Error>,
                        >(
                            ImageTy::REQUESTED_ROLES,
                            wasi_role_task::<_, <C::Local as Localside<C>>::Error>(
                                drive_canonical_wasi_engine::<C, ImageTy, ROLE>(ctx),
                            ),
                        );
                    } else {
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks.push(local_role_task(
                            <C::Local as Localside<C>>::engine::<ROLE>(ctx),
                        ));
                        #[cfg(all(not(test), target_os = "none"))]
                        poll_embedded_role_future::<
                            ROLE,
                            _,
                            RoleTaskError<<C::Local as Localside<C>>::Error>,
                        >(
                            ImageTy::REQUESTED_ROLES,
                            local_role_task(<C::Local as Localside<C>>::engine::<ROLE>(ctx)),
                        );
                    }
                }
                #[cfg(not(feature = "wasm-engine-core"))]
                {
                    assert!(
                        self.wasi_guest_bytes.is_none(),
                        "WASI P1 logical image requires wasm-engine-core"
                    );
                    #[cfg(any(test, not(target_os = "none")))]
                    self.tasks
                        .push(local_role_task(<C::Local as Localside<C>>::engine::<ROLE>(
                            ctx,
                        )));
                    #[cfg(all(not(test), target_os = "none"))]
                    poll_embedded_role_future::<
                        ROLE,
                        _,
                        RoleTaskError<<C::Local as Localside<C>>::Error>,
                    >(
                        ImageTy::REQUESTED_ROLES,
                        local_role_task(<C::Local as Localside<C>>::engine::<ROLE>(ctx)),
                    );
                }
            }
            RoleKind::Driver => {
                let ctx = DriverCtx::new(endpoint_ctx, self.endpoint_carrier, self.driver_facts);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks
                    .push(local_role_task(<C::Local as Localside<C>>::driver::<ROLE>(
                        ctx,
                    )));
                #[cfg(all(not(test), target_os = "none"))]
                poll_embedded_role_future::<
                    ROLE,
                    _,
                    RoleTaskError<<C::Local as Localside<C>>::Error>,
                >(
                    ImageTy::REQUESTED_ROLES,
                    local_role_task(<C::Local as Localside<C>>::driver::<ROLE>(ctx)),
                );
            }
            RoleKind::Boundary => {
                let ctx = BoundaryCtx::new(endpoint_ctx, self.endpoint_carrier);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks.push(local_role_task(
                    <C::Local as Localside<C>>::boundary::<ROLE>(ctx),
                ));
                #[cfg(all(not(test), target_os = "none"))]
                poll_embedded_role_future::<
                    ROLE,
                    _,
                    RoleTaskError<<C::Local as Localside<C>>::Error>,
                >(
                    ImageTy::REQUESTED_ROLES,
                    local_role_task(<C::Local as Localside<C>>::boundary::<ROLE>(ctx)),
                );
            }
            RoleKind::Link => {
                let ctx = LinkCtx::new(endpoint_ctx, self.endpoint_carrier);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks
                    .push(local_role_task(<C::Local as Localside<C>>::link::<ROLE>(
                        ctx,
                    )));
                #[cfg(all(not(test), target_os = "none"))]
                poll_embedded_role_future::<
                    ROLE,
                    _,
                    RoleTaskError<<C::Local as Localside<C>>::Error>,
                >(
                    ImageTy::REQUESTED_ROLES,
                    local_role_task(<C::Local as Localside<C>>::link::<ROLE>(ctx)),
                );
            }
            RoleKind::Supervisor => {
                let ctx = SupervisorCtx::new(endpoint_ctx, self.endpoint_carrier);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks
                    .push(local_role_task(<C::Local as Localside<C>>::supervisor::<
                        ROLE,
                    >(ctx)));
                #[cfg(all(not(test), target_os = "none"))]
                poll_embedded_role_future::<
                    ROLE,
                    _,
                    RoleTaskError<<C::Local as Localside<C>>::Error>,
                >(
                    ImageTy::REQUESTED_ROLES,
                    local_role_task(<C::Local as Localside<C>>::supervisor::<ROLE>(ctx)),
                );
            }
        }
        self.count = self.count.saturating_add(1);
    }
}

#[cfg(all(not(test), target_os = "none"))]
fn embedded_attach_storage<C, I>() -> EmbeddedAttachStorageRef<'static>
where
    C: Capsule,
    I: LogicalImage<C>,
{
    I::attach_storage()
}

fn attach_projected_roles<C, I>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
    endpoint_carrier: EndpointCarrierFacts,
    wasi_guest_bytes: Option<&[u8]>,
) -> AttachSummary
where
    C: Capsule,
    I: LogicalImage<C>,
{
    #[cfg(any(test, not(target_os = "none")))]
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
    #[cfg(any(test, not(target_os = "none")))]
    let mut slab_storage = [0u8; APPKIT_ATTACH_SLAB_BYTES];
    #[cfg(all(not(test), target_os = "none"))]
    let embedded_storage = embedded_attach_storage::<C, I>();
    #[cfg(all(not(test), target_os = "none"))]
    let attach_tap = embedded_storage.tap;
    #[cfg(all(not(test), target_os = "none"))]
    let attach_slab = embedded_storage.slab;
    #[cfg(any(test, not(target_os = "none")))]
    let attach_tap = &mut tap_buf;
    #[cfg(any(test, not(target_os = "none")))]
    let attach_slab = &mut slab_storage[..];
    let clock = hibana::integration::runtime::CounterClock::new();
    let carrier = I::carrier();
    let (kit_storage, rendezvous_slab) = carve_session_kit_storage::<
        I::Carrier<'_>,
        C::Universe,
        hibana::integration::runtime::CounterClock,
        APPKIT_SESSION_RV_SLOTS,
    >(attach_slab);
    let kit = hibana::integration::SessionKit::<
        I::Carrier<'_>,
        C::Universe,
        hibana::integration::runtime::CounterClock,
        APPKIT_SESSION_RV_SLOTS,
    >::init_in_place(kit_storage, &clock);
    let lane_range_end = endpoint_carrier.lanes().configured_range_end();
    let endpoint_slots: usize = I::REQUESTED_ROLES.count().into();
    let config = hibana::integration::runtime::Config::new(
        attach_tap,
        rendezvous_slab,
        0..lane_range_end,
        endpoint_slots,
        hibana::integration::runtime::CounterClock::new(),
        if I::OPERATIONAL_DEADLINE_TICKS == 0 {
            None
        } else {
            Some(I::OPERATIONAL_DEADLINE_TICKS)
        },
    );
    let rendezvous = kit
        .add_rendezvous_from_config(config, carrier)
        .expect("appkit attach carrier must register rendezvous");
    let session = hibana::integration::ids::SessionId::new(endpoint_carrier.session_id());
    {
        let mut resolver_registry = AttachResolverRegistry::<
            '_,
            '_,
            '_,
            C,
            _,
            I::Carrier<'_>,
            C::Universe,
            hibana::integration::runtime::CounterClock,
            APPKIT_SESSION_RV_SLOTS,
        > {
            kit: &kit,
            rendezvous,
            program,
            requested_roles: I::REQUESTED_ROLES,
            capsule: PhantomData,
        };
        C::register_resolvers(&mut resolver_registry);
    }
    #[cfg(any(test, not(target_os = "none")))]
    let mut tasks = ScheduledTasks::new();
    let summary = {
        let mut visitor = AttachProjectedRoles {
            kit: &kit,
            rendezvous,
            session,
            endpoint_carrier,
            wasi_guest_bytes,
            driver_facts: I::driver_facts(),
            count: 0,
            role_kinds: RoleKindCounts::default(),
            tasks_lifetime: PhantomData,
            capsule_lifetime: PhantomData::<C>,
            image_lifetime: PhantomData::<I>,
            #[cfg(any(test, not(target_os = "none")))]
            tasks: &mut tasks,
        };
        visit_requested_projected_roles::<C, _>(program, I::REQUESTED_ROLES, &mut visitor);
        AttachSummary {
            endpoint_count: visitor.count,
            role_kinds: visitor.role_kinds,
        }
    };
    #[cfg(any(test, not(target_os = "none")))]
    tasks.poll_until_quiescent();
    summary
}

impl hibana::integration::program::ProjectionMetadataVisitor for ProjectionCapsVisitor {
    fn visit_program(&mut self, facts: hibana::integration::program::ProjectionProgramFacts) {
        self.caps.role_count = facts.role_count;
        self.caps.eff_count = facts.eff_count;
        self.caps.scope_count = facts.scope_count;
        self.caps.route_scope_count = facts.route_scope_count;
        self.caps.fingerprint = facts.fingerprint;
        self.caps.has_parallel = facts.parallel_enter_count != 0;
    }

    fn visit_atom(&mut self, spec: hibana::integration::program::ProjectionAtomSpec) {
        self.caps.roles = self
            .caps
            .roles
            .union(RoleSet::single(spec.from))
            .union(RoleSet::single(spec.to));
        self.caps.lanes = self.caps.lanes.union(LaneSet::single(spec.lane));
        self.caps.has_control |= spec.is_control;
        self.push_label(spec.label);
        if let (Some(op), Some(tap_id)) = (spec.control_op, spec.control_tap_id) {
            self.push_control(op, tap_id);
        }
    }

    fn visit_message(&mut self, spec: hibana::integration::program::ProjectionMessageSpec) {
        let engine_req = hibana::integration::program::ProjectionTypeFingerprint::of::<EngineReq>();
        let engine_ret = hibana::integration::program::ProjectionTypeFingerprint::of::<EngineRet>();
        if spec.payload_type == engine_req {
            self.push_engine_req_label(spec.label);
        }
        if spec.payload_type == engine_ret {
            self.push_engine_ret_label(spec.label);
        }
        if spec.payload_type == engine_req {
            let Some(import) = wasi_import_for_engine_req_label(spec.label) else {
                return;
            };
            let include_import = match self.import_roles {
                Some(roles) => roles.contains(spec.from),
                None => true,
            };
            if include_import {
                self.caps.wasi_imports = self.caps.wasi_imports.union(import);
            }
        }
    }

    fn visit_policy(&mut self, spec: hibana::integration::program::ProjectionPolicySpec) {
        self.push_policy(spec.policy_id);
        self.caps.has_policy = true;
    }
}

/// Derive neutral capacity facts from official hibana projection metadata.
pub fn derive_projection_caps<C: Capsule>() -> ProjectionCaps {
    let program = C::choreography();
    derive_projection_caps_from_program::<C>(&program)
}

fn derive_projection_caps_from_program<C>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
) -> ProjectionCaps
where
    C: Capsule,
{
    let mut visitor = ProjectionCapsVisitor::new();
    program.visit_projection_metadata(&mut visitor);
    visitor.caps.wasi_completion_pair_count = visitor.wasi_completion_pair_count();
    visitor.caps
}

fn derive_projection_caps_for_roles_from_program<C>(
    program: &impl hibana::integration::program::Projectable<C::Universe>,
    requested_roles: RoleSet,
) -> ProjectionCaps
where
    C: Capsule,
{
    let mut visitor = ProjectionCapsVisitor::for_import_roles(requested_roles);
    program.visit_projection_metadata(&mut visitor);
    visitor.caps.wasi_completion_pair_count = visitor.wasi_completion_pair_count();
    visitor.caps
}

/// Validate a logical image requested role slice against projection metadata.
pub fn validate_requested_roles<C, I>() -> bool
where
    C: Capsule,
    I: LogicalImage<C>,
{
    let caps = derive_projection_caps::<C>();
    I::REQUESTED_ROLES.is_subset_of(caps.roles)
}

/// Projection-derived logical image validation report produced by [`run`].
pub struct RunReport<R, I> {
    image: I,
    image_id: ImageId,
    site_id: SiteId,
    requested_roles: RoleSet,
    projection: ProjectionCaps,
    manifest: ImageManifest,
    endpoint_carrier: EndpointCarrierFacts,
    validated_role_count: u8,
    attached_endpoint_count: u8,
    attached_role_kinds: RoleKindCounts,
    carrier: CarrierKind,
    artifact_len: usize,
    report: PhantomData<fn() -> R>,
}

/// Logical image metadata derived from placement and hibana projection facts.
///
/// This is attach/build metadata, not protocol authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageManifest {
    pub capsule_fingerprint: [u64; 2],
    pub placement_fingerprint: [u64; 2],
    pub label_universe_fingerprint: [u64; 2],
    pub choreography_fingerprint: [u64; 2],
    pub choreography_session_id: u32,
    pub logical_image_id: ImageId,
    pub site_id: SiteId,
    pub peer_image_ids: [ImageId; 8],
    pub peer_image_count: u8,
    pub requested_role_set: RoleSet,
    pub projected_role_set: RoleSet,
    pub lane_set: LaneSet,
    pub labels: [u8; 32],
    pub label_count: u8,
    pub policies: [u16; 16],
    pub policy_count: u16,
    pub control_ops: [u8; 16],
    pub control_tap_ids: [u16; 16],
    pub control_count: u16,
    pub wasi_imports: WasiImports,
    pub wasi_completion_pair_count: u8,
    pub role_count: u8,
    pub eff_count: u16,
    pub scope_count: u16,
    pub route_scope_count: u16,
    pub carrier: CarrierKind,
    pub has_parallel: bool,
    pub has_policy: bool,
    pub has_control: bool,
}

impl ImageManifest {
    fn new<C>(
        image_id: ImageId,
        site_id: SiteId,
        peer_images: PeerImageSet,
        requested_roles: RoleSet,
        carrier: CarrierKind,
        projection: ProjectionCaps,
    ) -> Self
    where
        C: Capsule,
    {
        let capsule_fingerprint =
            hibana::integration::program::ProjectionTypeFingerprint::of::<C>().words;
        let placement_fingerprint =
            hibana::integration::program::ProjectionTypeFingerprint::of::<C::Placement>().words;
        let label_universe_fingerprint =
            hibana::integration::program::ProjectionTypeFingerprint::of::<C::Universe>().words;
        Self {
            capsule_fingerprint,
            placement_fingerprint,
            label_universe_fingerprint,
            choreography_fingerprint: projection.fingerprint,
            choreography_session_id: session_id_from_projection(projection),
            logical_image_id: image_id,
            site_id,
            peer_image_ids: peer_images.ids(),
            peer_image_count: peer_images.len(),
            requested_role_set: requested_roles,
            projected_role_set: projection.roles,
            lane_set: projection.lanes,
            labels: projection.labels,
            label_count: projection.label_count,
            policies: projection.policies,
            policy_count: projection.policy_count,
            control_ops: projection.control_ops,
            control_tap_ids: projection.control_tap_ids,
            control_count: projection.control_count,
            wasi_imports: projection.wasi_imports,
            wasi_completion_pair_count: projection.wasi_completion_pair_count,
            role_count: projection.role_count,
            eff_count: projection.eff_count,
            scope_count: projection.scope_count,
            route_scope_count: projection.route_scope_count,
            carrier,
            has_parallel: projection.has_parallel,
            has_policy: projection.has_policy,
            has_control: projection.has_control,
        }
    }

    pub fn can_attach_peer(&self, peer: &Self) -> bool {
        self.logical_image_id != peer.logical_image_id
            && self.choreography_fingerprint == peer.choreography_fingerprint
            && self.capsule_fingerprint == peer.capsule_fingerprint
            && self.placement_fingerprint == peer.placement_fingerprint
            && self.label_universe_fingerprint == peer.label_universe_fingerprint
            && self.choreography_session_id == peer.choreography_session_id
            && self.carrier == peer.carrier
            && self.projected_role_set == peer.projected_role_set
            && self.peer_images().contains(peer.logical_image_id)
            && peer.peer_images().contains(self.logical_image_id)
    }

    pub const fn peer_images(&self) -> PeerImageSet {
        PeerImageSet {
            ids: self.peer_image_ids,
            len: self.peer_image_count,
        }
    }
}

impl<R, I> RunReport<R, I> {
    fn new<C>(
        image: I,
        image_id: ImageId,
        site_id: SiteId,
        requested_roles: RoleSet,
        validated_role_count: u8,
        attached_endpoint_count: u8,
        attached_role_kinds: RoleKindCounts,
        carrier: CarrierKind,
        artifact_len: usize,
        projection: ProjectionCaps,
    ) -> Self
    where
        C: Capsule,
        I: LogicalImage<C>,
    {
        let manifest = ImageManifest::new::<C>(
            image_id,
            site_id,
            I::PEER_IMAGES,
            requested_roles,
            carrier,
            projection,
        );
        let endpoint_carrier =
            EndpointCarrierFacts::new(image_id, site_id, requested_roles, carrier, projection);
        Self {
            image,
            image_id,
            site_id,
            requested_roles,
            projection,
            manifest,
            endpoint_carrier,
            validated_role_count,
            attached_endpoint_count,
            attached_role_kinds,
            carrier,
            artifact_len,
            report: PhantomData,
        }
    }

    pub const fn image(&self) -> &I {
        &self.image
    }

    pub fn image_mut(&mut self) -> &mut I {
        &mut self.image
    }

    pub const fn image_id(&self) -> ImageId {
        self.image_id
    }

    pub const fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub const fn requested_roles(&self) -> RoleSet {
        self.requested_roles
    }

    pub const fn projected_roles(&self) -> RoleSet {
        self.projection.roles
    }

    pub const fn wasi_imports(&self) -> WasiImports {
        self.projection.wasi_imports
    }

    pub const fn wasi_completion_pair_count(&self) -> u8 {
        self.projection.wasi_completion_pair_count
    }

    pub const fn projection(&self) -> ProjectionCaps {
        self.projection
    }

    pub const fn manifest(&self) -> ImageManifest {
        self.manifest
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn attached_role_kinds(&self) -> RoleKindCounts {
        self.attached_role_kinds
    }

    pub const fn validated_role_count(&self) -> u8 {
        self.validated_role_count
    }

    pub const fn attached_endpoint_count(&self) -> u8 {
        self.attached_endpoint_count
    }

    pub const fn carrier(&self) -> CarrierKind {
        self.carrier
    }

    pub const fn artifact_len(&self) -> usize {
        self.artifact_len
    }
}

/// Role-typed wrapper around a hibana endpoint attached by appkit.
///
/// This is the context shape that preserves hibana's typed `Endpoint<'_, ROLE>`
/// progress without exposing raw site or transport authority. It is not a
/// choreography wrapper and it does not name hibana's internal `steps` types.
pub struct RoleEndpointCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: hibana::Endpoint<'a, ROLE>,
    capsule: PhantomData<&'a C>,
}

impl<'a, C: Capsule, const ROLE: u8> RoleEndpointCtx<'a, C, ROLE> {
    fn new(endpoint: hibana::Endpoint<'a, ROLE>) -> Self {
        Self {
            endpoint,
            capsule: PhantomData,
        }
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        &mut self.endpoint
    }
}

/// Opaque object identifier resolved from ChoreoFS facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectId(pub u32);

/// Immutable path-to-object fact consumed by driver-side logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsFact<'a> {
    path: &'a [u8],
    object: ObjectId,
}

impl<'a> ChoreoFsFact<'a> {
    pub const EMPTY: Self = Self {
        path: &[],
        object: ObjectId(0),
    };

    pub const fn new(path: &'a [u8], object: ObjectId) -> Self {
        Self { path, object }
    }

    pub const fn path(&self) -> &'a [u8] {
        self.path
    }

    pub const fn object(&self) -> ObjectId {
        self.object
    }
}

/// Immutable fd materialization spec for one ChoreoFS object.
///
/// This helper is only shorthand for ledger facts. It does not own protocol
/// progress, route selection, or boundary authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FdSpec {
    fd: u32,
    rights: u64,
    generation: u32,
}

impl FdSpec {
    pub const fn new(fd: u32, rights: u64, generation: u32) -> Self {
        Self {
            fd,
            rights,
            generation,
        }
    }

    pub const fn fd(&self) -> u32 {
        self.fd
    }

    pub const fn rights(&self) -> u64 {
        self.rights
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

/// Const helper for writing ChoreoFS path/object and fd facts as one object.
///
/// `ChoreoFsObject` is not a manifest and not an authority table. It only expands
/// into [`ChoreoFsFact`] and [`LedgerFdFact`] for driver-local facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsObject {
    path: &'static [u8],
    object: ObjectId,
    fd: FdSpec,
}

impl ChoreoFsObject {
    pub const fn new(path: &'static [u8], object: ObjectId, fd: FdSpec) -> Self {
        Self { path, object, fd }
    }

    pub const fn path(&self) -> &'static [u8] {
        self.path
    }

    pub const fn object(&self) -> ObjectId {
        self.object
    }

    pub const fn fd(&self) -> FdSpec {
        self.fd
    }

    pub const fn choreofs_fact(&self) -> ChoreoFsFact<'static> {
        ChoreoFsFact::new(self.path, self.object)
    }

    pub const fn ledger_fd_fact(&self) -> LedgerFdFact {
        LedgerFdFact::new(self.fd.fd, self.object, self.fd.rights, self.fd.generation)
    }
}

/// Bounded static expansion of ChoreoFS object facts into driver facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsObjectSet<const N: usize> {
    choreofs: [ChoreoFsFact<'static>; N],
    ledger: [LedgerFdFact; N],
}

impl<const N: usize> ChoreoFsObjectSet<N> {
    pub const fn new(specs: [ChoreoFsObject; N]) -> Self {
        let mut choreofs = [ChoreoFsFact::EMPTY; N];
        let mut ledger = [LedgerFdFact::EMPTY; N];
        let mut idx = 0usize;
        while idx < N {
            choreofs[idx] = specs[idx].choreofs_fact();
            ledger[idx] = specs[idx].ledger_fd_fact();
            idx += 1;
        }
        Self { choreofs, ledger }
    }

    pub const fn choreofs_facts(&'static self) -> ChoreoFsFacts<'static> {
        ChoreoFsFacts::new(&self.choreofs)
    }

    pub const fn ledger_facts(&'static self) -> LedgerFacts<'static> {
        LedgerFacts::new(&self.ledger)
    }

    pub const fn driver_facts(&'static self) -> DriverFacts<'static> {
        DriverFacts::new(self.choreofs_facts(), self.ledger_facts())
    }
}

/// ChoreoFS fact resolver. It does not own protocol progress or route authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChoreoFsFacts<'a> {
    entries: &'a [ChoreoFsFact<'a>],
}

impl<'a> ChoreoFsFacts<'a> {
    pub const fn new(entries: &'a [ChoreoFsFact<'a>]) -> Self {
        Self { entries }
    }

    pub const fn entries(&self) -> &'a [ChoreoFsFact<'a>] {
        self.entries
    }

    pub fn resolve(&self, path: &[u8]) -> Option<ObjectId> {
        let mut idx = 0usize;
        while idx < self.entries.len() {
            let entry = self.entries[idx];
            if entry.path == path {
                return Some(entry.object);
            }
            idx += 1;
        }
        None
    }
}

/// Immutable fd/object materialization fact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LedgerFdFact {
    fd: u32,
    object: ObjectId,
    rights: u64,
    generation: u32,
}

impl LedgerFdFact {
    pub const EMPTY: Self = Self {
        fd: 0,
        object: ObjectId(0),
        rights: 0,
        generation: 0,
    };

    pub const fn new(fd: u32, object: ObjectId, rights: u64, generation: u32) -> Self {
        Self {
            fd,
            object,
            rights,
            generation,
        }
    }

    pub const fn fd(&self) -> u32 {
        self.fd
    }

    pub const fn object(&self) -> ObjectId {
        self.object
    }

    pub const fn rights(&self) -> u64 {
        self.rights
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

/// Read-only ledger facts. The choreography still owns progress authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LedgerFacts<'a> {
    fds: &'a [LedgerFdFact],
}

impl<'a> LedgerFacts<'a> {
    pub const fn new(fds: &'a [LedgerFdFact]) -> Self {
        Self { fds }
    }

    pub const fn fds(&self) -> &'a [LedgerFdFact] {
        self.fds
    }

    pub fn fd(&self, fd: u32) -> Option<LedgerFdFact> {
        let mut idx = 0usize;
        while idx < self.fds.len() {
            let fact = self.fds[idx];
            if fact.fd == fd {
                return Some(fact);
            }
            idx += 1;
        }
        None
    }
}

/// Driver-side service facts handed to sealed localside contexts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverFacts<'a> {
    choreofs: ChoreoFsFacts<'a>,
    ledger: LedgerFacts<'a>,
}

impl<'a> DriverFacts<'a> {
    pub const EMPTY: Self = Self {
        choreofs: ChoreoFsFacts { entries: &[] },
        ledger: LedgerFacts { fds: &[] },
    };

    pub const fn new(choreofs: ChoreoFsFacts<'a>, ledger: LedgerFacts<'a>) -> Self {
        Self { choreofs, ledger }
    }

    pub const fn choreofs(&self) -> ChoreoFsFacts<'a> {
        self.choreofs
    }

    pub const fn ledger(&self) -> LedgerFacts<'a> {
        self.ledger
    }
}

#[cfg(feature = "wasm-engine-core")]
struct WasiGuestSlot<'guest> {
    storage: Option<WasiGuestStorage<'guest>>,
    initialized: bool,
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestSlot<'guest> {
    fn init(
        mut storage: WasiGuestStorage<'guest>,
        module: &'guest [u8],
    ) -> Result<Self, crate::kernel::engine::wasm::Error> {
        let ptr = storage.guest_ptr();
        unsafe {
            crate::kernel::engine::wasm::Guest::init_in_place(ptr, module)?;
        }
        Ok(Self {
            storage: Some(storage),
            initialized: true,
        })
    }

    fn guest(&mut self) -> &mut crate::kernel::engine::wasm::Guest<'guest> {
        debug_assert!(self.initialized);
        let ptr = self
            .storage
            .as_mut()
            .expect("initialized WASI guest slot must retain storage")
            .guest_ptr();
        unsafe { &mut *ptr }
    }

    fn finish(mut self) -> WasiGuestStorage<'guest> {
        if self.initialized {
            unsafe {
                let ptr = self
                    .storage
                    .as_mut()
                    .expect("initialized WASI guest slot must retain storage")
                    .guest_ptr();
                core::ptr::drop_in_place(ptr);
            }
            self.initialized = false;
        }
        self.storage
            .take()
            .expect("finished WASI guest slot must return storage")
    }
}

#[cfg(feature = "wasm-engine-core")]
impl Drop for WasiGuestSlot<'_> {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                let ptr = self
                    .storage
                    .as_mut()
                    .expect("initialized WASI guest slot must retain storage")
                    .guest_ptr();
                core::ptr::drop_in_place(ptr);
            }
        }
    }
}

/// Engine-side localside context.
pub struct EngineCtx<'endpoint, 'guest, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'endpoint, C, ROLE>,
    endpoint_carrier: EndpointCarrierFacts,
    #[cfg(feature = "wasm-engine-core")]
    wasi_guest_bytes: Option<&'guest [u8]>,
    #[cfg(feature = "wasm-engine-core")]
    guest_storage: Option<WasiGuestStorage<'guest>>,
    #[cfg(feature = "wasm-engine-core")]
    guest_slot: Option<WasiGuestSlot<'guest>>,
    #[cfg(not(feature = "wasm-engine-core"))]
    guest_lifetime: core::marker::PhantomData<&'guest ()>,
}

impl<'endpoint, 'guest, C: Capsule, const ROLE: u8> EngineCtx<'endpoint, 'guest, C, ROLE> {
    fn new(
        endpoint: RoleEndpointCtx<'endpoint, C, ROLE>,
        endpoint_carrier: EndpointCarrierFacts,
        wasi_guest_bytes: Option<&'guest [u8]>,
        #[cfg(feature = "wasm-engine-core")] guest_storage: WasiGuestStorage<'guest>,
    ) -> Self {
        #[cfg(not(feature = "wasm-engine-core"))]
        core::hint::black_box(wasi_guest_bytes);
        Self {
            endpoint,
            endpoint_carrier,
            #[cfg(feature = "wasm-engine-core")]
            wasi_guest_bytes,
            #[cfg(feature = "wasm-engine-core")]
            guest_storage: Some(guest_storage),
            #[cfg(feature = "wasm-engine-core")]
            guest_slot: None,
            #[cfg(not(feature = "wasm-engine-core"))]
            guest_lifetime: core::marker::PhantomData,
        }
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'endpoint, ROLE> {
        self.endpoint.endpoint()
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn endpoint_send<const LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<(), WasiGuestError> {
        let flow = match self.endpoint().flow::<hibana::g::Msg<LABEL, EngineReq>>() {
            Ok(flow) => flow,
            Err(error) => {
                return Err(WasiGuestError::endpoint(0x5745_1000 | LABEL as u32, error));
            }
        };
        match flow.send(&request).await {
            Ok(()) => Ok(()),
            Err(error) => Err(WasiGuestError::endpoint(0x5745_2000 | LABEL as u32, error)),
        }
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn endpoint_call<const REQUEST_LABEL: u8, const REPLY_LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<EngineRet, WasiGuestError> {
        self.endpoint_send::<REQUEST_LABEL>(request).await?;
        match self
            .endpoint()
            .recv::<hibana::g::Msg<REPLY_LABEL, EngineRet>>()
            .await
        {
            Ok(reply) => Ok(reply),
            Err(error) => Err(WasiGuestError::endpoint(
                0x5745_3000 | REPLY_LABEL as u32,
                error,
            )),
        }
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn admit_wasi_import_loop_continue(&mut self) -> Result<(), WasiGuestError> {
        if !self.endpoint_carrier.has_wasi_import_loop_control {
            return Err(WasiGuestError::EndpointRejected(0x5745_6000));
        }
        let flow = match self.endpoint().flow::<WasiImportLoopContinue>() {
            Ok(flow) => flow,
            Err(..) => return Ok(()),
        };
        match flow.send(()).await {
            Ok(()) => Ok(()),
            Err(error) => Err(WasiGuestError::endpoint(0x5745_6100, error)),
        }
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn admit_wasi_import_loop_break(&mut self) -> Result<(), WasiGuestError> {
        if !self.endpoint_carrier.has_wasi_import_loop_control {
            return Err(WasiGuestError::EndpointRejected(0x5745_6200));
        }
        let flow = match self.endpoint().flow::<WasiImportLoopBreak>() {
            Ok(flow) => flow,
            Err(error) => return Err(WasiGuestError::endpoint(0x5745_6200, error)),
        };
        match flow.send(()).await {
            Ok(()) => Ok(()),
            Err(error) => Err(WasiGuestError::endpoint(0x5745_6300, error)),
        }
    }

    #[cfg(feature = "wasm-engine-core")]
    fn protocol_fdstat_to_vm(
        stat: crate::choreography::protocol::FdStat,
    ) -> crate::kernel::engine::wasm::FdStat {
        let rights = match stat.rights() {
            MemRights::Read => 1,
            MemRights::Write => 2,
        };
        crate::kernel::engine::wasm::FdStat::new(4, 0, rights, 0)
    }

    /// Drive the selected WASI P1 guest until it exits, finishes, or exhausts its budget.
    ///
    /// Each emitted WASI P1 import is normalized into an `EngineReq`, sent through
    /// this role's typed endpoint, and completed only after the corresponding
    /// `EngineRet` is received through the endpoint/carrier path.
    #[cfg(feature = "wasm-engine-core")]
    async fn drive_wasi_guest(
        &mut self,
        budget: BudgetRun,
    ) -> Result<WasiGuestStatus, WasiGuestError> {
        let Some(bytes) = self.wasi_guest_bytes else {
            return Err(WasiGuestError::NoWasiArtifact);
        };
        let mut guest_slot = match self.guest_slot.take() {
            Some(slot) => slot,
            None => {
                let guest_storage = self.guest_storage.take().expect(
                    "WASI engine context must receive in-place guest storage from its logical image",
                );
                WasiGuestSlot::init(guest_storage, bytes)?
            }
        };
        let result = loop {
            let guest = guest_slot.guest();
            match guest.resume(budget) {
                Ok(crate::kernel::engine::wasm::Event::Done) => {
                    break Ok(WasiGuestStatus::Done);
                }
                Ok(crate::kernel::engine::wasm::Event::BudgetExpired(expired)) => {
                    break Ok(WasiGuestStatus::BudgetExpired(expired));
                }
                Ok(crate::kernel::engine::wasm::Event::Exit(exit)) => {
                    let Some(status) = exit.as_protocol_status() else {
                        break Err(WasiGuestError::UnexpectedReply);
                    };
                    if let Err(error) = self.admit_wasi_import_loop_break().await {
                        break Err(error);
                    }
                    if let Err(error) = self
                        .endpoint_send::<LABEL_WASI_PROC_EXIT>(EngineReq::ProcExit(status))
                        .await
                    {
                        break Err(error);
                    }
                    break Ok(WasiGuestStatus::Exit(status));
                }
                Ok(crate::kernel::engine::wasm::Event::Call(call)) => {
                    if let Err(error) = self.admit_wasi_import_loop_continue().await {
                        break Err(error);
                    }
                    if let Err(error) = self.drive_wasi_call(call).await {
                        break Err(error);
                    }
                }
                Ok(crate::kernel::engine::wasm::Event::MemoryFence(pending)) => {
                    if let Err(error) = self.drive_memory_fence(pending).await {
                        break Err(error);
                    }
                }
                Err(error) => break Err(error.into()),
            }
        };
        match result {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(WasiGuestStatus::Done) | Ok(WasiGuestStatus::Exit(_)) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
        result
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn drive_wasi_call(
        &mut self,
        call: crate::kernel::engine::wasm::Call<'_, 'guest>,
    ) -> Result<(), WasiGuestError> {
        match call {
            crate::kernel::engine::wasm::Call::FdWrite(pending) => {
                let fd = pending.fd();
                let payload = pending.payload()?;
                let request = EngineReq::FdWrite(FdWrite::new(fd, payload.as_bytes())?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET>(request)
                    .await?;
                let EngineRet::FdWriteDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if done.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(done.errno() as u32)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::FdRead(pending) => {
                let fd = pending.fd();
                let max_len = pending.max_len()?;
                if max_len > u8::MAX as usize {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::FdRead(FdRead::new(fd, max_len as u8)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET>(request)
                    .await?;
                let EngineRet::FdReadDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if done.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(done.as_bytes(), 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::FdFdstatGet(pending) => {
                let fd = pending.fd();
                let reply = self
                    .endpoint_call::<LABEL_WASI_FD_FDSTAT_GET, LABEL_WASI_FD_FDSTAT_GET_RET>(
                        EngineReq::FdFdstatGet(FdRequest::new(fd)),
                    )
                    .await?;
                let EngineRet::FdStat(stat) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if stat.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(Self::protocol_fdstat_to_vm(stat), 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::FdClose(pending) => {
                let fd = pending.fd();
                let reply = self
                    .endpoint_call::<LABEL_WASI_FD_CLOSE, LABEL_WASI_FD_CLOSE_RET>(
                        EngineReq::FdClose(FdRequest::new(fd)),
                    )
                    .await?;
                let EngineRet::FdClosed(closed) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if closed.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::ClockResGet(pending) => {
                let clock_id = pending.clock_id();
                if clock_id > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let reply = self
                    .endpoint_call::<LABEL_WASI_CLOCK_RES_GET, LABEL_WASI_CLOCK_RES_GET_RET>(
                        EngineReq::ClockResGet(ClockResGet::new(clock_id as u8)),
                    )
                    .await?;
                let EngineRet::ClockResolution(resolution) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(resolution.nanos(), 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::ClockTimeGet(pending) => {
                let clock_id = pending.clock_id();
                if clock_id > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request =
                    EngineReq::ClockTimeGet(ClockTimeGet::new(clock_id as u8, pending.precision()));
                let reply = self
                    .endpoint_call::<LABEL_WASI_CLOCK_TIME_GET, LABEL_WASI_CLOCK_TIME_GET_RET>(
                        request,
                    )
                    .await?;
                let EngineRet::ClockTime(now) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(now.nanos(), 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::PollOneoff(pending) => {
                let delay = pending.delay_ticks()?;
                let reply = self
                    .endpoint_call::<LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET>(
                        EngineReq::PollOneoff(PollOneoff::new(delay)),
                    )
                    .await?;
                let EngineRet::PollReady(ready) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(ready.ready() as u32, 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::RandomGet(pending) => {
                let len = pending.buf_len();
                if len > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::RandomGet(RandomGet::new(len as u8)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_RANDOM_GET, LABEL_WASI_RANDOM_GET_RET>(request)
                    .await?;
                let EngineRet::RandomDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(done.as_bytes(), 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::FdReaddir(pending) => {
                let fd = pending.fd()?;
                let cookie = pending.cookie()?;
                let max_len = pending.max_len()?;
                if max_len > u8::MAX as usize {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::FdReaddir(FdReaddir::new(fd, cookie, max_len as u8)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_FD_READDIR, LABEL_WASI_FD_READDIR_RET>(request)
                    .await?;
                let EngineRet::FdReaddirDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if done.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(done.as_bytes(), done.errno() as u32)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::PathOpen(pending) => {
                let fd = pending.fd()?;
                let rights = pending.rights_base()?;
                let path = pending.path_bytes()?;
                let request = EngineReq::PathOpen(PathOpen::new(fd, 0, rights, path.as_bytes())?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET>(request)
                    .await?;
                let EngineRet::PathOpened(opened) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(opened.fd() as u32, opened.errno() as u32)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::ArgsSizesGet(pending) => {
                let reply = self
                    .endpoint_call::<LABEL_WASI_ARGS_SIZES_GET, LABEL_WASI_ARGS_SIZES_GET_RET>(
                        EngineReq::ArgsSizesGet(ArgsSizesGet::new()),
                    )
                    .await?;
                let EngineRet::ArgsSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(sizes.count() as u32, sizes.buf_size() as u32, 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::ArgsGet(pending) => {
                let max_len = pending.max_len();
                let request = EngineReq::ArgsGet(ArgsGet::new(max_len)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_ARGS_GET, LABEL_WASI_ARGS_GET_RET>(request)
                    .await?;
                let EngineRet::ArgsDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(&[done.as_bytes()], 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::EnvironSizesGet(pending) => {
                let reply = self
                    .endpoint_call::<LABEL_WASI_ENVIRON_SIZES_GET, LABEL_WASI_ENVIRON_SIZES_GET_RET>(
                        EngineReq::EnvironSizesGet(EnvironSizesGet::new()),
                    )
                    .await?;
                let EngineRet::EnvironSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(sizes.count() as u32, sizes.buf_size() as u32, 0)?;
                Ok(())
            }
            crate::kernel::engine::wasm::Call::EnvironGet(pending) => {
                let max_len = pending.max_len();
                let request = EngineReq::EnvironGet(EnvironGet::new(max_len)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_ENVIRON_GET, LABEL_WASI_ENVIRON_GET_RET>(request)
                    .await?;
                let EngineRet::EnvironDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(&[(done.as_bytes(), &[][..])], 0)?;
                Ok(())
            }
        }
    }

    #[cfg(feature = "wasm-engine-core")]
    async fn drive_memory_fence(
        &mut self,
        pending: crate::kernel::engine::wasm::Pending<
            '_,
            'guest,
            crate::kernel::engine::wasm::MemoryFence,
        >,
    ) -> Result<(), WasiGuestError> {
        let fence = MemFence::new(MemFenceReason::MemoryGrow, pending.fence_epoch());
        let flow = match self
            .endpoint()
            .flow::<hibana::g::Msg<LABEL_MEM_FENCE, MemFence>>()
        {
            Ok(flow) => flow,
            Err(error) => {
                return Err(WasiGuestError::endpoint(
                    0x5745_1000 | LABEL_MEM_FENCE as u32,
                    error,
                ));
            }
        };
        if let Err(error) = flow.send(&fence).await {
            return Err(WasiGuestError::endpoint(
                0x5745_2000 | LABEL_MEM_FENCE as u32,
                error,
            ));
        }
        pending.complete()?;
        Ok(())
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

#[cfg(feature = "wasm-engine-core")]
async fn drive_canonical_wasi_engine<'endpoint, 'guest, C, I, const ROLE: u8>(
    mut ctx: EngineCtx<'endpoint, 'guest, C, ROLE>,
) -> RoleResult<WasiGuestError>
where
    C: Capsule,
    I: LogicalImage<C>,
{
    loop {
        match ctx.drive_wasi_guest(I::wasi_budget::<ROLE>()).await {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {}
            Ok(WasiGuestStatus::Done) | Ok(WasiGuestStatus::Exit(_)) => {
                return ctx.pending().await;
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
}

/// Driver-side localside context.
pub struct DriverCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'a, C, ROLE>,
    endpoint_carrier: EndpointCarrierFacts,
    facts: DriverFacts<'a>,
}

impl<'a, C: Capsule, const ROLE: u8> DriverCtx<'a, C, ROLE> {
    fn new(
        endpoint: RoleEndpointCtx<'a, C, ROLE>,
        endpoint_carrier: EndpointCarrierFacts,
        facts: DriverFacts<'a>,
    ) -> Self {
        Self {
            endpoint,
            endpoint_carrier,
            facts,
        }
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn facts(&self) -> DriverFacts<'a> {
        self.facts
    }

    pub const fn choreofs(&self) -> ChoreoFsFacts<'a> {
        self.facts.choreofs()
    }

    pub const fn ledger(&self) -> LedgerFacts<'a> {
        self.facts.ledger()
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        self.endpoint.endpoint()
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

/// Site-local external boundary context.
pub struct BoundaryCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'a, C, ROLE>,
    endpoint_carrier: EndpointCarrierFacts,
}

impl<'a, C: Capsule, const ROLE: u8> BoundaryCtx<'a, C, ROLE> {
    fn new(endpoint: RoleEndpointCtx<'a, C, ROLE>, endpoint_carrier: EndpointCarrierFacts) -> Self {
        Self {
            endpoint,
            endpoint_carrier,
        }
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        self.endpoint.endpoint()
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

/// Carrier-only link context.
pub struct LinkCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'a, C, ROLE>,
    endpoint_carrier: EndpointCarrierFacts,
}

impl<'a, C: Capsule, const ROLE: u8> LinkCtx<'a, C, ROLE> {
    fn new(endpoint: RoleEndpointCtx<'a, C, ROLE>, endpoint_carrier: EndpointCarrierFacts) -> Self {
        Self {
            endpoint,
            endpoint_carrier,
        }
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        self.endpoint.endpoint()
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

/// Lifecycle and safe-state context.
pub struct SupervisorCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'a, C, ROLE>,
    endpoint_carrier: EndpointCarrierFacts,
}

impl<'a, C: Capsule, const ROLE: u8> SupervisorCtx<'a, C, ROLE> {
    fn new(endpoint: RoleEndpointCtx<'a, C, ROLE>, endpoint_carrier: EndpointCarrierFacts) -> Self {
        Self {
            endpoint,
            endpoint_carrier,
        }
    }

    pub const fn endpoint_carrier(&self) -> EndpointCarrierFacts {
        self.endpoint_carrier
    }

    pub const fn role(&self) -> u8 {
        ROLE
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        self.endpoint.endpoint()
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

/// Localside implementation contract for a capsule.
pub trait Localside<C: Capsule> {
    type Error: Debug;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: EngineCtx<'endpoint, 'guest, C, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn driver<'a, const ROLE: u8>(
        ctx: DriverCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn boundary<'a, const ROLE: u8>(
        ctx: BoundaryCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn link<'a, const ROLE: u8>(
        ctx: LinkCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn supervisor<'a, const ROLE: u8>(
        ctx: SupervisorCtx<'a, C, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;
}

/// Canonical appkit execution path.
pub fn run<I, C>(artifact: I::Artifact) -> I::Exit<C::Report>
where
    C: Capsule,
    I: LogicalImage<C>,
    I::Artifact: ArtifactEvidence,
{
    let program = C::choreography();
    let projection = derive_projection_caps_from_program::<C>(&program);
    let projected_roles = collect_projected_roles::<C, I>(&program);
    let image_projection =
        derive_projection_caps_for_roles_from_program::<C>(&program, I::REQUESTED_ROLES);
    let artifact_len = artifact.byte_len();
    let wasi_guest_bytes = artifact.wasi_bytes();
    assert!(
        artifact.validate(image_projection.wasi_imports),
        "logical image artifact must be a WASI Preview 1 artifact or explicit NoWasi"
    );
    assert!(
        C::Placement::requested_roles::<I>() == I::REQUESTED_ROLES,
        "logical image requested roles must match capsule placement"
    );
    assert!(
        I::REQUESTED_ROLES.is_subset_of(HIBANA_TYPED_ROLE_DOMAIN),
        "logical image requested roles must stay within current hibana typed role domain"
    );
    assert!(
        I::REQUESTED_ROLES.is_subset_of(projection.roles),
        "logical image requested roles must be present in hibana projection metadata"
    );
    assert!(
        projected_roles.roles() == I::REQUESTED_ROLES,
        "logical image requested roles must be materialized as hibana RoleProgram values"
    );
    assert!(
        projected_roles.count() == I::REQUESTED_ROLES.count(),
        "logical image projected RoleProgram count must match requested role count"
    );
    assert!(
        !I::PEER_IMAGES.contains(I::IMAGE_ID),
        "logical image peer metadata must not include the image itself"
    );
    let endpoint_carrier = EndpointCarrierFacts::new(
        I::IMAGE_ID,
        I::SITE_ID,
        I::REQUESTED_ROLES,
        I::CARRIER,
        image_projection,
    );
    let attach_summary =
        attach_projected_roles::<C, I>(&program, endpoint_carrier, wasi_guest_bytes);
    assert!(
        attach_summary.endpoint_count == projected_roles.count(),
        "logical image projected roles must attach through SessionKit"
    );
    assert!(
        attach_summary.role_kinds.total() == projected_roles.count(),
        "attached role kind counts must match projected RoleProgram count"
    );
    let image = I::init();
    I::Exit::from_run_report(RunReport::new::<C>(
        image,
        I::IMAGE_ID,
        I::SITE_ID,
        I::REQUESTED_ROLES,
        projected_roles.count(),
        attach_summary.endpoint_count,
        attach_summary.role_kinds,
        I::CARRIER,
        artifact_len,
        image_projection,
    ))
}

#[cfg(all(test, feature = "wasm-engine-core", feature = "wasip1-sys-fd-write"))]
mod tests {
    use super::*;
    use core::cell::Cell;
    use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::boxed::Box;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    };
    use std::vec::Vec;

    use hibana::{
        g,
        integration::{
            binding::NoBinding,
            cap::{
                GenericCapToken,
                advanced::{LoopBreakKind, LoopContinueKind},
            },
            ids::SessionId,
            program::Projectable,
            runtime::{Config, CounterClock, DefaultLabelUniverse},
        },
    };

    use crate::choreography::protocol::{FdWriteDone, LABEL_WASI_FD_WRITE_RET, PollReady};

    const SECTION_TYPE: u8 = 1;
    const SECTION_IMPORT: u8 = 2;
    const TEST_ATTACHED_QUEUE_CARRIER: CarrierKind = CarrierKind::new(1001);
    const SECTION_FUNCTION: u8 = 3;
    const SECTION_MEMORY: u8 = 5;
    const SECTION_EXPORT: u8 = 7;
    const SECTION_CODE: u8 = 10;
    const SECTION_DATA: u8 = 11;
    const EXTERNAL_KIND_FUNC: u8 = 0;
    const OPCODE_I32_CONST: u8 = 0x41;
    const OPCODE_CALL: u8 = 0x10;
    const OPCODE_DROP: u8 = 0x1a;
    const OPCODE_MEMORY_GROW: u8 = 0x40;
    const OPCODE_END: u8 = 0x0b;
    const VALTYPE_I32: u8 = 0x7f;

    const TEST_CARRIER_ROLES: usize = HIBANA_TYPED_ROLE_DOMAIN_SIZE as usize;
    const TEST_CARRIER_QUEUE_DEPTH: usize = 16;
    const TEST_CARRIER_FRAME_BYTES: usize = 256;

    #[derive(Clone, Copy, Debug)]
    struct AttachedQueueTestFrame {
        occupied: bool,
        frame_label: hibana::integration::transport::FrameLabel,
        len: usize,
        bytes: [u8; TEST_CARRIER_FRAME_BYTES],
    }

    impl AttachedQueueTestFrame {
        const EMPTY: Self = Self {
            occupied: false,
            frame_label: hibana::integration::transport::FrameLabel::new(0),
            len: 0,
            bytes: [0; TEST_CARRIER_FRAME_BYTES],
        };
    }

    #[derive(Clone, Copy, Debug)]
    struct AttachedQueueTestQueue {
        frames: [AttachedQueueTestFrame; TEST_CARRIER_QUEUE_DEPTH],
        head: usize,
        len: usize,
    }

    impl AttachedQueueTestQueue {
        const EMPTY: Self = Self {
            frames: [AttachedQueueTestFrame::EMPTY; TEST_CARRIER_QUEUE_DEPTH],
            head: 0,
            len: 0,
        };

        fn push_back(
            &mut self,
            frame_label: hibana::integration::transport::FrameLabel,
            payload: hibana::integration::wire::Payload<'_>,
        ) -> Result<(), hibana::integration::transport::TransportError> {
            let bytes = payload.as_bytes();
            if bytes.len() > TEST_CARRIER_FRAME_BYTES || self.len == TEST_CARRIER_QUEUE_DEPTH {
                return Err(hibana::integration::transport::TransportError::Failed);
            }
            let idx = (self.head + self.len) % TEST_CARRIER_QUEUE_DEPTH;
            self.frames[idx].occupied = true;
            self.frames[idx].frame_label = frame_label;
            self.frames[idx].len = bytes.len();
            self.frames[idx].bytes[..bytes.len()].copy_from_slice(bytes);
            self.len += 1;
            Ok(())
        }

        fn push_front(
            &mut self,
            frame_label: hibana::integration::transport::FrameLabel,
            bytes: &[u8],
        ) {
            if bytes.len() > TEST_CARRIER_FRAME_BYTES || self.len == TEST_CARRIER_QUEUE_DEPTH {
                return;
            }
            self.head = if self.head == 0 {
                TEST_CARRIER_QUEUE_DEPTH - 1
            } else {
                self.head - 1
            };
            self.frames[self.head].occupied = true;
            self.frames[self.head].frame_label = frame_label;
            self.frames[self.head].len = bytes.len();
            self.frames[self.head].bytes[..bytes.len()].copy_from_slice(bytes);
            self.len += 1;
        }

        fn pop_front(&mut self) -> Option<AttachedQueueTestFrame> {
            if self.len == 0 {
                return None;
            }
            let idx = self.head;
            let frame = self.frames[idx];
            self.frames[idx] = AttachedQueueTestFrame::EMPTY;
            self.head = (self.head + 1) % TEST_CARRIER_QUEUE_DEPTH;
            self.len -= 1;
            if frame.occupied { Some(frame) } else { None }
        }
    }

    struct AttachedQueueTestQueues {
        by_role: [AttachedQueueTestQueue; TEST_CARRIER_ROLES],
        recv_count: usize,
        hint_count: usize,
        requeue_count: usize,
    }

    impl AttachedQueueTestQueues {
        const EMPTY: Self = Self {
            by_role: [AttachedQueueTestQueue::EMPTY; TEST_CARRIER_ROLES],
            recv_count: 0,
            hint_count: 0,
            requeue_count: 0,
        };
    }

    #[derive(Clone)]
    struct AttachedQueueTestCarrier {
        queues: Arc<Mutex<AttachedQueueTestQueues>>,
    }

    impl AttachedQueueTestCarrier {
        fn new() -> Self {
            Self {
                queues: Arc::new(Mutex::new(AttachedQueueTestQueues::EMPTY)),
            }
        }

        fn queued_for(&self, role: u8) -> usize {
            let role = role as usize;
            if role >= TEST_CARRIER_ROLES {
                return 0;
            }
            self.queues.lock().expect("test carrier queue lock").by_role[role].len
        }

        fn counters(&self) -> (usize, usize, usize) {
            let queues = self.queues.lock().expect("test carrier queue lock");
            (queues.recv_count, queues.hint_count, queues.requeue_count)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct AttachedQueueTestTx {
        local_role: u8,
        session_id: u32,
        sent_frames: u16,
    }

    #[derive(Debug)]
    struct AttachedQueueTestRx {
        local_role: u8,
        session_id: u32,
        requeued_frames: u16,
        frame_label: Option<hibana::integration::transport::FrameLabel>,
        hint_frame_label: Cell<Option<hibana::integration::transport::FrameLabel>>,
        len: usize,
        bytes: [u8; TEST_CARRIER_FRAME_BYTES],
    }

    impl hibana::integration::Transport for AttachedQueueTestCarrier {
        type Error = hibana::integration::transport::TransportError;
        type Tx<'a>
            = AttachedQueueTestTx
        where
            Self: 'a;
        type Rx<'a>
            = AttachedQueueTestRx
        where
            Self: 'a;
        type Metrics = ();

        fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
            (
                AttachedQueueTestTx {
                    local_role,
                    session_id,
                    sent_frames: 0,
                },
                AttachedQueueTestRx {
                    local_role,
                    session_id,
                    requeued_frames: 0,
                    frame_label: None,
                    hint_frame_label: Cell::new(None),
                    len: 0,
                    bytes: [0; TEST_CARRIER_FRAME_BYTES],
                },
            )
        }

        fn poll_send<'a, 'f>(
            &'a self,
            tx: &'a mut Self::Tx<'a>,
            outgoing: hibana::integration::transport::Outgoing<'f>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>>
        where
            'a: 'f,
        {
            assert_ne!(tx.session_id, 0, "attached send must belong to a session");
            assert_ne!(
                outgoing.peer(),
                tx.local_role,
                "attached send must cross a role boundary"
            );
            let peer = outgoing.peer() as usize;
            if peer >= TEST_CARRIER_ROLES {
                return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
            }
            self.queues.lock().expect("test carrier queue lock").by_role[peer]
                .push_back(outgoing.frame_label(), outgoing.payload())?;
            tx.sent_frames = tx.sent_frames.saturating_add(1);
            cx.waker().wake_by_ref();
            Poll::Ready(Ok(()))
        }

        fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
            tx.sent_frames = 0;
        }

        fn poll_recv<'a>(
            &'a self,
            rx: &'a mut Self::Rx<'a>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<hibana::integration::wire::Payload<'a>, Self::Error>> {
            assert_ne!(
                rx.session_id, 0,
                "attached receive must belong to a session"
            );
            let local_role = rx.local_role as usize;
            if local_role >= TEST_CARRIER_ROLES {
                return Poll::Ready(Err(hibana::integration::transport::TransportError::Failed));
            }
            let Some(frame) = self.queues.lock().expect("test carrier queue lock").by_role
                [local_role]
                .pop_front()
            else {
                return Poll::Pending;
            };
            self.queues
                .lock()
                .expect("test carrier queue lock")
                .recv_count += 1;
            rx.frame_label = Some(frame.frame_label);
            rx.hint_frame_label.set(Some(frame.frame_label));
            rx.len = frame.len;
            rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
            cx.waker().wake_by_ref();
            Poll::Ready(Ok(hibana::integration::wire::Payload::new(
                &rx.bytes[..rx.len],
            )))
        }

        fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
            if let Some(frame_label) = rx.frame_label.take() {
                let local_role = rx.local_role as usize;
                if local_role < TEST_CARRIER_ROLES {
                    self.queues.lock().expect("test carrier queue lock").by_role[local_role]
                        .push_front(frame_label, &rx.bytes[..rx.len]);
                }
            }
            self.queues
                .lock()
                .expect("test carrier queue lock")
                .requeue_count += 1;
            rx.hint_frame_label.set(None);
            rx.requeued_frames = rx.requeued_frames.saturating_add(1);
        }

        fn drain_events(
            &self,
            emit: &mut dyn FnMut(hibana::integration::transport::advanced::TransportEvent),
        ) {
            emit(
                hibana::integration::transport::advanced::TransportEvent::new(
                    hibana::integration::transport::advanced::TransportEventKind::Ack,
                    0,
                    0,
                    0,
                ),
            );
        }

        fn recv_frame_hint<'a>(
            &'a self,
            rx: &'a Self::Rx<'a>,
        ) -> Option<hibana::integration::transport::FrameLabel> {
            assert!(
                (rx.local_role as usize) < TEST_CARRIER_ROLES,
                "attached receive role must be valid"
            );
            let hint = rx.hint_frame_label.get();
            if hint.is_some() {
                self.queues
                    .lock()
                    .expect("test carrier queue lock")
                    .hint_count += 1;
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

    struct BridgeCapsule;
    struct BridgePlacement;
    struct BridgeLocal;
    struct BridgeImage;
    struct NoLoopCapsule;
    struct NoLoopPlacement;
    struct NoLoopLocal;
    struct MemoryGrowCapsule;
    struct MemoryGrowLocal;
    struct MemoryGrowPlacement;
    static BRIDGE_WASI_GUEST_ARENA: WasiGuestArena = WasiGuestArena::empty();
    static NO_LOOP_WASI_GUEST_ARENA: WasiGuestArena = WasiGuestArena::empty();
    static MEMORY_GROW_WASI_GUEST_ARENA: WasiGuestArena = WasiGuestArena::empty();

    static RUN_BRIDGE_ENGINE_DONE: AtomicU8 = AtomicU8::new(0);
    static RUN_BRIDGE_DRIVER_DONE: AtomicU8 = AtomicU8::new(0);

    impl Capsule for BridgeCapsule {
        type Universe = DefaultLabelUniverse;
        type Placement = BridgePlacement;
        type Local = BridgeLocal;
        type Report = ();

        fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
            let fd_write = g::seq(
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            );
            g::route(
                g::seq(
                    g::send::<g::Role<0>, g::Role<0>, WasiImportLoopContinue, 0>(),
                    fd_write,
                ),
                g::send::<g::Role<0>, g::Role<0>, WasiImportLoopBreak, 0>(),
            )
        }
    }

    impl Placement<BridgeCapsule> for BridgePlacement {
        fn role_kind(role: u8) -> RoleKind {
            match role {
                0 => RoleKind::Engine,
                1 => RoleKind::Driver,
                _ => RoleKind::Boundary,
            }
        }
    }

    impl Localside<BridgeCapsule> for BridgeLocal {
        type Error = WasiGuestError;

        fn engine<'endpoint, 'guest, const ROLE: u8>(
            ctx: EngineCtx<'endpoint, 'guest, BridgeCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            async move {
                let mut ctx = ctx;
                if ROLE == 0 {
                    let status = ctx.drive_wasi_guest(BudgetRun::new(1, 0, 128, 0)).await?;
                    assert_eq!(status, WasiGuestStatus::Done);
                    RUN_BRIDGE_ENGINE_DONE.store(1, Ordering::SeqCst);
                }
                ctx.pending().await
            }
        }

        fn driver<'a, const ROLE: u8>(
            ctx: DriverCtx<'a, BridgeCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            async move {
                let mut ctx = ctx;
                if ROLE == 1 {
                    reply_to_fd_write(&mut ctx).await;
                    RUN_BRIDGE_DRIVER_DONE.store(1, Ordering::SeqCst);
                }
                ctx.pending().await
            }
        }

        fn boundary<'a, const ROLE: u8>(
            ctx: BoundaryCtx<'a, BridgeCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn link<'a, const ROLE: u8>(
            ctx: LinkCtx<'a, BridgeCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn supervisor<'a, const ROLE: u8>(
            ctx: SupervisorCtx<'a, BridgeCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }
    }

    impl Capsule for NoLoopCapsule {
        type Universe = DefaultLabelUniverse;
        type Placement = NoLoopPlacement;
        type Local = NoLoopLocal;
        type Report = ();

        fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
            g::seq(
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            )
        }
    }

    impl Placement<NoLoopCapsule> for NoLoopPlacement {
        fn role_kind(role: u8) -> RoleKind {
            match role {
                0 => RoleKind::Engine,
                1 => RoleKind::Driver,
                _ => RoleKind::Boundary,
            }
        }
    }

    impl Localside<NoLoopCapsule> for NoLoopLocal {
        type Error = Infallible;

        fn engine<'endpoint, 'guest, const ROLE: u8>(
            ctx: EngineCtx<'endpoint, 'guest, NoLoopCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn driver<'a, const ROLE: u8>(
            ctx: DriverCtx<'a, NoLoopCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn boundary<'a, const ROLE: u8>(
            ctx: BoundaryCtx<'a, NoLoopCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn link<'a, const ROLE: u8>(
            ctx: LinkCtx<'a, NoLoopCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn supervisor<'a, const ROLE: u8>(
            ctx: SupervisorCtx<'a, NoLoopCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }
    }

    impl Capsule for MemoryGrowCapsule {
        type Universe = DefaultLabelUniverse;
        type Placement = MemoryGrowPlacement;
        type Local = MemoryGrowLocal;
        type Report = ();

        fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_MEM_FENCE, MemFence>, 0>()
        }
    }

    impl Placement<MemoryGrowCapsule> for MemoryGrowPlacement {
        fn role_kind(role: u8) -> RoleKind {
            match role {
                0 => RoleKind::Engine,
                1 => RoleKind::Driver,
                _ => RoleKind::Boundary,
            }
        }
    }

    impl Localside<MemoryGrowCapsule> for MemoryGrowLocal {
        type Error = Infallible;

        fn engine<'endpoint, 'guest, const ROLE: u8>(
            ctx: EngineCtx<'endpoint, 'guest, MemoryGrowCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn driver<'a, const ROLE: u8>(
            ctx: DriverCtx<'a, MemoryGrowCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn boundary<'a, const ROLE: u8>(
            ctx: BoundaryCtx<'a, MemoryGrowCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn link<'a, const ROLE: u8>(
            ctx: LinkCtx<'a, MemoryGrowCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }

        fn supervisor<'a, const ROLE: u8>(
            ctx: SupervisorCtx<'a, MemoryGrowCapsule, ROLE>,
        ) -> impl core::future::Future<Output = RoleResult<Self::Error>> {
            ctx.pending()
        }
    }

    impl LogicalImage<BridgeCapsule> for crate::site::Local<BridgeImage> {
        type Artifact = WasiImage<'static>;
        type Exit<R> = RunReport<R, Self>;
        type Carrier<'a> = AttachedQueueTestCarrier;

        const IMAGE_ID: ImageId = ImageId(77);
        const SITE_ID: SiteId = SiteId(1);
        const REQUESTED_ROLES: RoleSet = RoleSet::from_bits(0b11);
        const CARRIER: CarrierKind = TEST_ATTACHED_QUEUE_CARRIER;

        fn init() -> Self {
            Self::new()
        }

        fn safe_state(&mut self) {}

        fn carrier<'a>() -> Self::Carrier<'a> {
            AttachedQueueTestCarrier::new()
        }

        fn wasi_guest_storage<'guest, const ROLE: u8>() -> WasiGuestStorage<'guest> {
            assert!(ROLE < 2);
            BRIDGE_WASI_GUEST_ARENA.storage()
        }
    }

    fn push_leb_u32(out: &mut Vec<u8>, mut value: u32) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    fn push_name(out: &mut Vec<u8>, name: &[u8]) {
        push_leb_u32(out, name.len() as u32);
        out.extend_from_slice(name);
    }

    fn push_section(module: &mut Vec<u8>, section: u8, bytes: &[u8]) {
        module.push(section);
        push_leb_u32(module, bytes.len() as u32);
        module.extend_from_slice(bytes);
    }

    fn fd_write_guest_module() -> Vec<u8> {
        let mut module = Vec::new();
        module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

        let mut types = Vec::new();
        push_leb_u32(&mut types, 2);
        types.push(0x60);
        push_leb_u32(&mut types, 4);
        types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32]);
        push_leb_u32(&mut types, 1);
        types.push(VALTYPE_I32);
        types.push(0x60);
        push_leb_u32(&mut types, 0);
        push_leb_u32(&mut types, 0);
        push_section(&mut module, SECTION_TYPE, &types);

        let mut imports = Vec::new();
        push_leb_u32(&mut imports, 1);
        push_name(&mut imports, b"wasi_snapshot_preview1");
        push_name(&mut imports, b"fd_write");
        imports.push(EXTERNAL_KIND_FUNC);
        push_leb_u32(&mut imports, 0);
        push_section(&mut module, SECTION_IMPORT, &imports);

        let mut functions = Vec::new();
        push_leb_u32(&mut functions, 1);
        push_leb_u32(&mut functions, 1);
        push_section(&mut module, SECTION_FUNCTION, &functions);

        push_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);

        let mut exports = Vec::new();
        push_leb_u32(&mut exports, 1);
        push_name(&mut exports, b"_start");
        exports.push(EXTERNAL_KIND_FUNC);
        push_leb_u32(&mut exports, 1);
        push_section(&mut module, SECTION_EXPORT, &exports);

        let mut body = Vec::new();
        push_leb_u32(&mut body, 0);
        body.extend_from_slice(&[
            OPCODE_I32_CONST,
            1,
            OPCODE_I32_CONST,
            0,
            OPCODE_I32_CONST,
            1,
            OPCODE_I32_CONST,
            8,
            OPCODE_CALL,
            0,
            OPCODE_DROP,
            OPCODE_END,
        ]);
        let mut code = Vec::new();
        push_leb_u32(&mut code, 1);
        push_leb_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
        push_section(&mut module, SECTION_CODE, &code);

        let mut segment = [0u8; 21];
        segment[0..4].copy_from_slice(&16u32.to_le_bytes());
        segment[4..8].copy_from_slice(&5u32.to_le_bytes());
        segment[16..21].copy_from_slice(b"hello");
        let mut data = Vec::new();
        push_leb_u32(&mut data, 1);
        push_leb_u32(&mut data, 0);
        data.push(OPCODE_I32_CONST);
        data.push(0);
        data.push(OPCODE_END);
        push_leb_u32(&mut data, segment.len() as u32);
        data.extend_from_slice(&segment);
        push_section(&mut module, SECTION_DATA, &data);

        module
    }

    fn memory_grow_guest_module() -> Vec<u8> {
        let mut module = Vec::new();
        module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

        let mut types = Vec::new();
        push_leb_u32(&mut types, 1);
        types.push(0x60);
        push_leb_u32(&mut types, 0);
        push_leb_u32(&mut types, 0);
        push_section(&mut module, SECTION_TYPE, &types);

        let mut functions = Vec::new();
        push_leb_u32(&mut functions, 1);
        push_leb_u32(&mut functions, 0);
        push_section(&mut module, SECTION_FUNCTION, &functions);

        push_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);

        let mut exports = Vec::new();
        push_leb_u32(&mut exports, 1);
        push_name(&mut exports, b"_start");
        exports.push(EXTERNAL_KIND_FUNC);
        push_leb_u32(&mut exports, 0);
        push_section(&mut module, SECTION_EXPORT, &exports);

        let mut body = Vec::new();
        push_leb_u32(&mut body, 0);
        body.extend_from_slice(&[
            OPCODE_I32_CONST,
            1,
            OPCODE_MEMORY_GROW,
            0,
            OPCODE_DROP,
            OPCODE_END,
        ]);
        let mut code = Vec::new();
        push_leb_u32(&mut code, 1);
        push_leb_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
        push_section(&mut module, SECTION_CODE, &code);

        module
    }

    fn noop_waker() -> Waker {
        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(core::ptr::null(), &VTABLE)
        }

        unsafe fn wake(_: *const ()) {}

        unsafe fn wake_by_ref(_: *const ()) {}

        unsafe fn drop(_: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

        unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
    }

    fn poll_ready<F>(future: F) -> F::Output
    where
        F: core::future::Future,
    {
        if let Some(output) = poll_bounded(future, 16) {
            return output;
        }
        panic!("future did not complete within bounded poll budget");
    }

    fn poll_bounded<F>(future: F, rounds: u8) -> Option<F::Output>
    where
        F: core::future::Future,
    {
        let waker = noop_waker();
        let mut task_context = Context::from_waker(&waker);
        let mut future = core::pin::pin!(future);
        let mut poll_round = 0u8;
        while poll_round < rounds {
            if let Poll::Ready(output) = future.as_mut().poll(&mut task_context) {
                return Some(output);
            }
            poll_round += 1;
        }
        None
    }

    async fn reply_to_fd_write<const ROLE: u8>(ctx: &mut DriverCtx<'_, BridgeCapsule, ROLE>) {
        let branch = ctx
            .endpoint()
            .offer()
            .await
            .expect("driver offers fd_write request branch");
        assert_eq!(branch.label(), LABEL_WASI_FD_WRITE);
        let request = branch
            .decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .await
            .expect("driver decodes fd_write request through endpoint");
        let EngineReq::FdWrite(write) = request else {
            panic!("expected fd_write request");
        };
        assert_eq!(write.fd(), 1);
        assert_eq!(write.as_bytes(), b"hello");
        let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
        ctx.endpoint()
            .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("driver opens fd_write reply flow")
            .send(&reply)
            .await
            .expect("driver sends fd_write reply through endpoint");
    }

    async fn receive_memory_grow_fence<const ROLE: u8>(
        mut ctx: DriverCtx<'_, MemoryGrowCapsule, ROLE>,
    ) {
        let fence = ctx
            .endpoint()
            .recv::<g::Msg<LABEL_MEM_FENCE, MemFence>>()
            .await
            .expect("driver receives memory.grow fence through endpoint");
        assert_eq!(fence.reason(), MemFenceReason::MemoryGrow);
        assert_eq!(fence.new_epoch(), 1);
    }

    const TEST_LOOP_CONTINUE_LABEL: u8 = 120;
    const TEST_LOOP_BREAK_LABEL: u8 = 121;

    type TestLoopContinue =
        g::Msg<{ TEST_LOOP_CONTINUE_LABEL }, GenericCapToken<LoopContinueKind>, LoopContinueKind>;
    type TestLoopBreak =
        g::Msg<{ TEST_LOOP_BREAK_LABEL }, GenericCapToken<LoopBreakKind>, LoopBreakKind>;

    fn no_policy_loop_fd_write_program() -> impl Projectable<DefaultLabelUniverse> {
        g::route(
            g::seq(
                g::send::<g::Role<1>, g::Role<1>, TestLoopContinue, 1>(),
                g::seq(
                    g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                    g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(
                    ),
                ),
            ),
            g::send::<g::Role<1>, g::Role<1>, TestLoopBreak, 1>(),
        )
    }

    fn no_policy_loop_fd_write_poll_program() -> impl Projectable<DefaultLabelUniverse> {
        g::route(
            g::seq(
                g::send::<g::Role<1>, g::Role<1>, TestLoopContinue, 1>(),
                g::seq(
                    g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                    g::seq(
                        g::send::<
                            g::Role<0>,
                            g::Role<1>,
                            g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>,
                            1,
                        >(),
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
                ),
            ),
            g::send::<g::Role<1>, g::Role<1>, TestLoopBreak, 1>(),
        )
    }

    #[test]
    fn no_policy_route_offer_decodes_one_shot_carrier_hint() {
        let program = no_policy_loop_fd_write_program();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            2,
        >::new(&clock);
        let rendezvous = kit
            .add_rendezvous_from_config(
                Config::new(&mut tap_buf, &mut slab, 0..8, 2, CounterClock::new(), None),
                carrier,
            )
            .expect("register appkit carrier rendezvous");
        let session = SessionId::new(0x52);
        let mut driver = kit
            .enter::<0, _>(rendezvous, session, &role0, NoBinding)
            .expect("enter driver role");
        let mut engine = kit
            .enter::<1, _>(rendezvous, session, &role1, NoBinding)
            .expect("enter engine role");

        poll_ready(
            engine
                .flow::<TestLoopContinue>()
                .expect("engine opens loop continue flow")
                .send(()),
        )
        .expect("engine sends loop continue");
        let request = EngineReq::FdWrite(FdWrite::new(3, b"1").expect("fd write request"));
        poll_ready(
            engine
                .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .expect("engine opens fd_write flow")
                .send(&request),
        )
        .expect("engine sends fd_write request");

        let branch = poll_ready(driver.offer()).expect("driver offers fd_write branch");
        assert_eq!(branch.label(), LABEL_WASI_FD_WRITE);
        let observed = poll_ready(branch.decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .expect("driver decodes staged fd_write payload");
        assert_eq!(observed, request);

        let reply = EngineRet::FdWriteDone(FdWriteDone::new(3, 1));
        poll_ready(
            driver
                .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                .expect("driver opens fd_write reply flow")
                .send(&reply),
        )
        .expect("driver sends fd_write reply");
        let observed_reply =
            poll_ready(engine.recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .expect("engine receives fd_write reply");
        assert_eq!(observed_reply, reply);
    }

    #[test]
    fn no_policy_route_offer_decodes_across_split_session_kits() {
        let program = no_policy_loop_fd_write_program();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap0 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut tap1 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab0 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let mut slab1 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock0 = CounterClock::new();
        let clock1 = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let inspector = carrier.clone();
        let kit0 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock0);
        let kit1 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock1);
        let rendezvous0 = kit0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, &mut slab0, 0..8, 1, CounterClock::new(), None),
                carrier.clone(),
            )
            .expect("register driver logical image carrier rendezvous");
        let rendezvous1 = kit1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, &mut slab1, 0..8, 1, CounterClock::new(), None),
                carrier,
            )
            .expect("register engine logical image carrier rendezvous");
        let session = SessionId::new(0x53);
        let mut driver = kit0
            .enter::<0, _>(rendezvous0, session, &role0, NoBinding)
            .expect("enter split driver role");
        let mut engine = kit1
            .enter::<1, _>(rendezvous1, session, &role1, NoBinding)
            .expect("enter split engine role");

        poll_ready(
            engine
                .flow::<TestLoopContinue>()
                .expect("engine opens split loop continue flow")
                .send(()),
        )
        .expect("engine sends split loop continue");
        let request = EngineReq::FdWrite(FdWrite::new(3, b"1").expect("fd write request"));
        poll_ready(
            engine
                .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .expect("engine opens split fd_write flow")
                .send(&request),
        )
        .expect("engine sends split fd_write request");
        assert_eq!(
            inspector.queued_for(0),
            1,
            "split transport must queue the fd_write frame for driver role"
        );

        let maybe_branch = poll_bounded(driver.offer(), 16);
        assert_eq!(
            inspector.queued_for(0),
            0,
            "split offer must consume or requeue the fd_write frame before waiting"
        );
        let (recv_count, hint_count, requeue_count) = inspector.counters();
        assert_eq!(
            recv_count, 1,
            "split offer must receive exactly one frame before materializing"
        );
        assert!(
            hint_count >= 1,
            "split offer must observe at least one non-consuming route hint before materializing"
        );
        assert_eq!(
            requeue_count, 0,
            "split offer must avoid requeue before materializing"
        );
        let branch = maybe_branch
            .expect("driver split offer must complete")
            .expect("driver offers split fd_write branch");
        assert_eq!(branch.label(), LABEL_WASI_FD_WRITE);
        let observed = poll_ready(branch.decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .expect("driver decodes split staged fd_write payload");
        assert_eq!(observed, request);

        let reply = EngineRet::FdWriteDone(FdWriteDone::new(3, 1));
        poll_ready(
            driver
                .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                .expect("driver opens split fd_write reply flow")
                .send(&reply),
        )
        .expect("driver sends split fd_write reply");
        let observed_reply =
            poll_ready(engine.recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .expect("engine receives split fd_write reply");
        assert_eq!(observed_reply, reply);
    }

    #[test]
    fn no_policy_route_offer_advances_from_fd_write_to_poll_across_split_session_kits() {
        let program = no_policy_loop_fd_write_poll_program();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap0 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut tap1 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab0 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let mut slab1 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock0 = CounterClock::new();
        let clock1 = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit0 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock0);
        let kit1 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock1);
        let rendezvous0 = kit0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, &mut slab0, 0..8, 1, CounterClock::new(), None),
                carrier.clone(),
            )
            .expect("register driver logical image carrier rendezvous");
        let rendezvous1 = kit1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, &mut slab1, 0..8, 1, CounterClock::new(), None),
                carrier,
            )
            .expect("register engine logical image carrier rendezvous");
        let session = SessionId::new(0x54);
        let mut driver = kit0
            .enter::<0, _>(rendezvous0, session, &role0, NoBinding)
            .expect("enter split driver role");
        let mut engine = kit1
            .enter::<1, _>(rendezvous1, session, &role1, NoBinding)
            .expect("enter split engine role");

        poll_ready(
            engine
                .flow::<TestLoopContinue>()
                .expect("engine opens split loop continue flow")
                .send(()),
        )
        .expect("engine sends split loop continue");

        let write_request = EngineReq::FdWrite(FdWrite::new(3, b"1").expect("fd write request"));
        poll_ready(
            engine
                .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .expect("engine opens split fd_write flow")
                .send(&write_request),
        )
        .expect("engine sends split fd_write request");
        let write_branch = poll_ready(driver.offer()).expect("driver offers fd_write branch");
        assert_eq!(write_branch.label(), LABEL_WASI_FD_WRITE);
        let observed_write =
            poll_ready(write_branch.decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
                .expect("driver decodes split staged fd_write payload");
        assert_eq!(observed_write, write_request);

        let write_reply = EngineRet::FdWriteDone(FdWriteDone::new(3, 1));
        poll_ready(
            driver
                .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                .expect("driver opens split fd_write reply flow")
                .send(&write_reply),
        )
        .expect("driver sends split fd_write reply");
        let observed_write_reply =
            poll_ready(engine.recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .expect("engine receives split fd_write reply");
        assert_eq!(observed_write_reply, write_reply);

        let poll_request = EngineReq::PollOneoff(PollOneoff::new(1));
        poll_ready(
            engine
                .flow::<g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
                .expect("engine opens split poll_oneoff flow")
                .send(&poll_request),
        )
        .expect("engine sends split poll_oneoff request");
        let observed_poll = poll_ready(driver.recv::<g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>())
            .expect("driver receives split poll_oneoff payload");
        assert_eq!(observed_poll, poll_request);

        let poll_reply = EngineRet::PollReady(PollReady::new(1));
        poll_ready(
            driver
                .flow::<g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
                .expect("driver opens split poll_oneoff reply flow")
                .send(&poll_reply),
        )
        .expect("driver sends split poll_oneoff reply");
        let observed_poll_reply =
            poll_ready(engine.recv::<g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>())
                .expect("engine receives split poll_oneoff reply");
        assert_eq!(observed_poll_reply, poll_reply);
    }

    #[test]
    fn no_policy_route_repeats_fd_write_poll_across_split_session_kits() {
        let program = no_policy_loop_fd_write_poll_program();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap0 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut tap1 = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab0 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let mut slab1 = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock0 = CounterClock::new();
        let clock1 = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit0 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock0);
        let kit1 = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock1);
        let rendezvous0 = kit0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, &mut slab0, 0..8, 1, CounterClock::new(), None),
                carrier.clone(),
            )
            .expect("register repeated driver logical image carrier rendezvous");
        let rendezvous1 = kit1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, &mut slab1, 0..8, 1, CounterClock::new(), None),
                carrier,
            )
            .expect("register repeated engine logical image carrier rendezvous");
        let session = SessionId::new(0x55);
        let mut driver = kit0
            .enter::<0, _>(rendezvous0, session, &role0, NoBinding)
            .expect("enter repeated split driver role");
        let mut engine = kit1
            .enter::<1, _>(rendezvous1, session, &role1, NoBinding)
            .expect("enter repeated split engine role");

        for iteration in 0..20 {
            let continue_send = poll_bounded(
                engine
                    .flow::<TestLoopContinue>()
                    .expect("engine opens repeated loop continue flow")
                    .send(()),
                64,
            );
            assert!(
                continue_send.is_some(),
                "loop continue send did not complete at iteration {iteration}"
            );
            continue_send
                .unwrap()
                .expect("engine sends repeated loop continue");

            let write_request =
                EngineReq::FdWrite(FdWrite::new(3, b"1").expect("fd write request"));
            let write_send = poll_bounded(
                engine
                    .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                    .expect("engine opens repeated fd_write flow")
                    .send(&write_request),
                64,
            );
            assert!(
                write_send.is_some(),
                "fd_write send did not complete at iteration {iteration}"
            );
            write_send
                .unwrap()
                .expect("engine sends repeated fd_write request");

            let write_branch = poll_bounded(driver.offer(), 64);
            assert!(
                write_branch.is_some(),
                "driver offer did not complete at iteration {iteration}"
            );
            let write_branch = write_branch
                .unwrap()
                .expect("driver offers repeated fd_write branch");
            assert_eq!(write_branch.label(), LABEL_WASI_FD_WRITE);
            let observed_write = poll_bounded(
                write_branch.decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>(),
                64,
            );
            assert!(
                observed_write.is_some(),
                "driver fd_write decode did not complete at iteration {iteration}"
            );
            assert_eq!(
                observed_write
                    .unwrap()
                    .expect("driver decodes repeated fd_write payload"),
                write_request
            );

            let write_reply = EngineRet::FdWriteDone(FdWriteDone::new(3, 1));
            let write_reply_send = poll_bounded(
                driver
                    .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                    .expect("driver opens repeated fd_write reply flow")
                    .send(&write_reply),
                64,
            );
            assert!(
                write_reply_send.is_some(),
                "driver fd_write reply send did not complete at iteration {iteration}"
            );
            write_reply_send
                .unwrap()
                .expect("driver sends repeated fd_write reply");
            let observed_write_reply = poll_bounded(
                engine.recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>(),
                64,
            );
            assert!(
                observed_write_reply.is_some(),
                "engine fd_write reply recv did not complete at iteration {iteration}"
            );
            assert_eq!(
                observed_write_reply
                    .unwrap()
                    .expect("engine receives repeated fd_write reply"),
                write_reply
            );

            let poll_request = EngineReq::PollOneoff(PollOneoff::new(1));
            let poll_send = poll_bounded(
                engine
                    .flow::<g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
                    .expect("engine opens repeated poll_oneoff flow")
                    .send(&poll_request),
                64,
            );
            assert!(
                poll_send.is_some(),
                "poll_oneoff send did not complete at iteration {iteration}"
            );
            poll_send
                .unwrap()
                .expect("engine sends repeated poll_oneoff request");
            let observed_poll = poll_bounded(
                driver.recv::<g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>(),
                64,
            );
            assert!(
                observed_poll.is_some(),
                "driver poll_oneoff recv did not complete at iteration {iteration}"
            );
            assert_eq!(
                observed_poll
                    .unwrap()
                    .expect("driver receives repeated poll_oneoff payload"),
                poll_request
            );

            let poll_reply = EngineRet::PollReady(PollReady::new(1));
            let poll_reply_send = poll_bounded(
                driver
                    .flow::<g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
                    .expect("driver opens repeated poll_oneoff reply flow")
                    .send(&poll_reply),
                64,
            );
            assert!(
                poll_reply_send.is_some(),
                "driver poll_oneoff reply send did not complete at iteration {iteration}"
            );
            poll_reply_send
                .unwrap()
                .expect("driver sends repeated poll_oneoff reply");
            let observed_poll_reply = poll_bounded(
                engine.recv::<g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>(),
                64,
            );
            assert!(
                observed_poll_reply.is_some(),
                "engine poll_oneoff reply recv did not complete at iteration {iteration}"
            );
            assert_eq!(
                observed_poll_reply
                    .unwrap()
                    .expect("engine receives repeated poll_oneoff reply"),
                poll_reply
            );
        }
    }

    #[test]
    fn drive_wasi_guest_completes_import_only_through_endpoint_carrier() {
        let module = fd_write_guest_module();
        let program = <BridgeCapsule as Capsule>::choreography();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            2,
        >::new(&clock);
        let rendezvous = kit
            .add_rendezvous_from_config(
                Config::new(&mut tap_buf, &mut slab, 0..8, 2, CounterClock::new(), None),
                carrier,
            )
            .expect("register appkit carrier rendezvous");
        let session = SessionId::new(0x51);
        let engine_endpoint = kit
            .enter::<0, _>(rendezvous, session, &role0, NoBinding)
            .expect("enter engine role");
        let driver_endpoint = kit
            .enter::<1, _>(rendezvous, session, &role1, NoBinding)
            .expect("enter driver role");
        let projection = derive_projection_caps_from_program::<BridgeCapsule>(&program);
        let endpoint_carrier = EndpointCarrierFacts::new(
            ImageId(1),
            SiteId(1),
            RoleSet::from_bits(0b11),
            TEST_ATTACHED_QUEUE_CARRIER,
            projection,
        );
        let mut engine_ctx: EngineCtx<'_, '_, BridgeCapsule, 0> = EngineCtx::new(
            RoleEndpointCtx::new(engine_endpoint),
            endpoint_carrier,
            Some(module.as_slice()),
            <crate::site::Local<BridgeImage> as LogicalImage<BridgeCapsule>>::wasi_guest_storage::<0>(
            ),
        );
        let mut driver_ctx: DriverCtx<'_, BridgeCapsule, 1> = DriverCtx::new(
            RoleEndpointCtx::new(driver_endpoint),
            endpoint_carrier,
            DriverFacts::EMPTY,
        );
        let engine = engine_ctx.drive_wasi_guest(BudgetRun::new(1, 0, 128, 0));
        let driver = reply_to_fd_write(&mut driver_ctx);
        let waker = noop_waker();
        let mut task_context = Context::from_waker(&waker);
        let mut engine = core::pin::pin!(engine);
        let mut driver = core::pin::pin!(driver);
        let mut engine_result = None;
        let mut driver_done = false;
        let mut poll_round = 0u8;
        while poll_round < 16 {
            if engine_result.is_none() {
                if let Poll::Ready(result) = engine.as_mut().poll(&mut task_context) {
                    engine_result = Some(result);
                }
            }
            if !driver_done {
                if let Poll::Ready(()) = driver.as_mut().poll(&mut task_context) {
                    driver_done = true;
                }
            }
            if engine_result.is_some() && driver_done {
                break;
            }
            poll_round += 1;
        }
        assert!(
            driver_done,
            "driver did not receive fd_write request; engine_result={engine_result:?}"
        );
        assert!(matches!(engine_result, Some(Ok(WasiGuestStatus::Done))));
    }

    #[test]
    fn drive_wasi_guest_requires_projected_loop_control_phase() {
        let module = fd_write_guest_module();
        let program = <NoLoopCapsule as Capsule>::choreography();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            1,
        >::new(&clock);
        let rendezvous = kit
            .add_rendezvous_from_config(
                Config::new(&mut tap_buf, &mut slab, 0..8, 1, CounterClock::new(), None),
                carrier,
            )
            .expect("register appkit carrier rendezvous");
        let session = SessionId::new(0x56);
        let engine_endpoint = kit
            .enter::<0, _>(rendezvous, session, &role0, NoBinding)
            .expect("enter no-loop engine role");
        let projection = derive_projection_caps_from_program::<NoLoopCapsule>(&program);
        let endpoint_carrier = EndpointCarrierFacts::new(
            ImageId(3),
            SiteId(1),
            RoleSet::single(0),
            TEST_ATTACHED_QUEUE_CARRIER,
            projection,
        );
        let mut engine_ctx: EngineCtx<'_, '_, NoLoopCapsule, 0> = EngineCtx::new(
            RoleEndpointCtx::new(engine_endpoint),
            endpoint_carrier,
            Some(module.as_slice()),
            NO_LOOP_WASI_GUEST_ARENA.storage(),
        );
        let result = poll_bounded(
            engine_ctx.drive_wasi_guest(BudgetRun::new(1, 0, 128, 0)),
            16,
        )
        .expect("missing loop-control phase must fail instead of falling back");
        assert!(matches!(
            result,
            Err(WasiGuestError::EndpointRejected(0x5745_6000))
        ));
    }

    #[test]
    fn drive_wasi_guest_completes_memory_grow_after_mem_fence_endpoint_carrier() {
        let module = memory_grow_guest_module();
        let program = <MemoryGrowCapsule as Capsule>::choreography();
        let role0 = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
        let role1 = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
        let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); APPKIT_ATTACH_TAP_EVENTS];
        let mut slab = [0u8; APPKIT_ATTACH_SLAB_BYTES];
        let clock = CounterClock::new();
        let carrier = AttachedQueueTestCarrier::new();
        let kit = hibana::integration::SessionKit::<
            AttachedQueueTestCarrier,
            DefaultLabelUniverse,
            CounterClock,
            2,
        >::new(&clock);
        let rendezvous = kit
            .add_rendezvous_from_config(
                Config::new(&mut tap_buf, &mut slab, 0..8, 2, CounterClock::new(), None),
                carrier,
            )
            .expect("register appkit carrier rendezvous");
        let session = SessionId::new(0x55);
        let engine_endpoint = kit
            .enter::<0, _>(rendezvous, session, &role0, NoBinding)
            .expect("enter memory grow engine role");
        let driver_endpoint = kit
            .enter::<1, _>(rendezvous, session, &role1, NoBinding)
            .expect("enter memory grow driver role");
        let projection = derive_projection_caps_from_program::<MemoryGrowCapsule>(&program);
        let endpoint_carrier = EndpointCarrierFacts::new(
            ImageId(2),
            SiteId(1),
            RoleSet::from_bits(0b11),
            TEST_ATTACHED_QUEUE_CARRIER,
            projection,
        );
        let mut engine_ctx: EngineCtx<'_, '_, MemoryGrowCapsule, 0> = EngineCtx::new(
            RoleEndpointCtx::new(engine_endpoint),
            endpoint_carrier,
            Some(module.as_slice()),
            MEMORY_GROW_WASI_GUEST_ARENA.storage(),
        );
        let driver_ctx: DriverCtx<'_, MemoryGrowCapsule, 1> = DriverCtx::new(
            RoleEndpointCtx::new(driver_endpoint),
            endpoint_carrier,
            DriverFacts::EMPTY,
        );
        let engine = engine_ctx.drive_wasi_guest(BudgetRun::new(1, 0, 128, 0));
        let driver = receive_memory_grow_fence(driver_ctx);
        let waker = noop_waker();
        let mut task_context = Context::from_waker(&waker);
        let mut engine = core::pin::pin!(engine);
        let mut driver = core::pin::pin!(driver);
        let mut engine_result = None;
        let mut driver_done = false;
        let mut poll_round = 0u8;
        while poll_round < 16 {
            if engine_result.is_none() {
                if let Poll::Ready(result) = engine.as_mut().poll(&mut task_context) {
                    engine_result = Some(result);
                }
            }
            if !driver_done {
                if let Poll::Ready(()) = driver.as_mut().poll(&mut task_context) {
                    driver_done = true;
                }
            }
            if engine_result.is_some() && driver_done {
                break;
            }
            poll_round += 1;
        }
        assert!(
            driver_done,
            "driver did not receive memory.grow fence; engine_result={engine_result:?}"
        );
        assert!(matches!(engine_result, Some(Ok(WasiGuestStatus::Done))));
    }

    #[test]
    fn run_drives_wasi_guest_import_completion_through_endpoint_carrier() {
        RUN_BRIDGE_ENGINE_DONE.store(0, Ordering::SeqCst);
        RUN_BRIDGE_DRIVER_DONE.store(0, Ordering::SeqCst);
        let module = fd_write_guest_module().into_boxed_slice();
        let module: &'static [u8] = Box::leak(module);
        let report =
            run::<crate::site::Local<BridgeImage>, BridgeCapsule>(WasiImage::from_static(module));
        assert_eq!(report.attached_endpoint_count(), 2);
        assert_eq!(report.wasi_completion_pair_count(), 1);
        let manifest = report.manifest();
        assert!(
            manifest.labels[..manifest.label_count as usize]
                .contains(&crate::choreography::protocol::LABEL_WASI_IMPORT_LOOP_CONTINUE_CONTROL),
            "auto loop-control must be visible in projection metadata"
        );
        assert_eq!(RUN_BRIDGE_DRIVER_DONE.load(Ordering::SeqCst), 1);
        assert_eq!(
            RUN_BRIDGE_ENGINE_DONE.load(Ordering::SeqCst),
            0,
            "WASI P1 engine roles are driven by appkit, not user Localside::engine"
        );
    }
}
