#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
};
use core::{assert, assert_eq};
use hibana_pico::appkit;

pub struct BakerPlacement;
pub struct BakerArtifacts;

pub struct DriverImage;
pub struct EngineImage;

impl DriverImage {
    pub const fn new() -> Self {
        Self
    }
}

impl EngineImage {
    pub const fn new() -> Self {
        Self
    }
}

mod rp2040_sio {
    use core::cell::Cell;

    use hibana_pico::appkit::CarrierKind;

    pub const SIO: CarrierKind = CarrierKind::new(2040);
    const SIO_FRAME_MAGIC: u32 = 0x4849_5301;
    const SIO_FRAME_BYTES: usize = 128;
    const SIO_FRAME_HEADER_WORDS: usize = 4;
    const SIO_FRAME_PAYLOAD_WORDS: usize = (SIO_FRAME_BYTES + 3) / 4;
    const SIO_FRAME_WORDS: usize = SIO_FRAME_HEADER_WORDS + SIO_FRAME_PAYLOAD_WORDS;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SioTransport;

    impl SioTransport {
        pub const fn new() -> Self {
            Self
        }

        pub fn open_with_session_xor<'a>(
            &'a self,
            port: hibana::integration::transport::PortOpen,
            session_xor: u32,
        ) -> (SioTx, SioRx) {
            let local_role = port.local_role();
            let session_id = port.session_id().raw() ^ session_xor;
            let lane = port.lane().as_wire();
            fifo::clear_errors();
            (
                SioTx {
                    local_role,
                    session_id,
                    sent_frames: 0,
                    pending: None,
                },
                SioRx::new(local_role, session_id, lane),
            )
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[inline(always)]
    fn signal_peer() {
        unsafe {
            core::arch::asm!("dsb sy", "sev", options(nostack, preserves_flags));
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    #[inline(always)]
    fn signal_peer() {}

    #[derive(Debug)]
    struct PendingTxFrame {
        peer_role: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        lane: u8,
        len: usize,
        word_index: usize,
        bytes: [u8; SIO_FRAME_BYTES],
    }

    impl PendingTxFrame {
        fn new(
            peer_role: u8,
            frame_label: hibana::integration::transport::FrameLabel,
            lane: u8,
            bytes: &[u8],
        ) -> Result<Self, hibana::integration::transport::TransportError> {
            if bytes.len() > SIO_FRAME_BYTES {
                return Err(hibana::integration::transport::TransportError::Failed);
            }

            let mut frame = Self {
                peer_role,
                frame_label,
                lane,
                len: bytes.len(),
                word_index: 0,
                bytes: [0; SIO_FRAME_BYTES],
            };
            frame.bytes[..bytes.len()].copy_from_slice(bytes);
            Ok(frame)
        }

        fn total_words(&self) -> usize {
            SIO_FRAME_HEADER_WORDS + payload_word_count(self.len)
        }

        fn word(&self, session_id: u32, local_role: u8, index: usize) -> u32 {
            match index {
                0 => SIO_FRAME_MAGIC,
                1 => session_id,
                2 => encode_meta(local_role, self.peer_role, self.frame_label, self.len),
                3 => u32::from(self.lane),
                _ => pack_payload_word(
                    &self.bytes[..self.len],
                    (index - SIO_FRAME_HEADER_WORDS) * 4,
                ),
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct SioTx {
        local_role: u8,
        session_id: u32,
        sent_frames: u16,
        pending: Option<PendingTxFrame>,
    }

    #[derive(Debug)]
    pub struct SioRx {
        local_role: u8,
        lane: u8,
        session_id: u32,
        requeued: bool,
        delivered: bool,
        pending_logged: bool,
        pending_polls: u32,
        sender_role: u8,
        frame_label: Option<hibana::integration::transport::FrameLabel>,
        hint_frame_label: Cell<Option<hibana::integration::transport::FrameLabel>>,
        len: usize,
        bytes: [u8; SIO_FRAME_BYTES],
    }

    impl SioRx {
        const fn new(local_role: u8, session_id: u32, lane: u8) -> Self {
            Self {
                local_role,
                lane,
                session_id,
                requeued: false,
                delivered: false,
                pending_logged: false,
                pending_polls: 0,
                sender_role: 0,
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; SIO_FRAME_BYTES],
            }
        }

        fn frame_header(&self) -> hibana::integration::transport::FrameHeader {
            hibana::integration::transport::FrameHeader::new(
                hibana::integration::ids::SessionId::new(self.session_id),
                hibana::integration::ids::Lane::new(self.lane as u32),
                self.sender_role,
                self.local_role,
                self.frame_label
                    .unwrap_or_else(|| hibana::integration::transport::FrameLabel::new(0)),
            )
        }

        fn incoming<'a>(&'a self) -> hibana::integration::transport::Incoming<'a> {
            hibana::integration::transport::Incoming::new(
                self.frame_header(),
                hibana::integration::wire::Payload::new(&self.bytes[..self.len]),
            )
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct DecodedSioFrame {
        session_id: u32,
        sender_role: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        lane: u8,
        len: usize,
        bytes: [u8; SIO_FRAME_BYTES],
    }

    #[derive(Clone, Copy, Debug)]
    struct SioRxAccumulator {
        words: [u32; SIO_FRAME_WORDS],
        word_count: usize,
        expected_words: usize,
    }

    impl SioRxAccumulator {
        const EMPTY: Self = Self {
            words: [0; SIO_FRAME_WORDS],
            word_count: 0,
            expected_words: 0,
        };

        fn reset(&mut self) {
            self.word_count = 0;
            self.expected_words = 0;
        }

        fn is_partial(&self) -> bool {
            self.word_count != 0
        }

        fn push_word(
            &mut self,
            local_role: u8,
            word: u32,
        ) -> Result<Option<DecodedSioFrame>, hibana::integration::transport::TransportError>
        {
            if self.word_count >= SIO_FRAME_WORDS {
                self.reset();
                return Err(hibana::integration::transport::TransportError::Failed);
            }

            self.words[self.word_count] = word;
            self.word_count += 1;

            if self.word_count == 1 && word != SIO_FRAME_MAGIC {
                self.reset();
                return Err(hibana::integration::transport::TransportError::Failed);
            }

            if self.word_count == SIO_FRAME_HEADER_WORDS {
                let (sender_role, peer_role, _, len) = decode_meta(self.words[2]);
                if peer_role != local_role || sender_role == local_role || len > SIO_FRAME_BYTES {
                    self.reset();
                    return Err(hibana::integration::transport::TransportError::Failed);
                }
                self.expected_words = SIO_FRAME_HEADER_WORDS + payload_word_count(len);
            }

            if self.expected_words == 0 || self.word_count < self.expected_words {
                return Ok(None);
            }

            let session_id = self.words[1];
            let meta_word = self.words[2];
            let lane_word = self.words[3];
            let (sender_role, _, frame_label, len) = decode_meta(meta_word);
            let lane = lane_word as u8;
            let mut bytes = [0; SIO_FRAME_BYTES];
            let mut offset = 0usize;
            while offset < len {
                let word_index = SIO_FRAME_HEADER_WORDS + offset / 4;
                unpack_payload_word(self.words[word_index], &mut bytes[..len], offset);
                offset += 4;
            }
            self.reset();

            Ok(Some(DecodedSioFrame {
                session_id,
                sender_role,
                frame_label,
                lane,
                len,
                bytes,
            }))
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[derive(Clone, Copy)]
    struct BufferedFrame {
        present: bool,
        session_id: u32,
        sender_role: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        len: usize,
        bytes: [u8; SIO_FRAME_BYTES],
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    impl BufferedFrame {
        const EMPTY: Self = Self {
            present: false,
            session_id: 0,
            sender_role: 0,
            frame_label: hibana::integration::transport::FrameLabel::new(0),
            len: 0,
            bytes: [0; SIO_FRAME_BYTES],
        };
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const SIO_DEMUX_LANES: usize = 2;

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut SIO_DEMUX_CORE0: [BufferedFrame; SIO_DEMUX_LANES] =
        [BufferedFrame::EMPTY; SIO_DEMUX_LANES];
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut SIO_DEMUX_CORE1: [BufferedFrame; SIO_DEMUX_LANES] =
        [BufferedFrame::EMPTY; SIO_DEMUX_LANES];

    static mut SIO_RX_ACCUM_CORE0: SioRxAccumulator = SioRxAccumulator::EMPTY;
    static mut SIO_RX_ACCUM_CORE1: SioRxAccumulator = SioRxAccumulator::EMPTY;

    fn rx_accumulator(local_role: u8) -> *mut SioRxAccumulator {
        if local_role == 0 {
            core::ptr::addr_of_mut!(SIO_RX_ACCUM_CORE0)
        } else {
            core::ptr::addr_of_mut!(SIO_RX_ACCUM_CORE1)
        }
    }

    #[inline(always)]
    pub fn core_id() -> u32 {
        #[cfg(all(target_arch = "arm", target_os = "none"))]
        {
            const SIO_CPUID: *const u32 = 0xd000_0000 as *const u32;
            unsafe { core::ptr::read_volatile(SIO_CPUID) & 1 }
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        {
            0
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    mod fifo {
        use core::ptr::{read_volatile, write_volatile};

        const SIO_BASE: usize = 0xd000_0000;
        const SIO_FIFO_ST: *const u32 = (SIO_BASE + 0x50) as *const u32;
        const SIO_FIFO_ST_WRITE: *mut u32 = (SIO_BASE + 0x50) as *mut u32;
        const SIO_FIFO_WR: *mut u32 = (SIO_BASE + 0x54) as *mut u32;
        const SIO_FIFO_RD: *const u32 = (SIO_BASE + 0x58) as *const u32;
        const FIFO_VLD: u32 = 1 << 0;
        const FIFO_RDY: u32 = 1 << 1;
        const FIFO_WOF: u32 = 1 << 2;
        const FIFO_ROE: u32 = 1 << 3;

        #[inline(always)]
        pub fn ready_to_recv() -> bool {
            status() & FIFO_VLD != 0
        }

        #[inline(always)]
        pub fn ready_to_send() -> bool {
            status() & FIFO_RDY != 0
        }

        #[inline(always)]
        pub fn status() -> u32 {
            unsafe { read_volatile(SIO_FIFO_ST) }
        }

        #[inline(always)]
        pub fn clear_errors() {
            unsafe {
                write_volatile(SIO_FIFO_ST_WRITE, FIFO_WOF | FIFO_ROE);
            }
        }

        #[inline(always)]
        pub fn try_push(word: u32) -> bool {
            if !ready_to_send() {
                return false;
            }
            unsafe {
                write_volatile(SIO_FIFO_WR, word);
            }
            true
        }

        #[inline(always)]
        pub fn try_pop() -> Option<u32> {
            if ready_to_recv() {
                Some(unsafe { read_volatile(SIO_FIFO_RD) })
            } else {
                None
            }
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    mod fifo {
        #[inline(always)]
        pub fn status() -> u32 {
            0
        }

        #[inline(always)]
        pub fn clear_errors() {}

        #[inline(always)]
        pub fn try_push(word: u32) -> bool {
            core::hint::black_box(word);
            false
        }

        #[inline(always)]
        pub fn try_pop() -> Option<u32> {
            None
        }
    }

    fn encode_meta(
        sender_role: u8,
        peer_role: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        len: usize,
    ) -> u32 {
        ((frame_label.raw() as u32) << 24)
            | ((peer_role as u32) << 16)
            | ((sender_role as u32) << 8)
            | (len as u32)
    }

    fn decode_meta(word: u32) -> (u8, u8, hibana::integration::transport::FrameLabel, usize) {
        let frame_label = hibana::integration::transport::FrameLabel::new((word >> 24) as u8);
        let peer_role = ((word >> 16) & 0xff) as u8;
        let sender_role = ((word >> 8) & 0xff) as u8;
        let len = (word & 0xff) as usize;
        (sender_role, peer_role, frame_label, len)
    }

    fn payload_word_count(len: usize) -> usize {
        (len + 3) / 4
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn store_demux_frame(local_role: u8, frame: &DecodedSioFrame) -> bool {
        let lane = frame.lane;
        if lane as usize >= SIO_DEMUX_LANES {
            return false;
        }
        unsafe {
            let table = if local_role == 0 {
                core::ptr::addr_of_mut!(SIO_DEMUX_CORE0)
            } else {
                core::ptr::addr_of_mut!(SIO_DEMUX_CORE1)
            };
            let slot = &mut (*table)[lane as usize];
            if slot.present {
                return false;
            }
            slot.present = true;
            slot.session_id = frame.session_id;
            slot.sender_role = frame.sender_role;
            slot.frame_label = frame.frame_label;
            slot.len = frame.len;
            slot.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
            true
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn take_demux_frame(local_role: u8, session_id: u32, lane: u8) -> Option<BufferedFrame> {
        if lane as usize >= SIO_DEMUX_LANES {
            return None;
        }
        unsafe {
            let table = if local_role == 0 {
                core::ptr::addr_of_mut!(SIO_DEMUX_CORE0)
            } else {
                core::ptr::addr_of_mut!(SIO_DEMUX_CORE1)
            };
            let slot = &mut (*table)[lane as usize];
            if !slot.present || slot.session_id != session_id {
                return None;
            }
            let frame = *slot;
            *slot = BufferedFrame::EMPTY;
            Some(frame)
        }
    }

    fn pack_payload_word(bytes: &[u8], offset: usize) -> u32 {
        let mut word = 0u32;
        let mut idx = 0usize;
        while idx < 4 {
            let source = offset + idx;
            if source < bytes.len() {
                word |= (bytes[source] as u32) << (idx * 8);
            }
            idx += 1;
        }
        word
    }

    fn unpack_payload_word(word: u32, bytes: &mut [u8], offset: usize) {
        let mut idx = 0usize;
        while idx < 4 {
            let target = offset + idx;
            if target < bytes.len() {
                bytes[target] = ((word >> (idx * 8)) & 0xff) as u8;
            }
            idx += 1;
        }
    }

    fn trace_frame(event: u8, local_role: u8, peer_role: u8, len: usize, frame_label: u8) -> u32 {
        0x5000_0000
            | ((event as u32) << 24)
            | ((local_role as u32) << 20)
            | ((peer_role as u32) << 16)
            | (((len as u32) & 0xff) << 8)
            | frame_label as u32
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pending_sio_frame_materializes_header_and_payload_words() {
            let label = hibana::integration::transport::FrameLabel::new(7);
            let frame = match PendingTxFrame::new(1, label, 3, b"abcd") {
                Ok(frame) => frame,
                Err(error) => panic!("{error:?}"),
            };

            assert_eq!(frame.total_words(), SIO_FRAME_HEADER_WORDS + 1);
            assert_eq!(frame.word(42, 0, 0), SIO_FRAME_MAGIC);
            assert_eq!(frame.word(42, 0, 1), 42);
            assert_eq!(frame.word(42, 0, 2), encode_meta(0, 1, label, 4));
            assert_eq!(frame.word(42, 0, 3), 3);
            assert_eq!(frame.word(42, 0, 4), u32::from_le_bytes(*b"abcd"));
        }

        #[test]
        fn sio_rx_accumulator_is_local_role_owned_across_lanes() {
            let label = hibana::integration::transport::FrameLabel::new(9);
            let frame = match PendingTxFrame::new(1, label, 3, b"abcd") {
                Ok(frame) => frame,
                Err(error) => panic!("{error:?}"),
            };
            let mut accumulator = SioRxAccumulator::EMPTY;

            assert!(
                accumulator
                    .push_word(1, frame.word(42, 0, 0))
                    .expect("magic accepted")
                    .is_none()
            );
            assert!(
                accumulator
                    .push_word(1, frame.word(42, 0, 1))
                    .expect("session accepted")
                    .is_none()
            );
            assert!(accumulator.is_partial());

            assert!(
                accumulator
                    .push_word(1, frame.word(42, 0, 2))
                    .expect("metadata accepted")
                    .is_none()
            );
            assert!(
                accumulator
                    .push_word(1, frame.word(42, 0, 3))
                    .expect("lane accepted")
                    .is_none()
            );
            let decoded = match accumulator
                .push_word(1, frame.word(42, 0, 4))
                .expect("payload accepted")
            {
                Some(decoded) => decoded,
                None => panic!("complete frame must be decoded after payload word"),
            };

            assert_eq!(decoded.session_id, 42);
            assert_eq!(decoded.sender_role, 0);
            assert_eq!(decoded.frame_label, label);
            assert_eq!(decoded.lane, 3);
            assert_eq!(&decoded.bytes[..decoded.len], b"abcd");
            assert!(!accumulator.is_partial());
        }

        #[test]
        fn delivered_sio_payload_emits_route_hint_once() {
            let transport = SioTransport::new();
            let label = hibana::integration::transport::FrameLabel::new(7);
            let mut rx = SioRx::new(0, 42, 1);
            rx.frame_label = Some(label);
            rx.hint_frame_label.set(Some(label));
            rx.delivered = true;

            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::peek_recv_frame(
                    &transport, &mut rx,
                )
                .map(|header| header.label),
                Some(label)
            );
            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::peek_recv_frame(
                    &transport, &mut rx,
                )
                .map(|header| header.label),
                Some(label)
            );
        }

        #[test]
        fn staged_sio_payload_emits_route_hint_before_delivery() {
            let transport = SioTransport::new();
            let label = hibana::integration::transport::FrameLabel::new(7);
            let mut rx = SioRx::new(0, 42, 1);
            rx.frame_label = Some(label);
            rx.hint_frame_label.set(Some(label));
            rx.delivered = false;

            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::peek_recv_frame(
                    &transport, &mut rx,
                )
                .map(|header| header.label),
                Some(label)
            );
            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::peek_recv_frame(
                    &transport, &mut rx,
                )
                .map(|header| header.label),
                Some(label)
            );
        }
    }

    impl hibana::integration::transport::Transport for SioTransport {
        type Error = hibana::integration::transport::TransportError;
        type Tx<'a>
            = SioTx
        where
            Self: 'a;
        type Rx<'a>
            = SioRx
        where
            Self: 'a;
        fn open<'a>(
            &'a self,
            port: hibana::integration::transport::PortOpen,
        ) -> (Self::Tx<'a>, Self::Rx<'a>) {
            let local_role = port.local_role();
            let session_id = port.session_id().raw();
            let lane = port.lane().as_wire();
            fifo::clear_errors();
            (
                SioTx {
                    local_role,
                    session_id,
                    sent_frames: 0,
                    pending: None,
                },
                SioRx::new(local_role, session_id, lane),
            )
        }

        fn poll_send<'a, 'f>(
            &self,
            tx: &'a mut Self::Tx<'a>,
            outgoing: hibana::integration::transport::Outgoing<'f>,
            context: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>>
        where
            'a: 'f,
        {
            core::hint::black_box(core::ptr::addr_of!(*context));
            let bytes = outgoing.payload().as_bytes();
            if outgoing.peer() == tx.local_role {
                super::record_choreofs_sio_trace(trace_frame(
                    6,
                    tx.local_role,
                    outgoing.peer(),
                    bytes.len(),
                    outgoing.frame_label().raw(),
                ));
                tx.sent_frames = tx.sent_frames.saturating_add(1);
                return core::task::Poll::Ready(Ok(()));
            }
            if bytes.len() > SIO_FRAME_BYTES {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
                ));
            }

            if tx.pending.is_none() {
                super::record_choreofs_sio_trace(trace_frame(
                    1,
                    tx.local_role,
                    outgoing.peer(),
                    bytes.len(),
                    outgoing.frame_label().raw(),
                ));
                super::record_choreofs_sio_trace(trace_frame(
                    11,
                    tx.local_role,
                    outgoing.peer(),
                    outgoing.lane() as usize,
                    outgoing.frame_label().raw(),
                ));
                let pending = match PendingTxFrame::new(
                    outgoing.peer(),
                    outgoing.frame_label(),
                    outgoing.lane(),
                    bytes,
                ) {
                    Ok(pending) => pending,
                    Err(error) => return core::task::Poll::Ready(Err(error)),
                };
                tx.pending = Some(pending);
            }

            let mut completed = false;
            while let Some(pending) = tx.pending.as_mut() {
                if pending.word_index >= pending.total_words() {
                    completed = true;
                    break;
                }

                let word = pending.word(tx.session_id, tx.local_role, pending.word_index);
                if !fifo::try_push(word) {
                    context.waker().wake_by_ref();
                    return core::task::Poll::Pending;
                }
                pending.word_index += 1;
            }

            if !completed {
                return core::task::Poll::Ready(Err(
                    hibana::integration::transport::TransportError::Failed,
                ));
            }

            let finished = match tx.pending.take() {
                Some(finished) => finished,
                None => {
                    return core::task::Poll::Ready(Err(
                        hibana::integration::transport::TransportError::Failed,
                    ));
                }
            };
            super::record_choreofs_sio_trace(trace_frame(
                8,
                tx.local_role,
                finished.peer_role,
                (fifo::status() & 0xff) as usize,
                finished.frame_label.raw(),
            ));
            super::record_sio_direction_tx(tx.local_role, finished.peer_role);
            signal_peer();
            tx.sent_frames = tx.sent_frames.saturating_add(1);
            core::task::Poll::Ready(Ok(()))
        }

        fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
            tx.sent_frames = 0;
            tx.pending = None;
        }

        fn poll_recv<'a>(
            &'a self,
            rx: &'a mut Self::Rx<'a>,
            context: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<hibana::integration::transport::Incoming<'a>, Self::Error>>
        {
            core::hint::black_box(core::ptr::addr_of!(*context));
            if rx.local_role == 1 {
                let seen_tx = super::read_core0_to_core1_tx_count();
                unsafe {
                    core::ptr::write_volatile(
                        core::ptr::addr_of_mut!(
                            super::HIBANA_CHOREOFS_SIO_ROLE1_POLL_SEEN_CORE0_TX
                        ),
                        seen_tx,
                    );
                }
            }
            if rx.frame_label.is_some() && (rx.requeued || !rx.delivered) {
                rx.requeued = false;
                rx.delivered = true;
                rx.pending_logged = false;
                rx.pending_polls = 0;
                rx.hint_frame_label.set(None);
                super::record_choreofs_sio_trace(trace_frame(
                    3,
                    rx.local_role,
                    rx.local_role,
                    rx.len,
                    rx.frame_label.map(|label| label.raw()).unwrap_or(0),
                ));
                return core::task::Poll::Ready(Ok(rx.incoming()));
            }
            if rx.frame_label.is_some() {
                rx.frame_label = None;
                rx.hint_frame_label.set(None);
                rx.delivered = false;
                rx.pending_logged = false;
                rx.pending_polls = 0;
                rx.len = 0;
            }
            #[cfg(all(target_arch = "arm", target_os = "none"))]
            {
                if let Some(frame) = take_demux_frame(rx.local_role, rx.session_id, rx.lane) {
                    rx.frame_label = Some(frame.frame_label);
                    rx.hint_frame_label.set(Some(frame.frame_label));
                    rx.sender_role = frame.sender_role;
                    rx.len = frame.len;
                    rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
                    rx.delivered = true;
                    rx.pending_logged = false;
                    rx.pending_polls = 0;
                    super::record_sio_direction_rx(rx.local_role, frame.sender_role);
                    super::record_choreofs_sio_trace(trace_frame(
                        9,
                        rx.local_role,
                        frame.sender_role,
                        frame.len,
                        frame.frame_label.raw(),
                    ));
                    return core::task::Poll::Ready(Ok(rx.incoming()));
                }
            }
            loop {
                let word = match fifo::try_pop() {
                    Some(word) => word,
                    None => {
                        rx.pending_polls = rx.pending_polls.wrapping_add(1);
                        if rx.local_role == 1 {
                            let seen_tx = super::read_core0_to_core1_tx_count();
                            unsafe {
                                core::ptr::write_volatile(
                                    core::ptr::addr_of_mut!(
                                        super::HIBANA_CHOREOFS_SIO_ROLE1_PENDING_SEEN_CORE0_TX
                                    ),
                                    seen_tx,
                                );
                            }
                            super::record_choreofs_engine_status(
                                0x5452_8000
                                    | ((fifo::status() & 0xff) << 12)
                                    | (rx.pending_polls & 0x0fff),
                            );
                        }
                        if !rx.pending_logged {
                            super::record_choreofs_sio_trace(trace_frame(
                                7,
                                rx.local_role,
                                rx.local_role,
                                rx.lane as usize,
                                rx.frame_label.map(|label| label.raw()).unwrap_or(0),
                            ));
                            rx.pending_logged = true;
                        }
                        let has_partial_frame =
                            unsafe { (*rx_accumulator(rx.local_role)).is_partial() };
                        if has_partial_frame {
                            context.waker().wake_by_ref();
                        }
                        return core::task::Poll::Pending;
                    }
                };

                if rx.local_role == 1 {
                    let seen_tx = super::read_core0_to_core1_tx_count();
                    unsafe {
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!(
                                super::HIBANA_CHOREOFS_SIO_ROLE1_READY_SEEN_CORE0_TX
                            ),
                            seen_tx,
                        );
                    }
                }
                rx.pending_logged = false;
                rx.pending_polls = 0;

                let frame = match unsafe {
                    (&mut *rx_accumulator(rx.local_role)).push_word(rx.local_role, word)
                } {
                    Ok(Some(frame)) => frame,
                    Ok(None) => continue,
                    Err(error) => return core::task::Poll::Ready(Err(error)),
                };

                if frame.session_id != rx.session_id || frame.lane != rx.lane {
                    let reason = if frame.session_id != rx.session_id {
                        super::EPF_REASON_SESSION_MISMATCH
                    } else {
                        super::EPF_REASON_LANE_MISMATCH
                    };
                    super::record_epf_transport_reject(
                        reason,
                        rx.session_id,
                        frame.session_id,
                        frame.lane,
                        frame.sender_role,
                        rx.local_role,
                        frame.frame_label.raw(),
                    );
                    #[cfg(all(target_arch = "arm", target_os = "none"))]
                    {
                        if !store_demux_frame(rx.local_role, &frame) {
                            return core::task::Poll::Ready(Err(
                                hibana::integration::transport::TransportError::Failed,
                            ));
                        }
                        super::record_choreofs_sio_trace(trace_frame(
                            10,
                            rx.local_role,
                            frame.sender_role,
                            frame.len,
                            frame.frame_label.raw(),
                        ));
                        continue;
                    }
                    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
                    {
                        core::hint::black_box(frame.lane);
                        return core::task::Poll::Ready(Err(
                            hibana::integration::transport::TransportError::Failed,
                        ));
                    }
                }

                rx.frame_label = Some(frame.frame_label);
                rx.hint_frame_label.set(Some(frame.frame_label));
                rx.sender_role = frame.sender_role;
                rx.len = frame.len;
                rx.delivered = true;
                rx.bytes[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
                super::record_sio_direction_rx(rx.local_role, frame.sender_role);
                super::record_choreofs_sio_trace(trace_frame(
                    2,
                    rx.local_role,
                    frame.sender_role,
                    frame.len,
                    frame.frame_label.raw(),
                ));
                return core::task::Poll::Ready(Ok(rx.incoming()));
            }
        }

        fn requeue<'a>(&self, rx: &mut Self::Rx<'a>) -> Result<(), Self::Error> {
            rx.requeued = rx.frame_label.is_some();
            if rx.requeued {
                rx.delivered = false;
            }
            super::record_choreofs_sio_trace(trace_frame(
                4,
                rx.local_role,
                rx.local_role,
                rx.len,
                rx.frame_label.map(|label| label.raw()).unwrap_or(0),
            ));
            Ok(())
        }

        fn peek_recv_frame<'a>(
            &self,
            rx: &mut Self::Rx<'a>,
        ) -> Option<hibana::integration::transport::FrameHeader> {
            let hint = rx.hint_frame_label.get();
            if let Some(frame_label) = hint {
                super::record_choreofs_sio_trace(trace_frame(
                    5,
                    rx.local_role,
                    rx.local_role,
                    rx.len,
                    frame_label.raw(),
                ));
            }
            hint.map(|_| rx.frame_header())
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
static mut BAKER_ENGINE_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_DRIVER_ATTACH_SLAB_BYTES: usize = 64 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_ENGINE_ATTACH_SLAB_BYTES: usize = 64 * 1024;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static BAKER_DRIVER_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<BAKER_DRIVER_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static BAKER_ENGINE_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<BAKER_ENGINE_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_driver_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    BAKER_DRIVER_ATTACH_STORAGE.lease()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_engine_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    BAKER_ENGINE_ATTACH_STORAGE.lease()
}

#[cfg(feature = "wasm-engine-core")]
fn baker_engine_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
    core::hint::black_box(ROLE);
    let arena = unsafe { &mut *core::ptr::addr_of_mut!(BAKER_ENGINE_WASI_GUEST_ARENA) };
    arena.lease()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(link_section = ".boot2")]
// Baker owns its RP2040 W25Q080 boot block locally; no board boot crate is part of the example.
static BAKER_BOOT2_W25Q080: [u8; 256] = [
    0x00, 0xb5, 0x32, 0x4b, 0x21, 0x20, 0x58, 0x60, 0x98, 0x68, 0x02, 0x21, 0x88, 0x43, 0x98, 0x60,
    0xd8, 0x60, 0x18, 0x61, 0x58, 0x61, 0x2e, 0x4b, 0x00, 0x21, 0x99, 0x60, 0x02, 0x21, 0x59, 0x61,
    0x01, 0x21, 0xf0, 0x22, 0x99, 0x50, 0x2b, 0x49, 0x19, 0x60, 0x01, 0x21, 0x99, 0x60, 0x35, 0x20,
    0x00, 0xf0, 0x44, 0xf8, 0x02, 0x22, 0x90, 0x42, 0x14, 0xd0, 0x06, 0x21, 0x19, 0x66, 0x00, 0xf0,
    0x34, 0xf8, 0x19, 0x6e, 0x01, 0x21, 0x19, 0x66, 0x00, 0x20, 0x18, 0x66, 0x1a, 0x66, 0x00, 0xf0,
    0x2c, 0xf8, 0x19, 0x6e, 0x19, 0x6e, 0x19, 0x6e, 0x05, 0x20, 0x00, 0xf0, 0x2f, 0xf8, 0x01, 0x21,
    0x08, 0x42, 0xf9, 0xd1, 0x00, 0x21, 0x99, 0x60, 0x1b, 0x49, 0x19, 0x60, 0x00, 0x21, 0x59, 0x60,
    0x1a, 0x49, 0x1b, 0x48, 0x01, 0x60, 0x01, 0x21, 0x99, 0x60, 0xeb, 0x21, 0x19, 0x66, 0xa0, 0x21,
    0x19, 0x66, 0x00, 0xf0, 0x12, 0xf8, 0x00, 0x21, 0x99, 0x60, 0x16, 0x49, 0x14, 0x48, 0x01, 0x60,
    0x01, 0x21, 0x99, 0x60, 0x01, 0xbc, 0x00, 0x28, 0x00, 0xd0, 0x00, 0x47, 0x12, 0x48, 0x13, 0x49,
    0x08, 0x60, 0x03, 0xc8, 0x80, 0xf3, 0x08, 0x88, 0x08, 0x47, 0x03, 0xb5, 0x99, 0x6a, 0x04, 0x20,
    0x01, 0x42, 0xfb, 0xd0, 0x01, 0x20, 0x01, 0x42, 0xf8, 0xd1, 0x03, 0xbd, 0x02, 0xb5, 0x18, 0x66,
    0x18, 0x66, 0xff, 0xf7, 0xf2, 0xff, 0x18, 0x6e, 0x18, 0x6e, 0x02, 0xbd, 0x00, 0x00, 0x02, 0x40,
    0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x07, 0x00, 0x00, 0x03, 0x5f, 0x00, 0x21, 0x22, 0x00, 0x00,
    0xf4, 0x00, 0x00, 0x18, 0x22, 0x20, 0x00, 0xa0, 0x00, 0x01, 0x00, 0x10, 0x08, 0xed, 0x00, 0xe0,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x74, 0xb2, 0x4e, 0x7a,
];

#[cfg(all(target_arch = "arm", target_os = "none"))]
type Handler = unsafe extern "C" fn() -> !;

#[cfg(all(target_arch = "arm", target_os = "none"))]
type IrqHandler = unsafe extern "C" fn();

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct VectorTable {
    initial_stack_pointer: *const u32,
    reset: Handler,
    exceptions: [Handler; 14],
    external_irqs: [IrqHandler; 32],
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
const fn external_irqs() -> [IrqHandler; 32] {
    let mut handlers = [default_irq_handler as IrqHandler; 32];
    handlers[0] = timer_alarm0_irq_handler;
    handlers
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
global_asm!(
    r#"
    .global hard_fault_trampoline
    .type hard_fault_trampoline,%function
    .thumb_func
hard_fault_trampoline:
    mrs r0, msp
    ldr r1, =HIBANA_DEMO_HARDFAULT_R4
    str r4, [r1]
    ldr r1, =HIBANA_DEMO_HARDFAULT_R5
    str r5, [r1]
    ldr r1, =HIBANA_DEMO_HARDFAULT_R6
    str r6, [r1]
    ldr r1, =HIBANA_DEMO_HARDFAULT_R7
    str r7, [r1]
    ldr r1, 1f
    bx r1
    .align 2
1:
    .word hard_fault_handler_with_sp
"#
);

#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_BASE: usize = 0x4005_4000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WATCHDOG_BASE: usize = 0x4005_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WATCHDOG_TICK: *mut u32 = (WATCHDOG_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WATCHDOG_TICK_ENABLE: u32 = 1 << 9;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_BASE: usize = 0x4002_4000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_CTRL: *mut u32 = XOSC_BASE as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_STATUS: *const u32 = (XOSC_BASE + 0x04) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_STARTUP: *mut u32 = (XOSC_BASE + 0x0c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_CTRL_FREQ_RANGE_1_15MHZ: u32 = 0x0000_0aa0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_CTRL_ENABLE: u32 = 0x00fa_b000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_STATUS_STABLE: u32 = 1 << 31;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_STARTUP_12MHZ_CONSERVATIVE: u32 = 0x0000_0b80;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_BASE: usize = 0x4000_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_RESUS_CTRL: *mut u32 = (CLOCKS_BASE + 0x78) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_CTRL: *mut u32 = (CLOCKS_BASE + 0x30) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_DIV: *mut u32 = (CLOCKS_BASE + 0x34) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_SELECTED: *const u32 = (CLOCKS_BASE + 0x38) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_SRC_XOSC: u32 = 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_SELECTED_XOSC: u32 = 1 << CLOCKS_CLK_REF_SRC_XOSC;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_REF_DIV_1: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_CTRL: *mut u32 = (CLOCKS_BASE + 0x3c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_DIV: *mut u32 = (CLOCKS_BASE + 0x40) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_SELECTED: *const u32 = (CLOCKS_BASE + 0x44) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_SRC_REF: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_SRC_AUX: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_AUXSRC_PLL_SYS: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_SELECTED_REF: u32 = 1 << CLOCKS_CLK_SYS_SRC_REF;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_SELECTED_AUX: u32 = 1 << CLOCKS_CLK_SYS_SRC_AUX;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_DIV_1: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_PERI_CTRL: *mut u32 = (CLOCKS_BASE + 0x48) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_PERI_SELECTED: *const u32 = (CLOCKS_BASE + 0x50) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_PERI_AUXSRC_CLK_SYS: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_PERI_ENABLE: u32 = 1 << 11;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_PERI_SELECTED_CLK_SYS: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_BASE: usize = 0x4002_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_CS: *mut u32 = PLL_SYS_BASE as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_PWR: *mut u32 = (PLL_SYS_BASE + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_FBDIV_INT: *mut u32 = (PLL_SYS_BASE + 0x08) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_PRIM: *mut u32 = (PLL_SYS_BASE + 0x0c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_CS_LOCK: u32 = 1 << 31;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_PWR_PD: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_PWR_POSTDIVPD: u32 = 1 << 3;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_PWR_VCOPD: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_REFDIV_1: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_FBDIV_125: u32 = 125;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_POSTDIV_125MHZ: u32 = (6 << 16) | (2 << 12);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_ALARM0: *mut u32 = (TIMER_BASE + 0x10) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_TIMERAWL: *const u32 = (TIMER_BASE + 0x28) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_DBGPAUSE: *mut u32 = (TIMER_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_INTR: *mut u32 = (TIMER_BASE + 0x34) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_INTE: *mut u32 = (TIMER_BASE + 0x38) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_ALARM0_BIT: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_ISER: *mut u32 = 0xe000_e100 as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_ICPR: *mut u32 = 0xe000_e280 as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_TIMER_IRQ0_BIT: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_XOSC_HZ: u32 = 12_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_TIMER_TICK_CYCLES: u32 = BAKER_XOSC_HZ / 1_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BAKER_TIMER_TICKS_PER_MS: u64 = 1_000;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut BAKER_TIMER_ALARM0_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut BAKER_TIMER_ROUTE_ARMED: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    fn hard_fault_trampoline() -> !;
    fn baker_selected_run() -> !;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
    static __stack_limit: u32;
    static __data_load_start: u8;
    static mut __data_start: u8;
    static mut __data_end: u8;
    static mut __bss_start: u8;
    static mut __bss_end: u8;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(link_section = ".vector_table.reset_vector")]
static VECTOR_TABLE: VectorTable = VectorTable {
    initial_stack_pointer: core::ptr::addr_of!(__stack_top),
    reset: Reset,
    exceptions: [
        default_handler,
        hard_fault_trampoline,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
        default_handler,
    ],
    external_irqs: external_irqs(),
};

pub const RESULT_SUCCESS: u32 = 0x4849_4f4b;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESULT_FAILURE: u32 = 0x4849_4641;
pub const RESULT_FAIL_SAFE_OK: u32 = 0x4849_4653;
pub const RESULT_RECOVERY_OK: u32 = 0x4849_5243;
pub const RESULT_MANY_REENTRY_OK: u32 = 0x4849_524d;
pub const RESULT_PREVIEW_PROBE_OK: u32 = 0x4849_5050;
pub const RESULT_TIMER_ROUTE_OK: u32 = 0x4849_5452;
pub const RESULT_SESSION_MISMATCH_OK: u32 = 0x4849_534d;

pub const EPF_KIND_TRANSPORT_REJECT: u32 = 1;
pub const EPF_REASON_SESSION_MISMATCH: u32 = 1;
pub const EPF_REASON_LANE_MISMATCH: u32 = 2;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE0_START: u32 = 0x4849_0001;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_LAUNCHED: u32 = 0x4849_0002;
const STAGE_RUNTIME_BEGIN: u32 = 0x4849_0004;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_PROGRAM_READY: u32 = 0x4849_0006;
const STAGE_RUNTIME_READY: u32 = 0x4849_000a;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_ENGINE_RUNTIME_READY_SEEN: u32 = 0x4849_0033;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_HARD_PANIC: u32 = 0x4849_0f00;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_LAUNCH_ERR: u32 = 0x4849_0f01;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_CORE1_START_TIMEOUT: u32 = 0x4849_0f02;
#[cfg(feature = "wasm-engine-core")]
pub const STAGE_WASI_ENGINE_ERROR: u32 = 0x4849_0f10;
pub const STAGE_CHOREOFS_DRIVER_ERROR: u32 = 0x4849_0f11;
pub const STAGE_CONTROL_FLOW_ERROR: u32 = 0x4849_0f12;

pub const CHOREOFS_DRIVER_STARTED: u32 = 0x5741_0010;
pub const CHOREOFS_GPIO_READY: u32 = 0x5741_0020;
const PANIC_MESSAGE_BYTES: usize = 384;

#[unsafe(no_mangle)]
static mut HIBANA_DEMO_RESULT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_FAILURE_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_PC: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_LR: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R0: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R1: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R2: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R3: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R12: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_SP: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R4: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R5: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R6: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_R7: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_HARDFAULT_STACK: [u32; 80] = [0; 80];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE0_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_CORE1_STAGE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_FILE_HASH: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_LINE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_COLUMN: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_MESSAGE_HASH: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_MESSAGE_LEN: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_MESSAGE_TOTAL_LEN: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_PANIC_MESSAGE: [u8; PANIC_MESSAGE_BYTES] = [0; PANIC_MESSAGE_BYTES];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_EPOCH: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_KIND: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_REASON: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_ARG0: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_ARG1: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_ARG2: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE0_FUEL_USED: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_EPOCH: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_KIND: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_REASON: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_ARG0: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_ARG1: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_ARG2: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_EPF_CORE1_FUEL_USED: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_ENGINE_STATUS: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_ENGINE_ERROR_CODE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_DRIVER_TRACE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE_CORE0_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE_CORE0: [u32; 16] = [0; 16];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE_CORE1_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_TRACE_CORE1: [u32; 16] = [0; 16];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_TX_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_RX_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_TX_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_RX_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_ROLE1_PENDING_SEEN_CORE0_TX: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_ROLE1_POLL_SEEN_CORE0_TX: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SIO_ROLE1_READY_SEEN_CORE0_TX: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_PATH_OPEN_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_FD_WRITE_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_POLL_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LAST_POLL_TICKS_LO: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LAST_POLL_TICKS_HI: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LAST_OBJECT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_LED_MASK: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_SEEN_LED_MASK: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut CORE1_STARTED: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_BASE: usize = 0xd000_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST: *const u32 = (SIO_BASE + 0x50) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_ST_WRITE: *mut u32 = (SIO_BASE + 0x50) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_WR: *mut u32 = (SIO_BASE + 0x54) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_FIFO_RD: *const u32 = (SIO_BASE + 0x58) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_VLD: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_RDY: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_WOF: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const FIFO_ROE: u32 = 1 << 3;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_FRCE_OFF: *mut u32 = (0x4001_0000 + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_PROC1: u32 = 1 << 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CORE1_LAUNCH_RETRIES: u8 = 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const IO_BANK0_BASE: usize = 0x4001_4000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PADS_BANK0_BASE: usize = 0x4001_c000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_BASE: usize = 0x4000_c000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_SET: *mut u32 = (RESETS_BASE + 0x2000) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_CLR: *mut u32 = (RESETS_BASE + 0x3000) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_DONE: *const u32 = (RESETS_BASE + 0x08) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PLL_SYS: u32 = 1 << 12;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_IO_BANK0: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PADS_BANK0: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_SET: *mut u32 = (SIO_BASE + 0x14) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_CLR: *mut u32 = (SIO_BASE + 0x18) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OE_SET: *mut u32 = (SIO_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_SIO: u32 = 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_PAD_DEFAULT: u32 = 0x56;

const BAKER_SAFE_STATE_LED_PINS: [u8; 3] = [22, 21, 20];
pub trait BakerCapsuleFacts: appkit::Capsule<Placement = BakerPlacement> {
    type DriverArtifact: appkit::ArtifactEvidence;
    type EngineArtifact: appkit::ArtifactEvidence;

    const DRIVER_IMAGE_ID: appkit::ImageId;
    const ENGINE_IMAGE_ID: appkit::ImageId;
    const SUCCESS_RESULT: u32 = RESULT_SUCCESS;
    const SIO_OPERATIONAL_DEADLINE_TICKS: u32 = 0;
    const SIO_ROLE0_SESSION_XOR: u32 = 0;
    const SIO_ROLE1_SESSION_XOR: u32 = 0;

    fn driver_facts() -> appkit::DriverFacts<'static> {
        appkit::DriverFacts::EMPTY
    }
}

pub struct BakerSioTransport<C>
where
    C: BakerCapsuleFacts + 'static,
{
    inner: rp2040_sio::SioTransport,
    capsule: core::marker::PhantomData<fn() -> C>,
}

impl<C> BakerSioTransport<C>
where
    C: BakerCapsuleFacts + 'static,
{
    pub const fn new() -> Self {
        Self {
            inner: rp2040_sio::SioTransport::new(),
            capsule: core::marker::PhantomData,
        }
    }
}

impl<C> Clone for BakerSioTransport<C>
where
    C: BakerCapsuleFacts,
{
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<C> Copy for BakerSioTransport<C> where C: BakerCapsuleFacts {}

impl<C> hibana::integration::transport::Transport for BakerSioTransport<C>
where
    C: BakerCapsuleFacts,
{
    type Error = <rp2040_sio::SioTransport as hibana::integration::transport::Transport>::Error;
    type Tx<'a>
        = <rp2040_sio::SioTransport as hibana::integration::transport::Transport>::Tx<'a>
    where
        Self: 'a;
    type Rx<'a>
        = <rp2040_sio::SioTransport as hibana::integration::transport::Transport>::Rx<'a>
    where
        Self: 'a;
    fn open<'a>(
        &'a self,
        port: hibana::integration::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        let session_xor = match port.local_role() {
            0 => C::SIO_ROLE0_SESSION_XOR,
            1 => C::SIO_ROLE1_SESSION_XOR,
            _ => 0,
        };
        self.inner.open_with_session_xor(port, session_xor)
    }

    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        context: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        hibana::integration::transport::Transport::poll_send(&self.inner, tx, outgoing, context)
    }

    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        hibana::integration::transport::Transport::cancel_send(&self.inner, tx);
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        context: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<hibana::integration::transport::Incoming<'a>, Self::Error>> {
        hibana::integration::transport::Transport::poll_recv(&self.inner, rx, context)
    }

    fn requeue<'a>(&self, rx: &mut Self::Rx<'a>) -> Result<(), Self::Error> {
        hibana::integration::transport::Transport::requeue(&self.inner, rx)
    }

    fn peek_recv_frame<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameHeader> {
        hibana::integration::transport::Transport::peek_recv_frame(&self.inner, rx)
    }
}

impl<C> appkit::Placement<C> for BakerPlacement
where
    C: appkit::Capsule<Placement = BakerPlacement>,
{
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            1 => appkit::RoleKind::Engine,
            0 => appkit::RoleKind::Driver,
            2 => appkit::RoleKind::Boundary,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn baker_poll_delay(timeout_ms: u64) {
    baker_timer_route_arm(timeout_ms);
    while !baker_timer_route_ready() {
        unsafe {
            asm!("wfi", options(nomem, nostack, preserves_flags));
        }
    }
    baker_timer_route_finish();
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn baker_poll_delay(timeout_ms: u64) {
    core::hint::black_box(timeout_ms);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn baker_timer_route_resolver_ready(timeout_ms: u64) -> bool {
    unsafe {
        if read_volatile(core::ptr::addr_of!(BAKER_TIMER_ROUTE_ARMED)) == 0 {
            write_volatile(core::ptr::addr_of_mut!(BAKER_TIMER_ROUTE_ARMED), 1);
            baker_timer_route_arm(timeout_ms);
            return false;
        }
    }

    if !baker_timer_route_ready() {
        return false;
    }

    baker_timer_route_finish();
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(BAKER_TIMER_ROUTE_ARMED), 0);
    }
    true
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn baker_timer_route_resolver_ready(timeout_ms: u64) -> bool {
    core::hint::black_box(timeout_ms);
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_timer_route_arm(timeout_ms: u64) {
    let delay_ticks = core::cmp::min(
        timeout_ms.saturating_mul(BAKER_TIMER_TICKS_PER_MS),
        u32::MAX as u64,
    );
    let delay_ticks = core::cmp::max(delay_ticks as u32, 1);
    let alarm = unsafe { read_volatile(TIMER_TIMERAWL) }.wrapping_add(delay_ticks);
    unsafe {
        write_volatile(TIMER_DBGPAUSE, 0);
        write_volatile(core::ptr::addr_of_mut!(BAKER_TIMER_ALARM0_READY), 0);
        write_volatile(TIMER_INTR, TIMER_ALARM0_BIT);
        write_volatile(NVIC_ICPR, NVIC_TIMER_IRQ0_BIT);
        write_volatile(TIMER_INTE, read_volatile(TIMER_INTE) | TIMER_ALARM0_BIT);
        write_volatile(NVIC_ISER, NVIC_TIMER_IRQ0_BIT);
        write_volatile(TIMER_ALARM0, alarm);
        asm!("cpsie i", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_timer_route_ready() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(BAKER_TIMER_ALARM0_READY)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_timer_route_finish() {
    unsafe {
        write_volatile(TIMER_INTE, read_volatile(TIMER_INTE) & !TIMER_ALARM0_BIT);
        write_volatile(TIMER_INTR, TIMER_ALARM0_BIT);
    }
}

fn park() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn default_handler() -> ! {
    record_failure_stage(STAGE_HARD_PANIC);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn default_irq_handler() {
    unsafe {
        default_handler();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn timer_alarm0_irq_handler() {
    unsafe {
        write_volatile(TIMER_INTR, TIMER_ALARM0_BIT);
        write_volatile(core::ptr::addr_of_mut!(BAKER_TIMER_ALARM0_READY), 1);
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
unsafe extern "C" fn hard_fault_handler_with_sp(sp: *const u32) -> ! {
    record_hard_fault_frame(sp);
    record_failure_stage(STAGE_HARD_PANIC);
    mark_result(RESULT_FAILURE);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_hard_fault_frame(sp: *const u32) {
    unsafe {
        let stacked_r0 = core::ptr::read_volatile(sp);
        let stacked_r1 = core::ptr::read_volatile(sp.add(1));
        let stacked_r2 = core::ptr::read_volatile(sp.add(2));
        let stacked_r3 = core::ptr::read_volatile(sp.add(3));
        let stacked_r12 = core::ptr::read_volatile(sp.add(4));
        let stacked_lr = core::ptr::read_volatile(sp.add(5));
        let stacked_pc = core::ptr::read_volatile(sp.add(6));
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_R0),
            stacked_r0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_R1),
            stacked_r1,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_R2),
            stacked_r2,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_R3),
            stacked_r3,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_R12),
            stacked_r12,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_LR),
            stacked_lr,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_PC),
            stacked_pc,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_SP), sp as u32);
        let mut index = 0usize;
        while index < 80 {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(HIBANA_DEMO_HARDFAULT_STACK[index]),
                core::ptr::read_volatile(sp.add(index)),
            );
            index += 1;
        }
    }
}

fn marker_core_id() -> u32 {
    rp2040_sio::core_id()
}

fn marker_stage_slot() -> *mut u32 {
    if marker_core_id() == 0 {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STAGE)
    } else {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STAGE)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn marker_stack_slot() -> *mut u32 {
    if marker_core_id() == 0 {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE0_STACK_MAX_USED_BYTES)
    } else {
        core::ptr::addr_of_mut!(HIBANA_DEMO_CORE1_STACK_MAX_USED_BYTES)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn record_stack_high_water() {
    let sp: u32;
    unsafe {
        asm!("mov {0}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
    }
    let (top, limit) = if marker_core_id() == 0 {
        (
            core::ptr::addr_of!(__stack_top) as u32,
            core::ptr::addr_of!(__core1_stack_top) as u32,
        )
    } else {
        (
            core::ptr::addr_of!(__core1_stack_top) as u32,
            core::ptr::addr_of!(__stack_limit) as u32,
        )
    };
    if sp < limit || sp > top {
        return;
    }
    let used = top.saturating_sub(sp);
    let slot = marker_stack_slot();
    unsafe {
        let current = read_volatile(slot);
        if used > current {
            write_volatile(slot, used);
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn record_stack_high_water() {}

fn mark_stage(stage: u32) {
    record_stack_high_water();
    unsafe {
        core::ptr::write_volatile(marker_stage_slot(), stage);
    }
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    event();
}

fn mark_result(result: u32) {
    record_stack_high_water();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_RESULT), result);
    }
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_failure_stage(stage: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(HIBANA_DEMO_FAILURE_STAGE), stage);
    }
}

fn write_marker(slot: *mut u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(slot, value);
    }
}

fn epf_marker_slots() -> (
    *mut u32,
    *mut u32,
    *mut u32,
    *mut u32,
    *mut u32,
    *mut u32,
    *mut u32,
) {
    if marker_core_id() == 0 {
        (
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_EPOCH),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_KIND),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_REASON),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_ARG0),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_ARG1),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_ARG2),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE0_FUEL_USED),
        )
    } else {
        (
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_EPOCH),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_KIND),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_REASON),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_ARG0),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_ARG1),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_ARG2),
            core::ptr::addr_of_mut!(HIBANA_EPF_CORE1_FUEL_USED),
        )
    }
}

