#![no_std]

use core::ptr::{read_volatile, write_volatile};

use hibana::{
    g,
    integration::{program::Projectable, runtime::DefaultLabelUniverse},
};
use hibana_pico::appkit::ArtifactBundle;
use hibana_pico::{appkit, site};

pub struct Control;
pub struct ControlPlacement;
pub struct ControlLocal;
pub struct ControlArtifacts;

const HETEROGENEOUS_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(3001);
const EXAMPLE_FRAME_BYTES: usize = 32;
const EXAMPLE_LANE_SLOTS: u8 = 4;

static mut ROLE0_PHASE: u8 = 0;
static mut ROLE0_TO_ROLE1_SENT: u8 = 0;
static mut ROLE0_TO_ROLE1_RECV: u8 = 0;
static mut ROLE1_TO_ROLE2_SENT: u8 = 0;
static mut ROLE1_TO_ROLE2_RECV: u8 = 0;
static mut ROLE2_TO_ROLE0_SENT: u8 = 0;

fn read_counter(counter: *const u8) -> u8 {
    unsafe { read_volatile(counter) }
}

fn write_counter(counter: *mut u8, value: u8) {
    unsafe {
        write_volatile(counter, value);
    }
}

fn bump_counter(counter: *mut u8) {
    let next = read_counter(counter).wrapping_add(1);
    write_counter(counter, next);
}

struct ExampleFrameSlot {
    ready: bool,
    session_id: u32,
    sender: u8,
    peer: u8,
    label: hibana::integration::transport::FrameLabel,
    len: usize,
    bytes: [u8; EXAMPLE_FRAME_BYTES],
}

impl ExampleFrameSlot {
    const fn empty() -> Self {
        Self {
            ready: false,
            session_id: 0,
            sender: 0,
            peer: 0,
            label: hibana::integration::transport::FrameLabel::new(0),
            len: 0,
            bytes: [0; EXAMPLE_FRAME_BYTES],
        }
    }

    fn clear(&mut self) {
        *self = Self::empty();
    }

    fn matches(&self, session_id: u32, peer: u8) -> bool {
        self.ready && self.session_id == session_id && self.peer == peer
    }

    fn push(
        &mut self,
        session_id: u32,
        sender: u8,
        peer: u8,
        label: hibana::integration::transport::FrameLabel,
        payload: hibana::integration::wire::Payload<'_>,
    ) -> Result<(), hibana::integration::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > EXAMPLE_FRAME_BYTES {
            return Err(hibana::integration::transport::TransportError::Failed);
        }
        if self.ready {
            return Err(hibana::integration::transport::TransportError::Failed);
        }
        self.session_id = session_id;
        self.sender = sender;
        self.peer = peer;
        self.label = label;
        self.len = bytes.len();
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.ready = true;
        Ok(())
    }

    fn pop_into<'a>(
        &mut self,
        session_id: u32,
        peer: u8,
        rx: &'a mut ExampleRx,
    ) -> Option<hibana::integration::wire::Payload<'a>> {
        if !self.matches(session_id, peer) {
            return None;
        }
        let len = self.len;
        rx.bytes[..len].copy_from_slice(&self.bytes[..len]);
        self.clear();
        Some(hibana::integration::wire::Payload::new(&rx.bytes[..len]))
    }

    fn frame_header(
        &self,
        session_id: u32,
        lane: u8,
        peer: u8,
    ) -> Option<hibana::integration::transport::FrameHeader> {
        if !self.matches(session_id, peer) {
            return None;
        }
        Some(hibana::integration::transport::FrameHeader::new(
            hibana::integration::ids::SessionId::new(self.session_id),
            hibana::integration::ids::Lane::new(lane as u32),
            self.sender,
            self.peer,
            self.label,
        ))
    }
}

struct ExampleEdgeSlots {
    lane0: ExampleFrameSlot,
    lane1: ExampleFrameSlot,
    lane2: ExampleFrameSlot,
    lane3: ExampleFrameSlot,
}

impl ExampleEdgeSlots {
    const fn empty() -> Self {
        Self {
            lane0: ExampleFrameSlot::empty(),
            lane1: ExampleFrameSlot::empty(),
            lane2: ExampleFrameSlot::empty(),
            lane3: ExampleFrameSlot::empty(),
        }
    }

    fn slot(&self, lane: u8) -> Option<&ExampleFrameSlot> {
        match lane {
            0 => Some(&self.lane0),
            1 => Some(&self.lane1),
            2 => Some(&self.lane2),
            3 => Some(&self.lane3),
            _ => None,
        }
    }

