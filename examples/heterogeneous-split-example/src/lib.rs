#![no_std]

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use hibana::{
    g,
    substrate::{program::Projectable, runtime::DefaultLabelUniverse},
};
use hibana_pico::appkit::ArtifactBundle;
use hibana_pico::{appkit, site};

pub struct Control;
pub struct ControlPlacement;
pub struct ControlLocal;
pub struct ControlArtifacts;

const HETEROGENEOUS_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(3001);
const EXAMPLE_FRAME_BYTES: usize = 32;

static ROLE0_PHASE: AtomicU8 = AtomicU8::new(0);
static ROLE0_TO_ROLE1_SENT: AtomicU8 = AtomicU8::new(0);
static ROLE0_TO_ROLE1_RECV: AtomicU8 = AtomicU8::new(0);
static ROLE1_TO_ROLE2_SENT: AtomicU8 = AtomicU8::new(0);
static ROLE1_TO_ROLE2_RECV: AtomicU8 = AtomicU8::new(0);
static ROLE2_TO_ROLE0_SENT: AtomicU8 = AtomicU8::new(0);

struct ExampleFrameSlot {
    occupied: AtomicBool,
    session_id: UnsafeCell<u32>,
    sender: UnsafeCell<u8>,
    peer: UnsafeCell<u8>,
    label: UnsafeCell<hibana::substrate::transport::FrameLabel>,
    len: UnsafeCell<usize>,
    bytes: UnsafeCell<[u8; EXAMPLE_FRAME_BYTES]>,
}

unsafe impl Sync for ExampleFrameSlot {}

impl ExampleFrameSlot {
    const fn empty() -> Self {
        Self {
            occupied: AtomicBool::new(false),
            session_id: UnsafeCell::new(0),
            sender: UnsafeCell::new(0),
            peer: UnsafeCell::new(0),
            label: UnsafeCell::new(hibana::substrate::transport::FrameLabel::new(0)),
            len: UnsafeCell::new(0),
            bytes: UnsafeCell::new([0; EXAMPLE_FRAME_BYTES]),
        }
    }

    fn clear(&self) {
        self.occupied.store(false, Ordering::Release);
    }

    fn matches(&self, session_id: u32, peer: u8) -> bool {
        if !self.occupied.load(Ordering::Acquire) {
            return false;
        }
        let stored_session_id = unsafe { *self.session_id.get() };
        let stored_peer = unsafe { *self.peer.get() };
        stored_session_id == session_id && stored_peer == peer
    }

    fn push(
        &self,
        session_id: u32,
        sender: u8,
        peer: u8,
        label: hibana::substrate::transport::FrameLabel,
        payload: hibana::substrate::wire::Payload<'_>,
    ) -> Result<(), hibana::substrate::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > EXAMPLE_FRAME_BYTES {
            return Err(hibana::substrate::transport::TransportError::Failed);
        }
        if self.occupied.swap(true, Ordering::AcqRel) {
            return Err(hibana::substrate::transport::TransportError::Failed);
        }
        unsafe {
            *self.session_id.get() = session_id;
            *self.sender.get() = sender;
            *self.peer.get() = peer;
            *self.label.get() = label;
            *self.len.get() = bytes.len();
            (&mut *self.bytes.get())[..bytes.len()].copy_from_slice(bytes);
        }
        Ok(())
    }

    fn pop_into<'a>(
        &self,
        session_id: u32,
        peer: u8,
        rx: &'a mut ExampleRx,
    ) -> Option<hibana::substrate::wire::Payload<'a>> {
        if !self.matches(session_id, peer) {
            return None;
        }
        let len = unsafe { *self.len.get() };
        unsafe {
            rx.bytes[..len].copy_from_slice(&(&*self.bytes.get())[..len]);
        }
        self.clear();
        Some(hibana::substrate::wire::Payload::new(&rx.bytes[..len]))
    }

    fn frame_label(
        &self,
        session_id: u32,
        peer: u8,
    ) -> Option<hibana::substrate::transport::FrameLabel> {
        if !self.matches(session_id, peer) {
            return None;
        }
        Some(unsafe { *self.label.get() })
    }
}

