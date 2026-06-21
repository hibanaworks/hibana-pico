#![no_std]

use core::ptr::{read_volatile, write_volatile};

use hibana::{g, runtime::program::Projectable};
use hibana_pico::appkit;

pub struct Control;
pub struct ControlPlacement;
pub struct ControlLocal;

const EXAMPLE_FRAME_BYTES: usize = 32;
const EXAMPLE_LANE_SLOTS: u8 = 4;

static mut ROLE0_PHASE: u8 = 0;
static mut ROLE0_TO_ROLE1_SENT: u8 = 0;
static mut ROLE0_TO_ROLE1_RECV: u8 = 0;
static mut ROLE1_TO_ROLE2_SENT: u8 = 0;
static mut ROLE1_TO_ROLE2_RECV: u8 = 0;
static mut ROLE2_TO_ROLE0_SENT: u8 = 0;
static mut ROLE2_TO_ROLE0_RECV: u8 = 0;

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
    label: u8,
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
            label: 0,
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
        label: u8,
        payload: hibana::runtime::wire::Payload<'_>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > EXAMPLE_FRAME_BYTES {
            return Err(hibana::runtime::transport::TransportError::Failed);
        }
        if self.ready {
            return Err(hibana::runtime::transport::TransportError::Failed);
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
        lane: u8,
        peer: u8,
        rx: &'a mut ExampleRx,
    ) -> Option<hibana::runtime::transport::ReceivedFrame<'a>> {
        if !self.matches(session_id, peer) {
            return None;
        }
        let session = self.session_id.to_be_bytes();
        let header = hibana::runtime::transport::FrameHeader::from_bytes([
            session[0],
            session[1],
            session[2],
            session[3],
            lane,
            self.sender,
            self.peer,
            self.label,
        ]);
        let len = self.len;
        rx.bytes[..len].copy_from_slice(&self.bytes[..len]);
        self.clear();
        Some(hibana::runtime::transport::ReceivedFrame::framed(
            header,
            hibana::runtime::wire::Payload::new(&rx.bytes[..len]),
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

    fn slot_mut(&mut self, lane: u8) -> &mut ExampleFrameSlot {
        match lane {
            0 => &mut self.lane0,
            1 => &mut self.lane1,
            2 => &mut self.lane2,
            3 => &mut self.lane3,
            other => panic!("heterogeneous split carrier has no lane {other}"),
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

fn edge_slots_for_send(local_role: u8, peer: u8) -> (*mut ExampleEdgeSlots, *mut u8) {
    match (local_role, peer) {
        (0, 1) => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1),
            core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_SENT),
        ),
        (1, 2) => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2),
            core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_SENT),
        ),
        (2, 0) => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0),
            core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_SENT),
        ),
        (sender, receiver) => {
            panic!("heterogeneous split carrier has no send edge {sender}->{receiver}")
        }
    }
}

fn edge_slots_for_recv(local_role: u8) -> (*mut ExampleEdgeSlots, *mut u8) {
    match local_role {
        0 => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0),
            core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_RECV),
        ),
        1 => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1),
            core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_RECV),
        ),
        2 => (
            core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2),
            core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_RECV),
        ),
        other => panic!("heterogeneous split carrier has no recv edge for role {other}"),
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

impl hibana::runtime::transport::Transport for ExampleCarrier {
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
        port: hibana::runtime::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_role = port.local_role();
        let session_id = port.session_id().raw();
        let lane = port.lane();
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
        outgoing: hibana::runtime::transport::Outgoing<'f>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), hibana::runtime::transport::TransportError>>
    where
        'a: 'f,
    {
        assert_ne!(tx.session_id, 0);
        assert_ne!(outgoing.target_role(), tx.local_role);
        if outgoing.lane() != tx.lane {
            return core::task::Poll::Ready(Err(
                hibana::runtime::transport::TransportError::Failed,
            ));
        }
        let (edge, sent_counter) = edge_slots_for_send(tx.local_role, outgoing.target_role());
        bump_counter(sent_counter);
        let edge = unsafe { &mut *edge };
        let slot = edge.slot_mut(outgoing.lane());
        match slot.push(
            tx.session_id,
            tx.local_role,
            outgoing.target_role(),
            outgoing.frame_label().raw(),
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
    ) -> core::task::Poll<
        Result<
            hibana::runtime::transport::ReceivedFrame<'a>,
            hibana::runtime::transport::TransportError,
        >,
    > {
        assert_ne!(rx.session_id, 0);
        let local_role = rx.local_role;
        let session_id = rx.session_id;
        let (edge, recv_counter) = edge_slots_for_recv(local_role);
        let edge = unsafe { &mut *edge };
        let slot = edge.slot_mut(rx.lane);
        match slot.pop_into(session_id, rx.lane, local_role, rx) {
            Some(received) => {
                bump_counter(recv_counter);
                core::task::Poll::Ready(Ok(received))
            }
            None => {
                core::hint::black_box(task_context);
                core::task::Poll::Pending
            }
        }
    }

    fn requeue<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Result<(), hibana::runtime::transport::TransportError> {
        assert_ne!(rx.session_id, 0);
        core::hint::black_box(rx.local_role);
        core::hint::black_box(rx.lane);
        Ok(())
    }
}

impl appkit::Capsule for Control {
    type Placement = ControlPlacement;
    type Local = ControlLocal;