fn record_epf_compact_out(kind: u32, reason: u32, arg0: u32, arg1: u32, arg2: u32, fuel: u32) {
    let (epoch_slot, kind_slot, reason_slot, arg0_slot, arg1_slot, arg2_slot, fuel_slot) =
        epf_marker_slots();
    let epoch = unsafe { core::ptr::read_volatile(epoch_slot) }.wrapping_add(1);
    write_marker(epoch_slot, epoch);
    write_marker(kind_slot, kind);
    write_marker(reason_slot, reason);
    write_marker(arg0_slot, arg0);
    write_marker(arg1_slot, arg1);
    write_marker(arg2_slot, arg2);
    write_marker(fuel_slot, fuel);
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    event();
}

fn pack_epf_transport_meta(observed_lane: u8, source_role: u8, peer_role: u8, label: u8) -> u32 {
    ((observed_lane as u32) << 24)
        | ((source_role as u32) << 16)
        | ((peer_role as u32) << 8)
        | (label as u32)
}

fn record_epf_transport_reject(
    reason: u32,
    expected_session: u32,
    observed_session: u32,
    observed_lane: u8,
    source_role: u8,
    peer_role: u8,
    label: u8,
) {
    record_epf_compact_out(
        EPF_KIND_TRANSPORT_REJECT,
        reason,
        expected_session,
        observed_session,
        pack_epf_transport_meta(observed_lane, source_role, peer_role, label),
        0,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn panic_hash_byte(hash: u32, byte: u8) -> u32 {
    (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn panic_hash_str(text: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash = panic_hash_byte(hash, *byte);
    }
    hash
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct PanicMessageRecorder {
    stored: usize,
    total: usize,
    hash: u32,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl PanicMessageRecorder {
    fn new() -> Self {
        let mut index = 0usize;
        while index < PANIC_MESSAGE_BYTES {
            unsafe {
                let base = core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_MESSAGE).cast::<u8>();
                core::ptr::write_volatile(base.add(index), 0);
            }
            index += 1;
        }
        Self {
            stored: 0,
            total: 0,
            hash: 0x811c_9dc5,
        }
    }

    fn finish(self) {
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_MESSAGE_LEN),
            self.stored as u32,
        );
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_MESSAGE_TOTAL_LEN),
            self.total as u32,
        );
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_MESSAGE_HASH),
            self.hash,
        );
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl core::fmt::Write for PanicMessageRecorder {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for byte in text.as_bytes() {
            self.hash = panic_hash_byte(self.hash, *byte);
            if self.stored < PANIC_MESSAGE_BYTES {
                unsafe {
                    let base = core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_MESSAGE).cast::<u8>();
                    core::ptr::write_volatile(base.add(self.stored), *byte);
                }
                self.stored += 1;
            }
            self.total = self.total.saturating_add(1);
        }
        Ok(())
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn record_panic_info(info: &core::panic::PanicInfo<'_>) {
    if let Some(location) = info.location() {
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_FILE_HASH),
            panic_hash_str(location.file()),
        );
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_LINE),
            location.line(),
        );
        write_marker(
            core::ptr::addr_of_mut!(HIBANA_DEMO_PANIC_COLUMN),
            location.column(),
        );
    }

    let mut recorder = PanicMessageRecorder::new();
    match core::fmt::Write::write_fmt(&mut recorder, format_args!("{info}")) {
        Ok(()) => {}
        Err(error) => {
            core::hint::black_box(error);
        }
    }
    recorder.finish();
}

