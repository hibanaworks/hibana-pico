use hibana::{
    g,
    substrate::{
        Transport,
        binding::NoBinding,
        cap::{
            CapShot, ControlResourceKind, GenericCapToken, ResourceKind,
            advanced::{
                CAP_HANDLE_LEN, CapError, ControlOp, ControlPath, ControlScopeKind, LoopBreakKind,
                LoopContinueKind,
            },
        },
        ids::{Lane, SessionId},
        policy::{LoopResolution, ResolverContext, ResolverError, ResolverRef, RouteResolution},
        program::{Projectable, RoleProgram},
        runtime::{Config, CounterClock, DefaultLabelUniverse},
        transport::{
            FrameLabel, Outgoing, TransportError,
            advanced::{TransportEvent, TransportEventKind},
        },
        wire::{CodecError, Payload, WireEncode, WirePayload},
    },
};
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineReq, EngineRet, FdWrite, FdWriteDone, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET,
        LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF,
        LABEL_WASI_POLL_ONEOFF_RET, PathOpen, PathOpened,
    },
    site,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

const WASM_FD_WRITE: &[u8] = b"\0asm\x01\0\0\0\
    \x01\x04\x01\x60\x00\x00\
    \x02\x23\x01\x16wasi_snapshot_preview1\x08fd_write\x00\x00";
const WASM_FD_READ: &[u8] = b"\0asm\x01\0\0\0\
    \x01\x04\x01\x60\x00\x00\
    \x02\x22\x01\x16wasi_snapshot_preview1\x07fd_read\x00\x00";
const WASM_FD_WRITE_AND_READ: &[u8] = b"\0asm\x01\0\0\0\
    \x01\x04\x01\x60\x00\x00\
    \x02\x44\x02\x16wasi_snapshot_preview1\x08fd_write\x00\x00\
    \x16wasi_snapshot_preview1\x07fd_read\x00\x00";
const WASM_FD_WRITE_AND_PATH_OPEN: &[u8] = b"\0asm\x01\0\0\0\
    \x01\x04\x01\x60\x00\x00\
    \x02\x46\x02\x16wasi_snapshot_preview1\x08fd_write\x00\x00\
    \x16wasi_snapshot_preview1\x09path_open\x00\x00";
const TEST_LOCAL_QUEUE_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(1001);
const TEST_TCP: appkit::CarrierKind = appkit::CarrierKind::new(1002);
const TEST_UART: appkit::CarrierKind = appkit::CarrierKind::new(1004);
const TEST_CARRIER_ROLES: usize = appkit::HIBANA_TYPED_ROLE_DOMAIN_SIZE as usize;
const TEST_CARRIER_QUEUE_DEPTH: usize = 16;
const TEST_CARRIER_FRAME_BYTES: usize = 256;

#[derive(Clone, Copy, Debug)]
struct TestLocalFrame {
    occupied: bool,
    frame_label: FrameLabel,
    len: usize,
    bytes: [u8; TEST_CARRIER_FRAME_BYTES],
}

impl TestLocalFrame {
    const EMPTY: Self = Self {
        occupied: false,
        frame_label: FrameLabel::new(0),
        len: 0,
        bytes: [0; TEST_CARRIER_FRAME_BYTES],
    };

    fn payload(&self) -> Payload<'_> {
        Payload::new(&self.bytes[..self.len])
    }
}

#[derive(Clone, Copy, Debug)]
struct TestLocalQueue {
    frames: [TestLocalFrame; TEST_CARRIER_QUEUE_DEPTH],
    head: usize,
    len: usize,
}

impl TestLocalQueue {
    const EMPTY: Self = Self {
        frames: [TestLocalFrame::EMPTY; TEST_CARRIER_QUEUE_DEPTH],
        head: 0,
        len: 0,
    };

    fn push_back(
        &mut self,
        frame_label: FrameLabel,
        payload: Payload<'_>,
    ) -> Result<(), TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > TEST_CARRIER_FRAME_BYTES || self.len == TEST_CARRIER_QUEUE_DEPTH {
            return Err(TransportError::Failed);
        }
        let idx = (self.head + self.len) % TEST_CARRIER_QUEUE_DEPTH;
        self.frames[idx].occupied = true;
        self.frames[idx].frame_label = frame_label;
        self.frames[idx].len = bytes.len();
        self.frames[idx].bytes[..bytes.len()].copy_from_slice(bytes);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, frame: TestLocalFrame) {
        if self.len == TEST_CARRIER_QUEUE_DEPTH {
            return;
        }
        self.head = if self.head == 0 {
            TEST_CARRIER_QUEUE_DEPTH - 1
        } else {
            self.head - 1
        };
        self.frames[self.head] = frame;
        self.len += 1;
    }

    fn pop_front(&mut self) -> Option<TestLocalFrame> {
        if self.len == 0 {
            return None;
        }
        let idx = self.head;
        let frame = self.frames[idx];
        self.frames[idx] = TestLocalFrame::EMPTY;
        self.head = (self.head + 1) % TEST_CARRIER_QUEUE_DEPTH;
        self.len -= 1;
        if frame.occupied { Some(frame) } else { None }
    }
}

#[derive(Debug)]
struct TestLocalQueues {
    by_role: [TestLocalQueue; TEST_CARRIER_ROLES],
}

impl TestLocalQueues {
    const EMPTY: Self = Self {
        by_role: [TestLocalQueue::EMPTY; TEST_CARRIER_ROLES],
    };
}

struct TestLocalQueueCarrier {
    queues: RefCell<TestLocalQueues>,
}

