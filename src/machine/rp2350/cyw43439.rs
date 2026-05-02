use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
    task::{Context, Poll},
};

use hibana::substrate::{
    Transport,
    transport::{FrameLabel, Outgoing, TransportError, advanced::TransportEvent},
    wire::Payload,
};

use crate::kernel::swarm::{
    NodeId, SWARM_FRAME_MAX_WIRE_LEN, SwarmDropTelemetry, SwarmError, SwarmFrame, SwarmSecurity,
};

const CYW_CMD_SELECT_NODE: u8 = 0x10;
const CYW_CMD_STATUS: u8 = 0x20;
const CYW_CMD_TX_FRAME: u8 = 0x30;
const CYW_CMD_RX_FRAME: u8 = 0x40;
const CYW_CMD_RESET: u8 = 0x50;
const CYW_CMD_CLEAR_STATUS: u8 = 0x51;
const CYW_CMD_POWER_ON: u8 = 0x52;
const CYW_CMD_RESET_ASSERT: u8 = 0x53;
const CYW_CMD_RESET_RELEASE: u8 = 0x54;
const CYW_CMD_PEEK_LABEL: u8 = 0x60;
const CYW_CMD_NODE_ROLE: u8 = 0x70;
const CYW_CMD_NODE_ID: u8 = 0x71;
const CYW_CMD_RADIO_MODE: u8 = 0x72;
const CYW_CMD_NODE_COUNT: u8 = 0x73;
const CYW_CMD_FW_BEGIN: u8 = 0x80;
const CYW_CMD_FW_CHUNK: u8 = 0x81;
const CYW_CMD_FW_COMMIT: u8 = 0x82;
const CYW_CMD_CLM_BEGIN: u8 = 0x83;
const CYW_CMD_CLM_CHUNK: u8 = 0x84;
const CYW_CMD_CLM_COMMIT: u8 = 0x85;
const CYW_CMD_NVRAM_APPLY: u8 = 0x86;
const CYW_CMD_BOOT: u8 = 0x87;
const CYW_CMD_FW_STATE: u8 = 0x88;
const CYW_CMD_IDENT: u8 = 0x9f;

const CYW_STATUS_RX_READY: u8 = 1 << 0;
const CYW_STATUS_TX_READY: u8 = 1 << 1;
const CYW_STATUS_JOINED: u8 = 1 << 2;
const CYW_STATUS_OVERFLOW: u8 = 1 << 4;

const CYW_FW_STATE_READY: u8 = 1 << 5;
const CYW_FW_STATE_ERROR: u8 = 1 << 7;
const CYW_ACK: u8 = 0xac;
const CYW_ERR: u8 = 0xee;
const CYW_FNV1A32_OFFSET: u32 = 0x811c_9dc5;
#[cfg(test)]
const CYW_FNV1A32_PRIME: u32 = 0x0100_0193;
const CYW_FIRMWARE_CHUNK: usize = 128;

pub const CYW43439_WIFI_FW_LEN: usize = 224_190;
pub const CYW43439_CLM_LEN: usize = 984;
pub const CYW43439_WIFI_FW_FNV1A32: u32 = 0xfa23_1a9f;
pub const CYW43439_CLM_FNV1A32: u32 = 0x5178_f94d;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static CYW43439_WIFI_FIRMWARE: &[u8; CYW43439_WIFI_FW_LEN] =
    include_bytes!("../../../firmware/cyw43/w43439A0_7_95_49_00_firmware.bin");
#[cfg(not(all(target_arch = "arm", target_os = "none")))]
static CYW43439_WIFI_FIRMWARE: &[u8] = &[];
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CYW43439_CLM: &[u8; CYW43439_CLM_LEN] =
    include_bytes!("../../../firmware/cyw43/w43439A0_7_95_49_00_clm.bin");
#[cfg(not(all(target_arch = "arm", target_os = "none")))]
static CYW43439_CLM: &[u8] = &[];

pub const NODE_ROLE_COORDINATOR: u8 = 0;
pub const NODE_ROLE_SENSOR: u8 = 1;
pub const NODE_ROLE_DUAL_CORE: u8 = 0xff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cyw43439Error {
    SpiUnavailable,
    NotPresent,
    NotJoined,
    FirmwareLoadFailed,
    TxNotReady,
    QueueOverflow,
    FrameTooLarge,
    Swarm(SwarmError),
}