fn record_choreofs_sio_trace(code: u32) {
    unsafe {
        let (count_slot, trace_slot) = if marker_core_id() == 0 {
            (
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE0_COUNT),
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE0),
            )
        } else {
            (
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE1_COUNT),
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE1),
            )
        };
        let count = core::ptr::read_volatile(count_slot);
        let index = (count as usize) & 15;
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*trace_slot)[index]), code);
        core::ptr::write_volatile(count_slot, count.wrapping_add(1));
    }
}

fn increment_sio_counter(slot: *mut u32) {
    let next = unsafe { core::ptr::read_volatile(slot) }.wrapping_add(1);
    unsafe {
        core::ptr::write_volatile(slot, next);
    }
}

fn record_sio_direction_tx(local_role: u8, peer_role: u8) {
    match (local_role, peer_role) {
        (0, 1) => increment_sio_counter(core::ptr::addr_of_mut!(
            HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_TX_COUNT
        )),
        (1, 0) => increment_sio_counter(core::ptr::addr_of_mut!(
            HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_TX_COUNT
        )),
        _ => {}
    }
}

fn record_sio_direction_rx(local_role: u8, sender_role: u8) {
    match (sender_role, local_role) {
        (0, 1) => increment_sio_counter(core::ptr::addr_of_mut!(
            HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_RX_COUNT
        )),
        (1, 0) => increment_sio_counter(core::ptr::addr_of_mut!(
            HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_RX_COUNT
        )),
        _ => {}
    }
}