impl TestLocalQueueCarrier {
    fn new() -> Self {
        Self {
            queues: RefCell::new(TestLocalQueues::EMPTY),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TestLocalQueueTx {
    local_role: u8,
    session_id: u32,
}

#[derive(Clone, Copy, Debug)]
struct TestLocalQueueRx {
    local_role: u8,
    session_id: u32,
    frame: Option<TestLocalFrame>,
}

impl hibana::substrate::Transport for TestLocalQueueCarrier {
    type Error = TransportError;
    type Tx<'a>
        = TestLocalQueueTx
    where
        Self: 'a;
    type Rx<'a>
        = TestLocalQueueRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            TestLocalQueueTx {
                local_role,
                session_id,
            },
            TestLocalQueueRx {
                local_role,
                session_id,
                frame: None,
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: Outgoing<'f>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        assert_ne!(tx.session_id, 0);
        assert_ne!(outgoing.peer(), tx.local_role);
        let peer = outgoing.peer() as usize;
        if peer >= TEST_CARRIER_ROLES {
            return Poll::Ready(Err(TransportError::Failed));
        }
        let result = self.queues.borrow_mut().by_role[peer]
            .push_back(outgoing.frame_label(), outgoing.payload());
        cx.waker().wake_by_ref();
        Poll::Ready(result)
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        assert_ne!(tx.session_id, 0);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        assert_ne!(rx.session_id, 0);
        let local_role = rx.local_role as usize;
        if local_role >= TEST_CARRIER_ROLES {
            return Poll::Ready(Err(TransportError::Failed));
        }
        let Some(frame) = self.queues.borrow_mut().by_role[local_role].pop_front() else {
            return Poll::Pending;
        };
        rx.frame = Some(frame);
        cx.waker().wake_by_ref();
        Poll::Ready(Ok(rx.frame.as_ref().expect("frame stored").payload()))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.frame.take() {
            let local_role = rx.local_role as usize;
            if local_role < TEST_CARRIER_ROLES {
                self.queues.borrow_mut().by_role[local_role].push_front(frame);
            }
        }
    }

    fn drain_events(&self, emit: &mut dyn FnMut(TransportEvent)) {
        emit(TransportEvent::new(TransportEventKind::Ack, 0, 0, 0));
    }

    fn recv_frame_hint<'a>(&'a self, rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        rx.frame.map(|frame| frame.frame_label)
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        assert!(interval_us > 0 || burst_bytes == 0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CustomPayload(u8);

impl WireEncode for CustomPayload {
    fn encoded_len(&self) -> Option<usize> {
        Some(1)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.is_empty() {
            return Err(CodecError::Truncated);
        }
        out[0] = self.0;
        Ok(1)
    }
}

impl WirePayload for CustomPayload {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        match input.as_bytes() {
            [value] => Ok(Self(*value)),
            [] => Err(CodecError::Truncated),
            _ => Err(CodecError::Invalid("custom payload length")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CustomRouteKind<const ARM: u8>;

impl<const ARM: u8> ResourceKind for CustomRouteKind<ARM> {
    type Handle = [u8; 4];

    const TAG: u8 = 0x72;
    const NAME: &'static str = "test-custom-route";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        let mut out = [0; CAP_HANDLE_LEN];
        out[..4].copy_from_slice(handle);
        out
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        let mut handle = [0; 4];
        handle.copy_from_slice(&data[..4]);
        Ok(handle)
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = [0; 4];
    }
}

impl<const ARM: u8> ControlResourceKind for CustomRouteKind<ARM> {
    const SCOPE: ControlScopeKind = ControlScopeKind::Route;
    const PATH: ControlPath = ControlPath::Local;
    const TAP_ID: u16 = 0x707;
    const SHOT: CapShot = CapShot::One;
    const OP: ControlOp = ControlOp::RouteDecision;
    const AUTO_MINT_WIRE: bool = false;

    fn mint_handle(
        session: SessionId,
        lane: Lane,
        scope: hibana::substrate::cap::advanced::ScopeId,
    ) -> Self::Handle {
        [
            ARM,
            session.raw() as u8,
            lane.raw() as u8,
            scope.raw() as u8,
        ]
    }
}

struct RichCapsule;
struct RichPlacement;
struct RichLocal;
struct IncompleteCapsule;
struct IncompletePlacement;
struct IncompleteLocal;
struct CustomLabelCapsule;
struct CustomLabelPlacement;
struct CustomLabelLocal;
struct CountingCapsule;
struct CountingPlacement;
struct CountingLocal;
struct CountingArtifacts;
struct ChoreoFsRuntimeCapsule;
struct ChoreoFsRuntimePlacement;
struct ChoreoFsRuntimeLocal;
struct RichArtifacts<'a> {
    image: appkit::WasiImage<'a>,
}

mod image {
    pub struct Composite;
    pub struct DriverOnly;
    pub struct BoundaryOnly;
    pub struct WrappedExit;
    pub struct Counting;
    pub struct ChoreoFsRuntime;
}

static COUNTING_ENGINE_POLLS: AtomicUsize = AtomicUsize::new(0);
static COUNTING_DRIVER_POLLS: AtomicUsize = AtomicUsize::new(0);
static COUNTING_BOUNDARY_POLLS: AtomicUsize = AtomicUsize::new(0);
static CHOREOFS_RUNTIME_COMPLETIONS: AtomicUsize = AtomicUsize::new(0);
const CHOREOFS_RUNTIME_OBJECT: appkit::ObjectSpec = appkit::ObjectSpec::new(
    b"device/led/green",
    appkit::ObjectId(7),
    appkit::FdSpec::new(4, 0x2, 11),
);
static CHOREOFS_RUNTIME_FACTS: appkit::ObjectSpecSet<1> =
    appkit::ObjectSpecSet::new([CHOREOFS_RUNTIME_OBJECT]);

struct WrappedRunExit<R, I> {
    report: appkit::RunReport<R, I>,
}

impl<R, I> appkit::FromRunReport<R, I> for WrappedRunExit<R, I> {
    fn from_run_report(report: appkit::RunReport<R, I>) -> Self {
        Self { report }
    }
}

impl appkit::Capsule for RichCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = RichPlacement;
    type Local = RichLocal;
    type Report = usize;

    fn choreography() -> impl Projectable<Self::Universe> {
        let direct = g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
        );
        let left = g::seq(
            g::send::<
                g::Role<1>,
                g::Role<1>,
                g::Msg<201, GenericCapToken<CustomRouteKind<0>>, CustomRouteKind<0>>,
                1,
            >()
            .policy::<7>(),
            g::send::<g::Role<1>, g::Role<2>, g::Msg<202, CustomPayload>, 1>(),
        );
        let right = g::seq(
            g::send::<
                g::Role<1>,
                g::Role<1>,
                g::Msg<203, GenericCapToken<CustomRouteKind<1>>, CustomRouteKind<1>>,
                1,
            >()
            .policy::<7>(),
            g::send::<g::Role<1>, g::Role<3>, g::Msg<204, CustomPayload>, 1>(),
        );
        g::par(direct, g::route(left, right))
    }
}

impl appkit::Placement<RichCapsule> for RichPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            2 | 3 => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

impl appkit::Localside<RichCapsule> for RichLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        assert_eq!(ctx.guest_artifact().bytes(), Some(WASM_FD_WRITE));
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl appkit::Capsule for IncompleteCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = IncompletePlacement;
    type Local = IncompleteLocal;
    type Report = usize;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>()
    }
}

impl appkit::Placement<IncompleteCapsule> for IncompletePlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            _ => appkit::RoleKind::Boundary,
        }
    }
}