impl From<SwarmError> for Cyw43439Error {
    fn from(value: SwarmError) -> Self {
        Self::Swarm(value)
    }
}

impl From<Cyw43439Error> for TransportError {
    fn from(_value: Cyw43439Error) -> Self {
        TransportError::Failed
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod rp2350_spi {
    use core::ptr::{read_volatile, write_volatile};

    const SIO_BASE: usize = 0xd000_0000;
    const SIO_CPUID: *const u32 = SIO_BASE as *const u32;
    const SPI0_BASE: usize = 0x4008_0000;
    const SPI_CR0: *mut u32 = SPI0_BASE as *mut u32;
    const SPI_CR1: *mut u32 = (SPI0_BASE + 0x04) as *mut u32;
    const SPI_DR: *mut u32 = (SPI0_BASE + 0x08) as *mut u32;
    const SPI_SR: *const u32 = (SPI0_BASE + 0x0c) as *const u32;
    const SPI_CPSR: *mut u32 = (SPI0_BASE + 0x10) as *mut u32;
    const SPI_SR_TNF: u32 = 1 << 1;
    const SPI_SR_RNE: u32 = 1 << 2;
    const SPI_CR1_SSE: u32 = 1 << 1;

    static mut SPI_LOCK_WANT: [u32; 2] = [0; 2];
    static mut SPI_LOCK_TURN: u32 = 0;

    fn core_id() -> usize {
        unsafe { read_volatile(SIO_CPUID) as usize & 1 }
    }

    fn lock() {
        let me = core_id();
        let other = 1usize.saturating_sub(me);
        unsafe {
            write_volatile(core::ptr::addr_of_mut!(SPI_LOCK_WANT[me]), 1);
            write_volatile(core::ptr::addr_of_mut!(SPI_LOCK_TURN), other as u32);
        }
        while unsafe { read_volatile(core::ptr::addr_of!(SPI_LOCK_WANT[other])) } != 0
            && unsafe { read_volatile(core::ptr::addr_of!(SPI_LOCK_TURN)) } == other as u32
        {
            core::hint::spin_loop();
        }
    }

    fn unlock() {
        let me = core_id();
        unsafe {
            write_volatile(core::ptr::addr_of_mut!(SPI_LOCK_WANT[me]), 0);
        }
    }

    pub fn with_lock<T>(f: impl FnOnce() -> T) -> T {
        lock();
        let out = f();
        unlock();
        out
    }

    pub fn init() {
        unsafe {
            write_volatile(SPI_CR1, 0);
            write_volatile(SPI_CPSR, 2);
            write_volatile(SPI_CR0, 7);
            write_volatile(SPI_CR1, SPI_CR1_SSE);
        }
    }

    pub fn transfer(byte: u8) -> u8 {
        while unsafe { read_volatile(SPI_SR) } & SPI_SR_TNF == 0 {}
        unsafe { write_volatile(SPI_DR, byte as u32) };
        while unsafe { read_volatile(SPI_SR) } & SPI_SR_RNE == 0 {}
        unsafe { read_volatile(SPI_DR) as u8 }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
mod rp2350_spi {
    pub fn with_lock<T>(f: impl FnOnce() -> T) -> T {
        f()
    }

    pub fn init() {}

    pub fn transfer(_byte: u8) -> u8 {
        0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cyw43439Status {
    bits: u8,
}

impl Cyw43439Status {
    pub const fn from_bits(bits: u8) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn rx_ready(self) -> bool {
        self.bits & CYW_STATUS_RX_READY != 0
    }

    pub const fn tx_ready(self) -> bool {
        self.bits & CYW_STATUS_TX_READY != 0
    }

    pub const fn joined(self) -> bool {
        self.bits & CYW_STATUS_JOINED != 0
    }

    pub const fn queue_overflow(self) -> bool {
        self.bits & CYW_STATUS_OVERFLOW != 0
    }
}

#[cfg(test)]
fn cyw_fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = CYW_FNV1A32_OFFSET;
    for byte in bytes {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(CYW_FNV1A32_PRIME);
    }
    hash
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Cyw43439Driver;

impl Cyw43439Driver {
    pub const fn new() -> Self {
        Self
    }

    pub fn init(self) -> Result<(), Cyw43439Error> {
        let ident = rp2350_spi::with_lock(|| {
            rp2350_spi::init();
            self.expect_ack_unlocked(CYW_CMD_POWER_ON)?;
            self.expect_ack_unlocked(CYW_CMD_RESET_ASSERT)?;
            self.expect_ack_unlocked(CYW_CMD_RESET_RELEASE)?;
            Ok::<u8, Cyw43439Error>(rp2350_spi::transfer(CYW_CMD_IDENT))
        })?;
        if ident != 0x43 {
            return Err(Cyw43439Error::NotPresent);
        }
        self.load_official_firmware()?;
        Ok(())
    }

    pub fn load_official_firmware(self) -> Result<(), Cyw43439Error> {
        rp2350_spi::with_lock(|| {
            self.send_load_begin_unlocked(
                CYW_CMD_FW_BEGIN,
                CYW43439_WIFI_FW_LEN as u32,
                CYW43439_WIFI_FW_FNV1A32,
            )?;
            self.send_load_chunks_unlocked(CYW_CMD_FW_CHUNK, CYW43439_WIFI_FIRMWARE)?;
            self.expect_ack_unlocked(CYW_CMD_FW_COMMIT)?;

            self.send_load_begin_unlocked(
                CYW_CMD_CLM_BEGIN,
                CYW43439_CLM_LEN as u32,
                CYW43439_CLM_FNV1A32,
            )?;
            self.send_load_chunks_unlocked(CYW_CMD_CLM_CHUNK, CYW43439_CLM)?;
            self.expect_ack_unlocked(CYW_CMD_CLM_COMMIT)?;

            self.send_load_begin_unlocked(CYW_CMD_NVRAM_APPLY, 0, CYW_FNV1A32_OFFSET)?;
            self.expect_ack_unlocked(CYW_CMD_BOOT)?;
            let state = rp2350_spi::transfer(CYW_CMD_FW_STATE);
            if state & CYW_FW_STATE_ERROR != 0 || state & CYW_FW_STATE_READY == 0 {
                return Err(Cyw43439Error::FirmwareLoadFailed);
            }
            Ok(())
        })
    }

    pub fn reset(self) {
        rp2350_spi::with_lock(|| {
            rp2350_spi::init();
            let _ = rp2350_spi::transfer(CYW_CMD_RESET);
        });
    }

    pub fn clear_status(self) {
        rp2350_spi::with_lock(|| self.clear_status_unlocked());
    }

    pub fn node_role(self) -> u8 {
        rp2350_spi::with_lock(|| rp2350_spi::transfer(CYW_CMD_NODE_ROLE))
    }

    pub fn node_id(self) -> u8 {
        rp2350_spi::with_lock(|| rp2350_spi::transfer(CYW_CMD_NODE_ID))
    }

    pub fn node_count(self) -> u8 {
        rp2350_spi::with_lock(|| rp2350_spi::transfer(CYW_CMD_NODE_COUNT))
    }

    pub fn radio_peer_mode(self) -> bool {
        rp2350_spi::with_lock(|| rp2350_spi::transfer(CYW_CMD_RADIO_MODE) != 0)
    }

    pub fn status(self, node: NodeId) -> Cyw43439Status {
        rp2350_spi::with_lock(|| self.status_unlocked(node))
    }

    pub fn send_frame(self, dst: NodeId, bytes: &[u8]) -> Result<(), Cyw43439Error> {
        if bytes.len() > SWARM_FRAME_MAX_WIRE_LEN {
            return Err(Cyw43439Error::FrameTooLarge);
        }
        if dst.raw() == 0 || dst.raw() > u8::MAX as u16 {
            return Err(Cyw43439Error::Swarm(SwarmError::BadNode));
        }

        rp2350_spi::with_lock(|| {
            let status = self.status_unlocked(dst);
            if !status.joined() {
                return Err(Cyw43439Error::NotJoined);
            }
            if status.queue_overflow() {
                return Err(Cyw43439Error::QueueOverflow);
            }
            if !status.tx_ready() {
                return Err(Cyw43439Error::TxNotReady);
            }

            let _ = rp2350_spi::transfer(CYW_CMD_TX_FRAME);
            let _ = rp2350_spi::transfer(dst.raw() as u8);
            let _ = rp2350_spi::transfer(bytes.len() as u8);
            for byte in bytes {
                let _ = rp2350_spi::transfer(*byte);
            }
            Ok(())
        })
    }

    pub fn recv_frame(
        self,
        node: NodeId,
        out: &mut [u8; SWARM_FRAME_MAX_WIRE_LEN],
    ) -> Result<Option<usize>, Cyw43439Error> {
        if node.raw() == 0 || node.raw() > u8::MAX as u16 {
            return Err(Cyw43439Error::Swarm(SwarmError::BadNode));
        }

        rp2350_spi::with_lock(|| {
            let status = self.status_unlocked(node);
            if !status.joined() {
                return Err(Cyw43439Error::NotJoined);
            }
            if status.queue_overflow() {
                return Err(Cyw43439Error::QueueOverflow);
            }
            if !status.rx_ready() {
                return Ok(None);
            }

            let len = rp2350_spi::transfer(CYW_CMD_RX_FRAME) as usize;
            if len == 0 {
                return Ok(None);
            }
            if len > SWARM_FRAME_MAX_WIRE_LEN {
                return Err(Cyw43439Error::FrameTooLarge);
            }
            for byte in out.iter_mut().take(len) {
                *byte = rp2350_spi::transfer(0);
            }
            Ok(Some(len))
        })
    }

    pub fn peek_label(self, node: NodeId) -> Option<u8> {
        if node.raw() == 0 || node.raw() > u8::MAX as u16 {
            return None;
        }

        rp2350_spi::with_lock(|| {
            let status = self.status_unlocked(node);
            if !status.joined() || status.queue_overflow() || !status.rx_ready() {
                return None;
            }
            match rp2350_spi::transfer(CYW_CMD_PEEK_LABEL) {
                0xff => None,
                label => Some(label),
            }
        })
    }

    fn select_node_unlocked(self, node: NodeId) {
        let _ = rp2350_spi::transfer(CYW_CMD_SELECT_NODE);
        let _ = rp2350_spi::transfer(node.raw() as u8);
    }

    fn status_unlocked(self, node: NodeId) -> Cyw43439Status {
        self.select_node_unlocked(node);
        Cyw43439Status::from_bits(rp2350_spi::transfer(CYW_CMD_STATUS))
    }

    fn clear_status_unlocked(self) {
        let _ = rp2350_spi::transfer(CYW_CMD_CLEAR_STATUS);
    }

    fn expect_ack_unlocked(self, byte: u8) -> Result<(), Cyw43439Error> {
        match rp2350_spi::transfer(byte) {
            CYW_ACK => Ok(()),
            CYW_ERR => Err(Cyw43439Error::FirmwareLoadFailed),
            _ => Err(Cyw43439Error::FirmwareLoadFailed),
        }
    }

    fn send_u32_unlocked(self, value: u32) -> Result<(), Cyw43439Error> {
        for byte in value.to_le_bytes() {
            self.expect_ack_unlocked(byte)?;
        }
        Ok(())
    }

    fn send_load_begin_unlocked(self, cmd: u8, len: u32, hash: u32) -> Result<(), Cyw43439Error> {
        self.expect_ack_unlocked(cmd)?;
        self.send_u32_unlocked(len)?;
        self.send_u32_unlocked(hash)
    }

    fn send_load_chunks_unlocked(self, cmd: u8, bytes: &[u8]) -> Result<(), Cyw43439Error> {
        let mut offset = 0u32;
        for chunk in bytes.chunks(CYW_FIRMWARE_CHUNK) {
            self.expect_ack_unlocked(cmd)?;
            self.send_u32_unlocked(offset)?;
            self.expect_ack_unlocked(chunk.len() as u8)?;
            for byte in chunk {
                self.expect_ack_unlocked(*byte)?;
            }
            offset += chunk.len() as u32;
        }
        Ok(())
    }
}

const DRIVER: Cyw43439Driver = Cyw43439Driver::new();

pub fn init() -> Result<(), Cyw43439Error> {
    DRIVER.init()
}

pub fn reset() {
    DRIVER.reset();
}

pub fn clear_status() {
    DRIVER.clear_status();
}

pub fn node_role() -> u8 {
    DRIVER.node_role()
}

pub fn node_id() -> u8 {
    DRIVER.node_id()
}

pub fn node_count() -> u8 {
    DRIVER.node_count()
}

pub fn radio_peer_mode() -> bool {
    DRIVER.radio_peer_mode()
}

pub struct QemuCyw43439Transport {
    role_nodes: [NodeId; QEMU_CYW43439_MAX_ROLES],
    role_count: u8,
    session_generation: u16,
    security: SwarmSecurity,
    next_seq: UnsafeCell<u32>,
    drop_telemetry: UnsafeCell<SwarmDropTelemetry>,
}

pub const QEMU_CYW43439_MAX_ROLES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QemuCyw43439RxMeta {
    src_node: NodeId,
    dst_node: NodeId,
    lane: u8,
}

impl QemuCyw43439RxMeta {
    pub const fn new(src_node: NodeId, dst_node: NodeId, lane: u8) -> Self {
        Self {
            src_node,
            dst_node,
            lane,
        }
    }

    pub const fn src_node(self) -> NodeId {
        self.src_node
    }

    pub const fn dst_node(self) -> NodeId {
        self.dst_node
    }

    pub const fn lane(self) -> u8 {
        self.lane
    }

    pub const fn matches(self, src_node: NodeId, dst_node: NodeId, lane: u8) -> bool {
        self.src_node.raw() == src_node.raw()
            && self.dst_node.raw() == dst_node.raw()
            && self.lane == lane
    }
}

const QEMU_RX_META_VALID: u32 = 1 << 31;
const QEMU_RX_META_NODE_MASK: u32 = 0xff;
const QEMU_RX_META_DST_SHIFT: u32 = 8;
const QEMU_RX_META_LANE_SHIFT: u32 = 16;

static QEMU_CYW43439_RX_META: [AtomicU32; QEMU_CYW43439_MAX_ROLES] =
    [const { AtomicU32::new(0) }; QEMU_CYW43439_MAX_ROLES];

pub fn qemu_take_last_rx_meta(local_role: u8) -> Option<QemuCyw43439RxMeta> {
    let index = local_role as usize;
    if index >= QEMU_CYW43439_MAX_ROLES {
        return None;
    }
    let encoded = QEMU_CYW43439_RX_META[index].swap(0, Ordering::Relaxed);
    if encoded & QEMU_RX_META_VALID == 0 {
        return None;
    }
    Some(QemuCyw43439RxMeta::new(
        NodeId::new((encoded & QEMU_RX_META_NODE_MASK) as u16),
        NodeId::new(((encoded >> QEMU_RX_META_DST_SHIFT) & QEMU_RX_META_NODE_MASK) as u16),
        ((encoded >> QEMU_RX_META_LANE_SHIFT) & QEMU_RX_META_NODE_MASK) as u8,
    ))
}

fn set_qemu_last_rx_meta(local_role: u8, meta: QemuCyw43439RxMeta) {
    let index = local_role as usize;
    if index >= QEMU_CYW43439_MAX_ROLES
        || meta.src_node.raw() > QEMU_RX_META_NODE_MASK as u16
        || meta.dst_node.raw() > QEMU_RX_META_NODE_MASK as u16
    {
        return;
    }
    let encoded = QEMU_RX_META_VALID
        | meta.src_node.raw() as u32
        | ((meta.dst_node.raw() as u32) << QEMU_RX_META_DST_SHIFT)
        | ((meta.lane as u32) << QEMU_RX_META_LANE_SHIFT);
    QEMU_CYW43439_RX_META[index].store(encoded, Ordering::Relaxed);
}

impl QemuCyw43439Transport {
    pub const fn new(
        role0_node: NodeId,
        role1_node: NodeId,
        session_generation: u16,
        security: SwarmSecurity,
    ) -> Self {
        let mut role_nodes = [NodeId::new(0); QEMU_CYW43439_MAX_ROLES];
        role_nodes[0] = role0_node;
        role_nodes[1] = role1_node;
        Self::new_role_map(role_nodes, 2, session_generation, security)
    }

    pub const fn new_role_map(
        role_nodes: [NodeId; QEMU_CYW43439_MAX_ROLES],
        role_count: u8,
        session_generation: u16,
        security: SwarmSecurity,
    ) -> Self {
        Self {
            role_nodes,
            role_count,
            session_generation,
            security,
            next_seq: UnsafeCell::new(1),
            drop_telemetry: UnsafeCell::new(SwarmDropTelemetry::new()),
        }
    }

    pub fn drop_telemetry(&self) -> SwarmDropTelemetry {
        unsafe { *(&*self.drop_telemetry.get()) }
    }

    fn record_drop(&self, error: SwarmError) {
        unsafe { (&mut *self.drop_telemetry.get()).record(error) }
    }

    fn node_for_role(&self, role: u8) -> Result<NodeId, Cyw43439Error> {
        let index = role as usize;
        if role >= self.role_count || index >= self.role_nodes.len() {
            return Err(Cyw43439Error::Swarm(SwarmError::BadRole));
        }
        let node = self.role_nodes[index];
        if node.raw() == 0 {
            return Err(Cyw43439Error::Swarm(SwarmError::BadNode));
        }
        Ok(node)
    }

    fn node_for_open(&self, role: u8) -> NodeId {
        self.node_for_role(role).unwrap_or(self.role_nodes[0])
    }

    fn role_for_node(&self, node: NodeId) -> Result<u8, Cyw43439Error> {
        let mut role = 0usize;
        while role < self.role_count as usize && role < self.role_nodes.len() {
            if self.role_nodes[role] == node {
                return Ok(role as u8);
            }
            role += 1;
        }
        Err(Cyw43439Error::Swarm(SwarmError::BadNode))
    }

    fn alloc_seq(&self) -> u32 {
        unsafe {
            let current = *self.next_seq.get();
            let mut next = current.wrapping_add(1);
            if next == 0 {
                next = 1;
            }
            *self.next_seq.get() = next;
            current
        }
    }

    fn validate_received_frame(&self, rx: &CywRx, frame: &SwarmFrame) -> Result<(), Cyw43439Error> {
        if frame.dst_node() != rx.local_node {
            self.record_drop(SwarmError::BadNode);
            return Err(Cyw43439Error::Swarm(SwarmError::BadNode));
        }
        if let Err(error) = self.role_for_node(frame.src_node()) {
            if let Cyw43439Error::Swarm(swarm_error) = error {
                self.record_drop(swarm_error);
            }
            return Err(error);
        }
        if frame.session_id() != rx.session_id
            || frame.session_generation() != self.session_generation
        {
            self.record_drop(SwarmError::BadGeneration);
            return Err(Cyw43439Error::Swarm(SwarmError::BadGeneration));
        }
        if let Err(error) = frame.verify(self.security) {
            self.record_drop(error);
            return Err(Cyw43439Error::Swarm(error));
        }
        Ok(())
    }
}

pub struct CywTx {
    session_id: u32,
    local_node: NodeId,
}

pub struct CywRx {
    local_role: u8,
    session_id: u32,
    local_node: NodeId,
    current: Option<SwarmFrame>,
    requeued: Option<SwarmFrame>,
    replay: [Option<(NodeId, u32)>; QEMU_CYW43439_MAX_ROLES],
    last_replay_slot: Option<usize>,
    previous_replay_entry: Option<(NodeId, u32)>,
}

fn accept_replay(
    replay: &mut [Option<(NodeId, u32)>; QEMU_CYW43439_MAX_ROLES],
    frame: &SwarmFrame,
) -> Result<(usize, Option<(NodeId, u32)>), Cyw43439Error> {
    let src = frame.src_node();
    let seq = frame.seq();
    let mut empty = None;
    let mut index = 0usize;
    while index < replay.len() {
        match replay[index] {
            Some((node, highest)) if node == src => {
                if seq <= highest {
                    return Err(Cyw43439Error::Swarm(SwarmError::Replay));
                }
                let previous = replay[index];
                replay[index] = Some((src, seq));
                return Ok((index, previous));
            }
            None if empty.is_none() => empty = Some(index),
            _ => {}
        }
        index += 1;
    }
    let Some(index) = empty else {
        return Err(Cyw43439Error::Swarm(SwarmError::TableFull));
    };
    replay[index] = Some((src, seq));
    Ok((index, None))
}

impl Transport for QemuCyw43439Transport {
    type Error = Cyw43439Error;
    type Tx<'a>
        = CywTx
    where
        Self: 'a;
    type Rx<'a>
        = CywRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let local_node = self.node_for_open(local_role);
        (
            CywTx {
                session_id,
                local_node,
            },
            CywRx {
                local_role,
                session_id,
                local_node,
                current: None,
                requeued: None,
                replay: [None; QEMU_CYW43439_MAX_ROLES],
                last_replay_slot: None,
                previous_replay_entry: None,
            },
        )
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: Outgoing<'f>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        let peer_node = self.node_for_role(outgoing.peer())?;
        let frame = SwarmFrame::new(
            tx.local_node,
            peer_node,
            tx.session_id,
            self.session_generation,
            outgoing.lane(),
            outgoing.frame_label().raw(),
            self.alloc_seq(),
            0,
            outgoing.payload().as_bytes(),
            self.security,
        )?;
        let mut wire = [0u8; SWARM_FRAME_MAX_WIRE_LEN];
        let len = frame.encode_into(&mut wire)?;
        let result = DRIVER.send_frame(peer_node, &wire[..len]);
        if result.is_ok() {
            crate::substrate::exec::signal();
        }
        Poll::Ready(result)
    }

    fn cancel_send<'a>(&'a self, _tx: &'a mut Self::Tx<'a>) {}

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        rx.current = None;
        let frame = if let Some(frame) = rx.requeued.take() {
            frame
        } else {
            let mut wire = [0u8; SWARM_FRAME_MAX_WIRE_LEN];
            let len = match DRIVER.recv_frame(rx.local_node, &mut wire) {
                Ok(Some(len)) => len,
                Ok(None) => return Poll::Pending,
                Err(error) => return Poll::Ready(Err(error)),
            };
            SwarmFrame::decode(&wire[..len])?
        };

        if let Err(error) = self.validate_received_frame(rx, &frame) {
            return Poll::Ready(Err(error));
        }
        let (slot, previous) = match accept_replay(&mut rx.replay, &frame) {
            Ok(accepted) => accepted,
            Err(error) => {
                if let Cyw43439Error::Swarm(swarm_error) = error {
                    self.record_drop(swarm_error);
                }
                return Poll::Ready(Err(error));
            }
        };
        rx.last_replay_slot = Some(slot);
        rx.previous_replay_entry = previous;
        rx.current = Some(frame);
        let frame = rx
            .current
            .as_ref()
            .expect("current frame was just installed");
        set_qemu_last_rx_meta(
            rx.local_role,
            QemuCyw43439RxMeta::new(frame.src_node(), frame.dst_node(), frame.lane()),
        );
        let bytes: &'a [u8] = unsafe { &*(frame.payload() as *const [u8]) };
        Poll::Ready(Ok(Payload::new(bytes)))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.current.take() {
            if let Some(slot) = rx.last_replay_slot.take() {
                rx.replay[slot] = rx.previous_replay_entry;
            }
            rx.requeued = Some(frame);
        }
    }

    fn drain_events(&self, _emit: &mut dyn FnMut(TransportEvent)) {}

    fn recv_frame_hint<'a>(&'a self, _rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        if let Some(frame) = _rx.current.as_ref().or(_rx.requeued.as_ref()) {
            Some(FrameLabel::new(frame.label_hint()))
        } else {
            DRIVER.peek_label(_rx.local_node).map(FrameLabel::new)
        }
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, _interval_us: u32, _burst_bytes: u16) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SESSION_ID: u32 = 42;
    const TEST_SESSION_GENERATION: u16 = 7;
    const TEST_CREDENTIAL: crate::kernel::swarm::SwarmCredential =
        crate::kernel::swarm::SwarmCredential::new(0x4849_4241);
    const TEST_SECURITY: SwarmSecurity = SwarmSecurity::Secure(TEST_CREDENTIAL);

    #[test]
    fn cyw43439_status_bits_are_driver_visible_readiness_evidence() {
        let status = Cyw43439Status::from_bits(
            CYW_STATUS_RX_READY | CYW_STATUS_TX_READY | CYW_STATUS_JOINED,
        );

        assert_eq!(status.bits(), 0x07);
        assert!(status.rx_ready());
        assert!(status.tx_ready());
        assert!(status.joined());
        assert!(!status.queue_overflow());
    }

    #[test]
    fn cyw43439_status_reports_queue_overflow_without_rx_authority() {
        let status = Cyw43439Status::from_bits(CYW_STATUS_JOINED | CYW_STATUS_OVERFLOW);

        assert!(status.joined());
        assert!(status.queue_overflow());
        assert!(!status.rx_ready());
        assert!(!status.tx_ready());
    }

    #[test]
    fn qemu_rx_meta_matches_only_exact_source_destination_and_lane() {
        let coordinator = NodeId::new(1);
        let gateway = NodeId::new(4);
        let meta = QemuCyw43439RxMeta::new(coordinator, gateway, 22);

        assert!(meta.matches(coordinator, gateway, 22));
        assert!(!meta.matches(NodeId::new(2), gateway, 22));
        assert!(!meta.matches(coordinator, NodeId::new(5), 22));
        assert!(!meta.matches(coordinator, gateway, 23));
    }

    #[test]
    fn qemu_rx_meta_is_consumed_once() {
        let coordinator = NodeId::new(1);
        let gateway = NodeId::new(4);
        let meta = QemuCyw43439RxMeta::new(coordinator, gateway, 22);

        set_qemu_last_rx_meta(0, meta);

        assert_eq!(qemu_take_last_rx_meta(0), Some(meta));
        assert_eq!(qemu_take_last_rx_meta(0), None);
    }

    #[test]
    fn qemu_transport_rejects_bad_rx_frame_metadata_before_payload_authority() {
        let coordinator = NodeId::new(1);
        let gateway = NodeId::new(4);
        let outside_route = NodeId::new(6);
        let role_nodes = [
            coordinator,
            gateway,
            NodeId::new(0),
            NodeId::new(0),
            NodeId::new(0),
            NodeId::new(0),
        ];
        let transport = QemuCyw43439Transport::new_role_map(
            role_nodes,
            2,
            TEST_SESSION_GENERATION,
            TEST_SECURITY,
        );
        let (_, rx) = transport.open(1, TEST_SESSION_ID);

        let valid = SwarmFrame::new(
            coordinator,
            gateway,
            TEST_SESSION_ID,
            TEST_SESSION_GENERATION,
            22,
            44,
            1,
            0,
            b"ok",
            TEST_SECURITY,
        )
        .expect("valid qemu frame");
        assert_eq!(transport.validate_received_frame(&rx, &valid), Ok(()));

        let wrong_destination = SwarmFrame::new(
            coordinator,
            outside_route,
            TEST_SESSION_ID,
            TEST_SESSION_GENERATION,
            22,
            44,
            2,
            0,
            b"wrong dst",
            TEST_SECURITY,
        )
        .expect("wrong destination frame");
        assert_eq!(
            transport.validate_received_frame(&rx, &wrong_destination),
            Err(Cyw43439Error::Swarm(SwarmError::BadNode))
        );

        let unknown_source = SwarmFrame::new(
            outside_route,
            gateway,
            TEST_SESSION_ID,
            TEST_SESSION_GENERATION,
            22,
            44,
            3,
            0,
            b"wrong src",
            TEST_SECURITY,
        )
        .expect("unknown source frame");
        assert_eq!(
            transport.validate_received_frame(&rx, &unknown_source),
            Err(Cyw43439Error::Swarm(SwarmError::BadNode))
        );

        let stale_generation = SwarmFrame::new(
            coordinator,
            gateway,
            TEST_SESSION_ID,
            TEST_SESSION_GENERATION.wrapping_add(1),
            22,
            44,
            4,
            0,
            b"stale",
            TEST_SECURITY,
        )
        .expect("stale generation frame");
        assert_eq!(
            transport.validate_received_frame(&rx, &stale_generation),
            Err(Cyw43439Error::Swarm(SwarmError::BadGeneration))
        );

        let telemetry = transport.drop_telemetry();
        assert_eq!(telemetry.other(), 2);
        assert_eq!(telemetry.bad_generation(), 1);
    }

    #[test]
    fn cyw43439_embeds_official_picosdk_firmware_artifacts() {
        let firmware = std::fs::read("firmware/cyw43/w43439A0_7_95_49_00_firmware.bin")
            .expect("run scripts/extract_cyw43_firmware.py before firmware artifact tests");
        let clm = std::fs::read("firmware/cyw43/w43439A0_7_95_49_00_clm.bin")
            .expect("run scripts/extract_cyw43_firmware.py before firmware artifact tests");

        assert_eq!(firmware.len(), CYW43439_WIFI_FW_LEN);
        assert_eq!(clm.len(), CYW43439_CLM_LEN);
        assert_eq!(cyw_fnv1a32(&firmware), CYW43439_WIFI_FW_FNV1A32);
        assert_eq!(cyw_fnv1a32(&clm), CYW43439_CLM_FNV1A32);
    }
}