fn read_core0_to_core1_tx_count() -> u32 {
    read_marker(core::ptr::addr_of!(
        HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_TX_COUNT
    ))
}

fn read_marker(slot: *const u32) -> u32 {
    unsafe { core::ptr::read_volatile(slot) }
}

fn increment_marker(slot: *mut u32) -> u32 {
    let next = read_marker(slot).saturating_add(1);
    write_marker(slot, next);
    next
}

pub fn reset_choreofs_markers() {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_STATUS), 0);
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_ERROR_CODE),
        0,
    );
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_DRIVER_TRACE), 0);
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE0_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE1_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_TX_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_CORE0_TO_CORE1_RX_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_TX_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_CORE1_TO_CORE0_RX_COUNT),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_ROLE1_PENDING_SEEN_CORE0_TX),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_ROLE1_POLL_SEEN_CORE0_TX),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_ROLE1_READY_SEEN_CORE0_TX),
        0,
    );
    let mut trace_index = 0usize;
    while trace_index < 16 {
        unsafe {
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE0[trace_index]),
                0,
            );
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SIO_TRACE_CORE1[trace_index]),
                0,
            );
        }
        trace_index += 1;
    }
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_PATH_OPEN_COUNT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_FD_WRITE_COUNT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_POLL_COUNT), 0);
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_POLL_TICKS_LO),
        0,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_POLL_TICKS_HI),
        0,
    );
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LED_MASK), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SEEN_LED_MASK), 0);
}