impl appkit::Localside<IncompleteCapsule> for IncompleteLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl appkit::Capsule for CustomLabelCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = CustomLabelPlacement;
    type Local = CustomLabelLocal;
    type Report = ();

    fn choreography() -> impl Projectable<Self::Universe> {
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, CustomPayload>, 0>()
    }
}

impl appkit::Placement<CustomLabelCapsule> for CustomLabelPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            _ => appkit::RoleKind::Boundary,
        }
    }
}

impl appkit::Localside<CustomLabelCapsule> for CustomLabelLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl appkit::Capsule for CountingCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = CountingPlacement;
    type Local = CountingLocal;
    type Report = ();

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<11, ()>, 0>(),
            g::send::<g::Role<1>, g::Role<2>, g::Msg<12, ()>, 0>(),
        )
    }
}

impl appkit::Placement<CountingCapsule> for CountingPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            2 => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

impl appkit::Localside<CountingCapsule> for CountingLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        assert_eq!(ctx.guest_artifact(), appkit::GuestArtifact::NONE);
        COUNTING_ENGINE_POLLS.fetch_add(1, Ordering::SeqCst);
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        COUNTING_DRIVER_POLLS.fetch_add(1, Ordering::SeqCst);
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        COUNTING_BOUNDARY_POLLS.fetch_add(1, Ordering::SeqCst);
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl appkit::Capsule for ChoreoFsRuntimeCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = ChoreoFsRuntimePlacement;
    type Local = ChoreoFsRuntimeLocal;
    type Report = ();

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(
                    ),
                ),
            ),
        )
    }
}

impl appkit::Placement<ChoreoFsRuntimeCapsule> for ChoreoFsRuntimePlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            _ => appkit::RoleKind::Boundary,
        }
    }
}

impl appkit::Localside<ChoreoFsRuntimeCapsule> for ChoreoFsRuntimeLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            assert_eq!(ROLE, 0);
            assert_eq!(
                ctx.guest_artifact().bytes(),
                Some(WASM_FD_WRITE_AND_PATH_OPEN)
            );
            let mut ctx = ctx;
            let open = EngineReq::PathOpen(
                PathOpen::new(3, 0, 0x2, b"device/led/green").expect("path_open request"),
            );
            ctx.endpoint()
                .flow::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
                .expect("engine path_open flow")
                .send(&open)
                .await
                .expect("send path_open through endpoint");
            let opened = ctx
                .endpoint()
                .recv::<g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
                .await
                .expect("receive path_open reply through endpoint");
            assert_eq!(opened, EngineRet::PathOpened(PathOpened::new(4, 0)));

            let write = EngineReq::FdWrite(FdWrite::new(4, b"green=on").expect("fd_write"));
            ctx.endpoint()
                .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .expect("engine fd_write flow")
                .send(&write)
                .await
                .expect("send fd_write through endpoint");
            let written = ctx
                .endpoint()
                .recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                .await
                .expect("receive fd_write reply through endpoint");
            assert_eq!(written, EngineRet::FdWriteDone(FdWriteDone::new(4, 8)));
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        async move {
            assert_eq!(ROLE, 1);
            let mut ctx = ctx;
            let open_request = ctx
                .endpoint()
                .recv::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
                .await
                .expect("driver receives path_open through endpoint");
            let EngineReq::PathOpen(path_open) = open_request else {
                panic!("expected path_open request");
            };
            assert_eq!(path_open.preopen_fd(), 3);
            assert_eq!(path_open.rights_base(), 0x2);
            let object = ctx
                .choreofs()
                .resolve(path_open.path())
                .expect("ChoreoFS resolves configured path");
            let fd_fact = ctx
                .ledger()
                .fds()
                .iter()
                .copied()
                .find(|fact| fact.object() == object)
                .expect("ledger materializes object fd");
            assert_eq!(fd_fact.fd(), 4);
            assert_eq!(fd_fact.rights(), path_open.rights_base());
            ctx.endpoint()
                .flow::<g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
                .expect("driver path_open reply flow")
                .send(&EngineRet::PathOpened(PathOpened::new(
                    fd_fact.fd() as u8,
                    0,
                )))
                .await
                .expect("send path_open reply through endpoint");

            let write_request = ctx
                .endpoint()
                .recv::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .await
                .expect("driver receives fd_write through endpoint");
            let EngineReq::FdWrite(write) = write_request else {
                panic!("expected fd_write request");
            };
            let write_fd = ctx
                .ledger()
                .fd(write.fd() as u32)
                .expect("fd_write uses materialized ledger fd");
            assert_eq!(write_fd.object(), object);
            assert_eq!(write_fd.generation(), 11);
            assert_eq!(write.as_bytes(), b"green=on");
            ctx.endpoint()
                .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                .expect("driver fd_write reply flow")
                .send(&EngineRet::FdWriteDone(FdWriteDone::new(
                    write.fd(),
                    write.len() as u8,
                )))
                .await
                .expect("send fd_write reply through endpoint");
            CHOREOFS_RUNTIME_COMPLETIONS.fetch_add(1, Ordering::SeqCst);
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl<'a, I> appkit::ArtifactForImage<RichCapsule, I> for RichArtifacts<'a>
where
    I: appkit::LogicalImage<RichCapsule, Artifact = appkit::WasiImage<'a>>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        self.image
    }
}

