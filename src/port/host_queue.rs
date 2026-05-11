use core::{cell::UnsafeCell, mem::MaybeUninit, task::Waker};

use hibana::substrate::transport::TransportError;

pub(crate) const ROLE_CAPACITY: usize = 4;
pub(crate) const QUEUE_CAPACITY: usize = 8;
pub(crate) const PAYLOAD_CAPACITY: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendError {
    PayloadTooLarge,
    QueueFull,
    QueueEmpty,
    RoleOutOfRange,
    InvalidFrame,
}

impl From<BackendError> for TransportError {
    fn from(_value: BackendError) -> Self {
        TransportError::Failed
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FrameOwned {
    label: u8,
    len: usize,
    payload: [u8; PAYLOAD_CAPACITY],
}

impl FrameOwned {
    pub(crate) fn from_bytes(label: u8, bytes: &[u8]) -> Result<Self, BackendError> {
        if bytes.len() > PAYLOAD_CAPACITY {
            return Err(BackendError::PayloadTooLarge);
        }
        let mut payload = [0u8; PAYLOAD_CAPACITY];
        payload[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            label,
            len: bytes.len(),
            payload,
        })
    }

    pub(crate) const fn label(&self) -> u8 {
        self.label
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.payload[..self.len]
    }
}

#[derive(Clone, Copy)]
struct FixedQueue {
    items: [Option<FrameOwned>; QUEUE_CAPACITY],
    head: usize,
    len: usize,
}

impl FixedQueue {
    const fn new() -> Self {
        Self {
            items: [None; QUEUE_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push_back(&mut self, item: FrameOwned) -> Result<(), BackendError> {
        if self.len >= QUEUE_CAPACITY {
            return Err(BackendError::QueueFull);
        }
        let idx = (self.head + self.len) % QUEUE_CAPACITY;
        self.items[idx] = Some(item);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, item: FrameOwned) -> Result<(), BackendError> {
        if self.len >= QUEUE_CAPACITY {
            return Err(BackendError::QueueFull);
        }
        self.head = if self.head == 0 {
            QUEUE_CAPACITY - 1
        } else {
            self.head - 1
        };
        self.items[self.head] = Some(item);
        self.len += 1;
        Ok(())
    }

    fn pop_front(&mut self) -> Option<FrameOwned> {
        if self.len == 0 {
            return None;
        }
        let idx = self.head;
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.len -= 1;
        self.items[idx].take()
    }

    fn peek_front(&self) -> Option<&FrameOwned> {
        if self.len == 0 {
            return None;
        }
        self.items[self.head].as_ref()
    }
}

#[derive(Clone, Copy)]
struct RoleState {
    queue: FixedQueue,
}

impl RoleState {
    const fn new() -> Self {
        Self {
            queue: FixedQueue::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BackendState {
    roles: [RoleState; ROLE_CAPACITY],
}

impl BackendState {
    pub(crate) const fn new() -> Self {
        Self {
            roles: [RoleState::new(); ROLE_CAPACITY],
        }
    }

    fn role_mut(&mut self, role: u8) -> Result<&mut RoleState, BackendError> {
        self.roles
            .get_mut(role as usize)
            .ok_or(BackendError::RoleOutOfRange)
    }

    pub(crate) fn push_back(&mut self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        self.role_mut(role)?.queue.push_back(frame)
    }

    pub(crate) fn push_front(&mut self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        self.role_mut(role)?.queue.push_front(frame)
    }

    pub(crate) fn pop_front(&mut self, role: u8) -> Result<Option<FrameOwned>, BackendError> {
        Ok(self.role_mut(role)?.queue.pop_front())
    }

    pub(crate) fn peek_label(&self, role: u8) -> Result<Option<u8>, BackendError> {
        self.roles
            .get(role as usize)
            .map(|state| state.queue.peek_front().map(FrameOwned::label))
            .ok_or(BackendError::RoleOutOfRange)
    }

    #[cfg(test)]
    fn queue_len(&self, role: u8) -> Result<usize, BackendError> {
        self.roles
            .get(role as usize)
            .map(|state| state.queue.len)
            .ok_or(BackendError::RoleOutOfRange)
    }
}

struct StoredWaker {
    present: bool,
    waker: MaybeUninit<Waker>,
}

impl StoredWaker {
    const fn new() -> Self {
        Self {
            present: false,
            waker: MaybeUninit::uninit(),
        }
    }

    fn store(&mut self, waker: &Waker) {
        if self.present {
            unsafe {
                self.waker.as_mut_ptr().replace(waker.clone());
            }
        } else {
            self.waker.write(waker.clone());
            self.present = true;
        }
    }

    fn take(&mut self) -> Option<Waker> {
        if !self.present {
            return None;
        }
        self.present = false;
        Some(unsafe { self.waker.assume_init_read() })
    }
}

pub(crate) struct WakerState {
    roles: [StoredWaker; ROLE_CAPACITY],
}

impl WakerState {
    pub(crate) const fn new() -> Self {
        Self {
            roles: [const { StoredWaker::new() }; ROLE_CAPACITY],
        }
    }

    pub(crate) fn store(&mut self, role: u8, waker: &Waker) -> Result<(), BackendError> {
        self.roles
            .get_mut(role as usize)
            .ok_or(BackendError::RoleOutOfRange)?
            .store(waker);
        Ok(())
    }

    pub(crate) fn take(&mut self, role: u8) -> Result<Option<Waker>, BackendError> {
        Ok(self
            .roles
            .get_mut(role as usize)
            .ok_or(BackendError::RoleOutOfRange)?
            .take())
    }
}

pub(crate) trait FifoBackend {
    fn enqueue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError>;
    fn dequeue(&self, role: u8) -> Result<Option<FrameOwned>, BackendError>;
    fn requeue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError>;
    fn peek_label(&self, role: u8) -> Result<Option<u8>, BackendError>;
    fn store_recv_waker(&self, role: u8, waker: &Waker) -> Result<(), BackendError>;
}

/// Host-side fixed-capacity queue backend used by parity tests.
pub struct HostQueueBackend {
    state: UnsafeCell<BackendState>,
    recv_wakers: UnsafeCell<WakerState>,
}

impl HostQueueBackend {
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(BackendState::new()),
            recv_wakers: UnsafeCell::new(WakerState::new()),
        }
    }

    fn with_state_mut<R>(&self, f: impl FnOnce(&mut BackendState) -> R) -> R {
        unsafe { f(&mut *self.state.get()) }
    }

    fn with_recv_wakers_mut<R>(&self, f: impl FnOnce(&mut WakerState) -> R) -> R {
        unsafe { f(&mut *self.recv_wakers.get()) }
    }

    #[cfg(test)]
    pub(crate) fn enqueue_bytes(&self, role: u8, bytes: &[u8]) -> Result<(), BackendError> {
        self.enqueue(role, FrameOwned::from_bytes(0, bytes)?)
    }

    #[cfg(test)]
    pub(crate) fn queue_len(&self, role: u8) -> Result<usize, BackendError> {
        self.with_state_mut(|state| state.queue_len(role))
    }
}

impl Default for HostQueueBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FifoBackend for HostQueueBackend {
    fn enqueue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        self.with_state_mut(|state| state.push_back(role, frame))?;
        if let Some(waker) = self.with_recv_wakers_mut(|state| state.take(role))? {
            waker.wake();
        }
        Ok(())
    }

    fn dequeue(&self, role: u8) -> Result<Option<FrameOwned>, BackendError> {
        let frame = self.with_state_mut(|state| state.pop_front(role))?;
        if frame.is_some() {
            let _ = self.with_recv_wakers_mut(|state| state.take(role))?;
        }
        Ok(frame)
    }

    fn requeue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        self.with_state_mut(|state| state.push_front(role, frame))?;
        if let Some(waker) = self.with_recv_wakers_mut(|state| state.take(role))? {
            waker.wake();
        }
        Ok(())
    }

    fn peek_label(&self, role: u8) -> Result<Option<u8>, BackendError> {
        self.with_state_mut(|state| state.peek_label(role))
    }

    fn store_recv_waker(&self, role: u8, waker: &Waker) -> Result<(), BackendError> {
        self.with_recv_wakers_mut(|state| state.store(role, waker))
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendError, HostQueueBackend, PAYLOAD_CAPACITY, QUEUE_CAPACITY};

    #[test]
    fn host_backend_rejects_payloads_that_exceed_fixed_capacity() {
        let backend = HostQueueBackend::new();
        let payload = [7u8; PAYLOAD_CAPACITY + 1];
        let result = backend.enqueue_bytes(0, &payload);
        assert_eq!(result, Err(BackendError::PayloadTooLarge));
    }

    #[test]
    fn host_backend_rejects_queue_overflow() {
        let backend = HostQueueBackend::new();
        for _ in 0..QUEUE_CAPACITY {
            backend.enqueue_bytes(0, &[1, 2, 3]).expect("room in queue");
        }
        assert_eq!(backend.enqueue_bytes(0, &[9]), Err(BackendError::QueueFull));
        assert_eq!(backend.queue_len(0), Ok(QUEUE_CAPACITY));
    }
}