static EXAMPLE_FRAME_0_TO_1: ExampleFrameSlot = ExampleFrameSlot::empty();
static EXAMPLE_FRAME_1_TO_2: ExampleFrameSlot = ExampleFrameSlot::empty();
static EXAMPLE_FRAME_2_TO_0: ExampleFrameSlot = ExampleFrameSlot::empty();

#[cfg(feature = "wasm-engine-core")]
static HETEROGENEOUS_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(all(not(test), target_os = "none"))]
const HETEROGENEOUS_ATTACH_SLAB_BYTES: usize = 32 * 1024;

#[cfg(all(not(test), target_os = "none"))]
static LINUX_CONTROL_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<
    HETEROGENEOUS_ATTACH_SLAB_BYTES,
> = appkit::EmbeddedAttachStorage::empty();

#[cfg(all(not(test), target_os = "none"))]
static M33_REALTIME_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<HETEROGENEOUS_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

#[cfg(all(not(test), target_os = "none"))]
static RP2040_IO_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<HETEROGENEOUS_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

pub mod image {
    pub struct LinuxControl;
    pub struct M33Realtime;
    pub struct Rp2040Io;
}

pub struct ExampleCarrier;
pub struct ExampleTx {
    local_role: u8,
    session_id: u32,
}
pub struct ExampleRx {
    local_role: u8,
    session_id: u32,
    bytes: [u8; EXAMPLE_FRAME_BYTES],
}

impl hibana::substrate::Transport for ExampleCarrier {
    type Error = hibana::substrate::transport::TransportError;
    type Tx<'a>
        = ExampleTx
    where
        Self: 'a;
    type Rx<'a>
        = ExampleRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            ExampleTx {
                local_role,
                session_id,
            },
            ExampleRx {
                local_role,
                session_id,
                bytes: [0; EXAMPLE_FRAME_BYTES],
            },
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
        assert_ne!(tx.session_id, 0);
        assert_ne!(outgoing.peer(), tx.local_role);
        let slot = match (tx.local_role, outgoing.peer()) {
            (0, 1) => {
                ROLE0_TO_ROLE1_SENT.fetch_add(1, Ordering::AcqRel);
                &EXAMPLE_FRAME_0_TO_1
            }
            (1, 2) => {
                ROLE1_TO_ROLE2_SENT.fetch_add(1, Ordering::AcqRel);
                &EXAMPLE_FRAME_1_TO_2
            }
            (2, 0) => {
                ROLE2_TO_ROLE0_SENT.fetch_add(1, Ordering::AcqRel);
                &EXAMPLE_FRAME_2_TO_0
            }
            _ => {
                return core::task::Poll::Ready(Err(
                    hibana::substrate::transport::TransportError::Failed,
                ));
            }
        };
        match slot.push(
            tx.session_id,
            tx.local_role,
            outgoing.peer(),
            outgoing.frame_label(),
            outgoing.payload(),
        ) {
            Ok(()) => {}
            Err(error) => return core::task::Poll::Ready(Err(error)),
        }
        cx.waker().wake_by_ref();
        core::task::Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        assert_ne!(tx.session_id, 0);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<hibana::substrate::wire::Payload<'a>, Self::Error>> {
        assert_ne!(rx.session_id, 0);
        let local_role = rx.local_role;
        let session_id = rx.session_id;
        let slot = match local_role {
            0 => &EXAMPLE_FRAME_2_TO_0,
            1 => &EXAMPLE_FRAME_0_TO_1,
            2 => &EXAMPLE_FRAME_1_TO_2,
            _ => {
                return core::task::Poll::Ready(Err(
                    hibana::substrate::transport::TransportError::Failed,
                ));
            }
        };
        match slot.pop_into(session_id, local_role, rx) {
            Some(payload) => {
                match local_role {
                    1 => {
                        ROLE0_TO_ROLE1_RECV.fetch_add(1, Ordering::AcqRel);
                    }
                    2 => {
                        ROLE1_TO_ROLE2_RECV.fetch_add(1, Ordering::AcqRel);
                    }
                    _ => {}
                }
                core::task::Poll::Ready(Ok(payload))
            }
            None => {
                core::hint::black_box(task_context);
                core::task::Poll::Pending
            }
        }
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        assert_ne!(rx.session_id, 0);
        core::hint::black_box(rx.local_role);
    }

    fn drain_events(
        &self,
        emit: &mut dyn FnMut(hibana::substrate::transport::advanced::TransportEvent),
    ) {
        emit(hibana::substrate::transport::advanced::TransportEvent::new(
            hibana::substrate::transport::advanced::TransportEventKind::Ack,
            0,
            0,
            0,
        ));
    }

    fn recv_frame_hint<'a>(
        &'a self,
        rx: &'a Self::Rx<'a>,
    ) -> Option<hibana::substrate::transport::FrameLabel> {
        let slot = match rx.local_role {
            0 => &EXAMPLE_FRAME_2_TO_0,
            1 => &EXAMPLE_FRAME_0_TO_1,
            2 => &EXAMPLE_FRAME_1_TO_2,
            _ => return None,
        };
        slot.frame_label(rx.session_id, rx.local_role)
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        assert!(interval_us > 0 || burst_bytes == 0);
    }
}