impl<I> appkit::ArtifactForImage<CountingCapsule, I> for CountingArtifacts
where
    I: appkit::LogicalImage<CountingCapsule, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl<'a, I> appkit::ArtifactForImage<IncompleteCapsule, I> for RichArtifacts<'a>
where
    I: appkit::LogicalImage<IncompleteCapsule, Artifact = appkit::WasiImage<'a>>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        self.image
    }
}

const MEMORY_ROLE_COUNT: usize = 4;

struct MemoryTransport {
    queues: [RefCell<VecDeque<Vec<u8>>>; MEMORY_ROLE_COUNT],
}

struct MemoryTx {
    role: u8,
}

struct MemoryRx {
    role: u8,
    current: Option<Vec<u8>>,
    delivered: bool,
}

impl MemoryTransport {
    fn new() -> Self {
        Self {
            queues: std::array::from_fn(|role| {
                assert!(role < MEMORY_ROLE_COUNT);
                RefCell::new(VecDeque::new())
            }),
        }
    }
}

impl Transport for MemoryTransport {
    type Error = TransportError;
    type Tx<'a>
        = MemoryTx
    where
        Self: 'a;
    type Rx<'a>
        = MemoryRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        assert!((local_role as usize) < MEMORY_ROLE_COUNT);
        core::hint::black_box(session_id);
        (
            MemoryTx { role: local_role },
            MemoryRx {
                role: local_role,
                current: None,
                delivered: false,
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: Outgoing<'f>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        core::hint::black_box(tx.role);
        core::hint::black_box(cx);
        let peer = outgoing.peer() as usize;
        assert!(peer < MEMORY_ROLE_COUNT);
        self.queues[peer]
            .borrow_mut()
            .push_back(outgoing.payload().as_bytes().to_vec());
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box(self);
        core::hint::black_box(tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        core::hint::black_box(cx);
        if rx.delivered {
            rx.current = None;
            rx.delivered = false;
        }
        if rx.current.is_none() {
            rx.current = self.queues[rx.role as usize].borrow_mut().pop_front();
        }
        match rx.current.as_ref() {
            Some(bytes) => {
                rx.delivered = true;
                Poll::Ready(Ok(Payload::new(bytes.as_slice())))
            }
            None => Poll::Pending,
        }
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.current.take() {
            self.queues[rx.role as usize].borrow_mut().push_front(frame);
        }
        rx.delivered = false;
    }

    fn drain_events(&self, emit: &mut dyn FnMut(TransportEvent)) {
        core::hint::black_box(self);
        core::hint::black_box(emit);
    }

    fn recv_frame_hint<'a>(&'a self, rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        core::hint::black_box(self);
        core::hint::black_box(rx);
        None
    }

    fn metrics(&self) -> Self::Metrics {
        core::hint::black_box(self);
    }

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box(self);
        core::hint::black_box(interval_us);
        core::hint::black_box(burst_bytes);
    }
}

fn noop_waker() -> Waker {
    unsafe fn clone(data: *const ()) -> RawWaker {
        core::hint::black_box(data);
        RawWaker::new(core::ptr::null(), &VTABLE)
    }
    unsafe fn wake(data: *const ()) {
        core::hint::black_box(data);
    }
    unsafe fn wake_by_ref(data: *const ()) {
        core::hint::black_box(data);
    }
    unsafe fn drop(data: *const ()) {
        core::hint::black_box(data);
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

fn block_on<F: core::future::Future>(future: F) -> F::Output {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut future = core::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => core::hint::spin_loop(),
        }
    }
}

fn choreofs_traffic_program() -> impl Projectable<DefaultLabelUniverse> {
    let path_open = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 0>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 0>(),
    );
    let green = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 0>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 0>(
                ),
            ),
        ),
    );
    let orange = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 0>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 0>(
                ),
            ),
        ),
    );
    let red = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 0>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 0>(
                ),
            ),
        ),
    );
    g::seq(path_open, g::seq(green, g::seq(orange, red)))
}

fn single_exchange_program() -> impl Projectable<DefaultLabelUniverse> {
    g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
    )
}

const TEST_LOOP_CONTINUE_LOGICAL: u8 = 0xA1;
const TEST_LOOP_BREAK_LOGICAL: u8 = 0xA2;

fn choreofs_traffic_loop_program() -> impl Projectable<DefaultLabelUniverse> {
    type Continue =
        g::Msg<{ TEST_LOOP_CONTINUE_LOGICAL }, GenericCapToken<LoopContinueKind>, LoopContinueKind>;
    type Break = g::Msg<{ TEST_LOOP_BREAK_LOGICAL }, GenericCapToken<LoopBreakKind>, LoopBreakKind>;

    let cycle = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 0>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 0>(
                ),
            ),
        ),
    );
    g::seq(
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 0>(),
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 0>(),
        ),
        g::route(
            g::seq(g::send::<g::Role<1>, g::Role<1>, Continue, 0>(), cycle),
            g::send::<g::Role<1>, g::Role<1>, Break, 0>(),
        ),
    )
}