pub fn record_choreofs_engine_status(status: u32) {
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_STATUS),
        status,
    );
}

pub fn record_choreofs_engine_error_code(code: u32) {
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_ENGINE_ERROR_CODE),
        code,
    );
}

pub fn record_choreofs_driver_trace(trace: u32) {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_DRIVER_TRACE), trace);
}

pub fn choreofs_endpoint_error_code(error: &hibana::EndpointError) -> u32 {
    let op = match error.operation() {
        "flow" => 0x1000,
        "send" => 0x2000,
        "recv" => 0x3000,
        "offer" => 0x4000,
        "decode" => 0x5000,
        _ => 0x0f00,
    };
    0x5745_0000 | op | (error.line() & 0x0fff)
}

pub fn record_endpoint_error(error: &hibana::EndpointError) {
    record_choreofs_engine_error_code(choreofs_endpoint_error_code(error));
}

pub fn record_choreofs_path_open(object: appkit::ObjectId) {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_PATH_OPEN_COUNT));
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT),
        object.0,
    );
}

pub fn record_choreofs_fd_write(object: appkit::ObjectId) {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_FD_WRITE_COUNT));
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_OBJECT),
        object.0,
    );
}

pub fn record_choreofs_poll() {
    increment_marker(core::ptr::addr_of_mut!(HIBANA_CHOREOFS_POLL_COUNT));
}