    fn choreography() -> impl Projectable {
        g::seq(
            g::send::<0, 1, g::Msg<31, ()>>(),
            g::seq(
                g::send::<1, 2, g::Msg<32, ()>>(),
                g::send::<2, 0, g::Msg<33, ()>>(),
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
            other => panic!("heterogeneous split placement has no role {other}"),
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
                .send::<g::Msg<31, ()>>(&())
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
                .send::<g::Msg<32, ()>>(&())
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
                .send::<g::Msg<33, ()>>(&())
                .await
                .expect("role2 sends through example carrier");
            ctx.pending().await
        }
    }
}

impl appkit::LogicalImage<Control> for appkit::Local<image::LinuxControl> {
    type Carrier<'a> = ExampleCarrier;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);

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

impl appkit::LogicalImage<Control> for appkit::Local<image::M33Realtime> {
    type Carrier<'a> = ExampleCarrier;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);

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

impl appkit::LogicalImage<Control> for appkit::Local<image::Rp2040Io> {
    type Carrier<'a> = ExampleCarrier;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(2);

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

fn reset_live_carrier_evidence() {
    write_counter(core::ptr::addr_of_mut!(ROLE0_PHASE), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_SENT), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE0_TO_ROLE1_RECV), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_SENT), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE1_TO_ROLE2_RECV), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_SENT), 0);
    write_counter(core::ptr::addr_of_mut!(ROLE2_TO_ROLE0_RECV), 0);
    unsafe {
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_0_TO_1)).clear();
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_1_TO_2)).clear();
        (&mut *core::ptr::addr_of_mut!(EXAMPLE_FRAME_2_TO_0)).clear();
    }
}

pub fn assert_projected_role_progress() {
    reset_live_carrier_evidence();
    appkit::run::<appkit::Local<image::LinuxControl>, Control>(appkit::NoWasi);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_PHASE)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_SENT)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_RECV)), 0);
    appkit::run::<appkit::Local<image::M33Realtime>, Control>(appkit::NoWasi);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE0_TO_ROLE1_RECV)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_SENT)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_RECV)), 0);
    appkit::run::<appkit::Local<image::Rp2040Io>, Control>(appkit::NoWasi);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE1_TO_ROLE2_RECV)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE2_TO_ROLE0_SENT)), 1);
    assert_eq!(read_counter(core::ptr::addr_of!(ROLE2_TO_ROLE0_RECV)), 0);
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
        let label = 31;
        let payload = hibana::runtime::wire::Payload::new(b"abc");
        let mut receiver = rx(1, 7, 0);

        assert!(slot.pop_into(7, 0, 1, &mut receiver).is_none());
        assert!(slot.push(7, 0, 1, label, payload).is_ok());
        assert!(
            slot.push(7, 0, 1, 32, hibana::runtime::wire::Payload::new(b"def"),)
                .is_err()
        );

        let received = match slot.pop_into(7, 0, 1, &mut receiver) {
            Some(payload) => payload,
            None => panic!("ready frame must be visible to the matching receiver"),
        };
        assert_eq!(received.payload().as_bytes(), b"abc");
        assert!(slot.pop_into(7, 0, 1, &mut receiver).is_none());
    }

    #[test]
    fn edge_slots_are_lane_scoped() {
        let mut edge = ExampleEdgeSlots::empty();
        let label0 = 31;
        let label1 = 41;

        assert!(
            edge.slot_mut(0)
                .push(
                    9,
                    0,
                    1,
                    label0,
                    hibana::runtime::wire::Payload::new(b"lane0"),
                )
                .is_ok()
        );
        assert!(
            edge.slot_mut(1)
                .push(
                    9,
                    0,
                    1,
                    label1,
                    hibana::runtime::wire::Payload::new(b"lane1"),
                )
                .is_ok()
        );

        let mut receiver0 = rx(1, 9, 0);
        let mut receiver1 = rx(1, 9, 1);
        let received0 = match edge.slot_mut(0).pop_into(9, 0, 1, &mut receiver0) {
            Some(payload) => payload,
            None => panic!("lane 0 payload must be ready"),
        };
        let received1 = match edge.slot_mut(1).pop_into(9, 1, 1, &mut receiver1) {
            Some(payload) => payload,
            None => panic!("lane 1 payload must be ready"),
        };

        assert_eq!(received0.payload().as_bytes(), b"lane0");
        assert_eq!(received1.payload().as_bytes(), b"lane1");
    }
}