fn choreofs_traffic_attach_succeeds<const ROLE: u8>(
    program: &impl Projectable<DefaultLabelUniverse>,
    slab_bytes: usize,
) -> bool {
    let role = program.project::<ROLE>();
    let mut tap_buf = [hibana::substrate::tap::TapEvent::zero(); 128];
    let mut slab = vec![0u8; slab_bytes];
    let clock = CounterClock::new();
    let transport = MemoryTransport::new();
    let kit =
        hibana::substrate::SessionKit::<MemoryTransport, DefaultLabelUniverse, CounterClock, 1>::new(
            &clock,
        );
    let Ok(rendezvous) = kit.add_rendezvous_from_config(
        Config::new(
            &mut tap_buf,
            slab.as_mut_slice(),
            0..8,
            1,
            CounterClock::new(),
        ),
        transport,
    ) else {
        return false;
    };
    kit.enter::<ROLE, _>(rendezvous, SessionId::new(2040), &role, NoBinding)
        .is_ok()
}

fn minimum_choreofs_traffic_attach_slab<const ROLE: u8>(
    program: &impl Projectable<DefaultLabelUniverse>,
) -> usize {
    let mut slab = 4 * 1024;
    while slab <= 128 * 1024 {
        if choreofs_traffic_attach_succeeds::<ROLE>(program, slab) {
            return slab;
        }
        slab += 1024;
    }
    panic!("role {ROLE} did not attach within 128 KiB");
}

#[test]
fn choreofs_traffic_role_slices_attach_with_bounded_storage() {
    let program = choreofs_traffic_program();
    let role0 = minimum_choreofs_traffic_attach_slab::<0>(&program);
    let role1 = minimum_choreofs_traffic_attach_slab::<1>(&program);
    let loop_program = choreofs_traffic_loop_program();
    let loop_role0 = minimum_choreofs_traffic_attach_slab::<0>(&loop_program);
    let loop_role1 = minimum_choreofs_traffic_attach_slab::<1>(&loop_program);
    let single_program = single_exchange_program();
    let single_role0 = minimum_choreofs_traffic_attach_slab::<0>(&single_program);
    let single_role1 = minimum_choreofs_traffic_attach_slab::<1>(&single_program);

    println!("single exchange role0 attach slab bytes: {single_role0}");
    println!("single exchange role1 attach slab bytes: {single_role1}");
    println!("choreofs traffic role0 attach slab bytes: {role0}");
    println!("choreofs traffic role1 attach slab bytes: {role1}");
    println!("choreofs loop traffic role0 attach slab bytes: {loop_role0}");
    println!("choreofs loop traffic role1 attach slab bytes: {loop_role1}");
    assert!(single_role0 <= role0);
    assert!(single_role1 <= role1);
}

fn exercise_fd_write_endpoint_round_trip(program: &impl Projectable<DefaultLabelUniverse>) {
    let role0: RoleProgram<0> = Projectable::<DefaultLabelUniverse>::project::<0>(program);
    let role1: RoleProgram<1> = Projectable::<DefaultLabelUniverse>::project::<1>(program);
    let mut tap_buf = [hibana::substrate::tap::TapEvent::zero(); 128];
    let mut slab = [0u8; 262_144];
    let clock = CounterClock::new();
    let kit = hibana::substrate::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        2,
    >::new(&clock);
    let rv = kit
        .add_rendezvous_from_config(
            Config::new(&mut tap_buf, &mut slab, 0..8, 2, CounterClock::new()),
            MemoryTransport::new(),
        )
        .expect("register in-process rendezvous");
    let sid = SessionId::new(0x5150);
    let mut engine = kit
        .enter::<0, _>(rv, sid, &role0, NoBinding)
        .expect("enter engine role");
    let mut driver = kit
        .enter::<1, _>(rv, sid, &role1, NoBinding)
        .expect("enter driver role");
    let request = EngineReq::FdWrite(FdWrite::new(1, b"hello").expect("fd write request"));
    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine fd_write flow")
            .send(&request),
    )
    .expect("send fd_write request through endpoint");
    let observed_request = block_on(driver.recv::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .expect("driver receives fd_write request through endpoint");
    assert_eq!(observed_request, request);

    let reply = EngineRet::FdWriteDone(FdWriteDone::new(1, 5));
    block_on(
        driver
            .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("driver fd_write reply flow")
            .send(&reply),
    )
    .expect("send fd_write reply through endpoint");
    let observed_reply = block_on(engine.recv::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
        .expect("engine receives fd_write reply through endpoint");
    assert_eq!(observed_reply, reply);
}

#[cfg(feature = "wasm-engine-core")]
static HOST_CAPSULE_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(feature = "wasm-engine-core")]
fn host_capsule_wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
    core::hint::black_box(ROLE);
    HOST_CAPSULE_WASI_GUEST_ARENA.storage()
}

impl appkit::LogicalImage<RichCapsule> for site::Local<image::Composite> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(44);
    const SITE_ID: appkit::SiteId = appkit::SiteId(1);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b1111);
    const CARRIER: appkit::CarrierKind = TEST_LOCAL_QUEUE_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<RichCapsule> for site::Local<image::DriverOnly> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(45);
    const SITE_ID: appkit::SiteId = appkit::SiteId(2);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = TEST_TCP;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<RichCapsule> for site::Local<image::BoundaryOnly> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(46);
    const SITE_ID: appkit::SiteId = appkit::SiteId(3);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(2);
    const CARRIER: appkit::CarrierKind = TEST_UART;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<RichCapsule> for site::Local<image::WrappedExit> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = WrappedRunExit<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(47);
    const SITE_ID: appkit::SiteId = appkit::SiteId(1);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b1111);
    const CARRIER: appkit::CarrierKind = TEST_LOCAL_QUEUE_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<IncompleteCapsule> for site::Local<image::Composite> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(48);
    const SITE_ID: appkit::SiteId = appkit::SiteId(1);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b11);
    const CARRIER: appkit::CarrierKind = TEST_LOCAL_QUEUE_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<CountingCapsule> for site::Local<image::Counting> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(49);
    const SITE_ID: appkit::SiteId = appkit::SiteId(1);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b111);
    const CARRIER: appkit::CarrierKind = TEST_LOCAL_QUEUE_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }
}