impl appkit::Capsule for Control {
    type Universe = DefaultLabelUniverse;
    type Placement = ControlPlacement;
    type Local = ControlLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<31, ()>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<2>, g::Msg<32, ()>, 1>(),
                g::send::<g::Role<2>, g::Role<0>, g::Msg<33, ()>, 2>(),
            ),
        )
    }
}

impl appkit::Placement<Control> for ControlPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            2 => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Link,
        }
    }
}

impl appkit::Localside<Control> for ControlLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, Control, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            assert_eq!(ROLE, 0);
            ctx.endpoint()
                .flow::<g::Msg<31, ()>>()
                .expect("role0->role1 flow")
                .send(&())
                .await
                .expect("role0 sends through example carrier");
            ROLE0_PHASE.store(1, Ordering::Release);
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            assert_eq!(ROLE, 1);
            ctx.endpoint()
                .recv::<g::Msg<31, ()>>()
                .await
                .expect("role1 receives through example carrier");
            ctx.endpoint()
                .flow::<g::Msg<32, ()>>()
                .expect("role1->role2 flow")
                .send(&())
                .await
                .expect("role1 sends through example carrier");
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        mut ctx: appkit::BoundaryCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            assert_eq!(ROLE, 2);
            ctx.endpoint()
                .recv::<g::Msg<32, ()>>()
                .await
                .expect("role2 receives through example carrier");
            ctx.endpoint()
                .flow::<g::Msg<33, ()>>()
                .expect("role2->role0 flow")
                .send(&())
                .await
                .expect("role2 sends through example carrier");
            ctx.pending().await
        }
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<Control, I> for ControlArtifacts
where
    I: appkit::LogicalImage<Control, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl appkit::LogicalImage<Control> for site::Local<image::LinuxControl> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ExampleCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(30);
    const SITE_ID: appkit::SiteId = appkit::SiteId(300);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = HETEROGENEOUS_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet =
        appkit::PeerImageSet::pair(appkit::ImageId(31), appkit::ImageId(32));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ExampleCarrier
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        LINUX_CONTROL_ATTACH_STORAGE.lease()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        core::hint::black_box(ROLE);
        HETEROGENEOUS_WASI_GUEST_ARENA.storage()
    }
}

impl appkit::LogicalImage<Control> for site::Local<image::M33Realtime> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ExampleCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(31);
    const SITE_ID: appkit::SiteId = appkit::SiteId(331);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = HETEROGENEOUS_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet =
        appkit::PeerImageSet::pair(appkit::ImageId(30), appkit::ImageId(32));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ExampleCarrier
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        M33_REALTIME_ATTACH_STORAGE.lease()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        core::hint::black_box(ROLE);
        HETEROGENEOUS_WASI_GUEST_ARENA.storage()
    }
}