    fn slot_mut(&mut self, lane: u8) -> Option<&mut ExampleFrameSlot> {
        match lane {
            0 => Some(&mut self.lane0),
            1 => Some(&mut self.lane1),
            2 => Some(&mut self.lane2),
            3 => Some(&mut self.lane3),
            _ => None,
        }
    }

    fn clear(&mut self) {
        self.lane0.clear();
        self.lane1.clear();
        self.lane2.clear();
        self.lane3.clear();
    }
}

static mut EXAMPLE_FRAME_0_TO_1: ExampleEdgeSlots = ExampleEdgeSlots::empty();
static mut EXAMPLE_FRAME_1_TO_2: ExampleEdgeSlots = ExampleEdgeSlots::empty();
static mut EXAMPLE_FRAME_2_TO_0: ExampleEdgeSlots = ExampleEdgeSlots::empty();

fn edge_slots_for_send(local_role: u8, peer: u8) -> Option<*mut ExampleEdgeSlots> {
    match (local_role, peer) {
        (0, 1) => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1)),
        (1, 2) => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2)),
        (2, 0) => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0)),
        _ => None,
    }
}

fn edge_slots_for_recv(local_role: u8) -> Option<*mut ExampleEdgeSlots> {
    match local_role {
        0 => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0)),
        1 => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1)),
        2 => Some(core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2)),
        _ => None,
    }
}

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
    lane: u8,
}
pub struct ExampleRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    bytes: [u8; EXAMPLE_FRAME_BYTES],
}

impl hibana::integration::transport::Transport for ExampleCarrier {
    type Error = hibana::integration::transport::TransportError;
    type Tx<'a>
        = ExampleTx
    where
        Self: 'a;
    type Rx<'a>
        = ExampleRx
    where
        Self: 'a;
    fn open<'a>(
        &'a self,
        port: hibana::integration::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane().as_wire();
        assert!(lane < EXAMPLE_LANE_SLOTS);
        (
            ExampleTx {
                local_role,
                session_id,
                lane,
            },
            ExampleRx {
                local_role,
                session_id,
                lane,
                bytes: [0; EXAMPLE_FRAME_BYTES],
            },
        )
    }

    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        assert_ne!(tx.session_id, 0);
        assert_ne!(outgoing.peer(), tx.local_role);
        if outgoing.lane() != tx.lane {
            return core::task::Poll::Ready(Err(
                hibana::integration::transport::TransportError::Failed,
            ));
        }
        let edge = match edge_slots_for_send(tx.local_role, outgoing.peer()) {
            Some(edge) => edge,
            None => {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
                ));
            }
        };
        match (tx.local_role, outgoing.peer()) {
            (0, 1) => {
                bump_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_SENT));
            }
            (1, 2) => {
                bump_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_SENT));
            }
            (2, 0) => {
                bump_counter(core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_SENT));
            }
            _ => {}
        };
        let edge = unsafe { &mut *edge };
        let slot = match edge.slot_mut(outgoing.lane()) {
            Some(slot) => slot,
            None => {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
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

    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        assert_ne!(tx.session_id, 0);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<hibana::integration::transport::Incoming<'a>, Self::Error>> {
        assert_ne!(rx.session_id, 0);
        let local_role = rx.local_role;
        let session_id = rx.session_id;
        let edge = match edge_slots_for_recv(local_role) {
            Some(edge) => edge,
            None => {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
                ));
            }
        };
        let edge = unsafe { &mut *edge };
        let slot = match edge.slot_mut(rx.lane) {
            Some(slot) => slot,
            None => {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
                ));
            }
        };
        let header = slot.frame_header(session_id, rx.lane, local_role);
        match slot.pop_into(session_id, local_role, rx) {
            Some(payload) => {
                match local_role {
                    1 => {
                        bump_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_RECV));
                    }
                    2 => {
                        bump_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_RECV));
                    }
                    _ => {}
                }
                let Some(header) = header else {
                    return core::task::Poll::Ready(Err(
                        hibana::integration::transport::TransportError::Failed,
                    ));
                };
                core::task::Poll::Ready(Ok(hibana::integration::transport::Incoming::new(
                    header, payload,
                )))
            }
            None => {
                core::hint::black_box(task_context);
                core::task::Poll::Pending
            }
        }
    }

    fn requeue<'a>(&self, rx: &mut Self::Rx<'a>) -> Result<(), Self::Error> {
        assert_ne!(rx.session_id, 0);
        core::hint::black_box(rx.local_role);
        core::hint::black_box(rx.lane);
        Ok(())
    }

    fn peek_recv_frame<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameHeader> {
        let edge = edge_slots_for_recv(rx.local_role)?;
        let edge = unsafe { &*edge };
        let slot = edge.slot(rx.lane)?;
        slot.frame_header(rx.session_id, rx.lane, rx.local_role)
    }
}