impl appkit::LogicalImage<ChoreoFsRuntimeCapsule> for site::Local<image::ChoreoFsRuntime> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = TestLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(50);
    const SITE_ID: appkit::SiteId = appkit::SiteId(1);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b11);
    const CARRIER: appkit::CarrierKind = TEST_LOCAL_QUEUE_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        TestLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        host_capsule_wasi_guest_storage::<ROLE>()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        CHOREOFS_RUNTIME_FACTS.driver_facts()
    }
}

#[test]
fn capsule_uses_projectable_raw_hibana_and_metadata() {
    let caps = appkit::derive_projection_caps::<RichCapsule>();

    assert!(caps.roles.contains(0));
    assert!(caps.roles.contains(1));
    assert!(caps.roles.contains(2));
    assert!(caps.roles.contains(3));
    assert!(caps.lanes.contains(0));
    assert!(caps.lanes.contains(1));
    assert!(caps.eff_count >= 6);
    assert!(caps.route_scope_count >= 1);
    assert!(caps.has_parallel);
    assert!(caps.has_policy);
    assert!(caps.has_control);
    assert_eq!(caps.policy_count, 1);
    assert_eq!(caps.policies[0], 7);
    assert_eq!(caps.control_count, 1);
    assert_eq!(caps.control_ops[0], ControlOp::RouteDecision.as_u8());
    assert_eq!(caps.control_tap_ids[0], 0x707);
    assert!(caps.wasi_imports.contains(appkit::WasiImports::FD_WRITE));
    assert_eq!(caps.wasi_completion_pair_count, 1);
    assert!(appkit::validate_requested_roles::<
        RichCapsule,
        site::Local<image::Composite>,
    >());

    let mut visitor = CaptureProgramFacts {
        seen_program: false,
    };
    let program = <RichCapsule as appkit::Capsule>::choreography();
    Projectable::<DefaultLabelUniverse>::visit_projection_metadata(&program, &mut visitor);
    let role0: RoleProgram<0> = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
    core::hint::black_box(role0);
    assert!(visitor.seen_program);
    assert!(caps.label_count >= 4);
}

#[test]
fn wasi_capacity_requires_typed_engine_request_metadata() {
    let caps = appkit::derive_projection_caps::<CustomLabelCapsule>();

    assert!(caps.labels[..caps.label_count as usize].contains(&LABEL_WASI_FD_WRITE));
    assert!(!caps.wasi_imports.contains(appkit::WasiImports::FD_WRITE));
    assert_eq!(caps.wasi_completion_pair_count, 0);
}

#[test]
fn raw_hibana_request_and_reply_cross_endpoint_carrier() {
    let program = <RichCapsule as appkit::Capsule>::choreography();
    exercise_fd_write_endpoint_round_trip(&program);
}

#[test]
fn role_set_distinguishes_storage_width_from_hibana_typed_role_domain() {
    let low = appkit::RoleSet::single(3);
    let high = appkit::RoleSet::single(15);
    let combined = low.union(high);

    assert!(combined.contains(3));
    assert!(combined.contains(15));
    assert_eq!(combined.count(), 2);
    assert!(high.is_subset_of(combined));
    assert_eq!(combined.words()[0], (1u64 << 3) | (1u64 << 15));
    assert_eq!(appkit::HIBANA_TYPED_ROLE_DOMAIN_SIZE, 16);
    assert!(appkit::HIBANA_TYPED_ROLE_DOMAIN.contains(0));
    assert!(appkit::HIBANA_TYPED_ROLE_DOMAIN.contains(15));
    assert!(!appkit::HIBANA_TYPED_ROLE_DOMAIN.contains(16));
    assert!(combined.is_subset_of(appkit::HIBANA_TYPED_ROLE_DOMAIN));
}

#[test]
fn run_polls_localside_for_attached_role_kinds() {
    let before_engine = COUNTING_ENGINE_POLLS.load(Ordering::SeqCst);
    let before_driver = COUNTING_DRIVER_POLLS.load(Ordering::SeqCst);
    let before_boundary = COUNTING_BOUNDARY_POLLS.load(Ordering::SeqCst);
    let artifacts = CountingArtifacts;
    let image_artifact = <CountingArtifacts as appkit::ArtifactBundle<CountingCapsule>>::for_image::<
        site::Local<image::Counting>,
    >(&artifacts);

    let report = appkit::run::<site::Local<image::Counting>, CountingCapsule>(image_artifact);

    assert_eq!(report.attached_endpoint_count(), 3);
    assert_eq!(
        report.attached_role_kinds(),
        appkit::RoleKindCounts {
            engine: 1,
            driver: 1,
            boundary: 1,
            link: 0,
            supervisor: 0,
        }
    );
    assert_eq!(
        COUNTING_ENGINE_POLLS.load(Ordering::SeqCst),
        before_engine + 1
    );
    assert_eq!(
        COUNTING_DRIVER_POLLS.load(Ordering::SeqCst),
        before_driver + 1
    );
    assert_eq!(
        COUNTING_BOUNDARY_POLLS.load(Ordering::SeqCst),
        before_boundary + 1
    );
}

