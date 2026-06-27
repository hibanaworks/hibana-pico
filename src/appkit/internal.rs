//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose engine internals,
//! or finish WASI P1 imports outside projected endpoint/carrier progress.

use core::{
    convert::Infallible,
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    mem::{align_of, size_of},
    num::NonZeroU32,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
use core::mem::MaybeUninit;

#[cfg(feature = "wasm-engine-core")]
use core::mem::ManuallyDrop;

#[cfg(any(feature = "wasm-engine-core", all(not(test), target_os = "none")))]
use core::cell::UnsafeCell;

#[cfg(feature = "wasm-engine-core")]
use hibana_wasip1_runtime::{
    WasiImport, WasiImportCompletion, WasiImportRequest,
    protocol::{self, BudgetRun},
};

#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ATTACH_SLAB_BYTES: usize = 262_144;
#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ROLE_FUTURE_ALIGN: usize = 16;
#[cfg(any(test, not(target_os = "none")))]
const APPKIT_ROLE_FUTURE_BYTES: usize = 8 * 1024;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_FUTURE_ALIGN: usize = 16;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_ROLE_FUTURE_BYTES: usize = 4 * 1024;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_ROLE_SLOTS: usize = 4;
#[cfg(all(not(test), target_os = "none"))]
const APPKIT_EMBEDDED_FUTURE_BYTES: usize =
    APPKIT_EMBEDDED_ROLE_FUTURE_BYTES * APPKIT_EMBEDDED_ROLE_SLOTS;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_WASI_GUEST_ARENA_ALIGN: usize = 16;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_WASI_GUEST_STORAGE_BYTES: usize =
    size_of::<hibana_wasip1_runtime::HibanaWasiGuestStorage<'static>>();
#[cfg(feature = "wasm-engine-core")]
const APPKIT_WASI_GUEST_MEMORY_BYTES: usize = hibana_wasip1_runtime::DEFAULT_GUEST_MEMORY_BYTES;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION: u32 = 1_000_000;

const APPKIT_DEFAULT_SESSION_ID: NonZeroU32 = nonzero_session_id(1);

const fn nonzero_session_id(raw: u32) -> NonZeroU32 {
    match NonZeroU32::new(raw) {
        Some(session) => session,
        None => panic!("appkit session id must be nonzero"),
    }
}

/// Current typed hibana role domain: `Role<0>` through `Role<15>`.
///
/// Raising this is a hibana representation change, not an appkit knob. The
/// carrier materialization deliberately follows the typed projection domain so
/// one logical image cannot request roles appkit cannot project.
const HIBANA_TYPED_ROLE_DOMAIN_SIZE: u8 = 16;
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

#[derive(Clone, Copy)]
enum RoleTaskError<E> {
    Localside(E),
    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    Wasi(WasiGuestError),
}

impl<E> Debug for RoleTaskError<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Localside(error) => formatter.debug_tuple("Localside").field(error).finish(),
            #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
            Self::Wasi(error) => formatter.debug_tuple("Wasi").field(error).finish(),
        }
    }
}

struct LocalsideRoleTask<F, E> {
    future: F,
    marker: PhantomData<fn() -> E>,
}

impl<F, E> LocalsideRoleTask<F, E> {
    const fn new(future: F) -> Self {
        Self {
            future,
            marker: PhantomData,
        }
    }
}

impl<F, E> Future for LocalsideRoleTask<F, E>
where
    F: core::future::Future<Output = RoleResult<E>>,
{
    type Output = RoleResult<RoleTaskError<E>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut state.future) };
        match future.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(done)) => match done {},
            Poll::Ready(Err(error)) => Poll::Ready(Err(RoleTaskError::Localside(error))),
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
struct WasiRoleTask<F, E> {
    future: F,
    marker: PhantomData<fn() -> E>,
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
impl<F, E> WasiRoleTask<F, E> {
    const fn new(future: F) -> Self {
        Self {
            future,
            marker: PhantomData,
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
impl<F, E> Future for WasiRoleTask<F, E>
where
    F: core::future::Future<Output = RoleResult<WasiGuestError>>,
{
    type Output = RoleResult<RoleTaskError<E>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut state.future) };
        match future.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(done)) => match done {},
            Poll::Ready(Err(error)) => Poll::Ready(Err(RoleTaskError::Wasi(error))),
        }
    }
}

