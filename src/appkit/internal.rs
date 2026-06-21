//! Capsule assembly API.
//!
//! `appkit` validates projectable raw hibana choreographies against logical
//! site images. It does not define a choreography DSL, expose engine internals,
//! or complete WASI P1 imports outside projected endpoint/carrier progress.

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
use hibana_wasip1_runtime::protocol::{
    LABEL_WASI_ARGS_GET, LABEL_WASI_ARGS_GET_RET, LABEL_WASI_ARGS_SIZES_GET,
    LABEL_WASI_ARGS_SIZES_GET_RET, LABEL_WASI_CLOCK_RES_GET, LABEL_WASI_CLOCK_RES_GET_RET,
    LABEL_WASI_CLOCK_TIME_GET, LABEL_WASI_CLOCK_TIME_GET_RET, LABEL_WASI_ENVIRON_GET,
    LABEL_WASI_ENVIRON_GET_RET, LABEL_WASI_ENVIRON_SIZES_GET, LABEL_WASI_ENVIRON_SIZES_GET_RET,
    LABEL_WASI_FD_CLOSE, LABEL_WASI_FD_CLOSE_RET, LABEL_WASI_FD_FDSTAT_GET,
    LABEL_WASI_FD_FDSTAT_GET_RET, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET,
    LABEL_WASI_FD_READDIR, LABEL_WASI_FD_READDIR_RET, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET,
    LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF,
    LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, LABEL_WASI_RANDOM_GET,
    LABEL_WASI_RANDOM_GET_RET,
};

#[cfg(feature = "wasm-engine-core")]
use hibana_wasip1_runtime::protocol::{EngineReq, EngineRet};

#[cfg(feature = "wasm-engine-core")]
use hibana_wasip1_runtime::protocol::{
    ArgsGet, ArgsSizesGet, BudgetExpired, BudgetRun, ClockResGet, ClockTimeGet, EnvironGet,
    EnvironSizesGet, FdRead, FdReaddir, FdRequest, FdWrite, LABEL_MEM_FENCE, MemFence,
    MemFenceReason, MemRights, PathOpen, PollOneoff, ProcExitStatus, RandomGet,
    WASIP1_IO_CHUNK_CAPACITY,
};

use hibana_wasip1_runtime::choreofs::{ChoreoFsFacts, DriverFacts, LedgerFacts};

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
const APPKIT_WASI_GUEST_BYTES: usize =
    size_of::<hibana_wasip1_runtime::engine::wasm::Guest<'static>>();
#[cfg(feature = "wasm-engine-core")]
const APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION: u32 = 1_000_000;
#[cfg(feature = "wasm-engine-core")]
const APPKIT_PROC_EXIT_FANOUT_CAP: u8 = HIBANA_TYPED_ROLE_DOMAIN_SIZE;

const APPKIT_DEFAULT_SESSION_ID: NonZeroU32 = nonzero_session_id(1);

const fn nonzero_session_id(raw: u32) -> NonZeroU32 {
    match NonZeroU32::new(raw) {
        Some(session) => session,
        None => panic!("appkit session id must be nonzero"),
    }
}

fn driver_facts_without_objects() -> DriverFacts<'static> {
    DriverFacts::new(ChoreoFsFacts::new(&[]), LedgerFacts::new(&[]))
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
    Local(E),
    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    Wasi(WasiGuestError),
}

impl<E> Debug for RoleTaskError<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Local(error) => formatter.debug_tuple("Local").field(error).finish(),
            #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
            Self::Wasi(error) => formatter.debug_tuple("Wasi").field(error).finish(),
        }
    }
}

struct LocalRoleTask<F, E> {
    future: F,
    marker: PhantomData<fn() -> E>,
}

impl<F, E> LocalRoleTask<F, E> {
    const fn new(future: F) -> Self {
        Self {
            future,
            marker: PhantomData,
        }
    }
}

impl<F, E> Future for LocalRoleTask<F, E>
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
            Poll::Ready(Err(error)) => Poll::Ready(Err(RoleTaskError::Local(error))),
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