#[test]
fn choreofs_facts_are_consumed_by_driver_ctx_during_endpoint_progress() {
    let before = CHOREOFS_RUNTIME_COMPLETIONS.load(Ordering::SeqCst);
    let report = appkit::run::<site::Local<image::ChoreoFsRuntime>, ChoreoFsRuntimeCapsule>(
        appkit::WasiImage::from_static(WASM_FD_WRITE_AND_PATH_OPEN),
    );

    assert_eq!(report.artifact_len(), WASM_FD_WRITE_AND_PATH_OPEN.len());
    assert_eq!(report.attached_endpoint_count(), 2);
    assert_eq!(
        report.endpoint_carrier().wasi_imports(),
        appkit::WasiImports::FD_WRITE.union(appkit::WasiImports::PATH_OPEN)
    );
    assert_eq!(report.endpoint_carrier().wasi_completion_pair_count(), 2);
    assert_eq!(
        CHOREOFS_RUNTIME_COMPLETIONS.load(Ordering::SeqCst),
        before + 1
    );
}

#[test]
fn run_takes_artifact_as_dynamic_input() {
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(WASM_FD_WRITE),
    };
    let image_artifact = <RichArtifacts<'static> as appkit::ArtifactBundle<RichCapsule>>::for_image::<
        site::Local<image::Composite>,
    >(&artifacts);

    let report = appkit::run::<site::Local<image::Composite>, RichCapsule>(image_artifact);

    assert!(report.projected_roles().contains(0));
    assert!(
        report
            .wasi_imports()
            .contains(appkit::WasiImports::FD_WRITE)
    );
    assert_eq!(report.validated_role_count(), 4);
    assert_eq!(report.attached_endpoint_count(), 4);
    assert_eq!(
        report.attached_role_kinds(),
        appkit::RoleKindCounts {
            engine: 1,
            driver: 1,
            boundary: 2,
            link: 0,
            supervisor: 0,
        }
    );
    assert_eq!(report.artifact_len(), WASM_FD_WRITE.len());
    let manifest = report.manifest();
    assert_eq!(manifest.logical_image_id, appkit::ImageId(44));
    assert_eq!(manifest.peer_image_count, 0);
    assert_eq!(
        manifest.requested_role_set,
        appkit::RoleSet::from_bits(0b1111)
    );
    assert_ne!(manifest.capsule_fingerprint, [0; 2]);
    assert_ne!(manifest.placement_fingerprint, [0; 2]);
    assert_ne!(manifest.label_universe_fingerprint, [0; 2]);
    assert_ne!(manifest.choreography_session_id, 0);
    assert_eq!(
        report.endpoint_carrier().session_id(),
        manifest.choreography_session_id
    );
    assert_ne!(manifest.capsule_fingerprint, manifest.placement_fingerprint);
    assert!(manifest.lane_set.contains(0));
    assert!(manifest.lane_set.contains(1));
    assert!(manifest.choreography_fingerprint != [0; 2]);
    assert!(
        manifest
            .wasi_imports
            .contains(appkit::WasiImports::FD_WRITE)
    );
    assert_eq!(manifest.policy_count, 1);
    assert_eq!(manifest.policies[0], 7);
    assert_eq!(manifest.control_count, 1);
    assert_eq!(manifest.control_ops[0], ControlOp::RouteDecision.as_u8());
    assert_eq!(manifest.control_tap_ids[0], 0x707);
    assert_eq!(manifest.wasi_completion_pair_count, 1);
    assert_eq!(report.wasi_completion_pair_count(), 1);
}

#[test]
fn image_manifest_peer_attach_requires_mutual_identity_and_matching_shape() {
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(WASM_FD_WRITE),
    };
    let report = appkit::run::<site::Local<image::Composite>, RichCapsule>(
        <RichArtifacts<'static> as appkit::ArtifactBundle<RichCapsule>>::for_image::<
            site::Local<image::Composite>,
        >(&artifacts),
    );
    let mut peer = report.manifest();
    peer.logical_image_id = appkit::ImageId(45);
    peer.peer_image_ids = [appkit::ImageId(44); 8];
    peer.peer_image_count = 1;
    assert!(!report.manifest().can_attach_peer(&peer));

    let mut this = report.manifest();
    this.peer_image_ids = [appkit::ImageId(45); 8];
    this.peer_image_count = 1;
    assert!(this.can_attach_peer(&peer));

    peer.carrier = TEST_TCP;
    assert!(!this.can_attach_peer(&peer));
    peer.carrier = TEST_LOCAL_QUEUE_CARRIER;
    peer.choreography_session_id = peer.choreography_session_id.wrapping_add(1);
    assert!(!this.can_attach_peer(&peer));
}

#[test]
fn run_returns_logical_image_exit_type() {
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(WASM_FD_WRITE),
    };
    let image_artifact = <RichArtifacts<'static> as appkit::ArtifactBundle<RichCapsule>>::for_image::<
        site::Local<image::WrappedExit>,
    >(&artifacts);

    let wrapped = appkit::run::<site::Local<image::WrappedExit>, RichCapsule>(image_artifact);

    assert_eq!(wrapped.report.image_id(), appkit::ImageId(47));
    assert_eq!(wrapped.report.attached_endpoint_count(), 4);
    assert_eq!(wrapped.report.artifact_len(), WASM_FD_WRITE.len());
}

#[test]
#[should_panic(
    expected = "WASI P1 import request label must have a projected typed EngineRet completion"
)]
fn run_rejects_wasi_request_without_projected_completion() {
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(WASM_FD_WRITE),
    };
    let image_artifact =
        <RichArtifacts<'static> as appkit::ArtifactBundle<IncompleteCapsule>>::for_image::<
            site::Local<image::Composite>,
        >(&artifacts);

    let report = appkit::run::<site::Local<image::Composite>, IncompleteCapsule>(image_artifact);
    core::hint::black_box(report);
}

