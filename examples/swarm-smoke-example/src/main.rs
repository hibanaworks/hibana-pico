use hibana::{
    g,
    substrate::{program::Projectable, runtime::DefaultLabelUniverse},
};
use hibana_pico::appkit::ArtifactBundle;
use hibana_pico::{appkit, site};

struct SwarmSmoke;
struct SwarmPlacement;
struct SwarmLocal;
struct SwarmArtifacts;
const SWARM_UDP: appkit::CarrierKind = appkit::CarrierKind::new(1003);

#[cfg(feature = "wasm-engine-core")]
static SWARM_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

const EXAMPLE_CARRIER_ROLES: usize = appkit::HIBANA_TYPED_ROLE_DOMAIN_SIZE as usize;
const EXAMPLE_CARRIER_QUEUE_DEPTH: usize = 16;
const EXAMPLE_CARRIER_FRAME_BYTES: usize = 256;

#[derive(Clone, Copy, Debug)]
struct ExampleLocalFrame {
    occupied: bool,
    frame_label: hibana::substrate::transport::FrameLabel,
    len: usize,
    bytes: [u8; EXAMPLE_CARRIER_FRAME_BYTES],
}

impl ExampleLocalFrame {
    const EMPTY: Self = Self {
        occupied: false,
        frame_label: hibana::substrate::transport::FrameLabel::new(0),
        len: 0,
        bytes: [0; EXAMPLE_CARRIER_FRAME_BYTES],
    };

    fn payload(&self) -> hibana::substrate::wire::Payload<'_> {
        hibana::substrate::wire::Payload::new(&self.bytes[..self.len])
    }
}

#[derive(Clone, Copy, Debug)]
struct ExampleLocalQueue {
    frames: [ExampleLocalFrame; EXAMPLE_CARRIER_QUEUE_DEPTH],
    head: usize,
    len: usize,
}

impl ExampleLocalQueue {
    const EMPTY: Self = Self {
        frames: [ExampleLocalFrame::EMPTY; EXAMPLE_CARRIER_QUEUE_DEPTH],
        head: 0,
        len: 0,
    };

    fn push_back(
        &mut self,
        frame_label: hibana::substrate::transport::FrameLabel,
        payload: hibana::substrate::wire::Payload<'_>,
    ) -> Result<(), hibana::substrate::transport::TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > EXAMPLE_CARRIER_FRAME_BYTES || self.len == EXAMPLE_CARRIER_QUEUE_DEPTH {
            return Err(hibana::substrate::transport::TransportError::Failed);
        }
        let idx = (self.head + self.len) % EXAMPLE_CARRIER_QUEUE_DEPTH;
        self.frames[idx].occupied = true;
        self.frames[idx].frame_label = frame_label;
        self.frames[idx].len = bytes.len();
        self.frames[idx].bytes[..bytes.len()].copy_from_slice(bytes);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, frame: ExampleLocalFrame) {
        if self.len == EXAMPLE_CARRIER_QUEUE_DEPTH {
            return;
        }
        self.head = if self.head == 0 {
            EXAMPLE_CARRIER_QUEUE_DEPTH - 1
        } else {
            self.head - 1
        };
        self.frames[self.head] = frame;
        self.len += 1;
    }

    fn pop_front(&mut self) -> Option<ExampleLocalFrame> {
        if self.len == 0 {
            return None;
        }
        let idx = self.head;
        let frame = self.frames[idx];
        self.frames[idx] = ExampleLocalFrame::EMPTY;
        self.head = (self.head + 1) % EXAMPLE_CARRIER_QUEUE_DEPTH;
        self.len -= 1;
        if frame.occupied { Some(frame) } else { None }
    }
}

#[derive(Debug)]
struct ExampleLocalQueues {
    by_role: [ExampleLocalQueue; EXAMPLE_CARRIER_ROLES],
}

impl ExampleLocalQueues {
    const EMPTY: Self = Self {
        by_role: [ExampleLocalQueue::EMPTY; EXAMPLE_CARRIER_ROLES],
    };
}

struct ExampleLocalQueueCarrier {
    queues: core::cell::RefCell<ExampleLocalQueues>,
}