fn localside_role_task<F, E>(future: F) -> LocalsideRoleTask<F, E>
where
    F: core::future::Future<Output = RoleResult<E>>,
{
    LocalsideRoleTask::new(future)
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
fn wasi_role_task<F, E>(future: F) -> WasiRoleTask<F, E>
where
    F: core::future::Future<Output = RoleResult<WasiGuestError>>,
{
    WasiRoleTask::new(future)
}

#[cfg(all(not(test), target_os = "none"))]
#[repr(C, align(16))]
struct EmbeddedFutureArena<const N: usize> {
    bytes: UnsafeCell<[u8; N]>,
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

#[cfg(all(not(test), target_os = "none"))]
#[repr(C, align(16))]
pub struct EmbeddedAttachStorage<const SLAB_BYTES: usize> {
    slab: UnsafeCell<[u8; SLAB_BYTES]>,
    future: EmbeddedFutureArena<APPKIT_EMBEDDED_FUTURE_BYTES>,
}

#[cfg(all(not(test), target_os = "none"))]
#[derive(Clone, Copy)]
pub struct EmbeddedAttachStorageRef<'a> {
    slab: *mut [u8],
    future: *mut u8,
    future_bytes: usize,
    marker: PhantomData<&'a mut ()>,
}

#[cfg(all(not(test), target_os = "none"))]
impl<const SLAB_BYTES: usize> EmbeddedAttachStorage<SLAB_BYTES> {
    pub const fn empty() -> Self {
        Self {
            slab: UnsafeCell::new([0; SLAB_BYTES]),
            future: EmbeddedFutureArena::EMPTY,
        }
    }

    pub fn lease(&'static self) -> EmbeddedAttachStorageRef<'static> {
        let slab = unsafe { &mut *self.slab.get() };
        slab.fill(0);
        EmbeddedAttachStorageRef {
            slab,
            future: self.future.as_mut_ptr(),
            future_bytes: APPKIT_EMBEDDED_FUTURE_BYTES,
            marker: PhantomData,
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
unsafe impl<const SLAB_BYTES: usize> Sync for EmbeddedAttachStorage<SLAB_BYTES> {}

#[repr(C, align(16))]
#[cfg(feature = "wasm-engine-core")]
pub struct WasiGuestArena {
    storage: UnsafeCell<[u8; APPKIT_WASI_GUEST_STORAGE_BYTES]>,
    memory: UnsafeCell<[u8; APPKIT_WASI_GUEST_MEMORY_BYTES]>,
    occupied: UnsafeCell<bool>,
    owner: PhantomData<*mut ()>,
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestArena {
    const EMPTY: Self = Self {
        storage: UnsafeCell::new([0; APPKIT_WASI_GUEST_STORAGE_BYTES]),
        memory: UnsafeCell::new([0; APPKIT_WASI_GUEST_MEMORY_BYTES]),
        occupied: UnsafeCell::new(false),
        owner: PhantomData,
    };

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    fn assert_guest_alignment() {
        assert!(
            align_of::<hibana_wasip1_runtime::HibanaWasiGuestStorage<'static>>()
                <= APPKIT_WASI_GUEST_ARENA_ALIGN,
            "WASI guest arena alignment is too small"
        );
    }

    /// Lease this arena through its single physical owner.
    ///
    /// This is storage for one WASI VM instance, not a shared protocol channel.
    /// The caller must own the logical image-local arena as ordinary Rust
    /// mutable state before calling this method.
    pub fn lease<'guest>(&'guest mut self) -> WasiGuestLease<'guest> {
        Self::assert_guest_alignment();
        unsafe {
            assert!(!*self.occupied.get(), "WASI guest arena is already leased");
            *self.occupied.get() = true;
        }
        WasiGuestLease {
            occupied: self.occupied.get(),
            storage: unsafe { (*self.storage.get()).as_mut_ptr().cast() },
            memory: self.memory.get(),
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
unsafe impl<const N: usize> Sync for EmbeddedFutureArena<N> {}

#[cfg(feature = "wasm-engine-core")]
pub struct WasiGuestLease<'guest> {
    occupied: *mut bool,
    storage: *mut hibana_wasip1_runtime::HibanaWasiGuestStorage<'guest>,
    memory: *mut [u8; APPKIT_WASI_GUEST_MEMORY_BYTES],
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestLease<'guest> {
    fn storage_ptr(&mut self) -> *mut hibana_wasip1_runtime::HibanaWasiGuestStorage<'guest> {
        self.storage
    }

    fn memory_ptr(&mut self) -> *mut [u8; APPKIT_WASI_GUEST_MEMORY_BYTES] {
        self.memory
    }
}

#[cfg(feature = "wasm-engine-core")]
impl Drop for WasiGuestLease<'_> {
    fn drop(&mut self) {
        unsafe {
            *self.occupied = false;
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
#[inline(always)]
fn embedded_wait_for_event() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
    }
    #[cfg(not(target_arch = "arm"))]
    core::hint::spin_loop();
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
type AppkitWasiMetricClock = extern "C" fn() -> u32;

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_METRIC_CLOCK: usize = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_METRIC_ENABLED: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_RESUME_COUNT: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_RESUME_LAST_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_RESUME_TOTAL_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_RESUME_MAX_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_REQUEST_SEND_COUNT: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_REQUEST_SEND_LAST_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_REQUEST_SEND_TOTAL_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_REQUEST_SEND_MAX_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETION_RECV_COUNT: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETION_RECV_LAST_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETION_RECV_TOTAL_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETION_RECV_MAX_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETE_COUNT: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETE_LAST_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETE_TOTAL_US: u32 = 0;
#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_APPKIT_WASI_COMPLETE_MAX_US: u32 = 0;

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn appkit_wasi_metric_start() -> Option<u32> {
    unsafe {
        if core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_APPKIT_WASI_METRIC_ENABLED)) == 0 {
            return None;
        }
        let raw_clock =
            core::ptr::read_volatile(core::ptr::addr_of!(HIBANA_APPKIT_WASI_METRIC_CLOCK));
        if raw_clock == 0 {
            return None;
        }
        let clock: AppkitWasiMetricClock = core::mem::transmute(raw_clock);
        Some(clock())
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn record_appkit_wasi_metric(
    count: *mut u32,
    last: *mut u32,
    total: *mut u32,
    max: *mut u32,
    start: Option<u32>,
) {
    let Some(start) = start else {
        return;
    };
    let Some(end) = appkit_wasi_metric_start() else {
        return;
    };
    let elapsed = end.wrapping_sub(start);
    unsafe {
        let next_count = core::ptr::read_volatile(count).saturating_add(1);
        core::ptr::write_volatile(count, next_count);
        core::ptr::write_volatile(last, elapsed);
        let next_total = core::ptr::read_volatile(total).saturating_add(elapsed);
        core::ptr::write_volatile(total, next_total);
        if elapsed > core::ptr::read_volatile(max) {
            core::ptr::write_volatile(max, elapsed);
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn record_appkit_wasi_resume(start: Option<u32>) {
    record_appkit_wasi_metric(
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_RESUME_COUNT),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_RESUME_LAST_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_RESUME_TOTAL_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_RESUME_MAX_US),
        start,
    );
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn record_appkit_wasi_request_send(start: Option<u32>) {
    record_appkit_wasi_metric(
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_REQUEST_SEND_COUNT),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_REQUEST_SEND_LAST_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_REQUEST_SEND_TOTAL_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_REQUEST_SEND_MAX_US),
        start,
    );
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn record_appkit_wasi_completion_recv(start: Option<u32>) {
    record_appkit_wasi_metric(
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETION_RECV_COUNT),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETION_RECV_LAST_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETION_RECV_TOTAL_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETION_RECV_MAX_US),
        start,
    );
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn record_appkit_wasi_complete(start: Option<u32>) {
    record_appkit_wasi_metric(
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETE_COUNT),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETE_LAST_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETE_TOTAL_US),
        core::ptr::addr_of_mut!(HIBANA_APPKIT_WASI_COMPLETE_MAX_US),
        start,
    );
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[inline(always)]
fn embedded_task_waker(woke: &mut bool) -> Waker {
    let raw_waker = RawWaker::new(
        core::ptr::addr_of_mut!(*woke).cast::<()>(),
        &WAKE_FLAG_WAKER_VTABLE,
    );
    unsafe {
        // SAFETY: The raw waker points to `woke`, which lives for one poll
        // pass. The vtable only writes `true` to that flag.
        Waker::from_raw(raw_waker)
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn poll_embedded_wasi_unit<F, E>(
    mut future: F,
    tasks: &mut EmbeddedScheduledTasks<'_, E>,
) -> Result<(), hibana::EndpointError>
where
    F: core::future::Future<Output = Result<(), hibana::EndpointError>>,
    E: Debug,
{
    let mut pinned = unsafe {
        // SAFETY: The future is stored in this stack frame and is never moved
        // while the pinned handle is used.
        Pin::new_unchecked(&mut future)
    };
    loop {
        let mut woke = false;
        let task_waker = embedded_task_waker(&mut woke);
        let mut task_context = Context::from_waker(&task_waker);
        match pinned.as_mut().poll(&mut task_context) {
            Poll::Ready(result) => return result,
            Poll::Pending => {
                if tasks.has_tasks() {
                    tasks.poll_once(&mut task_context);
                }
                if !woke {
                    embedded_wait_for_event();
                }
            }
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn poll_embedded_wasi_value<T, F, E>(
    mut future: F,
    output: &mut MaybeUninit<T>,
    tasks: &mut EmbeddedScheduledTasks<'_, E>,
) -> Result<(), hibana::EndpointError>
where
    F: core::future::Future<Output = Result<T, hibana::EndpointError>>,
    E: Debug,
{
    let mut pinned = unsafe {
        // SAFETY: The future is stored in this stack frame and is never moved
        // while the pinned handle is used.
        Pin::new_unchecked(&mut future)
    };
    loop {
        let mut woke = false;
        let task_waker = embedded_task_waker(&mut woke);
        let mut task_context = Context::from_waker(&task_waker);
        match pinned.as_mut().poll(&mut task_context) {
            Poll::Ready(Ok(value)) => {
                output.write(value);
                return Ok(());
            }
            Poll::Ready(Err(error)) => return Err(error),
            Poll::Pending => {
                if tasks.has_tasks() {
                    tasks.poll_once(&mut task_context);
                }
                if !woke {
                    embedded_wait_for_event();
                }
            }
        }
    }
}

/// Localside execution class assigned to one projected role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleKind {
    Engine,
    Driver,
    Boundary,
}

/// Requested projection slice for a logical image.
///
/// This is not protocol admission. The requested roles must match capsule
/// placement and the concrete hibana `RoleProgram` witnesses materialized for
/// the logical image before it is attached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleSet {
    bits: u16,
}

impl RoleSet {
    const EMPTY: Self = Self { bits: 0 };

    pub const fn single(role: u8) -> Self {
        assert!(role < HIBANA_TYPED_ROLE_DOMAIN_SIZE);
        Self { bits: 1u16 << role }
    }

    pub const fn from_bits(bits: u16) -> Self {
        assert!(bits != 0);
        Self { bits }
    }

    const fn count(self) -> u8 {
        self.bits.count_ones() as u8
    }

    const fn contains(self, role: u8) -> bool {
        role < HIBANA_TYPED_ROLE_DOMAIN_SIZE && (self.bits & (1u16 << role)) != 0
    }

    const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    const fn is_subset_of(self, other: Self) -> bool {
        (self.bits & !other.bits) == 0
    }
}

/// Roles currently accepted by typed hibana projection.
const HIBANA_TYPED_ROLE_DOMAIN: RoleSet = RoleSet::from_bits(0xffff);

/// Borrowed WASI artifact supplied to a capsule run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasiImage<'a> {
    bytes: &'a [u8],
}

impl<'a> WasiImage<'a> {
    pub const fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

/// Marker for capsules whose selected logical image embeds no WASI P1 guest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoWasi;

/// Runtime placement facts for a capsule.
///
/// Placement decides location, not protocol legality.
pub trait Placement<C: Capsule> {
    fn role_kind<const ROLE: u8>() -> RoleKind;
}

/// Resolver registration surface for Capsule-local hibana policy points.
pub trait ResolverRegistry<'cfg, C: Capsule, const ROLE: u8> {
    fn resolver<const POLICY: u16>(
        &mut self,
        resolver: hibana::runtime::resolver::ResolverRef<'cfg, POLICY>,
    );
}

/// A projectable raw hibana choreography plus its placement and localside code.
pub trait Capsule: Sized {
    type Placement: Placement<Self>;
    type Localside: Localside<Self>;

    const SESSION_ID: NonZeroU32 = APPKIT_DEFAULT_SESSION_ID;

    fn choreography() -> impl hibana::runtime::program::Projectable;

    fn register_resolvers<'cfg, R, const ROLE: u8>(_: &mut R)
    where
        R: ResolverRegistry<'cfg, Self, ROLE>,
    {
    }

    fn observe(_: &mut hibana::runtime::tap::TapPort<'_>) {}
}

/// Private artifact boundary consumed by [`run`].
///
/// User code passes `WasiImage` or `NoWasi`; it cannot implement new artifact
/// authority. Static WASI import tables are load evidence only, never
/// choreography admission authority.
/// `NoWasi` never leases storage. `WasiImage` requires the selected logical
/// image to implement [`WasiGuestImage`].
trait ArtifactInput<I: LogicalImage> {
    #[cfg(feature = "wasm-engine-core")]
    fn wasi_bytes(&self) -> Option<&[u8]>;

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> Option<WasiGuestLease<'guest>>;

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        BudgetRun::new(1, 0, APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION)
    }
}

/// One projection-derived logical site image.
pub trait LogicalImage: Sized {
    type Capsule: Capsule;

    type Carrier<'a>: hibana::runtime::transport::Transport + 'a
    where
        Self: 'a,
        Self::Capsule: 'a;

    const REQUESTED_ROLES: RoleSet;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        Self::Capsule: 'a;
    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> EmbeddedAttachStorageRef<'static>;
}

/// Site-local storage facts required only by logical images that actually run a WASI guest.
#[cfg(feature = "wasm-engine-core")]
pub trait WasiGuestImage: LogicalImage {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> WasiGuestLease<'guest>;

    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        BudgetRun::new(1, 0, APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION)
    }
}

/// Requested roles that were materialized as hibana RoleProgram values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn visit<const ROLE: u8>(&mut self, program: hibana::runtime::program::RoleProgram<ROLE>);
}

impl<C> ProjectedRoleVisitor<C> for ProjectedRoles
where
    C: Capsule,
{
    fn visit<const ROLE: u8>(&mut self, program: hibana::runtime::program::RoleProgram<ROLE>) {
        let role_program_size = core::mem::size_of_val(&program);
        assert!(
            role_program_size > 0,
            "projected RoleProgram witness must be materialized"
        );
        self.roles = self.roles.union(RoleSet::single(ROLE));
        self.count = self
            .count
            .checked_add(1)
            .expect("projected RoleProgram count must not overflow");
    }
}

fn visit_projected_role<C, V, const ROLE: u8>(
    program: &impl hibana::runtime::program::Projectable,
    visitor: &mut V,
) where
    C: Capsule,
    V: ProjectedRoleVisitor<C>,
{
    visitor.visit::<ROLE>(hibana::runtime::program::project::<ROLE, _>(program));
}

fn visit_requested_projected_roles<C, V>(
    program: &impl hibana::runtime::program::Projectable,
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
fn collect_projected_roles<I>(
    program: &impl hibana::runtime::program::Projectable,
) -> ProjectedRoles
where
    I: LogicalImage,
{
    let mut projected = ProjectedRoles::new();
    visit_requested_projected_roles::<I::Capsule, _>(program, I::REQUESTED_ROLES, &mut projected);
    projected
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasiGuestStatus {
    Exit,
    BudgetExpired,
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmbeddedWasiGuestStatus {
    Exit,
    BudgetExpired,
    TerminalEndpoint,
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasiGuestError {
    _private: (),
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestError {
    const fn rejected() -> Self {
        Self { _private: () }
    }
}

#[cfg(feature = "wasm-engine-core")]
fn wasi_runtime_result<T>(
    result: Result<T, hibana_wasip1_runtime::exchange::ExchangeError>,
) -> Result<T, WasiGuestError> {
    result.map_err(|_| WasiGuestError::rejected())
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
fn wasi_endpoint_result<T>(result: Result<T, hibana::EndpointError>) -> Result<T, WasiGuestError> {
    result.map_err(|_| WasiGuestError::rejected())
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn wasi_endpoint_terminal(_error: &hibana::EndpointError) -> bool {
    true
}

#[cfg(feature = "wasm-engine-core")]
async fn send_wasi_import_request<const ROLE: u8>(
    endpoint: &mut hibana::Endpoint<'_, ROLE>,
    request: WasiImportRequest,
) -> Result<(), hibana::EndpointError> {
    match request {
        WasiImportRequest::FdWrite(request) => {
            endpoint.send::<protocol::FdWriteReqMsg>(&request).await
        }
        WasiImportRequest::FdWriteObject(request) => {
            endpoint
                .send::<protocol::FdWriteObjectReqMsg>(&request)
                .await
        }
        WasiImportRequest::FdRead(request) => {
            endpoint.send::<protocol::FdReadReqMsg>(&request).await
        }
        WasiImportRequest::FdReaddir(request) => {
            endpoint.send::<protocol::FdReaddirReqMsg>(&request).await
        }
        WasiImportRequest::PathOpen(request) => {
            endpoint.send::<protocol::PathOpenReqMsg>(&request).await
        }
        WasiImportRequest::FdPrestatGet(request) => {
            endpoint
                .send::<protocol::FdPrestatGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::FdPrestatDirName(request) => {
            endpoint
                .send::<protocol::FdPrestatDirNameReqMsg>(&request)
                .await
        }
        WasiImportRequest::FdFilestatGet(request) => {
            endpoint
                .send::<protocol::FdFilestatGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::ArgsSizesGet(request) => {
            endpoint
                .send::<protocol::ArgsSizesGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::ArgsGet(request) => {
            endpoint.send::<protocol::ArgsGetReqMsg>(&request).await
        }
        WasiImportRequest::EnvironSizesGet(request) => {
            endpoint
                .send::<protocol::EnvironSizesGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::EnvironGet(request) => {
            endpoint.send::<protocol::EnvironGetReqMsg>(&request).await
        }
        WasiImportRequest::FdFdstatGet(request) => {
            endpoint.send::<protocol::FdFdstatGetReqMsg>(&request).await
        }
        WasiImportRequest::PathFilestatGet(request) => {
            endpoint
                .send::<protocol::PathFilestatGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::FdClose(request) => {
            endpoint.send::<protocol::FdCloseReqMsg>(&request).await
        }
        WasiImportRequest::ClockResGet(request) => {
            endpoint.send::<protocol::ClockResGetReqMsg>(&request).await
        }
        WasiImportRequest::ClockTimeGet(request) => {
            endpoint
                .send::<protocol::ClockTimeGetReqMsg>(&request)
                .await
        }
        WasiImportRequest::PollOneoff(request) => {
            endpoint.send::<protocol::PollOneoffReqMsg>(&request).await
        }
        WasiImportRequest::RandomGet(request) => {
            endpoint.send::<protocol::RandomGetReqMsg>(&request).await
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
async fn recv_wasi_import_completion<const ROLE: u8>(
    endpoint: &mut hibana::Endpoint<'_, ROLE>,
    import: WasiImport,
) -> Result<WasiImportCompletion, hibana::EndpointError> {
    match import {
        WasiImport::FdWrite => endpoint
            .recv::<protocol::FdWriteRetMsg>()
            .await
            .map(WasiImportCompletion::FdWrite),
        WasiImport::FdWriteObject => endpoint
            .recv::<protocol::FdWriteObjectRetMsg>()
            .await
            .map(WasiImportCompletion::FdWriteObject),
        WasiImport::FdRead => endpoint
            .recv::<protocol::FdReadRetMsg>()
            .await
            .map(WasiImportCompletion::FdRead),
        WasiImport::FdReaddir => endpoint
            .recv::<protocol::FdReaddirRetMsg>()
            .await
            .map(WasiImportCompletion::FdReaddir),
        WasiImport::PathOpen => endpoint
            .recv::<protocol::PathOpenRetMsg>()
            .await
            .map(WasiImportCompletion::PathOpen),
        WasiImport::FdPrestatGet => endpoint
            .recv::<protocol::FdPrestatGetRetMsg>()
            .await
            .map(WasiImportCompletion::FdPrestatGet),
        WasiImport::FdPrestatDirName => endpoint
            .recv::<protocol::FdPrestatDirNameRetMsg>()
            .await
            .map(WasiImportCompletion::FdPrestatDirName),
        WasiImport::FdFilestatGet => endpoint
            .recv::<protocol::FdFilestatGetRetMsg>()
            .await
            .map(WasiImportCompletion::FdFilestatGet),
        WasiImport::ArgsSizesGet => endpoint
            .recv::<protocol::ArgsSizesGetRetMsg>()
            .await
            .map(WasiImportCompletion::ArgsSizesGet),
        WasiImport::ArgsGet => endpoint
            .recv::<protocol::ArgsGetRetMsg>()
            .await
            .map(WasiImportCompletion::ArgsGet),
        WasiImport::EnvironSizesGet => endpoint
            .recv::<protocol::EnvironSizesGetRetMsg>()
            .await
            .map(WasiImportCompletion::EnvironSizesGet),
        WasiImport::EnvironGet => endpoint
            .recv::<protocol::EnvironGetRetMsg>()
            .await
            .map(WasiImportCompletion::EnvironGet),
        WasiImport::FdFdstatGet => endpoint
            .recv::<protocol::FdFdstatGetRetMsg>()
            .await
            .map(WasiImportCompletion::FdFdstatGet),
        WasiImport::PathFilestatGet => endpoint
            .recv::<protocol::PathFilestatGetRetMsg>()
            .await
            .map(WasiImportCompletion::PathFilestatGet),
        WasiImport::FdClose => endpoint
            .recv::<protocol::FdCloseRetMsg>()
            .await
            .map(WasiImportCompletion::FdClose),
        WasiImport::ClockResGet => endpoint
            .recv::<protocol::ClockResGetRetMsg>()
            .await
            .map(WasiImportCompletion::ClockResGet),
        WasiImport::ClockTimeGet => endpoint
            .recv::<protocol::ClockTimeGetRetMsg>()
            .await
            .map(WasiImportCompletion::ClockTimeGet),
        WasiImport::PollOneoff => endpoint
            .recv::<protocol::PollOneoffRetMsg>()
            .await
            .map(WasiImportCompletion::PollOneoff),
        WasiImport::RandomGet => endpoint
            .recv::<protocol::RandomGetRetMsg>()
            .await
            .map(WasiImportCompletion::RandomGet),
    }
}

#[cfg(feature = "wasm-engine-core")]
async fn send_memory_grow_request<const ROLE: u8>(
    endpoint: &mut hibana::Endpoint<'_, ROLE>,
    request: hibana_wasip1_runtime::protocol::MemoryGrowReq,
) -> Result<(), hibana::EndpointError> {
    endpoint
        .send::<hibana_wasip1_runtime::protocol::MemoryGrowReqMsg>(&request)
        .await?;
    Ok(())
}

#[cfg(feature = "wasm-engine-core")]
async fn recv_memory_grow_decision<const ROLE: u8>(
    endpoint: &mut hibana::Endpoint<'_, ROLE>,
) -> Result<hibana_wasip1_runtime::protocol::MemoryGrowRet, hibana::EndpointError> {
    let decision = endpoint
        .recv::<hibana_wasip1_runtime::protocol::MemoryGrowRetMsg>()
        .await?;
    Ok(decision)
}

const fn appkit_session(session_id: NonZeroU32) -> hibana::runtime::ids::SessionId {
    hibana::runtime::ids::SessionId::new(session_id.get())
}

#[cfg(feature = "wasm-engine-core")]
impl<'a, I> ArtifactInput<I> for WasiImage<'a>
where
    I: WasiGuestImage,
{
    fn wasi_bytes(&self) -> Option<&[u8]> {
        Some(self.bytes)
    }

    fn wasi_guest_lease<'guest, const ROLE: u8>() -> Option<WasiGuestLease<'guest>> {
        Some(I::wasi_guest_lease::<ROLE>())
    }

    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        I::wasi_budget::<ROLE>()
    }
}

impl<I> ArtifactInput<I> for NoWasi
where
    I: LogicalImage,
{
    #[cfg(feature = "wasm-engine-core")]
    fn wasi_bytes(&self) -> Option<&[u8]> {
        None
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> Option<WasiGuestLease<'guest>> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AttachSummary {
    endpoint_count: u8,
}

#[cfg(all(not(test), target_os = "none"))]
#[cold]
fn panic_appkit_attach_role_error<const ROLE: u8>(error: hibana::runtime::AttachError) -> ! {
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

type ScheduledTaskPoll<E> = unsafe fn(*mut u8, &mut Context<'_>) -> Poll<RoleResult<E>>;

#[cfg(any(test, not(target_os = "none")))]
type ScheduledTaskDrop = unsafe fn(*mut u8);

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

#[cfg(all(not(test), target_os = "none"))]
#[inline(never)]
unsafe fn poll_embedded_stored_task<E>(
    poll: ScheduledTaskPoll<E>,
    ptr: *mut u8,
    cx: &mut Context<'_>,
) -> Poll<RoleResult<E>> {
    unsafe { poll(ptr, cx) }
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

unsafe fn wake_flag_clone(data: *const ()) -> RawWaker {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    RawWaker::new(data, &WAKE_FLAG_WAKER_VTABLE)
}

unsafe fn wake_flag_wake(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    unsafe {
        *data.cast_mut().cast::<bool>() = true;
    }
}

unsafe fn wake_flag_wake_by_ref(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
    unsafe {
        *data.cast_mut().cast::<bool>() = true;
    }
}

unsafe fn wake_flag_drop(data: *const ()) {
    assert!(!data.is_null(), "appkit wake flag data must not be null");
}

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

    fn poll_until_quiescent(&mut self, mut observe: impl FnMut()) {
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
                        observe();
                        panic!("appkit role task failed: {error:?}");
                    }
                }
                task_idx += 1;
            }
            observe();
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
struct EmbeddedScheduledTasks<'task, E> {
    base: *mut u8,
    total_bytes: usize,
    polls: [Option<ScheduledTaskPoll<E>>; APPKIT_EMBEDDED_ROLE_SLOTS],
    len: usize,
    lifetime: PhantomData<&'task mut ()>,
}

#[cfg(all(not(test), target_os = "none"))]
impl<'task, E> EmbeddedScheduledTasks<'task, E>
where
    E: Debug,
{
    fn new(storage: EmbeddedAttachStorageRef<'static>) -> Self {
        Self {
            base: storage.future,
            total_bytes: storage.future_bytes,
            polls: [None; APPKIT_EMBEDDED_ROLE_SLOTS],
            len: 0,
            lifetime: PhantomData,
        }
    }

    fn slot_ptr(&self, index: usize) -> *mut u8 {
        let offset = index
            .checked_mul(APPKIT_EMBEDDED_ROLE_FUTURE_BYTES)
            .expect("appkit embedded scheduler slot offset overflow");
        assert!(
            offset + APPKIT_EMBEDDED_ROLE_FUTURE_BYTES <= self.total_bytes,
            "appkit embedded scheduler slot exceeds future arena"
        );
        unsafe { self.base.add(offset) }
    }

    fn push<F>(&mut self, future: F)
    where
        F: Future<Output = RoleResult<E>> + 'task,
    {
        assert!(
            self.len < self.polls.len(),
            "appkit embedded scheduler has no free role slot"
        );
        assert!(
            size_of::<F>() <= APPKIT_EMBEDDED_ROLE_FUTURE_BYTES,
            "appkit role future exceeds embedded scheduler arena"
        );
        assert!(
            align_of::<F>() <= APPKIT_EMBEDDED_FUTURE_ALIGN,
            "appkit role future alignment exceeds embedded scheduler arena"
        );
        let slot = self.slot_ptr(self.len);
        unsafe {
            // SAFETY: The slot is inside the image-owned embedded arena, aligned
            // to APPKIT_EMBEDDED_FUTURE_ALIGN, and this scheduler never returns.
            slot.cast::<F>().write(future);
        }
        self.polls[self.len] = Some(poll_scheduled_task::<F, E>);
        self.len += 1;
    }

    #[cfg(feature = "wasm-engine-core")]
    fn has_tasks(&self) -> bool {
        self.len != 0
    }

    #[cfg(feature = "wasm-engine-core")]
    fn blocking_engine_state<T>(&self) -> *mut T {
        assert!(
            self.len < APPKIT_EMBEDDED_ROLE_SLOTS,
            "appkit embedded WASI engine state needs one scheduler slot"
        );
        assert!(
            size_of::<T>() <= APPKIT_EMBEDDED_ROLE_FUTURE_BYTES,
            "appkit embedded WASI engine state exceeds one embedded scheduler slot"
        );
        assert!(
            align_of::<T>() <= APPKIT_EMBEDDED_FUTURE_ALIGN,
            "appkit embedded WASI engine state alignment exceeds embedded scheduler arena"
        );
        self.slot_ptr(APPKIT_EMBEDDED_ROLE_SLOTS - 1).cast::<T>()
    }

    fn poll_once(&mut self, task_context: &mut Context<'_>) {
        let mut task_idx = 0usize;
        while task_idx < self.len {
            let poll = self.polls[task_idx]
                .expect("appkit embedded scheduler active slot must have a poll function");
            match unsafe {
                // SAFETY: `push` initialized this slot with the future type
                // associated with the stored poll function.
                poll_embedded_stored_task(poll, self.slot_ptr(task_idx), task_context)
            } {
                Poll::Pending => {}
                Poll::Ready(Ok(done)) => match done {},
                Poll::Ready(Err(error)) => {
                    core::hint::black_box(&error);
                    panic!("appkit embedded role task failed: {error:?}");
                }
            }
            task_idx += 1;
        }
    }

    fn poll_forever(&mut self, mut observe: impl FnMut()) -> ! {
        assert!(self.len > 0, "appkit embedded scheduler has no role tasks");
        loop {
            let mut woke = false;
            let raw_waker = RawWaker::new(
                core::ptr::addr_of_mut!(woke).cast::<()>(),
                &WAKE_FLAG_WAKER_VTABLE,
            );
            let task_waker = unsafe {
                // SAFETY: The raw waker points to `woke`, which lives for this
                // poll pass. The vtable only writes `true` to that flag.
                Waker::from_raw(raw_waker)
            };
            let mut task_context = Context::from_waker(&task_waker);
            self.poll_once(&mut task_context);
            observe();
            if !woke {
                embedded_wait_for_event();
            }
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
#[cold]
fn panic_appkit_resolver_error<const POLICY: u16, const ROLE: u8>(
    error: hibana::runtime::resolver::ResolverError,
) -> ! {
    panic!("appkit resolver registration failed: policy={POLICY} role={ROLE} error={error:?}")
}

struct AttachResolverRegistry<'kit, 'cfg, C, TransportTy, const ROLE: u8>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    rendezvous: &'kit hibana::runtime::RendezvousKit<'kit, 'cfg, TransportTy>,
    program: hibana::runtime::program::RoleProgram<ROLE>,
    capsule: PhantomData<C>,
}

impl<'kit, 'cfg, C, TransportTy, const ROLE: u8> ResolverRegistry<'cfg, C, ROLE>
    for AttachResolverRegistry<'kit, 'cfg, C, TransportTy, ROLE>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    fn resolver<const POLICY: u16>(
        &mut self,
        resolver: hibana::runtime::resolver::ResolverRef<'cfg, POLICY>,
    ) {
        if let Err(error) = self.rendezvous.set_resolver(&self.program, resolver) {
            #[cfg(any(test, not(target_os = "none")))]
            panic!(
                "appkit resolver registration failed: policy={POLICY} role={ROLE} error={error:?}"
            );
            #[cfg(all(not(test), target_os = "none"))]
            panic_appkit_resolver_error::<POLICY, ROLE>(error);
        }
    }
}

struct AttachProjectedResolvers<'kit, 'cfg, C, TransportTy>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    rendezvous: &'kit hibana::runtime::RendezvousKit<'kit, 'cfg, TransportTy>,
    capsule: PhantomData<C>,
}

impl<'kit, 'cfg, C, TransportTy> ProjectedRoleVisitor<C>
    for AttachProjectedResolvers<'kit, 'cfg, C, TransportTy>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    fn visit<const ROLE: u8>(&mut self, program: hibana::runtime::program::RoleProgram<ROLE>) {
        let mut resolver_registry = AttachResolverRegistry::<'_, '_, C, TransportTy, ROLE> {
            rendezvous: self.rendezvous,
            program,
            capsule: PhantomData,
        };
        C::register_resolvers::<_, ROLE>(&mut resolver_registry);
    }
}

struct AttachProjectedRoles<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    rendezvous: &'kit hibana::runtime::RendezvousKit<'kit, 'cfg, TransportTy>,
    session: hibana::runtime::ids::SessionId,
    #[cfg(feature = "wasm-engine-core")]
    wasi_guest_bytes: Option<&'guest [u8]>,
    count: u8,
    tasks_lifetime: PhantomData<&'tasks mut ()>,
    capsule_lifetime: PhantomData<C>,
    image_lifetime: PhantomData<ImageTy>,
    artifact_lifetime: PhantomData<ArtifactTy>,
    #[cfg(not(feature = "wasm-engine-core"))]
    guest_lifetime: PhantomData<&'guest ()>,
    #[cfg(all(not(test), target_os = "none"))]
    embedded_tasks: &'tasks mut EmbeddedScheduledTasks<
        'kit,
        RoleTaskError<<C::Localside as Localside<C>>::Error>,
    >,
    #[cfg(any(test, not(target_os = "none")))]
    tasks: &'tasks mut ScheduledTasks<'kit, RoleTaskError<<C::Localside as Localside<C>>::Error>>,
}

impl<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy> ProjectedRoleVisitor<C>
    for AttachProjectedRoles<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy>
where
    C: Capsule + 'kit,
    C::Localside: 'kit,
    ImageTy: LogicalImage<Capsule = C> + 'kit,
    ArtifactTy: ArtifactInput<ImageTy> + 'kit,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
    'cfg: 'kit,
    'guest: 'kit,
{
    fn visit<const ROLE: u8>(&mut self, program: hibana::runtime::program::RoleProgram<ROLE>) {
        let endpoint = match self.rendezvous.enter(self.session, &program) {
            Ok(endpoint) => endpoint,
            #[cfg(any(test, not(target_os = "none")))]
            Err(error) => panic!("projected role {ROLE} must attach through SessionKit: {error:?}"),
            #[cfg(all(not(test), target_os = "none"))]
            Err(error) => panic_appkit_attach_role_error::<ROLE>(error),
        };
        let role_kind = C::Placement::role_kind::<ROLE>();
        match role_kind {
            RoleKind::Engine => {
                #[cfg(feature = "wasm-engine-core")]
                let guest_storage =
                    <ArtifactTy as ArtifactInput<ImageTy>>::wasi_guest_lease::<ROLE>();
                #[cfg(feature = "wasm-engine-core")]
                let has_wasi_guest = self.wasi_guest_bytes.is_some();
                #[cfg(feature = "wasm-engine-core")]
                assert_eq!(
                    has_wasi_guest,
                    guest_storage.is_some(),
                    "WASI guest artifact and logical image storage capability must match"
                );
                #[cfg(feature = "wasm-engine-core")]
                {
                    if has_wasi_guest {
                        let engine = CanonicalWasiEngine::new(
                            endpoint,
                            self.wasi_guest_bytes,
                            guest_storage,
                        );
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks.push(
                            wasi_role_task::<_, <C::Localside as Localside<C>>::Error>(
                                drive_canonical_wasi_engine::<C, ImageTy, ArtifactTy, ROLE>(engine),
                            ),
                        );
                        #[cfg(all(not(test), target_os = "none"))]
                        {
                            let mut tap = self.rendezvous.tap();
                            run_canonical_wasi_engine_forever::<C, ImageTy, ArtifactTy, ROLE>(
                                engine,
                                self.embedded_tasks,
                                &mut tap,
                            );
                        }
                    } else {
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks.push(localside_role_task(
                            <C::Localside as Localside<C>>::engine::<ROLE>(endpoint),
                        ));
                        #[cfg(all(not(test), target_os = "none"))]
                        let endpoint = endpoint;
                        #[cfg(all(not(test), target_os = "none"))]
                        self.embedded_tasks.push(localside_role_task(
                            <C::Localside as Localside<C>>::engine::<ROLE>(endpoint),
                        ));
                    }
                }
                #[cfg(not(feature = "wasm-engine-core"))]
                {
                    #[cfg(any(test, not(target_os = "none")))]
                    self.tasks.push(localside_role_task(
                        <C::Localside as Localside<C>>::engine::<ROLE>(endpoint),
                    ));
                    #[cfg(all(not(test), target_os = "none"))]
                    let endpoint = endpoint;
                    #[cfg(all(not(test), target_os = "none"))]
                    self.embedded_tasks.push(localside_role_task(
                        <C::Localside as Localside<C>>::engine::<ROLE>(endpoint),
                    ));
                }
            }
            RoleKind::Driver => {
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks.push(localside_role_task(
                    <C::Localside as Localside<C>>::driver::<ROLE>(endpoint),
                ));
                #[cfg(all(not(test), target_os = "none"))]
                let endpoint = endpoint;
                #[cfg(all(not(test), target_os = "none"))]
                self.embedded_tasks.push(localside_role_task(
                    <C::Localside as Localside<C>>::driver::<ROLE>(endpoint),
                ));
            }
            RoleKind::Boundary => {
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks.push(localside_role_task(
                    <C::Localside as Localside<C>>::boundary::<ROLE>(endpoint),
                ));
                #[cfg(all(not(test), target_os = "none"))]
                let endpoint = endpoint;
                #[cfg(all(not(test), target_os = "none"))]
                self.embedded_tasks.push(localside_role_task(
                    <C::Localside as Localside<C>>::boundary::<ROLE>(endpoint),
                ));
            }
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("attached projected role count must not overflow");
    }
}

#[cfg(all(not(test), target_os = "none"))]
fn embedded_attach_storage<I>() -> EmbeddedAttachStorageRef<'static>
where
    I: LogicalImage,
{
    I::attach_storage()
}

fn attach_projected_roles<I, A>(
    program: &impl hibana::runtime::program::Projectable,
    #[cfg(feature = "wasm-engine-core")] wasi_guest_bytes: Option<&[u8]>,
) -> AttachSummary
where
    I: LogicalImage,
    A: ArtifactInput<I>,
{
    #[cfg(any(test, not(target_os = "none")))]
    let mut slab_storage = [0u8; APPKIT_ATTACH_SLAB_BYTES];
    #[cfg(all(not(test), target_os = "none"))]
    let embedded_storage = embedded_attach_storage::<I>();
    #[cfg(all(not(test), target_os = "none"))]
    let attach_slab = unsafe { &mut *embedded_storage.slab };
    #[cfg(any(test, not(target_os = "none")))]
    let attach_slab = &mut slab_storage[..];
    let carrier = I::carrier();
    let rendezvous_slab = attach_slab;
    let mut kit_storage = hibana::runtime::SessionKitStorage::<I::Carrier<'_>>::uninit();
    let kit = kit_storage.init();
    let rendezvous = kit
        .rendezvous(rendezvous_slab, carrier)
        .expect("appkit attach carrier must register rendezvous");
    let session = appkit_session(<I::Capsule as Capsule>::SESSION_ID);
    #[cfg(any(test, not(target_os = "none")))]
    let mut tasks = ScheduledTasks::new();
    #[cfg(all(not(test), target_os = "none"))]
    let mut embedded_tasks = EmbeddedScheduledTasks::new(embedded_storage);
    {
        let mut resolver_registry = AttachProjectedResolvers::<'_, '_, I::Capsule, I::Carrier<'_>> {
            rendezvous: &rendezvous,
            capsule: PhantomData,
        };
        visit_requested_projected_roles::<I::Capsule, _>(
            program,
            I::REQUESTED_ROLES,
            &mut resolver_registry,
        );
    }
    let summary = {
        let mut visitor = AttachProjectedRoles {
            rendezvous: &rendezvous,
            session,
            #[cfg(feature = "wasm-engine-core")]
            wasi_guest_bytes,
            count: 0,
            tasks_lifetime: PhantomData,
            capsule_lifetime: PhantomData::<I::Capsule>,
            image_lifetime: PhantomData::<I>,
            artifact_lifetime: PhantomData::<A>,
            #[cfg(not(feature = "wasm-engine-core"))]
            guest_lifetime: PhantomData,
            #[cfg(all(not(test), target_os = "none"))]
            embedded_tasks: &mut embedded_tasks,
            #[cfg(any(test, not(target_os = "none")))]
            tasks: &mut tasks,
        };
        visit_requested_projected_roles::<I::Capsule, _>(program, I::REQUESTED_ROLES, &mut visitor);
        AttachSummary {
            endpoint_count: visitor.count,
        }
    };
    #[cfg(any(test, not(target_os = "none")))]
    {
        let mut tap = rendezvous.tap();
        tasks.poll_until_quiescent(|| <I::Capsule as Capsule>::observe(&mut tap));
        summary
    }
    #[cfg(all(not(test), target_os = "none"))]
    {
        core::hint::black_box(&summary);
        let mut tap = rendezvous.tap();
        embedded_tasks.poll_forever(|| <I::Capsule as Capsule>::observe(&mut tap))
    }
}

#[cfg(feature = "wasm-engine-core")]
struct WasiGuestSlot<'guest> {
    storage: ManuallyDrop<WasiGuestLease<'guest>>,
    guest: *mut hibana_wasip1_runtime::HibanaWasiGuest<'guest>,
    initialized: bool,
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestSlot<'guest> {
    fn init(
        mut storage: WasiGuestLease<'guest>,
        module: &'guest [u8],
    ) -> Result<Self, WasiGuestError> {
        unsafe {
            storage
                .storage_ptr()
                .write(hibana_wasip1_runtime::HibanaWasiGuestStorage::uninit());
            (*storage.memory_ptr()).fill(0);
        }
        let mut slot = Self {
            storage: ManuallyDrop::new(storage),
            guest: core::ptr::null_mut(),
            initialized: true,
        };
        let storage = unsafe { &mut *slot.storage.storage_ptr() };
        let memory = unsafe { &mut *slot.storage.memory_ptr() };
        let guest_memory = hibana_wasip1_runtime::GuestMemory::new(&mut memory[..]);
        let guest = wasi_runtime_result(storage.init(
            module,
            guest_memory,
            hibana_wasip1_runtime::FdBindingTable::empty(),
        ))?;
        slot.guest = guest as *mut _;
        Ok(slot)
    }

    fn guest(&mut self) -> &mut hibana_wasip1_runtime::HibanaWasiGuest<'guest> {
        debug_assert!(self.initialized);
        unsafe { &mut *self.guest }
    }

    fn finish(mut self) -> WasiGuestLease<'guest> {
        if self.initialized {
            unsafe {
                let ptr = self.storage.storage_ptr();
                core::ptr::drop_in_place(ptr);
            }
            self.initialized = false;
        }
        unsafe { ManuallyDrop::take(&mut self.storage) }
    }
}

#[cfg(feature = "wasm-engine-core")]
impl Drop for WasiGuestSlot<'_> {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                let ptr = self.storage.storage_ptr();
                core::ptr::drop_in_place(ptr);
                ManuallyDrop::drop(&mut self.storage);
            }
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
struct CanonicalWasiEngine<'endpoint, 'guest, C: Capsule, const ROLE: u8> {
    endpoint: hibana::Endpoint<'endpoint, ROLE>,
    wasi_guest_bytes: Option<&'guest [u8]>,
    guest_storage: Option<WasiGuestLease<'guest>>,
    guest_slot: Option<WasiGuestSlot<'guest>>,
    capsule: PhantomData<C>,
}

#[cfg(feature = "wasm-engine-core")]
impl<'endpoint, 'guest, C: Capsule, const ROLE: u8>
    CanonicalWasiEngine<'endpoint, 'guest, C, ROLE>
{
    fn new(
        endpoint: hibana::Endpoint<'endpoint, ROLE>,
        wasi_guest_bytes: Option<&'guest [u8]>,
        guest_storage: Option<WasiGuestLease<'guest>>,
    ) -> Self {
        Self {
            endpoint,
            wasi_guest_bytes,
            guest_storage,
            guest_slot: None,
            capsule: PhantomData,
        }
    }

    fn endpoint(&mut self) -> &mut hibana::Endpoint<'endpoint, ROLE> {
        &mut self.endpoint
    }

    #[cfg(feature = "wasm-engine-core")]
    fn take_wasi_guest_slot(&mut self) -> Result<WasiGuestSlot<'guest>, WasiGuestError> {
        if let Some(slot) = self.guest_slot.take() {
            return Ok(slot);
        }
        let Some(bytes) = self.wasi_guest_bytes else {
            return Err(WasiGuestError::rejected());
        };
        let guest_storage = self.guest_storage.take().expect(
            "WASI engine context must receive in-place guest storage from its logical image",
        );
        WasiGuestSlot::init(guest_storage, bytes)
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    fn store_wasi_guest_slot(
        &mut self,
        guest_slot: WasiGuestSlot<'guest>,
        result: &Result<WasiGuestStatus, WasiGuestError>,
    ) {
        match result {
            Ok(WasiGuestStatus::BudgetExpired) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(WasiGuestStatus::Exit) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    fn store_embedded_wasi_guest_slot(
        &mut self,
        guest_slot: WasiGuestSlot<'guest>,
        result: &Result<EmbeddedWasiGuestStatus, WasiGuestError>,
    ) {
        match result {
            Ok(EmbeddedWasiGuestStatus::BudgetExpired) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(EmbeddedWasiGuestStatus::Exit)
            | Ok(EmbeddedWasiGuestStatus::TerminalEndpoint)
            | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
    }

    /// Drive the selected WASI P1 guest through the hibana-native runtime boundary.
    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn drive_wasi_guest(
        &mut self,
        budget: BudgetRun,
    ) -> Result<WasiGuestStatus, WasiGuestError> {
        let mut guest_slot = self.take_wasi_guest_slot()?;
        let result = loop {
            let step = match wasi_runtime_result(guest_slot.guest().resume_wasi_boundary(budget)) {
                Ok(step) => step,
                Err(error) => break Err(error),
            };
            match step {
                hibana_wasip1_runtime::WasiBoundaryStep::ImportPending(pending) => {
                    let request = pending.request();
                    let import = pending.import();
                    if let Err(error) = wasi_endpoint_result(
                        send_wasi_import_request(self.endpoint(), request).await,
                    ) {
                        break Err(error);
                    }
                    let completion = match wasi_endpoint_result(
                        recv_wasi_import_completion(self.endpoint(), import).await,
                    ) {
                        Ok(completion) => completion,
                        Err(error) => break Err(error),
                    };
                    if let Err(error) =
                        wasi_runtime_result(pending.complete(guest_slot.guest(), completion))
                    {
                        break Err(error);
                    }
                }
                hibana_wasip1_runtime::WasiBoundaryStep::MemoryGrowPending(pending) => {
                    if let Err(error) = wasi_endpoint_result(
                        send_memory_grow_request(self.endpoint(), pending.request()).await,
                    ) {
                        break Err(error);
                    }
                    let decision = match wasi_endpoint_result(
                        recv_memory_grow_decision(self.endpoint()).await,
                    ) {
                        Ok(decision) => decision,
                        Err(error) => break Err(error),
                    };
                    if let Err(error) =
                        wasi_runtime_result(pending.complete(guest_slot.guest(), decision))
                    {
                        break Err(error);
                    }
                }
                hibana_wasip1_runtime::WasiBoundaryStep::BudgetExpired(_) => {
                    break Ok(WasiGuestStatus::BudgetExpired);
                }
                hibana_wasip1_runtime::WasiBoundaryStep::Exit(_) => {
                    break Ok(WasiGuestStatus::Exit);
                }
            }
        };
        self.store_wasi_guest_slot(guest_slot, &result);
        result
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_wasi_guest_blocking(
        &mut self,
        budget: BudgetRun,
        embedded_tasks: &mut EmbeddedScheduledTasks<
            '_,
            RoleTaskError<<C::Localside as Localside<C>>::Error>,
        >,
    ) -> Result<EmbeddedWasiGuestStatus, WasiGuestError> {
        let mut guest_slot = self.take_wasi_guest_slot()?;
        let result = loop {
            let resume_start = appkit_wasi_metric_start();
            let step = match guest_slot.guest().resume_wasi_boundary(budget) {
                Ok(step) => step,
                Err(_) => {
                    record_appkit_wasi_resume(resume_start);
                    break Err(WasiGuestError::rejected());
                }
            };
            record_appkit_wasi_resume(resume_start);
            match step {
                hibana_wasip1_runtime::WasiBoundaryStep::ImportPending(pending) => {
                    let request = pending.request();
                    let import = pending.import();
                    let request_send_start = appkit_wasi_metric_start();
                    let request_send_result = poll_embedded_wasi_unit(
                        send_wasi_import_request(self.endpoint(), request),
                        embedded_tasks,
                    );
                    record_appkit_wasi_request_send(request_send_start);
                    if let Err(error) = request_send_result {
                        if wasi_endpoint_terminal(&error) {
                            break Ok(EmbeddedWasiGuestStatus::TerminalEndpoint);
                        }
                        break Err(WasiGuestError::rejected());
                    }
                    let mut completion =
                        MaybeUninit::<hibana_wasip1_runtime::WasiImportCompletion>::uninit();
                    let completion_recv_start = appkit_wasi_metric_start();
                    let completion_recv_result = poll_embedded_wasi_value(
                        recv_wasi_import_completion(self.endpoint(), import),
                        &mut completion,
                        embedded_tasks,
                    );
                    record_appkit_wasi_completion_recv(completion_recv_start);
                    if let Err(error) = completion_recv_result {
                        if wasi_endpoint_terminal(&error) {
                            break Ok(EmbeddedWasiGuestStatus::TerminalEndpoint);
                        }
                        break Err(WasiGuestError::rejected());
                    }
                    let complete_start = appkit_wasi_metric_start();
                    let complete_result =
                        pending.complete(guest_slot.guest(), unsafe { completion.assume_init() });
                    record_appkit_wasi_complete(complete_start);
                    if wasi_runtime_result(complete_result).is_err() {
                        break Err(WasiGuestError::rejected());
                    }
                }
                hibana_wasip1_runtime::WasiBoundaryStep::MemoryGrowPending(pending) => {
                    if let Err(error) = poll_embedded_wasi_unit(
                        send_memory_grow_request(self.endpoint(), pending.request()),
                        embedded_tasks,
                    ) {
                        if wasi_endpoint_terminal(&error) {
                            break Ok(EmbeddedWasiGuestStatus::TerminalEndpoint);
                        }
                        break Err(WasiGuestError::rejected());
                    }
                    let mut decision =
                        MaybeUninit::<hibana_wasip1_runtime::protocol::MemoryGrowRet>::uninit();
                    if let Err(error) = poll_embedded_wasi_value(
                        recv_memory_grow_decision(self.endpoint()),
                        &mut decision,
                        embedded_tasks,
                    ) {
                        if wasi_endpoint_terminal(&error) {
                            break Ok(EmbeddedWasiGuestStatus::TerminalEndpoint);
                        }
                        break Err(WasiGuestError::rejected());
                    }
                    if wasi_runtime_result(
                        pending.complete(guest_slot.guest(), unsafe { decision.assume_init() }),
                    )
                    .is_err()
                    {
                        break Err(WasiGuestError::rejected());
                    }
                }
                hibana_wasip1_runtime::WasiBoundaryStep::BudgetExpired(_) => {
                    break Ok(EmbeddedWasiGuestStatus::BudgetExpired);
                }
                hibana_wasip1_runtime::WasiBoundaryStep::Exit(_) => {
                    break Ok(EmbeddedWasiGuestStatus::Exit);
                }
            }
        };
        self.store_embedded_wasi_guest_slot(guest_slot, &result);
        result
    }

    #[cfg(any(test, not(target_os = "none")))]
    fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

pub fn pending<'endpoint, E: 'endpoint, const ROLE: u8>(
    endpoint: hibana::Endpoint<'endpoint, ROLE>,
) -> impl core::future::Future<Output = RoleResult<E>> + 'endpoint {
    PendingRole::new(endpoint)
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
async fn drive_canonical_wasi_engine<'endpoint, 'guest, C, I, A, const ROLE: u8>(
    mut engine: CanonicalWasiEngine<'endpoint, 'guest, C, ROLE>,
) -> RoleResult<WasiGuestError>
where
    C: Capsule,
    I: LogicalImage<Capsule = C>,
    A: ArtifactInput<I>,
{
    loop {
        match engine
            .drive_wasi_guest(<A as ArtifactInput<I>>::wasi_budget::<ROLE>())
            .await
        {
            Ok(WasiGuestStatus::BudgetExpired) => {}
            Ok(WasiGuestStatus::Exit) => {
                return engine.pending().await;
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn embedded_observe_forever<C, T>(context: T, tap: &mut hibana::runtime::tap::TapPort<'_>) -> !
where
    C: Capsule,
{
    let context = context;
    loop {
        C::observe(tap);
        core::hint::black_box(&context);
        embedded_wait_for_event();
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn run_canonical_wasi_engine_forever<'endpoint, 'guest, C, I, A, const ROLE: u8>(
    engine: CanonicalWasiEngine<'endpoint, 'guest, C, ROLE>,
    embedded_tasks: &mut EmbeddedScheduledTasks<
        '_,
        RoleTaskError<<C::Localside as Localside<C>>::Error>,
    >,
    tap: &mut hibana::runtime::tap::TapPort<'_>,
) -> !
where
    C: Capsule,
    I: LogicalImage<Capsule = C>,
    A: ArtifactInput<I>,
{
    unsafe {
        let engine_ptr = embedded_tasks
            .blocking_engine_state::<CanonicalWasiEngine<'endpoint, 'guest, C, ROLE>>();
        engine_ptr.write(engine);
        let engine = &mut *engine_ptr;
        if embedded_tasks.has_tasks() {
            let mut woke = false;
            let task_waker = embedded_task_waker(&mut woke);
            let mut task_context = Context::from_waker(&task_waker);
            embedded_tasks.poll_once(&mut task_context);
        }
        loop {
            match engine.drive_wasi_guest_blocking(
                <A as ArtifactInput<I>>::wasi_budget::<ROLE>(),
                embedded_tasks,
            ) {
                Ok(EmbeddedWasiGuestStatus::BudgetExpired) => {
                    C::observe(tap);
                }
                Ok(EmbeddedWasiGuestStatus::Exit | EmbeddedWasiGuestStatus::TerminalEndpoint) => {
                    embedded_observe_forever::<C, _>(engine, tap);
                }
                Err(error) => {
                    core::hint::black_box(&error);
                    C::observe(tap);
                    panic!("appkit embedded WASI role task failed: {error:?}");
                }
            }
        }
    }
}

/// Localside implementation contract for a capsule.
pub trait Localside<C: Capsule> {
    type Error: Debug;

    fn engine<'endpoint, const ROLE: u8>(
        endpoint: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn driver<'endpoint, const ROLE: u8>(
        endpoint: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;

    fn boundary<'endpoint, const ROLE: u8>(
        endpoint: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = RoleResult<Self::Error>>;
}

/// Canonical appkit execution path.
// `ArtifactInput` intentionally stays private: callers pass `NoWasi` or
// `WasiImage`, but never name or implement the artifact boundary trait.
#[allow(private_bounds)]
pub fn run<I>(artifact: impl ArtifactInput<I>)
where
    I: LogicalImage,
{
    run_with_artifact::<I, _>(artifact)
}

fn run_with_artifact<I, A>(artifact: A)
where
    I: LogicalImage,
    A: ArtifactInput<I>,
{
    let program = <I::Capsule as Capsule>::choreography();
    let projected_roles = collect_projected_roles::<I>(&program);
    #[cfg(feature = "wasm-engine-core")]
    let wasi_guest_bytes = artifact.wasi_bytes();
    #[cfg(not(feature = "wasm-engine-core"))]
    let _ = artifact;
    assert!(
        I::REQUESTED_ROLES.is_subset_of(HIBANA_TYPED_ROLE_DOMAIN),
        "logical image requested roles must stay within current hibana typed role domain"
    );
    assert!(
        projected_roles.roles() == I::REQUESTED_ROLES,
        "logical image requested roles must be materialized as hibana RoleProgram values"
    );
    assert!(
        projected_roles.count() == I::REQUESTED_ROLES.count(),
        "logical image projected RoleProgram count must match requested role count"
    );
    let attach_summary = attach_projected_roles::<I, A>(
        &program,
        #[cfg(feature = "wasm-engine-core")]
        wasi_guest_bytes,
    );
    assert!(
        attach_summary.endpoint_count == projected_roles.count(),
        "logical image projected roles must attach through SessionKit"
    );
    let mut image = I::init();
    image.safe_state();
}
