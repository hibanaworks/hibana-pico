use core::task::{Context, Poll, Waker};

use hibana::substrate::{
    Transport,
    transport::{FrameLabel, Outgoing, advanced::TransportEvent},
    wire::Payload,
};

use crate::port::host_queue::{BackendError, FifoBackend, FrameOwned};

impl<T> FifoBackend for &T
where
    T: FifoBackend + ?Sized,
{
    fn enqueue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        (*self).enqueue(role, frame)
    }

    fn dequeue(&self, role: u8) -> Result<Option<FrameOwned>, BackendError> {
        (*self).dequeue(role)
    }

    fn requeue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        (*self).requeue(role, frame)
    }

    fn peek_label(&self, role: u8) -> Result<Option<u8>, BackendError> {
        (*self).peek_label(role)
    }

    fn store_recv_waker(&self, role: u8, waker: &Waker) -> Result<(), BackendError> {
        (*self).store_recv_waker(role, waker)
    }
}

pub struct SioTransport<B> {
    backend: B,
}

impl<B> SioTransport<B> {
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }
}

pub struct PicoTx;

pub struct PicoRx<'a, B> {
    backend: &'a B,
    role: u8,
    current: Option<FrameOwned>,
}

impl<B> Transport for SioTransport<B>
where
    B: FifoBackend,
{
    type Error = BackendError;
    type Tx<'a>
        = PicoTx
    where
        Self: 'a;
    type Rx<'a>
        = PicoRx<'a, B>
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, _session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            PicoTx,
            PicoRx {
                backend: &self.backend,
                role: local_role,
                current: None,
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        _tx: &'a mut Self::Tx<'a>,
        outgoing: Outgoing<'f>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        let frame = match FrameOwned::from_bytes(
            outgoing.frame_label().raw(),
            outgoing.payload().as_bytes(),
        ) {
            Ok(frame) => frame,
            Err(err) => return Poll::Ready(Err(err)),
        };
        Poll::Ready(self.backend.enqueue(outgoing.peer(), frame))
    }

    fn cancel_send<'a>(&'a self, _tx: &'a mut Self::Tx<'a>) {}

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        rx.current = None;
        rx.current = rx.backend.dequeue(rx.role)?;
        if rx.current.is_none() {
            rx.backend.store_recv_waker(rx.role, cx.waker())?;
            rx.current = rx.backend.dequeue(rx.role)?;
        }
        let Some(frame) = rx.current.as_ref() else {
            return Poll::Pending;
        };
        let bytes: &'a [u8] = unsafe { &*(frame.as_slice() as *const [u8]) };
        Poll::Ready(Ok(Payload::new(bytes)))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.current.take() {
            rx.backend
                .requeue(rx.role, frame)
                .expect("requeue must preserve the previously received frame");
        }
    }

    fn drain_events(&self, _emit: &mut dyn FnMut(TransportEvent)) {}

    fn recv_frame_hint<'a>(&'a self, rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        self.backend
            .peek_label(rx.role)
            .ok()
            .flatten()
            .map(FrameLabel::new)
    }

    fn metrics(&self) -> Self::Metrics {
        ()
    }

    fn apply_pacing_update(&self, _interval_us: u32, _burst_bytes: u16) {}
}

#[cfg(test)]
mod tests {
    use super::SioTransport;
    use crate::port::host_queue::{FifoBackend, FrameOwned, HostQueueBackend};
    use core::task::{Context, Poll};
    use hibana::substrate::Transport;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CountWake {
        wakes: AtomicUsize,
    }

    impl std::task::Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn recv_future_registers_waker_and_enqueue_wakes_it() {
        let backend = HostQueueBackend::new();
        let transport = SioTransport::new(&backend);
        let (_tx, mut rx_pending) = transport.open(0, 7);
        let wake_counter = Arc::new(CountWake {
            wakes: AtomicUsize::new(0),
        });
        let waker = std::task::Waker::from(Arc::clone(&wake_counter));
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(
            transport.poll_recv(&mut rx_pending, &mut cx),
            Poll::Pending
        ));
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 0);

        backend
            .enqueue(0, FrameOwned::from_bytes(1, &[0x2a]).expect("frame"))
            .expect("enqueue should wake pending recv");

        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 1);

        let (_tx, mut rx_ready) = transport.open(0, 7);
        let payload = match transport.poll_recv(&mut rx_ready, &mut cx) {
            Poll::Ready(Ok(payload)) => payload,
            other => panic!("expected ready payload after wake, got {other:?}"),
        };
        assert_eq!(payload.as_bytes(), &[0x2a]);
    }
}