impl ExampleLocalQueueCarrier {
    fn new() -> Self {
        Self {
            queues: core::cell::RefCell::new(ExampleLocalQueues::EMPTY),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ExampleLocalQueueTx {
    local_role: u8,
    session_id: u32,
}

#[derive(Clone, Copy, Debug)]
struct ExampleLocalQueueRx {
    local_role: u8,
    session_id: u32,
    frame: Option<ExampleLocalFrame>,
}

impl hibana::substrate::Transport for ExampleLocalQueueCarrier {
    type Error = hibana::substrate::transport::TransportError;
    type Tx<'a>
        = ExampleLocalQueueTx
    where
        Self: 'a;
    type Rx<'a>
        = ExampleLocalQueueRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            ExampleLocalQueueTx {
                local_role,
                session_id,
            },
            ExampleLocalQueueRx {
                local_role,
                session_id,
                frame: None,
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
        let peer = outgoing.peer() as usize;
        if peer >= EXAMPLE_CARRIER_ROLES {
            return core::task::Poll::Ready(Err(
                hibana::substrate::transport::TransportError::Failed,
            ));
        }
        let result = self.queues.borrow_mut().by_role[peer]
            .push_back(outgoing.frame_label(), outgoing.payload());
        cx.waker().wake_by_ref();
        core::task::Poll::Ready(result)
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        assert_ne!(tx.session_id, 0);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<hibana::substrate::wire::Payload<'a>, Self::Error>> {
        assert_ne!(rx.session_id, 0);
        let local_role = rx.local_role as usize;
        if local_role >= EXAMPLE_CARRIER_ROLES {
            return core::task::Poll::Ready(Err(
                hibana::substrate::transport::TransportError::Failed,
            ));
        }
        let Some(frame) = self.queues.borrow_mut().by_role[local_role].pop_front() else {
            return core::task::Poll::Pending;
        };
        rx.frame = Some(frame);
        cx.waker().wake_by_ref();
        core::task::Poll::Ready(Ok(rx.frame.as_ref().expect("frame stored").payload()))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.frame.take() {
            let local_role = rx.local_role as usize;
            if local_role < EXAMPLE_CARRIER_ROLES {
                self.queues.borrow_mut().by_role[local_role].push_front(frame);
            }
        }
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
        rx.frame.map(|frame| frame.frame_label)
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        assert!(interval_us > 0 || burst_bytes == 0);
    }
}

mod image {
    pub struct Main;
}

impl appkit::Capsule for SwarmSmoke {
    type Universe = DefaultLabelUniverse;
    type Placement = SwarmPlacement;
    type Local = SwarmLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<3, ()>, 0>(),
            g::send::<g::Role<1>, g::Role<0>, g::Msg<4, ()>, 0>(),
        )
    }
}

impl appkit::Placement<SwarmSmoke> for SwarmPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            0 => appkit::RoleKind::Engine,
            1 => appkit::RoleKind::Driver,
            _ => appkit::RoleKind::Link,
        }
    }
}

impl appkit::Localside<SwarmSmoke> for SwarmLocal {
    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, SwarmSmoke, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, SwarmSmoke, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, SwarmSmoke, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, SwarmSmoke, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, SwarmSmoke, ROLE>,
    ) -> impl core::future::Future<Output = core::convert::Infallible> {
        ctx.pending()
    }
}

impl<I> appkit::ArtifactForImage<SwarmSmoke, I> for SwarmArtifacts
where
    I: appkit::LogicalImage<SwarmSmoke, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

impl appkit::LogicalImage<SwarmSmoke> for site::Local<image::Main> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ExampleLocalQueueCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(10);
    const SITE_ID: appkit::SiteId = appkit::SiteId(10);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b11);
    const CARRIER: appkit::CarrierKind = SWARM_UDP;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ExampleLocalQueueCarrier::new()
    }

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest> {
        core::hint::black_box(ROLE);
        SWARM_WASI_GUEST_ARENA.storage()
    }
}

static ARTIFACTS: SwarmArtifacts = SwarmArtifacts;

fn main() -> ! {
    let mut report = appkit::run::<site::Local<image::Main>, SwarmSmoke>(
        ARTIFACTS.for_image::<site::Local<image::Main>>(),
    );
    assert_eq!(report.artifact_len(), 0);
    assert!(report.projected_roles().contains(0));
    assert!(report.projected_roles().contains(1));
    assert_eq!(report.validated_role_count(), 2);
    assert_eq!(report.attached_endpoint_count(), 2);
    <site::Local<image::Main> as appkit::LogicalImage<SwarmSmoke>>::safe_state(report.image_mut());
    loop {
        core::hint::spin_loop();
    }
}