#[test]
fn logical_image_wasi_requirements_follow_requested_role_slice() {
    let driver = appkit::run::<site::Local<image::DriverOnly>, RichCapsule>(appkit::NoWasi);
    assert_eq!(driver.image_id(), appkit::ImageId(45));
    assert_eq!(driver.site_id(), appkit::SiteId(2));
    assert_eq!(driver.wasi_imports(), appkit::WasiImports::EMPTY);
    assert_eq!(driver.artifact_len(), 0);
    assert_eq!(driver.attached_endpoint_count(), 1);
    assert!(driver.projected_roles().contains(0));
    assert!(driver.projected_roles().contains(1));

    let boundary = appkit::run::<site::Local<image::BoundaryOnly>, RichCapsule>(appkit::NoWasi);
    assert_eq!(boundary.image_id(), appkit::ImageId(46));
    assert_eq!(boundary.site_id(), appkit::SiteId(3));
    assert_eq!(boundary.wasi_imports(), appkit::WasiImports::EMPTY);
    assert_eq!(boundary.artifact_len(), 0);
    assert_eq!(boundary.attached_endpoint_count(), 1);
}

#[test]
fn hibana_substrate_surfaces_remain_available_to_capsules() {
    fn route_resolution(ctx: ResolverContext) -> Result<RouteResolution, ResolverError> {
        let retry_hint = ctx.input(0) as u8;
        if ctx
            .attr(hibana::substrate::policy::signals::core::LANE)
            .is_some()
        {
            Ok(RouteResolution::Arm(0))
        } else {
            Ok(RouteResolution::Defer { retry_hint })
        }
    }

    fn loop_resolution(ctx: ResolverContext) -> Result<LoopResolution, ResolverError> {
        if ctx.input(1) == 0 {
            Ok(LoopResolution::Continue)
        } else {
            Ok(LoopResolution::Defer {
                retry_hint: ctx.input(1) as u8,
            })
        }
    }

    let route_resolver = ResolverRef::route_fn(route_resolution);
    let loop_resolver = ResolverRef::loop_fn(loop_resolution);
    assert!(core::mem::size_of_val(&route_resolver) > 0);
    assert!(core::mem::size_of_val(&loop_resolver) > 0);

    let binding = NoBinding;
    assert_eq!(core::mem::size_of_val(&binding), 0);

    let transport_event = TransportEvent::new(TransportEventKind::Ack, 7, 64, 0);
    assert_eq!(transport_event.kind(), TransportEventKind::Ack);
    let (packet_number, encoded) = transport_event.encode_tap_args();
    assert_eq!(packet_number, 7);
    assert_ne!(encoded, 0);
}

#[test]
fn driver_facts_are_separate_from_progress_authority() {
    const LED_DEVICE: appkit::ObjectSpec = appkit::ObjectSpec::new(
        b"device/led/green",
        appkit::ObjectId(7),
        appkit::FdSpec::new(3, 0x2, 11),
    );
    static FACTS: appkit::ObjectSpecSet<1> = appkit::ObjectSpecSet::new([LED_DEVICE]);

    let facts = FACTS.driver_facts();
    let report = appkit::run::<site::Local<image::Composite>, RichCapsule>(
        appkit::WasiImage::from_static(WASM_FD_WRITE),
    );

    assert_eq!(report.artifact_len(), WASM_FD_WRITE.len());
    assert_eq!(report.endpoint_carrier().wasi_completion_pair_count(), 1);
    assert_eq!(
        report.endpoint_carrier().carrier(),
        TEST_LOCAL_QUEUE_CARRIER
    );
    assert_eq!(
        facts.choreofs().resolve(b"device/led/green"),
        Some(appkit::ObjectId(7))
    );
    assert_eq!(facts.choreofs().resolve(b"host/fs"), None);

    let fd = facts.ledger().fd(3).expect("fd fact");
    assert_eq!(fd.object(), appkit::ObjectId(7));
    assert_eq!(fd.rights(), 0x2);
    assert_eq!(fd.generation(), 11);
}

#[test]
fn wasi_image_rejects_non_p1_artifacts() {
    use hibana_pico::appkit::ArtifactEvidence;

    let p1 = appkit::WasiImage::from_static(WASM_FD_WRITE);
    assert_eq!(p1.validate(appkit::WasiImports::FD_WRITE), Ok(()));

    let empty = appkit::WasiImage::from_static(b"");
    assert_eq!(
        empty.validate(appkit::WasiImports::EMPTY),
        Err(appkit::ArtifactError::Empty)
    );

    let preview2 = appkit::WasiImage::from_static(
        b"\0asm\x01\0\0\0wasi_snapshot_preview1 wasi_snapshot_preview2",
    );
    assert_eq!(
        preview2.validate(appkit::WasiImports::FD_WRITE),
        Err(appkit::ArtifactError::ForbiddenPreview2Surface)
    );

    let missing = appkit::WasiImage::from_static(WASM_FD_READ);
    assert_eq!(
        missing.validate(appkit::WasiImports::FD_WRITE),
        Err(appkit::ArtifactError::MissingRequiredWasiImport)
    );

    let extra = appkit::WasiImage::from_static(WASM_FD_WRITE_AND_READ);
    assert_eq!(
        extra.validate(appkit::WasiImports::FD_WRITE),
        Err(appkit::ArtifactError::UnsupportedWasiImport)
    );

    assert_eq!(
        appkit::NoWasi.validate(appkit::WasiImports::FD_WRITE),
        Err(appkit::ArtifactError::MissingRequiredWasiImport)
    );
}

struct CaptureProgramFacts {
    seen_program: bool,
}

impl hibana::substrate::program::ProjectionMetadataVisitor for CaptureProgramFacts {
    fn visit_program(&mut self, facts: hibana::substrate::program::ProjectionProgramFacts) {
        self.seen_program = true;
        assert!(facts.eff_count >= 4);
        assert!(facts.parallel_enter_count >= 1);
        assert!(facts.route_scope_count >= 1);
    }

    fn visit_atom(&mut self, spec: hibana::substrate::program::ProjectionAtomSpec) {
        if spec.is_control {
            assert_eq!(spec.control_op, Some(ControlOp::RouteDecision.as_u8()));
            assert_eq!(spec.control_tap_id, Some(0x707));
            assert!(spec.control_scope.is_some());
            assert!(spec.control_path.is_some());
            assert!(spec.control_shot.is_some());
        }
    }
}