impl appkit::Capsule for Control {
    type Universe = DefaultLabelUniverse;
    type Placement = ControlPlacement;
    type Local = ControlLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable {
        g::seq(
            g::send::<0, 1, g::Msg<31, ()>, 0>(),
            g::seq(
                g::send::<1, 2, g::Msg<32, ()>, 1>(),
                g::send::<2, 0, g::Msg<33, ()>, 2>(),
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, Control, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            assert_eq!(ROLE, 0);
            ctx.endpoint()
                .flow::<g::Msg<31, ()>>()
                .expect("role0->role1 flow")
                .send(&())
                .await
                .expect("role0 sends through example carrier");
            write_counter(core::ptr::addr_of_mut!(ROLE0_PHASE), 1);
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, Control, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
}

pub static ARTIFACTS: ControlArtifacts = ControlArtifacts;

fn reset_live_carrier_evidence() {
    write_counter(core::ptr::addr_of_mut!(ROLE0_PHASE), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_SENT), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_RECV), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_SENT), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_RECV), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_SENT), 0);
    unsafe {
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1)).clear();
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2)).clear();
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0)).clear();
    }
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
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_PHASE)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_SENT)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_RECV)), 0);
    let m33 = appkit::run::<site::Local<image::M33Realtime>, Control>(
        ARTIFACTS.for_image::<site::Local<image::M33Realtime>>(),
    );
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_RECV)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_SENT)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_RECV)), 0);
    let rp2040 = appkit::run::<site::Local<image::Rp2040Io>, Control>(
        ARTIFACTS.for_image::<site::Local<image::Rp2040Io>>(),
    );
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_RECV)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE2_TO_ROLE0_SENT)), 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rx(role: u8, session_id: u32, lane: u8) -> ExampleRx {
        ExampleRx {
            local_role: role,
            session_id,
            lane,
            bytes: [0; EXAMPLE_FRAME_BYTES],
        }
    }

    #[test]
    fn frame_slot_publishes_only_complete_ready_frames() {
        let mut slot = ExampleFrameSlot::empty();
        let label = hibana::integration::transport::FrameLabel::new(31);
        let payload = hibana::integration::wire::Payload::new(b"abc");
        let mut receiver = rx(1, 7, 0);

        assert!(slot.pop_into(7, 1, &mut receiver).is_none());
        assert!(slot.push(7, 0, 1, label, payload).is_ok());
        assert_eq!(slot.frame_label(7, 1), Some(label));
        assert!(
            slot.push(
                7,
                0,
                1,
                hibana::integration::transport::FrameLabel::new(32),
                hibana::integration::wire::Payload::new(b"def"),
            )
            .is_err()
        );

        let received = match slot.pop_into(7, 1, &mut receiver) {
            Some(payload) => payload,
            None => panic!("ready frame must be visible to the matching receiver"),
        };
        assert_eq!(received.as_bytes(), b"abc");
        assert!(slot.pop_into(7, 1, &mut receiver).is_none());
    }

    #[test]
    fn edge_slots_are_lane_scoped() {
        let mut edge = ExampleEdgeSlots::empty();
        let label0 = hibana::integration::transport::FrameLabel::new(31);
        let label1 = hibana::integration::transport::FrameLabel::new(41);

        assert!(
            edge.slot_mut(0)
                .expect("lane 0 slot must exist")
                .push(
                    9,
                    0,
                    1,
                    label0,
                    hibana::integration::wire::Payload::new(b"lane0"),
                )
                .is_ok()
        );
        assert!(
            edge.slot_mut(1)
                .expect("lane 1 slot must exist")
                .push(
                    9,
                    0,
                    1,
                    label1,
                    hibana::integration::wire::Payload::new(b"lane1"),
                )
                .is_ok()
        );

        let mut receiver0 = rx(1, 9, 0);
        let mut receiver1 = rx(1, 9, 1);
        let received0 =
            match edge
                .slot_mut(0)
                .expect("lane 0 slot must exist")
                .pop_into(9, 1, &mut receiver0)
            {
                Some(payload) => payload,
                None => panic!("lane 0 payload must be ready"),
            };
        let received1 =
            match edge
                .slot_mut(1)
                .expect("lane 1 slot must exist")
                .pop_into(9, 1, &mut receiver1)
            {
                Some(payload) => payload,
                None => panic!("lane 1 payload must be ready"),
            };

        assert_eq!(received0.as_bytes(), b"lane0");
        assert_eq!(received1.as_bytes(), b"lane1");
        assert!(edge.slot(4).is_none());
    }
}