impl appkit::LogicalImage<Control> for site::Local<image::Rp2040Io> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ExampleCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(32);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(2);
    const CARRIER: appkit::CarrierKind = HETEROGENEOUS_CARRIER;
    const PEER_IMAGES: appkit::PeerImageSet =
        appkit::PeerImageSet::pair(appkit::ImageId(30), appkit::ImageId(31));

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ExampleCarrier
    }

    #[cfg(all(not(test), target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        RP2040_IO_ATTACH_STORAGE.lease()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        core::hint::black_box(ROLE);
        HETEROGENEOUS_WASI_GUEST_ARENA.storage()
    }
}

pub static ARTIFACTS: ControlArtifacts = ControlArtifacts;

fn reset_live_carrier_evidence() {
    ROLE0_PHASE.store(0, Ordering::Release);
    ROLE0_TO_ROLE1_SENT.store(0, Ordering::Release);
    ROLE0_TO_ROLE1_RECV.store(0, Ordering::Release);
    ROLE1_TO_ROLE2_SENT.store(0, Ordering::Release);
    ROLE1_TO_ROLE2_RECV.store(0, Ordering::Release);
    ROLE2_TO_ROLE0_SENT.store(0, Ordering::Release);
    EXAMPLE_FRAME_0_TO_1.clear();
    EXAMPLE_FRAME_1_TO_2.clear();
    EXAMPLE_FRAME_2_TO_0.clear();
}

pub fn assert_single_role_image<R, I>(
    report: &appkit::RunReport<R, I>,
    image_id: appkit::ImageId,
    site_id: appkit::SiteId,
    role: u8,
) {
    assert_eq!(report.image_id(), image_id);
    assert_eq!(report.site_id(), site_id);
    assert_eq!(report.requested_roles(), appkit::RoleSet::single(role));
    assert_eq!(report.validated_role_count(), 1);
    assert_eq!(report.attached_endpoint_count(), 1);
    assert_eq!(report.manifest().peer_image_count, 2);
    assert!(
        !report
            .manifest()
            .peer_images()
            .contains(report.manifest().logical_image_id)
    );
    assert!(report.projected_roles().contains(0));
    assert!(report.projected_roles().contains(1));
    assert!(report.projected_roles().contains(2));
    assert_eq!(
        report.manifest().requested_role_set,
        appkit::RoleSet::single(role)
    );
}

pub fn assert_peer_manifests() {
    reset_live_carrier_evidence();
    let linux = appkit::run::<site::Local<image::LinuxControl>, Control>(
        ARTIFACTS.for_image::<site::Local<image::LinuxControl>>(),
    );
    assert_eq!(ROLE0_PHASE.load(Ordering::Acquire), 1);
    assert_eq!(ROLE0_TO_ROLE1_SENT.load(Ordering::Acquire), 1);
    assert_eq!(ROLE0_TO_ROLE1_RECV.load(Ordering::Acquire), 0);
    let m33 = appkit::run::<site::Local<image::M33Realtime>, Control>(
        ARTIFACTS.for_image::<site::Local<image::M33Realtime>>(),
    );
    assert_eq!(ROLE0_TO_ROLE1_RECV.load(Ordering::Acquire), 1);
    assert_eq!(ROLE1_TO_ROLE2_SENT.load(Ordering::Acquire), 1);
    assert_eq!(ROLE1_TO_ROLE2_RECV.load(Ordering::Acquire), 0);
    let rp2040 = appkit::run::<site::Local<image::Rp2040Io>, Control>(
        ARTIFACTS.for_image::<site::Local<image::Rp2040Io>>(),
    );
    assert_eq!(ROLE1_TO_ROLE2_RECV.load(Ordering::Acquire), 1);
    assert_eq!(ROLE2_TO_ROLE0_SENT.load(Ordering::Acquire), 1);
    assert_eq!(
        linux.manifest().choreography_session_id,
        m33.manifest().choreography_session_id
    );
    assert_eq!(
        m33.manifest().choreography_session_id,
        rp2040.manifest().choreography_session_id
    );
    assert!(linux.manifest().can_attach_peer(&m33.manifest()));
    assert!(m33.manifest().can_attach_peer(&rp2040.manifest()));
    assert!(rp2040.manifest().can_attach_peer(&linux.manifest()));
}
