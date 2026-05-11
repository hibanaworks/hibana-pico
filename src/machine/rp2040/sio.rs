use core::task::Waker;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::asm,
    cell::UnsafeCell,
    ptr::{read_volatile, write_volatile},
};

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
use crate::port::host_queue::HostQueueBackend;
use crate::port::host_queue::{BackendError, FifoBackend, FrameOwned};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::port::host_queue::{BackendState, PAYLOAD_CAPACITY, ROLE_CAPACITY, WakerState};

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_BASE: usize = 0xD000_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_CPUID: *const u32 = SIO_BASE as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST: *const u32 = (SIO_BASE + 0x50) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST_WRITE: *mut u32 = (SIO_BASE + 0x50) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_WR: *mut u32 = (SIO_BASE + 0x54) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_RD: *const u32 = (SIO_BASE + 0x58) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_BASE: usize = 0x4001_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_FRCE_OFF: *mut u32 = (PSM_BASE + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_VLD: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_RDY: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_WOF: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_ROE: u32 = 1 << 3;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RP2040_FRAME_MAGIC: u32 = 0x4849_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RP2040_FRAME_MAGIC_MASK: u32 = 0xffff_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RP2040_ROUTE_MAGIC: u32 = 0x5254_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_PROC1: u32 = 1 << 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CORE1_LAUNCH_RETRIES: u8 = 16;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn core_id() -> u32 {
    unsafe { read_volatile(SIO_CPUID) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn fifo_drain() {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD != 0 {
        let _ = unsafe { read_volatile(SIO_FIFO_RD) };
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn fifo_clear_errors() {
    unsafe { write_volatile(SIO_FIFO_ST_WRITE, FIFO_WOF | FIFO_ROE) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn reset_core1_to_bootrom() {
    let frce_off = unsafe { read_volatile(PSM_FRCE_OFF) };
    unsafe {
        write_volatile(PSM_FRCE_OFF, frce_off | PSM_PROC1);
    }
    for _ in 0..32 {
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(PSM_FRCE_OFF, frce_off & !PSM_PROC1);
    }
    for _ in 0..32 {
        core::hint::spin_loop();
    }
    fifo_drain();
    fifo_clear_errors();
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn launch_core1(vector_table: u32, stack_top: u32, entry: u32) -> bool {
    reset_core1_to_bootrom();

    let sequence = [0, 0, 1, vector_table, stack_top, entry];
    let mut index = 0usize;
    let mut failures = 0u8;
    while index < sequence.len() {
        let word = sequence[index];
        if word == 0 {
            fifo_drain();
            fifo_clear_errors();
            event();
        }
        fifo_push_blocking(word);
        if fifo_pop_blocking() == word {
            index += 1;
            continue;
        }
        index = 0;
        failures = failures.saturating_add(1);
        if failures > CORE1_LAUNCH_RETRIES {
            return false;
        }
    }
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn fifo_push_blocking(word: u32) {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_RDY == 0 {
        core::hint::spin_loop();
    }
    unsafe { write_volatile(SIO_FIFO_WR, word) };
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn fifo_pop_blocking() -> u32 {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD == 0 {
        core::hint::spin_loop();
    }
    unsafe { read_volatile(SIO_FIFO_RD) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn event() {
    unsafe { asm!("sev", options(nomem, nostack, preserves_flags)) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn wait_event() {
    unsafe { asm!("wfe", options(nomem, nostack, preserves_flags)) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_status() -> u32 {
    unsafe { read_volatile(SIO_FIFO_ST) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_try_pop() -> Option<u32> {
    if fifo_status() & FIFO_VLD == 0 {
        return None;
    }
    let word = unsafe { read_volatile(SIO_FIFO_RD) };
    event();
    Some(word)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_read_blocking() -> u32 {
    loop {
        if let Some(word) = fifo_try_pop() {
            return word;
        }
        wait_event();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn role_core(role: u8) -> Result<u8, BackendError> {
    match role {
        0 | 2 | 3 => Ok(0),
        1 => Ok(1),
        _ => Err(BackendError::RoleOutOfRange),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn validate_role(role: u8) -> Result<usize, BackendError> {
    if (role as usize) < ROLE_CAPACITY {
        Ok(role as usize)
    } else {
        Err(BackendError::RoleOutOfRange)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_word_count(len: usize) -> usize {
    len.div_ceil(4)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pack_header(frame: FrameOwned) -> u32 {
    RP2040_FRAME_MAGIC | (frame.label() as u32) | (((frame.len() as u32) & 0xff) << 8)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pack_route(role: u8) -> u32 {
    RP2040_ROUTE_MAGIC | (role as u32)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn decode_header(header: u32) -> Result<Option<(u8, usize)>, BackendError> {
    if header & RP2040_FRAME_MAGIC_MASK != RP2040_FRAME_MAGIC {
        return Ok(None);
    }
    let label = (header & 0xff) as u8;
    let len = ((header >> 8) & 0xff) as usize;
    if len > PAYLOAD_CAPACITY {
        return Err(BackendError::PayloadTooLarge);
    }
    Ok(Some((label, len)))
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn decode_route(route: u32) -> Result<u8, BackendError> {
    if route & RP2040_FRAME_MAGIC_MASK != RP2040_ROUTE_MAGIC {
        return Err(BackendError::InvalidFrame);
    }
    let role = (route & 0xff) as u8;
    let _ = validate_role(role)?;
    Ok(role)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pack_payload_word(bytes: &[u8]) -> u32 {
    let mut word_bytes = [0u8; 4];
    word_bytes[..bytes.len()].copy_from_slice(bytes);
    u32::from_le_bytes(word_bytes)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct RequeueSlots(UnsafeCell<[Option<FrameOwned>; ROLE_CAPACITY]>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for RequeueSlots {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static REQUEUE_SLOTS: RequeueSlots = RequeueSlots(UnsafeCell::new([None; ROLE_CAPACITY]));

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct LocalState(UnsafeCell<BackendState>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for LocalState {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static LOCAL_STATE: LocalState = LocalState(UnsafeCell::new(BackendState::new()));

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct RecvWakers(UnsafeCell<WakerState>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for RecvWakers {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static RECV_WAKERS: RecvWakers = RecvWakers(UnsafeCell::new(WakerState::new()));

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn requeue_take(role: u8) -> Result<Option<FrameOwned>, BackendError> {
    let slot = validate_role(role)?;
    let slots = unsafe { &mut *REQUEUE_SLOTS.0.get() };
    Ok(slots[slot].take())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn store_recv_waker(role: u8, waker: &Waker) -> Result<(), BackendError> {
    let recv_wakers = unsafe { &mut *RECV_WAKERS.0.get() };
    recv_wakers.store(role, waker)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn take_recv_waker(role: u8) -> Result<Option<Waker>, BackendError> {
    let recv_wakers = unsafe { &mut *RECV_WAKERS.0.get() };
    recv_wakers.take(role)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn wake_recv(role: u8) -> Result<(), BackendError> {
    if let Some(waker) = take_recv_waker(role)? {
        waker.wake();
    }
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn requeue_put(role: u8, frame: FrameOwned) -> Result<(), BackendError> {
    let slot = validate_role(role)?;
    let slots = unsafe { &mut *REQUEUE_SLOTS.0.get() };
    if slots[slot].is_some() {
        return Err(BackendError::QueueFull);
    }
    slots[slot] = Some(frame);
    let _ = wake_recv(role);
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn local_enqueue(role: u8, frame: FrameOwned) -> Result<(), BackendError> {
    let state = unsafe { &mut *LOCAL_STATE.0.get() };
    state.push_back(role, frame)?;
    let _ = wake_recv(role);
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn local_dequeue(role: u8) -> Result<Option<FrameOwned>, BackendError> {
    let state = unsafe { &mut *LOCAL_STATE.0.get() };
    state.pop_front(role)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn local_peek_label(role: u8) -> Result<Option<u8>, BackendError> {
    let state = unsafe { &*LOCAL_STATE.0.get() };
    state.peek_label(role)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn sio_enqueue(role: u8, frame: FrameOwned) -> Result<(), BackendError> {
    let _ = validate_role(role)?;
    if frame.len() > PAYLOAD_CAPACITY {
        return Err(BackendError::PayloadTooLarge);
    }
    if role_core(role)? == core_id() as u8 {
        return local_enqueue(role, frame);
    }
    fifo_push_blocking(pack_header(frame));
    fifo_push_blocking(pack_route(role));
    for chunk in frame.as_slice().chunks(4) {
        fifo_push_blocking(pack_payload_word(chunk));
    }
    let _ = wake_recv(role);
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn sio_dequeue(role: u8) -> Result<Option<FrameOwned>, BackendError> {
    let _ = validate_role(role)?;
    if let Some(frame) = local_dequeue(role)? {
        let _ = take_recv_waker(role)?;
        return Ok(Some(frame));
    }
    if let Some(frame) = requeue_take(role)? {
        let _ = take_recv_waker(role)?;
        return Ok(Some(frame));
    }

    loop {
        let Some(header) = fifo_try_pop() else {
            return Ok(None);
        };
        let Some((label, len)) = decode_header(header)? else {
            continue;
        };
        let dst_role = decode_route(fifo_read_blocking())?;
        let mut payload = [0u8; PAYLOAD_CAPACITY];
        let word_count = fifo_word_count(len);

        for word_index in 0..word_count {
            let word = fifo_read_blocking();
            let start = word_index * 4;
            let end = core::cmp::min(start + 4, len);
            let chunk_len = end.checked_sub(start).ok_or(BackendError::InvalidFrame)?;
            if chunk_len > 4 || end > PAYLOAD_CAPACITY {
                return Err(BackendError::InvalidFrame);
            }
            payload[start..end].copy_from_slice(&word.to_le_bytes()[..chunk_len]);
        }

        let frame = FrameOwned::from_bytes(label, &payload[..len])?;
        if dst_role == role {
            let _ = take_recv_waker(role)?;
            return Ok(Some(frame));
        }
        requeue_put(dst_role, frame)?;
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn sio_peek_label(role: u8) -> Result<Option<u8>, BackendError> {
    let slot = validate_role(role)?;
    if let Some(label) = local_peek_label(role)? {
        return Ok(Some(label));
    }
    let slots = unsafe { &*REQUEUE_SLOTS.0.get() };
    Ok(slots[slot].map(|frame| frame.label()))
}

pub struct Rp2040SioBackend {
    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    host: HostQueueBackend,
}

impl Rp2040SioBackend {
    pub const fn new() -> Self {
        Self {
            #[cfg(not(all(target_arch = "arm", target_os = "none")))]
            host: HostQueueBackend::new(),
        }
    }
}

impl Default for Rp2040SioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FifoBackend for Rp2040SioBackend {
    fn enqueue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            return sio_enqueue(role, frame);
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        FifoBackend::enqueue(&self.host, role, frame)
    }

    fn dequeue(&self, role: u8) -> Result<Option<FrameOwned>, BackendError> {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            return sio_dequeue(role);
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        FifoBackend::dequeue(&self.host, role)
    }

    fn requeue(&self, role: u8, frame: FrameOwned) -> Result<(), BackendError> {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            return requeue_put(role, frame);
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        FifoBackend::requeue(&self.host, role, frame)
    }

    fn peek_label(&self, role: u8) -> Result<Option<u8>, BackendError> {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            return sio_peek_label(role);
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        FifoBackend::peek_label(&self.host, role)
    }

    fn store_recv_waker(&self, role: u8, waker: &Waker) -> Result<(), BackendError> {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            return store_recv_waker(role, waker);
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        FifoBackend::store_recv_waker(&self.host, role, waker)
    }
}