pub fn record_choreofs_poll_timeout(timeout_ticks: u64) {
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_POLL_TICKS_LO),
        timeout_ticks as u32,
    );
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LAST_POLL_TICKS_HI),
        (timeout_ticks >> 32) as u32,
    );
}

pub fn record_choreofs_led_mask(mask: u32, high: bool) {
    let bit = mask;
    let slot = core::ptr::addr_of_mut!(HIBANA_CHOREOFS_LED_MASK);
    let current = read_marker(slot);
    let next = if high { current | bit } else { current & !bit };
    write_marker(slot, next);
    if high {
        let seen_slot = core::ptr::addr_of_mut!(HIBANA_CHOREOFS_SEEN_LED_MASK);
        write_marker(seen_slot, read_marker(seen_slot) | bit);
    }
}

pub fn mark_runtime_ready() {
    mark_stage(STAGE_RUNTIME_READY);
}

pub fn mark_success(result: u32) {
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(result);
}

pub fn assert_choreofs_markers(
    expected_path_opens: u32,
    expected_writes: u32,
    expected_final_led_mask: u32,
    expected_seen_led_mask: u32,
) {
    let path_opens = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_PATH_OPEN_COUNT));
    let writes = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_FD_WRITE_COUNT));
    let polls = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_POLL_COUNT));
    let led_mask = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_LED_MASK));
    let seen_led_mask = read_marker(core::ptr::addr_of!(HIBANA_CHOREOFS_SEEN_LED_MASK));
    assert!(path_opens == expected_path_opens);
    assert!(writes == expected_writes);
    assert!(polls == expected_writes);
    assert!(led_mask == expected_final_led_mask);
    assert!(seen_led_mask == expected_seen_led_mask);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn gpio_ctrl(pin: u8) -> *mut u32 {
    (IO_BANK0_BASE + 0x04 + pin as usize * 8) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn gpio_pad(pin: u8) -> *mut u32 {
    (PADS_BANK0_BASE + 0x04 + pin as usize * 4) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn baker_gpio_bank_init() {
    unsafe {
        write_volatile(RESETS_RESET_CLR, RESETS_IO_BANK0 | RESETS_PADS_BANK0);
    }
    while unsafe { read_volatile(RESETS_RESET_DONE) } & (RESETS_IO_BANK0 | RESETS_PADS_BANK0)
        != (RESETS_IO_BANK0 | RESETS_PADS_BANK0)
    {
        core::hint::spin_loop();
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn baker_gpio_bank_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn baker_gpio_init_output(pin: u8) {
    baker_gpio_bank_init();
    unsafe {
        write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
        write_volatile(GPIO_OE_SET, 1u32 << pin);
        write_volatile(GPIO_OUT_CLR, 1u32 << pin);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn baker_gpio_init_output(pin: u8) {
    baker_gpio_bank_init();
    core::hint::black_box(pin);
}

fn init_baker_safe_state_outputs() {
    let mut index = 0usize;
    while index < BAKER_SAFE_STATE_LED_PINS.len() {
        baker_gpio_init_output(BAKER_SAFE_STATE_LED_PINS[index]);
        index += 1usize;
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn baker_gpio_write(pin: u8, high: bool) {
    let bit = 1u32 << pin;
    unsafe {
        if high {
            write_volatile(GPIO_OUT_SET, bit);
        } else {
            write_volatile(GPIO_OUT_CLR, bit);
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn baker_gpio_write(pin: u8, high: bool) {
    core::hint::black_box((pin, high));
}

fn write_baker_safe_state_leds() {
    let mut index = 0usize;
    while index < BAKER_SAFE_STATE_LED_PINS.len() {
        baker_gpio_write(BAKER_SAFE_STATE_LED_PINS[index], false);
        index += 1usize;
    }
}

pub fn mark_safe_state() {
    init_baker_safe_state_outputs();
    write_baker_safe_state_leds();
    record_stack_high_water();
}

fn check_report<R, I>(report: &appkit::RunReport<R, I>, required_role: u8) {
    assert!(report.projected_roles().contains(required_role));
    assert_eq!(
        report.attached_endpoint_count(),
        report.validated_role_count()
    );
}

impl<C> appkit::LogicalImage<C> for DriverImage
where
    C: BakerCapsuleFacts + 'static,
{
    type Artifact = C::DriverArtifact;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = BakerSioTransport<C>
    where
        C: 'a;

    const IMAGE_ID: appkit::ImageId = C::DRIVER_IMAGE_ID;
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(C::ENGINE_IMAGE_ID);

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        C: 'a,
    {
        BakerSioTransport::<C>::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_driver_attach_storage()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        C::driver_facts()
    }
}

impl<C> appkit::LogicalImage<C> for EngineImage
where
    C: BakerCapsuleFacts + 'static,
{
    type Artifact = C::EngineArtifact;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = BakerSioTransport<C>
    where
        C: 'a;

    const IMAGE_ID: appkit::ImageId = C::ENGINE_IMAGE_ID;
    const SITE_ID: appkit::SiteId = appkit::SiteId(2040);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2040_sio::SIO;
    const PEER_IMAGES: appkit::PeerImageSet = appkit::PeerImageSet::single(C::DRIVER_IMAGE_ID);

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {
        mark_safe_state();
    }

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        C: 'a,
    {
        BakerSioTransport::<C>::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        baker_engine_attach_storage()
    }
}

#[cfg(feature = "wasm-engine-core")]
impl<C> appkit::WasiGuestImage<C> for EngineImage
where
    C: BakerCapsuleFacts + 'static,
{
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        baker_engine_wasi_guest_lease::<ROLE>()
    }
}

static ARTIFACTS: BakerArtifacts = BakerArtifacts;

pub fn run<C>() -> !
where
    C: BakerCapsuleFacts + 'static,
    C::DriverArtifact: appkit::ArtifactGuestStorage<C, DriverImage>,
    C::EngineArtifact: appkit::ArtifactGuestStorage<C, EngineImage>,
    BakerArtifacts:
        appkit::ArtifactForImage<C, DriverImage> + appkit::ArtifactForImage<C, EngineImage>,
{
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2040_sio::core_id() == 0 {
        let mut report = appkit::run::<DriverImage, C>(
            <BakerArtifacts as appkit::ArtifactBundle<C>>::for_image::<DriverImage>(&ARTIFACTS),
        );
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<C>>::safe_state(report.image_mut());
    } else {
        let mut report = appkit::run::<EngineImage, C>(
            <BakerArtifacts as appkit::ArtifactBundle<C>>::for_image::<EngineImage>(&ARTIFACTS),
        );
        check_report(&report, 1);
        <EngineImage as appkit::LogicalImage<C>>::safe_state(report.image_mut());
    }
    mark_stage(STAGE_RUNTIME_READY);
    mark_result(C::SUCCESS_RESULT);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let stage = info
        .location()
        .map(|location| 0x4c00_0000 | (location.line() & 0x0000_ffff))
        .unwrap_or(STAGE_HARD_PANIC);
    record_failure_stage(stage);
    mark_result(RESULT_FAILURE);
    record_panic_info(info);
    park()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_ram() {
    unsafe {
        let data_src = core::ptr::addr_of!(__data_load_start);
        let data_start = core::ptr::addr_of_mut!(__data_start);
        let data_end = core::ptr::addr_of_mut!(__data_end);
        let data_len = data_end as usize - data_start as usize;
        core::ptr::copy_nonoverlapping(data_src, data_start, data_len);

        let bss_start = core::ptr::addr_of_mut!(__bss_start);
        let bss_end = core::ptr::addr_of_mut!(__bss_end);
        let bss_len = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, bss_len);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_baker_clock_tick() {
    unsafe {
        write_volatile(XOSC_STARTUP, XOSC_STARTUP_12MHZ_CONSERVATIVE);
        write_volatile(XOSC_CTRL, XOSC_CTRL_FREQ_RANGE_1_15MHZ | XOSC_CTRL_ENABLE);
        while read_volatile(XOSC_STATUS) & XOSC_STATUS_STABLE == 0 {
            core::hint::spin_loop();
        }

        write_volatile(CLOCKS_CLK_SYS_RESUS_CTRL, 0);
        write_volatile(CLOCKS_CLK_REF_DIV, CLOCKS_CLK_REF_DIV_1);
        write_volatile(CLOCKS_CLK_REF_CTRL, CLOCKS_CLK_REF_SRC_XOSC);
        while read_volatile(CLOCKS_CLK_REF_SELECTED) & CLOCKS_CLK_REF_SELECTED_XOSC == 0 {
            core::hint::spin_loop();
        }

        write_volatile(CLOCKS_CLK_SYS_CTRL, CLOCKS_CLK_SYS_SRC_REF);
        while read_volatile(CLOCKS_CLK_SYS_SELECTED) & CLOCKS_CLK_SYS_SELECTED_REF == 0 {
            core::hint::spin_loop();
        }

        write_volatile(RESETS_RESET_SET, RESETS_PLL_SYS);
        write_volatile(RESETS_RESET_CLR, RESETS_PLL_SYS);
        while read_volatile(RESETS_RESET_DONE) & RESETS_PLL_SYS == 0 {
            core::hint::spin_loop();
        }

        write_volatile(PLL_SYS_CS, PLL_SYS_REFDIV_1);
        write_volatile(PLL_SYS_FBDIV_INT, PLL_SYS_FBDIV_125);
        write_volatile(
            PLL_SYS_PWR,
            read_volatile(PLL_SYS_PWR) & !(PLL_PWR_PD | PLL_PWR_VCOPD),
        );
        while read_volatile(PLL_SYS_CS) & PLL_CS_LOCK == 0 {
            core::hint::spin_loop();
        }
        write_volatile(PLL_SYS_PRIM, PLL_SYS_POSTDIV_125MHZ);
        write_volatile(PLL_SYS_PWR, read_volatile(PLL_SYS_PWR) & !PLL_PWR_POSTDIVPD);

        write_volatile(CLOCKS_CLK_SYS_DIV, CLOCKS_CLK_SYS_DIV_1);
        write_volatile(
            CLOCKS_CLK_SYS_CTRL,
            CLOCKS_CLK_SYS_SRC_AUX | (CLOCKS_CLK_SYS_AUXSRC_PLL_SYS << 5),
        );
        while read_volatile(CLOCKS_CLK_SYS_SELECTED) & CLOCKS_CLK_SYS_SELECTED_AUX == 0 {
            core::hint::spin_loop();
        }

        write_volatile(
            CLOCKS_CLK_PERI_CTRL,
            CLOCKS_CLK_PERI_ENABLE | (CLOCKS_CLK_PERI_AUXSRC_CLK_SYS << 5),
        );
        while read_volatile(CLOCKS_CLK_PERI_SELECTED) & CLOCKS_CLK_PERI_SELECTED_CLK_SYS == 0 {
            core::hint::spin_loop();
        }

        write_volatile(
            WATCHDOG_TICK,
            WATCHDOG_TICK_ENABLE | (BAKER_TIMER_TICK_CYCLES & 0x01ff),
        );
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn event() {
    unsafe {
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_drain() {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD != 0 {
        unsafe {
            read_volatile(SIO_FIFO_RD);
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_clear_errors() {
    unsafe {
        write_volatile(SIO_FIFO_ST_WRITE, FIFO_WOF | FIFO_ROE);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn reset_core1_to_bootrom() {
    let force_off = unsafe { read_volatile(PSM_FRCE_OFF) };
    unsafe {
        write_volatile(PSM_FRCE_OFF, force_off | PSM_PROC1);
    }
    for spin in 0..32 {
        core::hint::black_box(spin);
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(PSM_FRCE_OFF, force_off & !PSM_PROC1);
    }
    for spin in 0..32 {
        core::hint::black_box(spin);
        core::hint::spin_loop();
    }
    fifo_drain();
    fifo_clear_errors();
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_push_blocking(word: u32) {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_RDY == 0 {
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(SIO_FIFO_WR, word);
    }
    event();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fifo_pop_blocking() -> u32 {
    while unsafe { read_volatile(SIO_FIFO_ST) } & FIFO_VLD == 0 {
        core::hint::spin_loop();
    }
    unsafe { read_volatile(SIO_FIFO_RD) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn launch_core1(vector_table: u32, stack_top: u32, entry: u32) -> bool {
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
fn mark_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 1);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn clear_core1_started() {
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(CORE1_STARTED), 0);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core1_started() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(CORE1_STARTED)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn ensure_core1_launched() {
    clear_core1_started();
    let launched = launch_core1(
        core::ptr::addr_of!(VECTOR_TABLE) as u32,
        core::ptr::addr_of!(__core1_stack_top) as u32,
        core1_entry as *const () as usize as u32,
    );
    if !launched {
        record_failure_stage(STAGE_CORE1_LAUNCH_ERR);
        mark_result(RESULT_FAILURE);
        park();
    }
    for spin in 0..100_000 {
        core::hint::black_box(spin);
        if core1_started() {
            mark_stage(STAGE_CORE1_LAUNCHED);
            return;
        }
        core::hint::spin_loop();
    }
    record_failure_stage(STAGE_CORE1_START_TIMEOUT);
    mark_result(RESULT_FAILURE);
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn core1_entry() -> ! {
    fifo_drain();
    mark_core1_started();
    event();
    mark_stage(STAGE_ENGINE_RUNTIME_READY_SEEN);
    unsafe { baker_selected_run() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    init_ram();
    init_baker_clock_tick();
    mark_stage(STAGE_CORE0_START);
    ensure_core1_launched();
    mark_stage(STAGE_PROGRAM_READY);
    unsafe { baker_selected_run() }
}
