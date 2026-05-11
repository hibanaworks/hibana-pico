use core::{
    cell::UnsafeCell,
    task::{Context, Poll},
};

use hibana::substrate::{
    Transport,
    transport::{FrameLabel, Outgoing, TransportError, advanced::TransportEvent},
    wire::Payload,
};

pub const SWARM_FRAME_VERSION: u8 = 1;
pub const SWARM_FRAME_PAYLOAD_CAPACITY: usize = 96;
pub const SWARM_AUTH_TAG_LEN: usize = 4;
pub const SWARM_FRAME_HEADER_LEN: usize = 28;
pub const SWARM_FRAME_MAX_WIRE_LEN: usize =
    SWARM_FRAME_HEADER_LEN + SWARM_FRAME_PAYLOAD_CAPACITY + SWARM_AUTH_TAG_LEN;
pub const SWARM_FRAGMENT_HEADER_LEN: usize = 6;
pub const SWARM_FRAGMENT_CHUNK_CAPACITY: usize =
    SWARM_FRAME_PAYLOAD_CAPACITY - SWARM_FRAGMENT_HEADER_LEN;
const HOST_SWARM_REPLAY_SOURCES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwarmError {
    BadNode,
    BadRole,
    BadGeneration,
    BadVersion,
    PayloadTooLarge,
    FrameTooSmall,
    LengthMismatch,
    AuthFailed,
    Replay,
    TableFull,
    QueueFull,
    QueueEmpty,
    Revoked,
    FragmentDuplicate,
    FragmentSetMismatch,
    FragmentIncomplete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwarmDropTelemetry {
    auth_failed: u16,
    replayed: u16,
    bad_generation: u16,
    revoked: u16,
    other: u16,
}

impl SwarmDropTelemetry {
    pub const fn new() -> Self {
        Self {
            auth_failed: 0,
            replayed: 0,
            bad_generation: 0,
            revoked: 0,
            other: 0,
        }
    }

    pub const fn auth_failed(self) -> u16 {
        self.auth_failed
    }

    pub const fn replayed(self) -> u16 {
        self.replayed
    }

    pub const fn bad_generation(self) -> u16 {
        self.bad_generation
    }

    pub const fn revoked(self) -> u16 {
        self.revoked
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.auth_failed
            .saturating_add(self.replayed)
            .saturating_add(self.bad_generation)
            .saturating_add(self.revoked)
            .saturating_add(self.other)
    }

    pub(crate) fn record(&mut self, error: SwarmError) {
        let slot = match error {
            SwarmError::AuthFailed => &mut self.auth_failed,
            SwarmError::Replay => &mut self.replayed,
            SwarmError::BadGeneration => &mut self.bad_generation,
            SwarmError::Revoked => &mut self.revoked,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

impl From<SwarmError> for TransportError {
    fn from(_value: SwarmError) -> Self {
        TransportError::Failed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(u16);

impl NodeId {
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwarmCredential {
    key: u32,
}

impl SwarmCredential {
    pub const fn new(key: u32) -> Self {
        Self { key }
    }

    pub const fn key(self) -> u32 {
        self.key
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwarmSecurity {
    Secure(SwarmCredential),
    InsecureDemoOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwarmFrame {
    version: u8,
    flags: u8,
    src_node: NodeId,
    dst_node: NodeId,
    rendezvous_id: u32,
    session_id: u32,
    session_generation: u16,
    lane: u8,
    label_hint: u8,
    seq: u32,
    ack: u32,
    payload_len: u16,
    payload: [u8; SWARM_FRAME_PAYLOAD_CAPACITY],
    auth_tag: [u8; SWARM_AUTH_TAG_LEN],
}

impl SwarmFrame {
    pub fn new(
        src_node: NodeId,
        dst_node: NodeId,
        session_id: u32,
        session_generation: u16,
        lane: u8,
        label_hint: u8,
        seq: u32,
        ack: u32,
        payload: &[u8],
        security: SwarmSecurity,
    ) -> Result<Self, SwarmError> {
        if payload.len() > SWARM_FRAME_PAYLOAD_CAPACITY {
            return Err(SwarmError::PayloadTooLarge);
        }
        let mut out = Self {
            version: SWARM_FRAME_VERSION,
            flags: match security {
                SwarmSecurity::Secure(_) => 1,
                SwarmSecurity::InsecureDemoOnly => 0,
            },
            src_node,
            dst_node,
            rendezvous_id: 0,
            session_id,
            session_generation,
            lane,
            label_hint,
            seq,
            ack,
            payload_len: payload.len() as u16,
            payload: [0; SWARM_FRAME_PAYLOAD_CAPACITY],
            auth_tag: [0; SWARM_AUTH_TAG_LEN],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        if let SwarmSecurity::Secure(credential) = security {
            out.auth_tag = out.compute_auth_tag(credential);
        }
        Ok(out)
    }

    pub const fn src_node(&self) -> NodeId {
        self.src_node
    }

    pub const fn dst_node(&self) -> NodeId {
        self.dst_node
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn session_id(&self) -> u32 {
        self.session_id
    }

    pub const fn lane(&self) -> u8 {
        self.lane
    }

    pub const fn label_hint(&self) -> u8 {
        self.label_hint
    }

    pub const fn seq(&self) -> u32 {
        self.seq
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len as usize]
    }

    pub fn verify(&self, security: SwarmSecurity) -> Result<(), SwarmError> {
        if self.version != SWARM_FRAME_VERSION {
            return Err(SwarmError::BadVersion);
        }
        match security {
            SwarmSecurity::Secure(credential) => {
                if self.auth_tag != self.compute_auth_tag(credential) {
                    return Err(SwarmError::AuthFailed);
                }
            }
            SwarmSecurity::InsecureDemoOnly => {}
        }
        Ok(())
    }

    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, SwarmError> {
        let payload_len = self.payload_len as usize;
        let wire_len = SWARM_FRAME_HEADER_LEN + payload_len + SWARM_AUTH_TAG_LEN;
        if out.len() < wire_len {
            return Err(SwarmError::FrameTooSmall);
        }
        out[0] = self.version;
        out[1] = self.flags;
        out[2..4].copy_from_slice(&self.src_node.raw().to_be_bytes());
        out[4..6].copy_from_slice(&self.dst_node.raw().to_be_bytes());
        out[6..10].copy_from_slice(&self.rendezvous_id.to_be_bytes());
        out[10..14].copy_from_slice(&self.session_id.to_be_bytes());
        out[14..16].copy_from_slice(&self.session_generation.to_be_bytes());
        out[16] = self.lane;
        out[17] = self.label_hint;
        out[18..22].copy_from_slice(&self.seq.to_be_bytes());
        out[22..26].copy_from_slice(&self.ack.to_be_bytes());
        out[26..28].copy_from_slice(&self.payload_len.to_be_bytes());
        out[28..28 + payload_len].copy_from_slice(self.payload());
        out[28 + payload_len..wire_len].copy_from_slice(&self.auth_tag);
        Ok(wire_len)
    }

    pub fn decode(input: &[u8]) -> Result<Self, SwarmError> {
        if input.len() < SWARM_FRAME_HEADER_LEN + SWARM_AUTH_TAG_LEN {
            return Err(SwarmError::FrameTooSmall);
        }
        let payload_len = u16::from_be_bytes([input[26], input[27]]) as usize;
        if payload_len > SWARM_FRAME_PAYLOAD_CAPACITY {
            return Err(SwarmError::PayloadTooLarge);
        }
        let expected_len = SWARM_FRAME_HEADER_LEN + payload_len + SWARM_AUTH_TAG_LEN;
        if input.len() != expected_len {
            return Err(SwarmError::LengthMismatch);
        }
        let mut payload = [0u8; SWARM_FRAME_PAYLOAD_CAPACITY];
        payload[..payload_len].copy_from_slice(&input[28..28 + payload_len]);
        let mut auth_tag = [0u8; SWARM_AUTH_TAG_LEN];
        auth_tag.copy_from_slice(&input[28 + payload_len..expected_len]);
        Ok(Self {
            version: input[0],
            flags: input[1],
            src_node: NodeId::new(u16::from_be_bytes([input[2], input[3]])),
            dst_node: NodeId::new(u16::from_be_bytes([input[4], input[5]])),
            rendezvous_id: u32::from_be_bytes([input[6], input[7], input[8], input[9]]),
            session_id: u32::from_be_bytes([input[10], input[11], input[12], input[13]]),
            session_generation: u16::from_be_bytes([input[14], input[15]]),
            lane: input[16],
            label_hint: input[17],
            seq: u32::from_be_bytes([input[18], input[19], input[20], input[21]]),
            ack: u32::from_be_bytes([input[22], input[23], input[24], input[25]]),
            payload_len: payload_len as u16,
            payload,
            auth_tag,
        })
    }

    fn compute_auth_tag(&self, credential: SwarmCredential) -> [u8; SWARM_AUTH_TAG_LEN] {
        let mut acc = credential.key() ^ 0x4849_4241;
        acc = acc.rotate_left(5) ^ self.src_node.raw() as u32;
        acc = acc.rotate_left(5) ^ self.dst_node.raw() as u32;
        acc = acc.rotate_left(5) ^ self.session_id;
        acc = acc.rotate_left(5) ^ self.session_generation as u32;
        acc = acc.rotate_left(5) ^ self.lane as u32;
        acc = acc.rotate_left(5) ^ self.label_hint as u32;
        acc = acc.rotate_left(5) ^ self.seq;
        acc = acc.rotate_left(5) ^ self.ack;
        for byte in self.payload() {
            acc = acc.rotate_left(3) ^ *byte as u32;
        }
        acc.to_be_bytes()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwarmFragment {
    fragment_id: u8,
    index: u8,
    count: u8,
    total_len: u16,
    chunk_len: u8,
    chunk: [u8; SWARM_FRAGMENT_CHUNK_CAPACITY],
}

impl SwarmFragment {
    pub fn fragment_count(payload_len: usize) -> Result<u8, SwarmError> {
        if payload_len == 0 {
            return Ok(0);
        }
        let count = payload_len.div_ceil(SWARM_FRAGMENT_CHUNK_CAPACITY);
        if count > u8::MAX as usize {
            return Err(SwarmError::PayloadTooLarge);
        }
        Ok(count as u8)
    }

    pub fn from_payload(fragment_id: u8, payload: &[u8], index: u8) -> Result<Self, SwarmError> {
        let count = Self::fragment_count(payload.len())?;
        if count == 0 || index >= count {
            return Err(SwarmError::LengthMismatch);
        }
        if payload.len() > u16::MAX as usize {
            return Err(SwarmError::PayloadTooLarge);
        }
        let start = index as usize * SWARM_FRAGMENT_CHUNK_CAPACITY;
        let end = core::cmp::min(start + SWARM_FRAGMENT_CHUNK_CAPACITY, payload.len());
        Self::new(
            fragment_id,
            index,
            count,
            payload.len() as u16,
            &payload[start..end],
        )
    }

    pub fn new(
        fragment_id: u8,
        index: u8,
        count: u8,
        total_len: u16,
        chunk: &[u8],
    ) -> Result<Self, SwarmError> {
        if count == 0 || index >= count || total_len == 0 {
            return Err(SwarmError::LengthMismatch);
        }
        if chunk.len() > SWARM_FRAGMENT_CHUNK_CAPACITY {
            return Err(SwarmError::PayloadTooLarge);
        }
        let expected_count = Self::fragment_count(total_len as usize)?;
        if count != expected_count {
            return Err(SwarmError::LengthMismatch);
        }
        let start = index as usize * SWARM_FRAGMENT_CHUNK_CAPACITY;
        let end = start + chunk.len();
        if start >= total_len as usize || end > total_len as usize {
            return Err(SwarmError::LengthMismatch);
        }
        if index + 1 < count && chunk.len() != SWARM_FRAGMENT_CHUNK_CAPACITY {
            return Err(SwarmError::LengthMismatch);
        }

        let mut out = Self {
            fragment_id,
            index,
            count,
            total_len,
            chunk_len: chunk.len() as u8,
            chunk: [0; SWARM_FRAGMENT_CHUNK_CAPACITY],
        };
        out.chunk[..chunk.len()].copy_from_slice(chunk);
        Ok(out)
    }

    pub const fn fragment_id(&self) -> u8 {
        self.fragment_id
    }

    pub const fn index(&self) -> u8 {
        self.index
    }

    pub const fn count(&self) -> u8 {
        self.count
    }

    pub const fn total_len(&self) -> u16 {
        self.total_len
    }

    pub fn chunk(&self) -> &[u8] {
        &self.chunk[..self.chunk_len as usize]
    }

    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, SwarmError> {
        let wire_len = SWARM_FRAGMENT_HEADER_LEN + self.chunk_len as usize;
        if out.len() < wire_len {
            return Err(SwarmError::FrameTooSmall);
        }
        out[0] = self.fragment_id;
        out[1] = self.index;
        out[2] = self.count;
        out[3..5].copy_from_slice(&self.total_len.to_be_bytes());
        out[5] = self.chunk_len;
        out[SWARM_FRAGMENT_HEADER_LEN..wire_len].copy_from_slice(self.chunk());
        Ok(wire_len)
    }

    pub fn decode(input: &[u8]) -> Result<Self, SwarmError> {
        if input.len() < SWARM_FRAGMENT_HEADER_LEN {
            return Err(SwarmError::FrameTooSmall);
        }
        let chunk_len = input[5] as usize;
        if chunk_len > SWARM_FRAGMENT_CHUNK_CAPACITY {
            return Err(SwarmError::PayloadTooLarge);
        }
        let expected_len = SWARM_FRAGMENT_HEADER_LEN + chunk_len;
        if input.len() != expected_len {
            return Err(SwarmError::LengthMismatch);
        }
        Self::new(
            input[0],
            input[1],
            input[2],
            u16::from_be_bytes([input[3], input[4]]),
            &input[SWARM_FRAGMENT_HEADER_LEN..],
        )
    }
}

#[derive(Clone, Copy)]
pub struct SwarmReassemblyBuffer<const CAP: usize, const FRAGMENTS: usize> {
    fragment_id: Option<u8>,
    total_len: usize,
    count: usize,
    received_count: usize,
    received: [bool; FRAGMENTS],
    bytes: [u8; CAP],
}

impl<const CAP: usize, const FRAGMENTS: usize> SwarmReassemblyBuffer<CAP, FRAGMENTS> {
    pub const fn new() -> Self {
        Self {
            fragment_id: None,
            total_len: 0,
            count: 0,
            received_count: 0,
            received: [false; FRAGMENTS],
            bytes: [0; CAP],
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub const fn is_complete(&self) -> bool {
        self.count != 0 && self.received_count == self.count
    }

    pub fn push(&mut self, fragment: SwarmFragment) -> Result<Option<&[u8]>, SwarmError> {
        let total_len = fragment.total_len() as usize;
        let count = fragment.count() as usize;
        let index = fragment.index() as usize;

        if total_len > CAP {
            return Err(SwarmError::PayloadTooLarge);
        }
        if count > FRAGMENTS || index >= FRAGMENTS {
            return Err(SwarmError::TableFull);
        }

        match self.fragment_id {
            Some(id)
                if id != fragment.fragment_id()
                    || self.total_len != total_len
                    || self.count != count =>
            {
                return Err(SwarmError::FragmentSetMismatch);
            }
            None => {
                self.fragment_id = Some(fragment.fragment_id());
                self.total_len = total_len;
                self.count = count;
            }
            _ => {}
        }

        if self.received[index] {
            return Err(SwarmError::FragmentDuplicate);
        }

        let start = index * SWARM_FRAGMENT_CHUNK_CAPACITY;
        let end = start + fragment.chunk().len();
        if end > self.total_len {
            return Err(SwarmError::LengthMismatch);
        }
        self.bytes[start..end].copy_from_slice(fragment.chunk());
        self.received[index] = true;
        self.received_count += 1;

        if self.is_complete() {
            Ok(Some(&self.bytes[..self.total_len]))
        } else {
            Ok(None)
        }
    }

    pub fn finish(&self) -> Result<&[u8], SwarmError> {
        if self.is_complete() {
            Ok(&self.bytes[..self.total_len])
        } else {
            Err(SwarmError::FragmentIncomplete)
        }
    }
}

impl<const CAP: usize, const FRAGMENTS: usize> Default for SwarmReassemblyBuffer<CAP, FRAGMENTS> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayWindow {
    highest_seq: Option<u32>,
}

impl ReplayWindow {
    pub const fn new() -> Self {
        Self { highest_seq: None }
    }

    pub fn accept(&mut self, seq: u32) -> Result<(), SwarmError> {
        if let Some(highest) = self.highest_seq
            && seq <= highest
        {
            return Err(SwarmError::Replay);
        }
        self.highest_seq = Some(seq);
        Ok(())
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeighborEntry {
    node_id: NodeId,
    role: u8,
    session_generation: u16,
    revoked: bool,
}

impl NeighborEntry {
    pub const fn new(node_id: NodeId, role: u8, session_generation: u16) -> Self {
        Self {
            node_id,
            role,
            session_generation,
            revoked: false,
        }
    }

    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub const fn role(&self) -> u8 {
        self.role
    }
}

pub struct NeighborTable<const N: usize> {
    entries: [Option<NeighborEntry>; N],
}

impl<const N: usize> NeighborTable<N> {
    pub const fn new() -> Self {
        Self { entries: [None; N] }
    }

    pub fn add(&mut self, entry: NeighborEntry) -> Result<(), SwarmError> {
        let index = self
            .entries
            .iter()
            .position(|slot| slot.is_none_or(|old| old.node_id == entry.node_id))
            .ok_or(SwarmError::TableFull)?;
        self.entries[index] = Some(entry);
        Ok(())
    }

    pub fn node_for_role(&self, role: u8) -> Result<NodeId, SwarmError> {
        self.entries
            .iter()
            .flatten()
            .find(|entry| entry.role == role && !entry.revoked)
            .map(|entry| entry.node_id)
            .ok_or(SwarmError::BadRole)
    }

    pub fn validate(&self, node_id: NodeId, generation: u16) -> Result<(), SwarmError> {
        let entry = self
            .entries
            .iter()
            .flatten()
            .find(|entry| entry.node_id == node_id)
            .ok_or(SwarmError::BadNode)?;
        if entry.revoked {
            return Err(SwarmError::Revoked);
        }
        if entry.session_generation != generation {
            return Err(SwarmError::BadGeneration);
        }
        Ok(())
    }

    pub fn revoke(&mut self, node_id: NodeId) -> Result<(), SwarmError> {
        let entry = self
            .entries
            .iter_mut()
            .flatten()
            .find(|entry| entry.node_id == node_id)
            .ok_or(SwarmError::BadNode)?;
        entry.revoked = true;
        Ok(())
    }

    pub fn revoke_generation(
        &mut self,
        node_id: NodeId,
        generation: u16,
    ) -> Result<(), SwarmError> {
        let entry = self
            .entries
            .iter_mut()
            .flatten()
            .find(|entry| entry.node_id == node_id)
            .ok_or(SwarmError::BadNode)?;
        if entry.revoked {
            return Err(SwarmError::Revoked);
        }
        if entry.session_generation != generation {
            return Err(SwarmError::BadGeneration);
        }
        entry.revoked = true;
        Ok(())
    }
}

impl<const N: usize> Default for NeighborTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct FrameQueue<const N: usize> {
    items: [Option<SwarmFrame>; N],
    head: usize,
    len: usize,
}

impl<const N: usize> FrameQueue<N> {
    const fn new() -> Self {
        Self {
            items: [None; N],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, frame: SwarmFrame) -> Result<(), SwarmError> {
        if self.len == N {
            return Err(SwarmError::QueueFull);
        }
        let index = (self.head + self.len) % N;
        self.items[index] = Some(frame);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, frame: SwarmFrame) -> Result<(), SwarmError> {
        if self.len == N {
            return Err(SwarmError::QueueFull);
        }
        if self.len != 0 {
            self.head = if self.head == 0 { N - 1 } else { self.head - 1 };
        }
        self.items[self.head] = Some(frame);
        self.len += 1;
        Ok(())
    }

    fn pop_for(&mut self, dst_node: NodeId) -> Result<SwarmFrame, SwarmError> {
        let mut offset = 0usize;
        while offset < self.len {
            let index = (self.head + offset) % N;
            if let Some(frame) = self.items[index]
                && frame.dst_node() == dst_node
            {
                let out = frame;
                self.remove_at_offset(offset);
                return Ok(out);
            }
            offset += 1;
        }
        Err(SwarmError::QueueEmpty)
    }

    fn peek_label_for(&self, dst_node: NodeId) -> Option<u8> {
        let mut offset = 0usize;
        while offset < self.len {
            let index = (self.head + offset) % N;
            if let Some(frame) = self.items[index]
                && frame.dst_node() == dst_node
            {
                return Some(frame.label_hint());
            }
            offset += 1;
        }
        None
    }

    fn remove_at_offset(&mut self, offset: usize) {
        let mut cursor = offset;
        while cursor + 1 < self.len {
            let from = (self.head + cursor + 1) % N;
            let to = (self.head + cursor) % N;
            self.items[to] = self.items[from];
            cursor += 1;
        }
        let tail = (self.head + self.len - 1) % N;
        self.items[tail] = None;
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
    }
}

pub struct HostSwarmMedium<const N: usize> {
    queue: UnsafeCell<FrameQueue<N>>,
    drop_telemetry: UnsafeCell<SwarmDropTelemetry>,
}

impl<const N: usize> HostSwarmMedium<N> {
    pub const fn new() -> Self {
        Self {
            queue: UnsafeCell::new(FrameQueue::new()),
            drop_telemetry: UnsafeCell::new(SwarmDropTelemetry::new()),
        }
    }

    pub fn send(&self, frame: SwarmFrame) -> Result<(), SwarmError> {
        unsafe { (&mut *self.queue.get()).push(frame) }
    }

    pub fn requeue_front(&self, frame: SwarmFrame) -> Result<(), SwarmError> {
        unsafe { (&mut *self.queue.get()).push_front(frame) }
    }

    pub fn peek_label(&self, node: NodeId) -> Option<u8> {
        unsafe { (&*self.queue.get()).peek_label_for(node) }
    }

    pub fn drop_for(&self, node: NodeId) -> Result<SwarmFrame, SwarmError> {
        self.take_for(node)
    }

    pub fn drop_telemetry(&self) -> SwarmDropTelemetry {
        unsafe { *(&*self.drop_telemetry.get()) }
    }

    fn record_drop(&self, error: SwarmError) {
        unsafe { (&mut *self.drop_telemetry.get()).record(error) }
    }

    fn take_for(&self, node: NodeId) -> Result<SwarmFrame, SwarmError> {
        unsafe { (&mut *self.queue.get()).pop_for(node) }
    }

    pub fn recv(
        &self,
        node: NodeId,
        replay: &mut ReplayWindow,
        security: SwarmSecurity,
    ) -> Result<SwarmFrame, SwarmError> {
        let frame = self.take_for(node)?;
        if let Err(error) = frame.verify(security) {
            self.record_drop(error);
            return Err(error);
        }
        if let Err(error) = replay.accept(frame.seq()) {
            self.record_drop(error);
            return Err(error);
        }
        Ok(frame)
    }
}

impl<const N: usize> Default for HostSwarmMedium<N> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HostSwarmTransport<'a, const N: usize> {
    medium: &'a HostSwarmMedium<N>,
    local_node: NodeId,
    peer_node: NodeId,
    session_generation: u16,
    security: SwarmSecurity,
    next_seq: UnsafeCell<u32>,
}

impl<'a, const N: usize> HostSwarmTransport<'a, N> {
    pub const fn new(
        medium: &'a HostSwarmMedium<N>,
        local_node: NodeId,
        peer_node: NodeId,
        session_generation: u16,
        security: SwarmSecurity,
    ) -> Self {
        Self {
            medium,
            local_node,
            peer_node,
            session_generation,
            security,
            next_seq: UnsafeCell::new(1),
        }
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
}

pub struct HostSwarmTx {
    session_id: u32,
}

pub struct HostSwarmRx {
    session_id: u32,
    current: Option<SwarmFrame>,
    replay: [Option<(NodeId, u32)>; HOST_SWARM_REPLAY_SOURCES],
    last_replay_slot: Option<usize>,
    previous_replay_entry: Option<(NodeId, u32)>,
}

fn accept_transport_replay(
    replay: &mut [Option<(NodeId, u32)>; HOST_SWARM_REPLAY_SOURCES],
    frame: &SwarmFrame,
) -> Result<(usize, Option<(NodeId, u32)>), SwarmError> {
    let src = frame.src_node();
    let seq = frame.seq();
    let mut empty = None;
    let mut index = 0usize;
    while index < replay.len() {
        match replay[index] {
            Some((node, highest)) if node == src => {
                if seq <= highest {
                    return Err(SwarmError::Replay);
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
        return Err(SwarmError::TableFull);
    };
    replay[index] = Some((src, seq));
    Ok((index, None))
}

impl<const N: usize> Transport for HostSwarmTransport<'_, N> {
    type Error = SwarmError;
    type Tx<'a>
        = HostSwarmTx
    where
        Self: 'a;
    type Rx<'a>
        = HostSwarmRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, _local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            HostSwarmTx { session_id },
            HostSwarmRx {
                session_id,
                current: None,
                replay: [None; HOST_SWARM_REPLAY_SOURCES],
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
        let frame = SwarmFrame::new(
            self.local_node,
            self.peer_node,
            tx.session_id,
            self.session_generation,
            outgoing.lane(),
            outgoing.frame_label().raw(),
            self.alloc_seq(),
            0,
            outgoing.payload().as_bytes(),
            self.security,
        )?;
        Poll::Ready(self.medium.send(frame))
    }

    fn cancel_send<'a>(&'a self, _tx: &'a mut Self::Tx<'a>) {}

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        rx.current = None;
        let frame = match self.medium.take_for(self.local_node) {
            Ok(frame) => frame,
            Err(SwarmError::QueueEmpty) => return Poll::Pending,
            Err(err) => return Poll::Ready(Err(err)),
        };
        if frame.session_id() != rx.session_id
            || frame.session_generation() != self.session_generation
        {
            self.medium.record_drop(SwarmError::BadGeneration);
            return Poll::Ready(Err(SwarmError::BadGeneration));
        }
        if let Err(err) = frame.verify(self.security) {
            self.medium.record_drop(err);
            return Poll::Ready(Err(err));
        }
        let (slot, previous) = match accept_transport_replay(&mut rx.replay, &frame) {
            Ok(accepted) => accepted,
            Err(err) => {
                self.medium.record_drop(err);
                return Poll::Ready(Err(err));
            }
        };
        rx.last_replay_slot = Some(slot);
        rx.previous_replay_entry = previous;
        rx.current = Some(frame);
        let frame = rx
            .current
            .as_ref()
            .expect("current frame was just installed");
        let bytes: &'a [u8] = unsafe { &*(frame.payload() as *const [u8]) };
        Poll::Ready(Ok(Payload::new(bytes)))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.current.take() {
            if let Some(slot) = rx.last_replay_slot.take() {
                rx.replay[slot] = rx.previous_replay_entry;
            }
            self.medium
                .requeue_front(frame)
                .expect("requeue must preserve the previously received swarm frame");
        }
    }

    fn drain_events(&self, _emit: &mut dyn FnMut(TransportEvent)) {}

    fn recv_frame_hint<'a>(&'a self, _rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        self.medium.peek_label(self.local_node).map(FrameLabel::new)
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, _interval_us: u32, _burst_bytes: u16) {}
}

pub struct HostSwarmRoleTransport<'a, const N: usize, const R: usize> {
    medium: &'a HostSwarmMedium<N>,
    local_node: NodeId,
    role_nodes: [NodeId; R],
    role_count: u8,
    session_generation: u16,
    security: SwarmSecurity,
    next_seq: UnsafeCell<u32>,
}

impl<'a, const N: usize, const R: usize> HostSwarmRoleTransport<'a, N, R> {
    pub const fn new(
        medium: &'a HostSwarmMedium<N>,
        local_node: NodeId,
        role_nodes: [NodeId; R],
        role_count: u8,
        session_generation: u16,
        security: SwarmSecurity,
    ) -> Self {
        Self {
            medium,
            local_node,
            role_nodes,
            role_count,
            session_generation,
            security,
            next_seq: UnsafeCell::new(1),
        }
    }

    fn node_for_role(&self, role: u8) -> Result<NodeId, SwarmError> {
        let index = role as usize;
        if role >= self.role_count || index >= self.role_nodes.len() {
            return Err(SwarmError::BadRole);
        }
        let node = self.role_nodes[index];
        if node.raw() == 0 {
            return Err(SwarmError::BadNode);
        }
        Ok(node)
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
}

impl<const N: usize, const R: usize> Transport for HostSwarmRoleTransport<'_, N, R> {
    type Error = SwarmError;
    type Tx<'a>
        = HostSwarmTx
    where
        Self: 'a;
    type Rx<'a>
        = HostSwarmRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(&'a self, _local_role: u8, session_id: u32) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            HostSwarmTx { session_id },
            HostSwarmRx {
                session_id,
                current: None,
                replay: [None; HOST_SWARM_REPLAY_SOURCES],
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
            self.local_node,
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
        Poll::Ready(self.medium.send(frame))
    }

    fn cancel_send<'a>(&'a self, _tx: &'a mut Self::Tx<'a>) {}

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Payload<'a>, Self::Error>> {
        rx.current = None;
        let frame = match self.medium.take_for(self.local_node) {
            Ok(frame) => frame,
            Err(SwarmError::QueueEmpty) => return Poll::Pending,
            Err(err) => return Poll::Ready(Err(err)),
        };
        if frame.session_id() != rx.session_id
            || frame.session_generation() != self.session_generation
        {
            self.medium.record_drop(SwarmError::BadGeneration);
            return Poll::Ready(Err(SwarmError::BadGeneration));
        }
        if let Err(err) = frame.verify(self.security) {
            self.medium.record_drop(err);
            return Poll::Ready(Err(err));
        }
        let (slot, previous) = match accept_transport_replay(&mut rx.replay, &frame) {
            Ok(accepted) => accepted,
            Err(err) => {
                self.medium.record_drop(err);
                return Poll::Ready(Err(err));
            }
        };
        rx.last_replay_slot = Some(slot);
        rx.previous_replay_entry = previous;
        rx.current = Some(frame);
        let frame = rx
            .current
            .as_ref()
            .expect("current frame was just installed");
        let bytes: &'a [u8] = unsafe { &*(frame.payload() as *const [u8]) };
        Poll::Ready(Ok(Payload::new(bytes)))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.current.take() {
            if let Some(slot) = rx.last_replay_slot.take() {
                rx.replay[slot] = rx.previous_replay_entry;
            }
            self.medium
                .requeue_front(frame)
                .expect("requeue must preserve the previously received swarm frame");
        }
    }

    fn drain_events(&self, _emit: &mut dyn FnMut(TransportEvent)) {}

    fn recv_frame_hint<'a>(&'a self, _rx: &'a Self::Rx<'a>) -> Option<FrameLabel> {
        self.medium.peek_label(self.local_node).map(FrameLabel::new)
    }

    fn metrics(&self) -> Self::Metrics {}

    fn apply_pacing_update(&self, _interval_us: u32, _burst_bytes: u16) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BleProvisioningBundle {
    node_id: NodeId,
    credential: SwarmCredential,
    role_mask: u16,
    wifi_credentials_installed: bool,
}

impl BleProvisioningBundle {
    pub const fn new(node_id: NodeId, credential: SwarmCredential, role_mask: u16) -> Self {
        Self {
            node_id,
            credential,
            role_mask,
            wifi_credentials_installed: false,
        }
    }

    pub const fn with_wifi_credentials(mut self) -> Self {
        self.wifi_credentials_installed = true;
        self
    }

    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub const fn credential(&self) -> SwarmCredential {
        self.credential
    }

    pub const fn role_mask(&self) -> u16 {
        self.role_mask
    }

    pub const fn wifi_credentials_installed(&self) -> bool {
        self.wifi_credentials_installed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvisioningRecord {
    node_id: NodeId,
    credential: SwarmCredential,
    role_mask: u16,
    wifi_configured: bool,
    join_triggered: bool,
}

impl ProvisioningRecord {
    pub const fn new(node_id: NodeId, credential: SwarmCredential, role_mask: u16) -> Self {
        Self {
            node_id,
            credential,
            role_mask,
            wifi_configured: false,
            join_triggered: false,
        }
    }

    pub const fn from_ble(bundle: BleProvisioningBundle) -> Self {
        Self {
            node_id: bundle.node_id(),
            credential: bundle.credential(),
            role_mask: bundle.role_mask(),
            wifi_configured: bundle.wifi_credentials_installed(),
            join_triggered: false,
        }
    }

    pub fn install_wifi_credentials(&mut self) {
        self.wifi_configured = true;
    }

    pub fn trigger_join(&mut self) -> Result<(), SwarmError> {
        if !self.wifi_configured {
            return Err(SwarmError::BadNode);
        }
        self.join_triggered = true;
        Ok(())
    }

    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub const fn role_mask(&self) -> u16 {
        self.role_mask
    }

    pub const fn credential(&self) -> SwarmCredential {
        self.credential
    }

    pub const fn wifi_configured(&self) -> bool {
        self.wifi_configured
    }

    pub const fn join_triggered(&self) -> bool {
        self.join_triggered
    }
}