fn local_role_task<F, E>(future: F) -> LocalRoleTask<F, E>
where
    F: core::future::Future<Output = RoleResult<E>>,
{
    LocalRoleTask::new(future)
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
    bytes: UnsafeCell<[u8; APPKIT_WASI_GUEST_BYTES]>,
    occupied: UnsafeCell<bool>,
    owner: PhantomData<*mut ()>,
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestArena {
    const EMPTY: Self = Self {
        bytes: UnsafeCell::new([0; APPKIT_WASI_GUEST_BYTES]),
        occupied: UnsafeCell::new(false),
        owner: PhantomData,
    };

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    fn assert_guest_alignment() {
        assert!(
            align_of::<hibana_wasip1_runtime::engine::wasm::Guest<'static>>()
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
            ptr: unsafe { (*self.bytes.get()).as_mut_ptr().cast() },
        }
    }
}

#[cfg(all(not(test), target_os = "none"))]
unsafe impl<const N: usize> Sync for EmbeddedFutureArena<N> {}

#[cfg(feature = "wasm-engine-core")]
pub struct WasiGuestLease<'guest> {
    occupied: *mut bool,
    ptr: *mut hibana_wasip1_runtime::engine::wasm::Guest<'guest>,
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestLease<'guest> {
    fn guest_ptr(&mut self) -> *mut hibana_wasip1_runtime::engine::wasm::Guest<'guest> {
        self.ptr
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
    core::hint::spin_loop();
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
#[inline(always)]
fn embedded_task_waker() -> &'static Waker {
    Waker::noop()
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn poll_embedded_endpoint_unit<F>(mut future: F) -> Result<(), hibana::EndpointError>
where
    F: core::future::Future<Output = Result<(), hibana::EndpointError>>,
{
    let task_waker = embedded_task_waker();
    let mut pinned = unsafe {
        // SAFETY: The future is stored in this stack frame and is never moved
        // while the pinned handle is used.
        Pin::new_unchecked(&mut future)
    };
    loop {
        let mut task_context = Context::from_waker(task_waker);
        match pinned.as_mut().poll(&mut task_context) {
            Poll::Ready(result) => return result,
            Poll::Pending => embedded_wait_for_event(),
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn poll_embedded_endpoint_value<T, F>(
    mut future: F,
    output: &mut MaybeUninit<T>,
) -> Result<(), hibana::EndpointError>
where
    F: core::future::Future<Output = Result<T, hibana::EndpointError>>,
{
    let task_waker = embedded_task_waker();
    let mut pinned = unsafe {
        // SAFETY: The future is stored in this stack frame and is never moved
        // while the pinned handle is used.
        Pin::new_unchecked(&mut future)
    };
    loop {
        let mut task_context = Context::from_waker(task_waker);
        match pinned.as_mut().poll(&mut task_context) {
            Poll::Ready(Ok(value)) => {
                output.write(value);
                return Ok(());
            }
            Poll::Ready(Err(error)) => return Err(error),
            Poll::Pending => embedded_wait_for_event(),
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn embedded_pending_forever<T>(context: T) -> ! {
    let context = context;
    loop {
        core::hint::black_box(&context);
        embedded_wait_for_event();
    }
}

/// Generic logical-site marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Local<Image>(PhantomData<Image>);

impl<Image> Local<Image> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

/// Localside context family assigned to one projected role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleKind {
    Engine,
    Driver,
    Boundary,
}

/// Requested projection slice for a logical image.
///
/// This is not protocol authority. The requested roles must match capsule
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

    pub const fn contains(self, role: u8) -> bool {
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
    pub const fn from_static(bytes: &'a [u8]) -> Self {
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
    fn role_kind(role: u8) -> RoleKind;
}

/// Resolver registration surface for Capsule-local hibana policy points.
pub trait ResolverRegistry<'cfg, C: Capsule> {
    fn resolver<const POLICY: u16, const ROLE: u8>(
        &mut self,
        resolver: hibana::runtime::resolver::ResolverRef<'cfg, POLICY>,
    );
}

/// A projectable raw hibana choreography plus its placement and localside code.
pub trait Capsule: Sized {
    type Placement: Placement<Self>;
    type Local: Localside<Self>;

    const SESSION_ID: NonZeroU32 = APPKIT_DEFAULT_SESSION_ID;

    fn choreography() -> impl hibana::runtime::program::Projectable;

    fn register_resolvers<'cfg, R>(_: &mut R)
    where
        R: ResolverRegistry<'cfg, Self>,
    {
    }

    fn observe(_: &mut hibana::runtime::tap::TapPort<'_>) {}

    #[cfg(feature = "wasm-engine-core")]
    const WASI_GUEST_DRIVE: WasiGuestDrive = WasiGuestDrive::Canonical;
}

/// Driver ownership for a selected WASI P1 guest image.
#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasiGuestDrive {
    /// Appkit owns the canonical endpoint/carrier WASI P1 import loop.
    Canonical,
    /// The capsule localside owns explicit WASI guest stepping.
    Localside,
}

/// Private artifact boundary consumed by [`run`].
///
/// User code passes `WasiImage` or `NoWasi`; it cannot implement new artifact
/// authority. Static WASI import tables are load evidence only, never
/// choreography admission authority.
/// `NoWasi` never leases storage. `WasiImage` requires the selected logical
/// image to implement [`WasiGuestImage`].
trait ArtifactInput<C: Capsule, I> {
    fn wasi_bytes(&self) -> Option<&[u8]>;

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> Option<WasiGuestLease<'guest>>;

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        core::hint::black_box(ROLE);
        BudgetRun::new(1, 0, APPKIT_DEFAULT_WASI_FUEL_PER_ACTIVATION)
    }
}

/// One projection-derived logical site image.
pub trait LogicalImage<C: Capsule>: Sized {
    type Carrier<'a>: hibana::runtime::transport::Transport + 'a
    where
        Self: 'a,
        C: 'a;

    const REQUESTED_ROLES: RoleSet;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        C: 'a;
    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> EmbeddedAttachStorageRef<'static>;
    fn driver_facts() -> DriverFacts<'static> {
        driver_facts_without_objects()
    }
}

/// Site-local storage facts required only by logical images that actually run a WASI guest.
#[cfg(feature = "wasm-engine-core")]
pub trait WasiGuestImage<C: Capsule>: LogicalImage<C> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> WasiGuestLease<'guest>;

    fn wasi_budget<const ROLE: u8>() -> BudgetRun {
        core::hint::black_box(ROLE);
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
fn collect_projected_roles<C, I>(
    program: &impl hibana::runtime::program::Projectable,
) -> ProjectedRoles
where
    C: Capsule,
    I: LogicalImage<C>,
{
    let mut projected = ProjectedRoles::new();
    visit_requested_projected_roles::<C, _>(program, I::REQUESTED_ROLES, &mut projected);
    projected
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasiGuestStatus {
    Exit(ProcExitStatus),
    BudgetExpired(BudgetExpired),
}

#[cfg(feature = "wasm-engine-core")]
#[derive(Clone, Copy, Debug)]
pub enum WasiGuestError {
    NoWasiArtifact,
    GuestRejected(hibana_wasip1_runtime::engine::wasm::Error),
    EndpointRejected(u32),
    Endpoint {
        code: u32,
        source: hibana::EndpointError,
    },
    ProtocolRejected(hibana::runtime::wire::CodecError),
    UnexpectedReply,
}

#[cfg(feature = "wasm-engine-core")]
impl WasiGuestError {
    fn endpoint(code: u32, source: hibana::EndpointError) -> Self {
        Self::Endpoint { code, source }
    }
}

#[cfg(feature = "wasm-engine-core")]
impl From<hibana_wasip1_runtime::engine::wasm::Error> for WasiGuestError {
    fn from(error: hibana_wasip1_runtime::engine::wasm::Error) -> Self {
        Self::GuestRejected(error)
    }
}

#[cfg(feature = "wasm-engine-core")]
impl From<hibana::runtime::wire::CodecError> for WasiGuestError {
    fn from(error: hibana::runtime::wire::CodecError) -> Self {
        Self::ProtocolRejected(error)
    }
}

const fn appkit_session(session_id: NonZeroU32) -> hibana::runtime::ids::SessionId {
    hibana::runtime::ids::SessionId::new(session_id.get())
}

#[cfg(feature = "wasm-engine-core")]
impl<'a, C, I> ArtifactInput<C, I> for WasiImage<'a>
where
    C: Capsule,
    I: WasiGuestImage<C>,
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

#[cfg(not(feature = "wasm-engine-core"))]
impl<'a, C, I> ArtifactInput<C, I> for WasiImage<'a>
where
    C: Capsule,
    I: LogicalImage<C>,
{
    fn wasi_bytes(&self) -> Option<&[u8]> {
        Some(self.bytes)
    }
}

impl<C, I> ArtifactInput<C, I> for NoWasi
where
    C: Capsule,
    I: LogicalImage<C>,
{
    fn wasi_bytes(&self) -> Option<&[u8]> {
        None
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> Option<WasiGuestLease<'guest>> {
        core::hint::black_box(ROLE);
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
            let mut task_idx = 0usize;
            while task_idx < self.len {
                let poll = self.polls[task_idx]
                    .expect("appkit embedded scheduler active slot must have a poll function");
                match unsafe {
                    // SAFETY: `push` initialized this slot with the future type
                    // associated with the stored poll function.
                    poll_embedded_stored_task(poll, self.slot_ptr(task_idx), &mut task_context)
                } {
                    Poll::Pending => {}
                    Poll::Ready(Ok(done)) => match done {},
                    Poll::Ready(Err(error)) => {
                        core::hint::black_box(&error);
                        observe();
                        panic!("appkit embedded role task failed: {error:?}");
                    }
                }
                task_idx += 1;
            }
            observe();
            if !woke && self.len == 1 {
                embedded_wait_for_event();
            } else if !woke {
                core::hint::spin_loop();
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

struct AttachResolverRegistry<'kit, 'prog, 'cfg, C, ProgramTy, TransportTy>
where
    C: Capsule,
    ProgramTy: hibana::runtime::program::Projectable + ?Sized,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    rendezvous: &'kit hibana::runtime::RendezvousKit<'kit, 'cfg, TransportTy>,
    program: &'prog ProgramTy,
    requested_roles: RoleSet,
    capsule: PhantomData<C>,
}

impl<'kit, 'prog, 'cfg, C, ProgramTy, TransportTy> ResolverRegistry<'cfg, C>
    for AttachResolverRegistry<'kit, 'prog, 'cfg, C, ProgramTy, TransportTy>
where
    C: Capsule,
    ProgramTy: hibana::runtime::program::Projectable + ?Sized,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    fn resolver<const POLICY: u16, const ROLE: u8>(
        &mut self,
        resolver: hibana::runtime::resolver::ResolverRef<'cfg, POLICY>,
    ) {
        if !self.requested_roles.contains(ROLE) {
            return;
        }
        let role_program = hibana::runtime::program::project::<ROLE, _>(self.program);
        if let Err(error) = self.rendezvous.set_resolver(&role_program, resolver) {
            #[cfg(any(test, not(target_os = "none")))]
            panic!(
                "appkit resolver registration failed: policy={POLICY} role={ROLE} error={error:?}"
            );
            #[cfg(all(not(test), target_os = "none"))]
            panic_appkit_resolver_error::<POLICY, ROLE>(error);
        }
    }
}

struct AttachProjectedRoles<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy>
where
    C: Capsule,
    TransportTy: hibana::runtime::transport::Transport + 'cfg,
{
    rendezvous: &'kit hibana::runtime::RendezvousKit<'kit, 'cfg, TransportTy>,
    session: hibana::runtime::ids::SessionId,
    wasi_guest_bytes: Option<&'guest [u8]>,
    driver_facts: DriverFacts<'static>,
    count: u8,
    tasks_lifetime: PhantomData<&'tasks mut ()>,
    capsule_lifetime: PhantomData<C>,
    image_lifetime: PhantomData<ImageTy>,
    artifact_lifetime: PhantomData<ArtifactTy>,
    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    embedded_storage: EmbeddedAttachStorageRef<'static>,
    #[cfg(all(not(test), target_os = "none"))]
    embedded_tasks:
        &'tasks mut EmbeddedScheduledTasks<'kit, RoleTaskError<<C::Local as Localside<C>>::Error>>,
    #[cfg(any(test, not(target_os = "none")))]
    tasks: &'tasks mut ScheduledTasks<'kit, RoleTaskError<<C::Local as Localside<C>>::Error>>,
}

impl<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy> ProjectedRoleVisitor<C>
    for AttachProjectedRoles<'kit, 'tasks, 'cfg, 'guest, C, ImageTy, ArtifactTy, TransportTy>
where
    C: Capsule + 'kit,
    C::Local: 'kit,
    ImageTy: LogicalImage<C> + 'kit,
    ArtifactTy: ArtifactInput<C, ImageTy>,
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
        let endpoint_ctx = RoleEndpointCtx::<C, ROLE>::new(endpoint);
        let role_kind = C::Placement::role_kind(ROLE);
        match role_kind {
            RoleKind::Engine => {
                #[cfg(feature = "wasm-engine-core")]
                let guest_storage =
                    <ArtifactTy as ArtifactInput<C, ImageTy>>::wasi_guest_lease::<ROLE>();
                #[cfg(feature = "wasm-engine-core")]
                let has_wasi_guest = self.wasi_guest_bytes.is_some();
                #[cfg(feature = "wasm-engine-core")]
                assert_eq!(
                    has_wasi_guest,
                    guest_storage.is_some(),
                    "WASI guest artifact and logical image storage capability must match"
                );
                #[cfg(feature = "wasm-engine-core")]
                let ctx = EngineCtx::new(endpoint_ctx, self.wasi_guest_bytes, guest_storage);
                #[cfg(not(feature = "wasm-engine-core"))]
                let ctx = EngineCtx::new(endpoint_ctx, self.wasi_guest_bytes);
                #[cfg(feature = "wasm-engine-core")]
                {
                    if has_wasi_guest && matches!(C::WASI_GUEST_DRIVE, WasiGuestDrive::Canonical) {
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks
                            .push(wasi_role_task::<_, <C::Local as Localside<C>>::Error>(
                                drive_canonical_wasi_engine::<C, ImageTy, ArtifactTy, ROLE>(ctx),
                            ));
                        #[cfg(all(not(test), target_os = "none"))]
                        {
                            assert!(
                                ImageTy::REQUESTED_ROLES.count() == 1,
                                "bare-metal WASI logical images attach exactly one role; split peer roles into separate logical images"
                            );
                            run_canonical_wasi_engine_forever::<C, ImageTy, ArtifactTy, ROLE>(
                                self.embedded_storage,
                                ctx,
                            );
                        }
                    } else {
                        #[cfg(any(test, not(target_os = "none")))]
                        self.tasks.push(local_role_task(
                            <C::Local as Localside<C>>::engine::<ROLE>(ctx),
                        ));
                        #[cfg(all(not(test), target_os = "none"))]
                        self.embedded_tasks.push(local_role_task(
                            <C::Local as Localside<C>>::engine::<ROLE>(ctx),
                        ));
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
                    self.embedded_tasks.push(local_role_task(
                        <C::Local as Localside<C>>::engine::<ROLE>(ctx),
                    ));
                }
            }
            RoleKind::Driver => {
                let ctx = DriverCtx::new(endpoint_ctx, self.driver_facts);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks
                    .push(local_role_task(<C::Local as Localside<C>>::driver::<ROLE>(
                        ctx,
                    )));
                #[cfg(all(not(test), target_os = "none"))]
                self.embedded_tasks.push(local_role_task(
                    <C::Local as Localside<C>>::driver::<ROLE>(ctx),
                ));
            }
            RoleKind::Boundary => {
                let ctx = BoundaryCtx::new(endpoint_ctx);
                #[cfg(any(test, not(target_os = "none")))]
                self.tasks.push(local_role_task(
                    <C::Local as Localside<C>>::boundary::<ROLE>(ctx),
                ));
                #[cfg(all(not(test), target_os = "none"))]
                self.embedded_tasks
                    .push(local_role_task(
                        <C::Local as Localside<C>>::boundary::<ROLE>(ctx),
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
fn embedded_attach_storage<C, I>() -> EmbeddedAttachStorageRef<'static>
where
    C: Capsule,
    I: LogicalImage<C>,
{
    I::attach_storage()
}

fn attach_projected_roles<C, I, A>(
    program: &impl hibana::runtime::program::Projectable,
    wasi_guest_bytes: Option<&[u8]>,
) -> AttachSummary
where
    C: Capsule,
    I: LogicalImage<C>,
    A: ArtifactInput<C, I>,
{
    #[cfg(any(test, not(target_os = "none")))]
    let mut slab_storage = [0u8; APPKIT_ATTACH_SLAB_BYTES];
    #[cfg(all(not(test), target_os = "none"))]
    let embedded_storage = embedded_attach_storage::<C, I>();
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
    let session = appkit_session(C::SESSION_ID);
    #[cfg(any(test, not(target_os = "none")))]
    let mut tasks = ScheduledTasks::new();
    #[cfg(all(not(test), target_os = "none"))]
    let mut embedded_tasks = EmbeddedScheduledTasks::new(embedded_storage);
    {
        let mut resolver_registry = AttachResolverRegistry::<'_, '_, '_, C, _, I::Carrier<'_>> {
            rendezvous: &rendezvous,
            program,
            requested_roles: I::REQUESTED_ROLES,
            capsule: PhantomData,
        };
        C::register_resolvers(&mut resolver_registry);
    }
    let summary = {
        let mut visitor = AttachProjectedRoles {
            rendezvous: &rendezvous,
            session,
            wasi_guest_bytes,
            driver_facts: I::driver_facts(),
            count: 0,
            tasks_lifetime: PhantomData,
            capsule_lifetime: PhantomData::<C>,
            image_lifetime: PhantomData::<I>,
            artifact_lifetime: PhantomData::<A>,
            #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
            embedded_storage,
            #[cfg(all(not(test), target_os = "none"))]
            embedded_tasks: &mut embedded_tasks,
            #[cfg(any(test, not(target_os = "none")))]
            tasks: &mut tasks,
        };
        visit_requested_projected_roles::<C, _>(program, I::REQUESTED_ROLES, &mut visitor);
        AttachSummary {
            endpoint_count: visitor.count,
        }
    };
    #[cfg(any(test, not(target_os = "none")))]
    {
        let mut tap = rendezvous.tap();
        tasks.poll_until_quiescent(|| C::observe(&mut tap));
        summary
    }
    #[cfg(all(not(test), target_os = "none"))]
    {
        core::hint::black_box(&summary);
        let mut tap = rendezvous.tap();
        embedded_tasks.poll_forever(|| C::observe(&mut tap))
    }
}

/// Role-typed wrapper around a hibana endpoint attached by appkit.
///
/// This is the context shape that preserves hibana's typed `Endpoint<'_, ROLE>`
/// progress without exposing raw site or transport authority. It is not a
/// choreography wrapper and it does not name hibana's internal `steps` types.
struct RoleEndpointCtx<'a, C: Capsule, const ROLE: u8> {
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

    fn endpoint(&mut self) -> &mut hibana::Endpoint<'a, ROLE> {
        &mut self.endpoint
    }
}

#[cfg(feature = "wasm-engine-core")]
struct WasiGuestSlot<'guest> {
    storage: ManuallyDrop<WasiGuestLease<'guest>>,
    initialized: bool,
}

#[cfg(feature = "wasm-engine-core")]
impl<'guest> WasiGuestSlot<'guest> {
    fn init(
        mut storage: WasiGuestLease<'guest>,
        module: &'guest [u8],
    ) -> Result<Self, hibana_wasip1_runtime::engine::wasm::Error> {
        let ptr = storage.guest_ptr();
        unsafe {
            hibana_wasip1_runtime::engine::wasm::Guest::init_in_place(ptr, module)?;
        }
        Ok(Self {
            storage: ManuallyDrop::new(storage),
            initialized: true,
        })
    }

    fn guest(&mut self) -> &mut hibana_wasip1_runtime::engine::wasm::Guest<'guest> {
        debug_assert!(self.initialized);
        let ptr = self.storage.guest_ptr();
        unsafe { &mut *ptr }
    }

    fn finish(mut self) -> WasiGuestLease<'guest> {
        if self.initialized {
            unsafe {
                let ptr = self.storage.guest_ptr();
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
                let ptr = self.storage.guest_ptr();
                core::ptr::drop_in_place(ptr);
                ManuallyDrop::drop(&mut self.storage);
            }
        }
    }
}

/// Engine-side localside context.
pub struct EngineCtx<'endpoint, 'guest, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'endpoint, C, ROLE>,
    #[cfg(feature = "wasm-engine-core")]
    wasi_guest_bytes: Option<&'guest [u8]>,
    #[cfg(feature = "wasm-engine-core")]
    guest_storage: Option<WasiGuestLease<'guest>>,
    #[cfg(feature = "wasm-engine-core")]
    guest_slot: Option<WasiGuestSlot<'guest>>,
    #[cfg(not(feature = "wasm-engine-core"))]
    guest_lifetime: core::marker::PhantomData<&'guest ()>,
}

impl<'endpoint, 'guest, C: Capsule, const ROLE: u8> EngineCtx<'endpoint, 'guest, C, ROLE> {
    fn new(
        endpoint: RoleEndpointCtx<'endpoint, C, ROLE>,
        wasi_guest_bytes: Option<&'guest [u8]>,
        #[cfg(feature = "wasm-engine-core")] guest_storage: Option<WasiGuestLease<'guest>>,
    ) -> Self {
        #[cfg(not(feature = "wasm-engine-core"))]
        core::hint::black_box(wasi_guest_bytes);
        Self {
            endpoint,
            #[cfg(feature = "wasm-engine-core")]
            wasi_guest_bytes,
            #[cfg(feature = "wasm-engine-core")]
            guest_storage,
            #[cfg(feature = "wasm-engine-core")]
            guest_slot: None,
            #[cfg(not(feature = "wasm-engine-core"))]
            guest_lifetime: core::marker::PhantomData,
        }
    }

    pub fn endpoint(&mut self) -> &mut hibana::Endpoint<'endpoint, ROLE> {
        self.endpoint.endpoint()
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn endpoint_send<const LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<(), WasiGuestError> {
        match self
            .endpoint()
            .send::<hibana::g::Msg<LABEL, EngineReq>>(&request)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => Err(WasiGuestError::endpoint(0x5745_2000 | LABEL as u32, error)),
        }
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn endpoint_send_consecutive<const LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<(), WasiGuestError> {
        let mut sent = 0u8;
        loop {
            match self
                .endpoint()
                .send::<hibana::g::Msg<LABEL, EngineReq>>(&request)
                .await
            {
                Ok(()) => {}
                Err(error) => {
                    if sent > 0 {
                        return Ok(());
                    }
                    return Err(WasiGuestError::endpoint(0x5745_2000 | LABEL as u32, error));
                }
            }
            sent = sent.saturating_add(1);
            if sent == APPKIT_PROC_EXIT_FANOUT_CAP {
                return Err(WasiGuestError::EndpointRejected(0x5745_64ff));
            }
        }
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn endpoint_send_blocking<const LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<(), WasiGuestError> {
        match poll_embedded_endpoint_unit(
            self.endpoint()
                .send::<hibana::g::Msg<LABEL, EngineReq>>(&request),
        ) {
            Ok(()) => Ok(()),
            Err(error) => Err(WasiGuestError::endpoint(0x5745_2000 | LABEL as u32, error)),
        }
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn endpoint_send_consecutive_blocking<const LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<(), WasiGuestError> {
        let mut sent = 0u8;
        loop {
            match poll_embedded_endpoint_unit(
                self.endpoint()
                    .send::<hibana::g::Msg<LABEL, EngineReq>>(&request),
            ) {
                Ok(()) => {}
                Err(error) => {
                    if sent > 0 {
                        return Ok(());
                    }
                    return Err(WasiGuestError::endpoint(0x5745_2000 | LABEL as u32, error));
                }
            }
            sent = sent.saturating_add(1);
            if sent == APPKIT_PROC_EXIT_FANOUT_CAP {
                return Err(WasiGuestError::EndpointRejected(0x5745_64ff));
            }
        }
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
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

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn endpoint_call_blocking<const REQUEST_LABEL: u8, const REPLY_LABEL: u8>(
        &mut self,
        request: EngineReq,
    ) -> Result<EngineRet, WasiGuestError> {
        self.endpoint_send_blocking::<REQUEST_LABEL>(request)?;
        let mut reply = MaybeUninit::<EngineRet>::uninit();
        match poll_embedded_endpoint_value(
            self.endpoint()
                .recv::<hibana::g::Msg<REPLY_LABEL, EngineRet>>(),
            &mut reply,
        ) {
            Ok(()) => unsafe { Ok(reply.assume_init()) },
            Err(error) => Err(WasiGuestError::endpoint(
                0x5745_3000 | REPLY_LABEL as u32,
                error,
            )),
        }
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn endpoint_proc_exit(&mut self, status: ProcExitStatus) -> Result<(), WasiGuestError> {
        self.endpoint_send_consecutive::<LABEL_WASI_PROC_EXIT>(EngineReq::ProcExit(status))
            .await
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn endpoint_proc_exit_blocking(
        &mut self,
        status: ProcExitStatus,
    ) -> Result<(), WasiGuestError> {
        self.endpoint_send_consecutive_blocking::<LABEL_WASI_PROC_EXIT>(EngineReq::ProcExit(status))
    }

    #[cfg(feature = "wasm-engine-core")]
    fn protocol_fdstat_to_vm(
        stat: hibana_wasip1_runtime::protocol::FdStat,
    ) -> hibana_wasip1_runtime::engine::wasm::FdStat {
        let rights = match stat.rights() {
            MemRights::Read => 1,
            MemRights::Write => 2,
        };
        hibana_wasip1_runtime::engine::wasm::FdStat::new(4, 0, rights, 0)
    }

    /// Drive the selected WASI P1 guest until it exits, finishes, or exhausts its budget.
    ///
    /// Each emitted WASI P1 import is normalized into an `EngineReq`, sent through
    /// this role's typed endpoint, and completed only after the corresponding
    /// `EngineRet` is received through the endpoint/carrier path.
    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
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
            match guest_slot.guest().resume(budget) {
                Ok(hibana_wasip1_runtime::engine::wasm::Event::BudgetExpired(expired)) => {
                    break Ok(WasiGuestStatus::BudgetExpired(expired));
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::Exit(exit)) => {
                    let Some(status) = exit.as_protocol_status() else {
                        break Err(WasiGuestError::UnexpectedReply);
                    };
                    if let Err(error) = self.endpoint_proc_exit(status).await {
                        break Err(error);
                    }
                    break Ok(WasiGuestStatus::Exit(status));
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::Call(call)) => {
                    if let Err(error) = self.drive_wasi_call(&mut guest_slot, call).await {
                        break Err(error);
                    }
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::MemoryFence(pending)) => {
                    if let Err(error) = self.drive_memory_fence(&mut guest_slot, pending).await {
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
            Ok(WasiGuestStatus::Exit(_)) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
        result
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    /// Drive exactly one pending WASI P1 import after the caller has admitted the
    /// surrounding choreography step.
    pub async fn drive_wasi_guest_once(
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
        let result = match guest_slot.guest().resume(budget) {
            Ok(hibana_wasip1_runtime::engine::wasm::Event::BudgetExpired(expired)) => {
                Ok(WasiGuestStatus::BudgetExpired(expired))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::Exit(exit)) => {
                let Some(status) = exit.as_protocol_status() else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                self.endpoint_proc_exit(status).await?;
                Ok(WasiGuestStatus::Exit(status))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::Call(call)) => {
                self.drive_wasi_call(&mut guest_slot, call).await?;
                Ok(WasiGuestStatus::BudgetExpired(BudgetExpired::new(
                    budget.run_id(),
                    budget.generation(),
                )))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::MemoryFence(pending)) => {
                self.drive_memory_fence(&mut guest_slot, pending).await?;
                Ok(WasiGuestStatus::BudgetExpired(BudgetExpired::new(
                    budget.run_id(),
                    budget.generation(),
                )))
            }
            Err(error) => Err(error.into()),
        };
        match result {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(WasiGuestStatus::Exit(_)) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
        result
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_wasi_guest_blocking(
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
            match guest_slot.guest().resume(budget) {
                Ok(hibana_wasip1_runtime::engine::wasm::Event::Call(call)) => {
                    if let Err(error) = self.drive_wasi_call_blocking(&mut guest_slot, call) {
                        break Err(error);
                    }
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::BudgetExpired(expired)) => {
                    break Ok(WasiGuestStatus::BudgetExpired(expired));
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::Exit(exit)) => {
                    let Some(status) = exit.as_protocol_status() else {
                        break Err(WasiGuestError::UnexpectedReply);
                    };
                    if let Err(error) = self.endpoint_proc_exit_blocking(status) {
                        break Err(error);
                    }
                    break Ok(WasiGuestStatus::Exit(status));
                }
                Ok(hibana_wasip1_runtime::engine::wasm::Event::MemoryFence(pending)) => {
                    if let Err(error) = self.drive_memory_fence_blocking(&mut guest_slot, pending) {
                        break Err(error);
                    }
                }
                Err(error) => {
                    break Err(error.into());
                }
            }
        };
        match result {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(WasiGuestStatus::Exit(_)) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
        result
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    /// Drive exactly one pending WASI P1 import after the caller has admitted the
    /// surrounding choreography step.
    pub fn drive_wasi_guest_once_blocking(
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
        let result = match guest_slot.guest().resume(budget) {
            Ok(hibana_wasip1_runtime::engine::wasm::Event::Call(call)) => {
                self.drive_wasi_call_blocking(&mut guest_slot, call)?;
                Ok(WasiGuestStatus::BudgetExpired(BudgetExpired::new(
                    budget.run_id(),
                    budget.generation(),
                )))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::BudgetExpired(expired)) => {
                Ok(WasiGuestStatus::BudgetExpired(expired))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::Exit(exit)) => {
                let Some(status) = exit.as_protocol_status() else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                self.endpoint_proc_exit_blocking(status)?;
                Ok(WasiGuestStatus::Exit(status))
            }
            Ok(hibana_wasip1_runtime::engine::wasm::Event::MemoryFence(pending)) => {
                self.drive_memory_fence_blocking(&mut guest_slot, pending)?;
                Ok(WasiGuestStatus::BudgetExpired(BudgetExpired::new(
                    budget.run_id(),
                    budget.generation(),
                )))
            }
            Err(error) => Err(error.into()),
        };
        match result {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {
                self.guest_slot = Some(guest_slot);
            }
            Ok(WasiGuestStatus::Exit(_)) | Err(_) => {
                let guest_storage = guest_slot.finish();
                self.guest_storage = Some(guest_storage);
            }
        }
        result
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn drive_wasi_call(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        call: hibana_wasip1_runtime::engine::wasm::Call,
    ) -> Result<(), WasiGuestError> {
        match call {
            hibana_wasip1_runtime::engine::wasm::Call::FdWrite(pending) => {
                self.drive_fd_write_call(guest_slot, pending).await
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdRead(pending) => {
                let fd = pending.fd();
                let max_len = pending.max_len(guest_slot.guest())?;
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
                pending.complete(guest_slot.guest(), done.as_bytes(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdFdstatGet(pending) => {
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
                pending.complete(guest_slot.guest(), Self::protocol_fdstat_to_vm(stat), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdClose(pending) => {
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
                pending.complete(guest_slot.guest(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ClockResGet(pending) => {
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
                pending.complete(guest_slot.guest(), resolution.nanos(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ClockTimeGet(pending) => {
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
                pending.complete(guest_slot.guest(), now.nanos(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::PollOneoff(pending) => {
                self.drive_poll_oneoff_call(guest_slot, pending).await
            }
            hibana_wasip1_runtime::engine::wasm::Call::RandomGet(pending) => {
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
                pending.complete(guest_slot.guest(), done.as_bytes(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdReaddir(pending) => {
                let fd = pending.fd();
                let cookie = pending.cookie();
                let max_len = pending.max_len();
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
                pending.complete(guest_slot.guest(), done.as_bytes(), done.errno() as u32)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::PathOpen(pending) => {
                let fd = pending.fd();
                let rights = pending.rights_base();
                let path = pending.path_bytes(guest_slot.guest())?;
                let request = EngineReq::PathOpen(PathOpen::new(fd, rights, path.as_bytes())?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET>(request)
                    .await?;
                let EngineRet::PathOpened(opened) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    opened.fd() as u32,
                    opened.errno() as u32,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ArgsSizesGet(pending) => {
                let reply = self
                    .endpoint_call::<LABEL_WASI_ARGS_SIZES_GET, LABEL_WASI_ARGS_SIZES_GET_RET>(
                        EngineReq::ArgsSizesGet(ArgsSizesGet),
                    )
                    .await?;
                let EngineRet::ArgsSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    sizes.count() as u32,
                    sizes.buf_size() as u32,
                    0,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ArgsGet(pending) => {
                let request = EngineReq::ArgsGet(ArgsGet::new(WASIP1_IO_CHUNK_CAPACITY as u8)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_ARGS_GET, LABEL_WASI_ARGS_GET_RET>(request)
                    .await?;
                let EngineRet::ArgsDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), &[done.as_bytes()], 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::EnvironSizesGet(pending) => {
                let reply = self
                    .endpoint_call::<LABEL_WASI_ENVIRON_SIZES_GET, LABEL_WASI_ENVIRON_SIZES_GET_RET>(
                        EngineReq::EnvironSizesGet(EnvironSizesGet),
                    )
                    .await?;
                let EngineRet::EnvironSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    sizes.count() as u32,
                    sizes.buf_size() as u32,
                    0,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::EnvironGet(pending) => {
                let request =
                    EngineReq::EnvironGet(EnvironGet::new(WASIP1_IO_CHUNK_CAPACITY as u8)?);
                let reply = self
                    .endpoint_call::<LABEL_WASI_ENVIRON_GET, LABEL_WASI_ENVIRON_GET_RET>(request)
                    .await?;
                let EngineRet::EnvironDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), &[(done.as_bytes(), &[][..])], 0)?;
                Ok(())
            }
        }
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_wasi_call_blocking(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        call: hibana_wasip1_runtime::engine::wasm::Call,
    ) -> Result<(), WasiGuestError> {
        match call {
            hibana_wasip1_runtime::engine::wasm::Call::FdWrite(pending) => {
                self.drive_fd_write_call_blocking(guest_slot, pending)
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdRead(pending) => {
                let fd = pending.fd();
                let max_len = pending.max_len(guest_slot.guest())?;
                if max_len > u8::MAX as usize {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::FdRead(FdRead::new(fd, max_len as u8)?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET>(
                        request,
                    )?;
                let EngineRet::FdReadDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if done.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(guest_slot.guest(), done.as_bytes(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdFdstatGet(pending) => {
                let fd = pending.fd();
                let reply = self.endpoint_call_blocking::<
                    LABEL_WASI_FD_FDSTAT_GET,
                    LABEL_WASI_FD_FDSTAT_GET_RET,
                >(EngineReq::FdFdstatGet(FdRequest::new(fd)))?;
                let EngineRet::FdStat(stat) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if stat.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(guest_slot.guest(), Self::protocol_fdstat_to_vm(stat), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdClose(pending) => {
                let fd = pending.fd();
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_FD_CLOSE, LABEL_WASI_FD_CLOSE_RET>(
                        EngineReq::FdClose(FdRequest::new(fd)),
                    )?;
                let EngineRet::FdClosed(closed) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if closed.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(guest_slot.guest(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ClockResGet(pending) => {
                let clock_id = pending.clock_id();
                if clock_id > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let reply = self.endpoint_call_blocking::<
                    LABEL_WASI_CLOCK_RES_GET,
                    LABEL_WASI_CLOCK_RES_GET_RET,
                >(EngineReq::ClockResGet(ClockResGet::new(clock_id as u8)))?;
                let EngineRet::ClockResolution(resolution) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), resolution.nanos(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ClockTimeGet(pending) => {
                let clock_id = pending.clock_id();
                if clock_id > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request =
                    EngineReq::ClockTimeGet(ClockTimeGet::new(clock_id as u8, pending.precision()));
                let reply = self.endpoint_call_blocking::<
                    LABEL_WASI_CLOCK_TIME_GET,
                    LABEL_WASI_CLOCK_TIME_GET_RET,
                >(request)?;
                let EngineRet::ClockTime(now) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), now.nanos(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::PollOneoff(pending) => {
                self.drive_poll_oneoff_call_blocking(guest_slot, pending)
            }
            hibana_wasip1_runtime::engine::wasm::Call::RandomGet(pending) => {
                let len = pending.buf_len();
                if len > u8::MAX as u32 {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::RandomGet(RandomGet::new(len as u8)?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_RANDOM_GET, LABEL_WASI_RANDOM_GET_RET>(
                        request,
                    )?;
                let EngineRet::RandomDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), done.as_bytes(), 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::FdReaddir(pending) => {
                let fd = pending.fd();
                let cookie = pending.cookie();
                let max_len = pending.max_len();
                if max_len > u8::MAX as usize {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                let request = EngineReq::FdReaddir(FdReaddir::new(fd, cookie, max_len as u8)?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_FD_READDIR, LABEL_WASI_FD_READDIR_RET>(
                        request,
                    )?;
                let EngineRet::FdReaddirDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                if done.fd() != fd {
                    return Err(WasiGuestError::UnexpectedReply);
                }
                pending.complete(guest_slot.guest(), done.as_bytes(), done.errno() as u32)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::PathOpen(pending) => {
                let fd = pending.fd();
                let rights = pending.rights_base();
                let path = pending.path_bytes(guest_slot.guest())?;
                let request = EngineReq::PathOpen(PathOpen::new(fd, rights, path.as_bytes())?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET>(
                        request,
                    )?;
                let EngineRet::PathOpened(opened) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    opened.fd() as u32,
                    opened.errno() as u32,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ArgsSizesGet(pending) => {
                let reply = self.endpoint_call_blocking::<
                    LABEL_WASI_ARGS_SIZES_GET,
                    LABEL_WASI_ARGS_SIZES_GET_RET,
                >(EngineReq::ArgsSizesGet(ArgsSizesGet))?;
                let EngineRet::ArgsSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    sizes.count() as u32,
                    sizes.buf_size() as u32,
                    0,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::ArgsGet(pending) => {
                let request = EngineReq::ArgsGet(ArgsGet::new(WASIP1_IO_CHUNK_CAPACITY as u8)?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_ARGS_GET, LABEL_WASI_ARGS_GET_RET>(
                        request,
                    )?;
                let EngineRet::ArgsDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), &[done.as_bytes()], 0)?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::EnvironSizesGet(pending) => {
                let reply = self.endpoint_call_blocking::<
                    LABEL_WASI_ENVIRON_SIZES_GET,
                    LABEL_WASI_ENVIRON_SIZES_GET_RET,
                >(EngineReq::EnvironSizesGet(EnvironSizesGet))?;
                let EngineRet::EnvironSizes(sizes) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(
                    guest_slot.guest(),
                    sizes.count() as u32,
                    sizes.buf_size() as u32,
                    0,
                )?;
                Ok(())
            }
            hibana_wasip1_runtime::engine::wasm::Call::EnvironGet(pending) => {
                let request =
                    EngineReq::EnvironGet(EnvironGet::new(WASIP1_IO_CHUNK_CAPACITY as u8)?);
                let reply = self
                    .endpoint_call_blocking::<LABEL_WASI_ENVIRON_GET, LABEL_WASI_ENVIRON_GET_RET>(
                        request,
                    )?;
                let EngineRet::EnvironDone(done) = reply else {
                    return Err(WasiGuestError::UnexpectedReply);
                };
                pending.complete(guest_slot.guest(), &[(done.as_bytes(), &[][..])], 0)?;
                Ok(())
            }
        }
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn drive_fd_write_call(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::FdWrite,
    ) -> Result<(), WasiGuestError> {
        let fd = pending.fd();
        let payload = pending.payload(guest_slot.guest())?;
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
        pending.complete(guest_slot.guest(), done.errno() as u32)?;
        Ok(())
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_fd_write_call_blocking(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::FdWrite,
    ) -> Result<(), WasiGuestError> {
        let fd = pending.fd();
        let payload = pending.payload(guest_slot.guest())?;
        let request = EngineReq::FdWrite(FdWrite::new(fd, payload.as_bytes())?);
        let reply =
            self.endpoint_call_blocking::<LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET>(request)?;
        let EngineRet::FdWriteDone(done) = reply else {
            return Err(WasiGuestError::UnexpectedReply);
        };
        if done.fd() != fd {
            return Err(WasiGuestError::UnexpectedReply);
        }
        pending.complete(guest_slot.guest(), done.errno() as u32)?;
        Ok(())
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn drive_poll_oneoff_call(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::PollOneoff,
    ) -> Result<(), WasiGuestError> {
        let delay = pending.delay_ticks(guest_slot.guest())?;
        let reply = self
            .endpoint_call::<LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET>(
                EngineReq::PollOneoff(PollOneoff::new(delay)),
            )
            .await?;
        let EngineRet::PollReady(ready) = reply else {
            return Err(WasiGuestError::UnexpectedReply);
        };
        pending.complete(guest_slot.guest(), ready.ready() as u32, 0)?;
        Ok(())
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_poll_oneoff_call_blocking(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::PollOneoff,
    ) -> Result<(), WasiGuestError> {
        let delay = pending.delay_ticks(guest_slot.guest())?;
        let reply = self
            .endpoint_call_blocking::<LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET>(
                EngineReq::PollOneoff(PollOneoff::new(delay)),
            )?;
        let EngineRet::PollReady(ready) = reply else {
            return Err(WasiGuestError::UnexpectedReply);
        };
        pending.complete(guest_slot.guest(), ready.ready() as u32, 0)?;
        Ok(())
    }

    #[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
    async fn drive_memory_fence(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::MemoryFence,
    ) -> Result<(), WasiGuestError> {
        let fence = MemFence::new(MemFenceReason::MemoryGrow, pending.fence_epoch());
        if let Err(error) = self
            .endpoint()
            .send::<hibana::g::Msg<LABEL_MEM_FENCE, MemFence>>(&fence)
            .await
        {
            return Err(WasiGuestError::endpoint(
                0x5745_2000 | LABEL_MEM_FENCE as u32,
                error,
            ));
        }
        pending.complete(guest_slot.guest())?;
        Ok(())
    }

    #[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
    #[inline(never)]
    fn drive_memory_fence_blocking(
        &mut self,
        guest_slot: &mut WasiGuestSlot<'guest>,
        pending: hibana_wasip1_runtime::engine::wasm::MemoryFence,
    ) -> Result<(), WasiGuestError> {
        let fence = MemFence::new(MemFenceReason::MemoryGrow, pending.fence_epoch());
        if let Err(error) = poll_embedded_endpoint_unit(
            self.endpoint()
                .send::<hibana::g::Msg<LABEL_MEM_FENCE, MemFence>>(&fence),
        ) {
            return Err(WasiGuestError::endpoint(
                0x5745_2000 | LABEL_MEM_FENCE as u32,
                error,
            ));
        }
        pending.complete(guest_slot.guest())?;
        Ok(())
    }

    pub fn pending<E>(self) -> impl core::future::Future<Output = RoleResult<E>> {
        PendingRole::new(self)
    }
}

#[cfg(all(feature = "wasm-engine-core", any(test, not(target_os = "none"))))]
async fn drive_canonical_wasi_engine<'endpoint, 'guest, C, I, A, const ROLE: u8>(
    mut ctx: EngineCtx<'endpoint, 'guest, C, ROLE>,
) -> RoleResult<WasiGuestError>
where
    C: Capsule,
    I: LogicalImage<C>,
    A: ArtifactInput<C, I>,
{
    loop {
        match ctx
            .drive_wasi_guest(<A as ArtifactInput<C, I>>::wasi_budget::<ROLE>())
            .await
        {
            Ok(WasiGuestStatus::BudgetExpired(_)) => {}
            Ok(WasiGuestStatus::Exit(_)) => {
                return ctx.pending().await;
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
}

#[cfg(all(feature = "wasm-engine-core", not(test), target_os = "none"))]
fn run_canonical_wasi_engine_forever<'endpoint, 'guest, C, I, A, const ROLE: u8>(
    storage: EmbeddedAttachStorageRef<'static>,
    ctx: EngineCtx<'endpoint, 'guest, C, ROLE>,
) -> !
where
    C: Capsule,
    I: LogicalImage<C>,
    A: ArtifactInput<C, I>,
{
    assert!(
        size_of::<EngineCtx<'endpoint, 'guest, C, ROLE>>() <= storage.future_bytes,
        "appkit embedded WASI engine context exceeds embedded role arena"
    );
    assert!(
        align_of::<EngineCtx<'endpoint, 'guest, C, ROLE>>() <= APPKIT_EMBEDDED_FUTURE_ALIGN,
        "appkit embedded WASI engine context alignment exceeds embedded role arena"
    );

    unsafe {
        let ctx_ptr = storage
            .future
            .cast::<EngineCtx<'endpoint, 'guest, C, ROLE>>();
        ctx_ptr.write(ctx);
        let ctx = &mut *ctx_ptr;
        loop {
            match ctx.drive_wasi_guest_blocking(<A as ArtifactInput<C, I>>::wasi_budget::<ROLE>()) {
                Ok(WasiGuestStatus::BudgetExpired(_)) => {}
                Ok(WasiGuestStatus::Exit(_)) => {
                    embedded_pending_forever(ctx);
                }
                Err(error) => {
                    core::hint::black_box(&error);
                    panic!("appkit embedded WASI role task failed: {error:?}");
                }
            }
        }
    }
}

/// Driver-side localside context.
pub struct DriverCtx<'a, C: Capsule, const ROLE: u8> {
    endpoint: RoleEndpointCtx<'a, C, ROLE>,
    facts: DriverFacts<'a>,
}

impl<'a, C: Capsule, const ROLE: u8> DriverCtx<'a, C, ROLE> {
    fn new(endpoint: RoleEndpointCtx<'a, C, ROLE>, facts: DriverFacts<'a>) -> Self {
        Self { endpoint, facts }
    }

    pub const fn choreofs(&self) -> ChoreoFsFacts<'a> {
        self.facts.choreofs()
    }

    pub const fn ledger(&self) -> LedgerFacts<'a> {
        self.facts.ledger()
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
}

impl<'a, C: Capsule, const ROLE: u8> BoundaryCtx<'a, C, ROLE> {
    fn new(endpoint: RoleEndpointCtx<'a, C, ROLE>) -> Self {
        Self { endpoint }
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
}

/// Canonical appkit execution path.
// `ArtifactInput` intentionally stays private: callers pass `NoWasi` or
// `WasiImage`, but never name or implement the artifact boundary trait.
#[allow(private_bounds)]
pub fn run<I, C>(artifact: impl ArtifactInput<C, I>)
where
    C: Capsule,
    I: LogicalImage<C>,
{
    run_with_artifact::<I, C, _>(artifact)
}

fn run_with_artifact<I, C, A>(artifact: A)
where
    C: Capsule,
    I: LogicalImage<C>,
    A: ArtifactInput<C, I>,
{
    let program = C::choreography();
    let projected_roles = collect_projected_roles::<C, I>(&program);
    let wasi_guest_bytes = artifact.wasi_bytes();
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
    let attach_summary = attach_projected_roles::<C, I, A>(&program, wasi_guest_bytes);
    assert!(
        attach_summary.endpoint_count == projected_roles.count(),
        "logical image projected roles must attach through SessionKit"
    );
    let mut image = I::init();
    image.safe_state();
}
