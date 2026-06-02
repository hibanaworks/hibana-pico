#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
};
use core::{assert, assert_eq};
use hibana_pico::appkit;

pub struct Rpw2Placement;
pub struct Rpw2Artifacts;

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

mod rp2350_sio {
    use core::cell::Cell;

    use hibana_pico::appkit::CarrierKind;

    pub const SIO: CarrierKind = CarrierKind::new(2350);
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
                frame_label: None,
                hint_frame_label: Cell::new(None),
                len: 0,
                bytes: [0; SIO_FRAME_BYTES],
            }
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
    static mut SIO_DEMUX_ROLE0: [BufferedFrame; SIO_DEMUX_LANES] =
        [BufferedFrame::EMPTY; SIO_DEMUX_LANES];
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut SIO_DEMUX_ROLE1: [BufferedFrame; SIO_DEMUX_LANES] =
        [BufferedFrame::EMPTY; SIO_DEMUX_LANES];
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut SIO_DEMUX_ROLE2: [BufferedFrame; SIO_DEMUX_LANES] =
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

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn demux_table(local_role: u8) -> *mut [BufferedFrame; SIO_DEMUX_LANES] {
        match local_role {
            0 => core::ptr::addr_of_mut!(SIO_DEMUX_ROLE0),
            2 => core::ptr::addr_of_mut!(SIO_DEMUX_ROLE2),
            _ => core::ptr::addr_of_mut!(SIO_DEMUX_ROLE1),
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
            let table = demux_table(local_role);
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
            let table = demux_table(local_role);
            let slot = &mut (*table)[lane as usize];
            if !slot.present || slot.session_id != session_id {
                return None;
            }
            let frame = *slot;
            *slot = BufferedFrame::EMPTY;
            Some(frame)
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn is_core0_local_peer(local_role: u8, peer_role: u8) -> bool {
        matches!((local_role, peer_role), (0, 2) | (2, 0))
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn store_core0_local_frame(
        local_role: u8,
        peer_role: u8,
        session_id: u32,
        lane: u8,
        frame_label: hibana::integration::transport::FrameLabel,
        bytes: &[u8],
    ) -> bool {
        if bytes.len() > SIO_FRAME_BYTES {
            return false;
        }
        let mut frame = DecodedSioFrame {
            session_id,
            sender_role: local_role,
            frame_label,
            lane,
            len: bytes.len(),
            bytes: [0; SIO_FRAME_BYTES],
        };
        frame.bytes[..bytes.len()].copy_from_slice(bytes);
        store_demux_frame(peer_role, &frame)
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
                <SioTransport as hibana::integration::transport::Transport>::recv_frame_hint(
                    &transport, &mut rx,
                ),
                Some(label)
            );
            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::recv_frame_hint(
                    &transport, &mut rx,
                ),
                None
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
                <SioTransport as hibana::integration::transport::Transport>::recv_frame_hint(
                    &transport, &mut rx,
                ),
                Some(label)
            );
            assert_eq!(
                <SioTransport as hibana::integration::transport::Transport>::recv_frame_hint(
                    &transport, &mut rx,
                ),
                None
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
            #[cfg(all(target_arch = "arm", target_os = "none"))]
            if is_core0_local_peer(tx.local_role, outgoing.peer()) {
                if store_core0_local_frame(
                    tx.local_role,
                    outgoing.peer(),
                    tx.session_id,
                    outgoing.lane(),
                    outgoing.frame_label(),
                    bytes,
                ) {
                    super::record_choreofs_sio_trace(trace_frame(
                        12,
                        tx.local_role,
                        outgoing.peer(),
                        bytes.len(),
                        outgoing.frame_label().raw(),
                    ));
                    super::record_sio_direction_tx(tx.local_role, outgoing.peer());
                    signal_peer();
                    tx.sent_frames = tx.sent_frames.saturating_add(1);
                    return core::task::Poll::Ready(Ok(()));
                }
                context.waker().wake_by_ref();
                return core::task::Poll::Pending;
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
        ) -> core::task::Poll<Result<hibana::integration::wire::Payload<'a>, Self::Error>> {
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
                return core::task::Poll::Ready(Ok(hibana::integration::wire::Payload::new(
                    &rx.bytes[..rx.len],
                )));
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
                    return core::task::Poll::Ready(Ok(hibana::integration::wire::Payload::new(
                        &rx.bytes[..rx.len],
                    )));
                }
            }
            #[cfg(all(target_arch = "arm", target_os = "none"))]
            if rx.local_role == 2 {
                rx.pending_polls = rx.pending_polls.wrapping_add(1);
                if !rx.pending_logged {
                    super::record_choreofs_sio_trace(trace_frame(
                        13,
                        rx.local_role,
                        rx.local_role,
                        rx.lane as usize,
                        rx.frame_label.map(|label| label.raw()).unwrap_or(0),
                    ));
                    rx.pending_logged = true;
                }
                return core::task::Poll::Pending;
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
                return core::task::Poll::Ready(Ok(hibana::integration::wire::Payload::new(
                    &rx.bytes[..rx.len],
                )));
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

        fn recv_frame_hint<'a>(
            &self,
            rx: &mut Self::Rx<'a>,
        ) -> Option<hibana::integration::transport::FrameLabel> {
            let hint = rx.hint_frame_label.take();
            if let Some(frame_label) = hint {
                super::record_choreofs_sio_trace(trace_frame(
                    5,
                    rx.local_role,
                    rx.local_role,
                    rx.len,
                    frame_label.raw(),
                ));
            }
            hint
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
static mut RPW2_ENGINE_WASI_GUEST_ARENA: appkit::WasiGuestArena = appkit::WasiGuestArena::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
const RPW2_DRIVER_ATTACH_SLAB_BYTES: usize = 64 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RPW2_ENGINE_ATTACH_SLAB_BYTES: usize = 64 * 1024;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static RPW2_DRIVER_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<RPW2_DRIVER_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static RPW2_ENGINE_ATTACH_STORAGE: appkit::EmbeddedAttachStorage<RPW2_ENGINE_ATTACH_SLAB_BYTES> =
    appkit::EmbeddedAttachStorage::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_driver_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    RPW2_DRIVER_ATTACH_STORAGE.lease()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_engine_attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
    RPW2_ENGINE_ATTACH_STORAGE.lease()
}

#[cfg(feature = "wasm-engine-core")]
fn rpw2_engine_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
    core::hint::black_box(ROLE);
    let arena = unsafe { &mut *core::ptr::addr_of_mut!(RPW2_ENGINE_WASI_GUEST_ARENA) };
    arena.lease()
}

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
    external_irqs: [IrqHandler; 52],
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct Rpw2ImageDefBlock {
    marker_start: u32,
    image_type: u32,
    length: u32,
    offset: *const u32,
    marker_end: u32,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for Rpw2ImageDefBlock {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[used]
#[unsafe(link_section = ".start_block")]
static RPW2_IMAGE_DEF: Rpw2ImageDefBlock = Rpw2ImageDefBlock {
    marker_start: 0xffff_ded3,
    image_type: (0x1021 << 16) | (1 << 8) | 0x42,
    length: (1 << 8) | 0xff,
    offset: core::ptr::null(),
    marker_end: 0xab12_3579,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
const fn external_irqs() -> [IrqHandler; 52] {
    let mut handlers = [default_irq_handler as IrqHandler; 52];
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
const TIMER_BASE: usize = 0x400b_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const XOSC_BASE: usize = 0x4004_8000;
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
const CLOCKS_BASE: usize = 0x4001_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_SYS_RESUS_CTRL: *mut u32 = (CLOCKS_BASE + 0x84) as *mut u32;
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
const CLOCKS_CLK_REF_DIV_1: u32 = 1 << 16;
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
const CLOCKS_CLK_SYS_DIV_1: u32 = 1 << 16;
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
const CLOCKS_CLK_ADC_CTRL: *mut u32 = (CLOCKS_BASE + 0x6c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_ADC_DIV: *mut u32 = (CLOCKS_BASE + 0x70) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_ADC_AUXSRC_PLL_SYS: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_ADC_ENABLE: u32 = 1 << 11;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CLOCKS_CLK_ADC_DIV_3: u32 = 3 << 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PLL_SYS_BASE: usize = 0x4005_0000;
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
const TIMER_INTR: *mut u32 = (TIMER_BASE + 0x3c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_INTE: *mut u32 = (TIMER_BASE + 0x40) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_ALARM0_BIT: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_ISER: *mut u32 = 0xe000_e100 as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_ICPR: *mut u32 = 0xe000_e280 as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_TIMER_IRQ0_BIT: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RPW2_XOSC_HZ: u32 = 12_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RPW2_TIMER_TICK_CYCLES: u32 = RPW2_XOSC_HZ / 1_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RPW2_TIMER_TICKS_PER_MS: u64 = 1_000;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const TICKS_BASE: usize = 0x4010_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TICKS_TIMER0_CTRL: *mut u32 = (TICKS_BASE + 0x18) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TICKS_TIMER0_CYCLES: *mut u32 = (TICKS_BASE + 0x1c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TICKS_TIMER0_CTRL_ENABLE: u32 = 1 << 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_TIMER_ALARM0_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_TIMER_ROUTE_ARMED: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    fn hard_fault_trampoline() -> !;
    fn rpw2_selected_run() -> !;
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
static mut HIBANA_CHOREOFS_ENGINE_STATUS: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_ENGINE_ERROR_CODE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CHOREOFS_DRIVER_TRACE: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_LINE_DIAG: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_LAST_TX_WORD: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_LAST_RX_WORD: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_TRANSFER_TRACE_COUNT: u32 = 0;
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_TX_TRACE: [u32; 48] = [0; 48];
#[used]
#[unsafe(no_mangle)]
static mut HIBANA_CYW43_RX_TRACE: [u32; 48] = [0; 48];
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
const PSM_FRCE_OFF: *mut u32 = (0x4001_8000 + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PSM_PROC1: u32 = 1 << 24;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const CORE1_LAUNCH_RETRIES: u8 = 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const IO_BANK0_BASE: usize = 0x4002_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PADS_BANK0_BASE: usize = 0x4003_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_BASE: usize = 0x4002_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_SET: *mut u32 = (RESETS_BASE + 0x2000) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_CLR: *mut u32 = (RESETS_BASE + 0x3000) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_RESET_DONE: *const u32 = (RESETS_BASE + 0x08) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PLL_SYS: u32 = 1 << 14;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_IO_BANK0: u32 = 1 << 6;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PADS_BANK0: u32 = 1 << 9;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_ADC: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_I2C0: u32 = 1 << 4;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_I2C1: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_PIO0: u32 = 1 << 11;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_SPI0: u32 = 1 << 18;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESETS_UART0: u32 = 1 << 26;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_SET: *mut u32 = (SIO_BASE + 0x18) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OUT_CLR: *mut u32 = (SIO_BASE + 0x20) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OE_SET: *mut u32 = (SIO_BASE + 0x38) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_OE_CLR: *mut u32 = (SIO_BASE + 0x40) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_IN: *const u32 = (SIO_BASE + 0x04) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_SIO: u32 = 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_PIO0: u32 = 6;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_UART: u32 = 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_I2C: u32 = 3;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_FUNC_NULL: u32 = 31;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_PAD_DEFAULT: u32 = 0x56;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const GPIO_PAD_ANALOG: u32 = 0x80;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO0_BASE: usize = 0x5020_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_CTRL: *mut u32 = (PIO0_BASE) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_FSTAT: *const u32 = (PIO0_BASE + 0x04) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_FDEBUG: *mut u32 = (PIO0_BASE + 0x08) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_TXF0: *mut u32 = (PIO0_BASE + 0x10) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_RXF0: *const u32 = (PIO0_BASE + 0x20) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_INPUT_SYNC_BYPASS: *mut u32 = (PIO0_BASE + 0x38) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_INSTR_MEM0: usize = PIO0_BASE + 0x48;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_CLKDIV: *mut u32 = (PIO0_BASE + 0xc8) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_EXECCTRL: *mut u32 = (PIO0_BASE + 0xcc) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_SHIFTCTRL: *mut u32 = (PIO0_BASE + 0xd0) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_ADDR: *mut u32 = (PIO0_BASE + 0xd4) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_INSTR: *mut u32 = (PIO0_BASE + 0xd8) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_PINCTRL: *mut u32 = (PIO0_BASE + 0xdc) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_FSTAT_RXEMPTY_SM0: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_FSTAT_TXFULL_SM0: u32 = 1 << 16;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_FDEBUG_SM0_FLAGS: u32 = (1 << 0) | (1 << 8) | (1 << 16) | (1 << 24);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_ENABLE: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_RESTART: u32 = 1 << 4;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const PIO_SM0_CLKDIV_RESTART: u32 = 1 << 8;

const RPW2_SAFE_STATE_LED_PINS: [u8; 3] = [22, 21, 20];
pub trait Rpw2CapsuleFacts: appkit::Capsule<Placement = Rpw2Placement> {
    type DriverArtifact: appkit::ArtifactEvidence;
    type EngineArtifact: appkit::ArtifactEvidence;

    const DRIVER_IMAGE_ID: appkit::ImageId;
    const ENGINE_IMAGE_ID: appkit::ImageId;
    const SUCCESS_RESULT: u32 = RESULT_SUCCESS;

    fn driver_facts() -> appkit::DriverFacts<'static> {
        appkit::DriverFacts::EMPTY
    }
}

pub struct Rpw2SioTransport<C>
where
    C: Rpw2CapsuleFacts + 'static,
{
    inner: rp2350_sio::SioTransport,
    capsule: core::marker::PhantomData<fn() -> C>,
}

impl<C> Rpw2SioTransport<C>
where
    C: Rpw2CapsuleFacts + 'static,
{
    pub const fn new() -> Self {
        Self {
            inner: rp2350_sio::SioTransport::new(),
            capsule: core::marker::PhantomData,
        }
    }
}

impl<C> Clone for Rpw2SioTransport<C>
where
    C: Rpw2CapsuleFacts + 'static,
{
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<C> Copy for Rpw2SioTransport<C> where C: Rpw2CapsuleFacts + 'static {}

impl<C> hibana::integration::transport::Transport for Rpw2SioTransport<C>
where
    C: Rpw2CapsuleFacts + 'static,
{
    type Error = <rp2350_sio::SioTransport as hibana::integration::transport::Transport>::Error;
    type Tx<'a>
        = <rp2350_sio::SioTransport as hibana::integration::transport::Transport>::Tx<'a>
    where
        Self: 'a;
    type Rx<'a>
        = <rp2350_sio::SioTransport as hibana::integration::transport::Transport>::Rx<'a>
    where
        Self: 'a;

    fn open<'a>(
        &'a self,
        port: hibana::integration::transport::PortOpen,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        hibana::integration::transport::Transport::open(&self.inner, port)
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
    ) -> core::task::Poll<Result<hibana::integration::wire::Payload<'a>, Self::Error>> {
        hibana::integration::transport::Transport::poll_recv(&self.inner, rx, context)
    }

    fn requeue<'a>(&self, rx: &mut Self::Rx<'a>) -> Result<(), Self::Error> {
        hibana::integration::transport::Transport::requeue(&self.inner, rx)
    }

    fn recv_frame_hint<'a>(
        &self,
        rx: &mut Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameLabel> {
        hibana::integration::transport::Transport::recv_frame_hint(&self.inner, rx)
    }
}

impl<C> appkit::Placement<C> for Rpw2Placement
where
    C: appkit::Capsule<Placement = Rpw2Placement>,
{
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            1 => appkit::RoleKind::Engine,
            0 => appkit::RoleKind::Driver,
            2 => appkit::RoleKind::Driver,
            _ => appkit::RoleKind::Supervisor,
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_poll_delay(timeout_ms: u64) {
    let delay_ticks = core::cmp::min(
        timeout_ms.saturating_mul(RPW2_TIMER_TICKS_PER_MS),
        u32::MAX as u64,
    );
    let delay_ticks = core::cmp::max(delay_ticks as u32, 1);
    let start = unsafe { read_volatile(TIMER_TIMERAWL) };
    while unsafe { read_volatile(TIMER_TIMERAWL).wrapping_sub(start) } < delay_ticks {
        core::hint::spin_loop();
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_poll_delay(timeout_ms: u64) {
    core::hint::black_box(timeout_ms);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_timer_route_resolver_ready(timeout_ms: u64) -> bool {
    unsafe {
        if read_volatile(core::ptr::addr_of!(RPW2_TIMER_ROUTE_ARMED)) == 0 {
            write_volatile(core::ptr::addr_of_mut!(RPW2_TIMER_ROUTE_ARMED), 1);
            rpw2_timer_route_arm(timeout_ms);
            return false;
        }
    }

    if !rpw2_timer_route_ready() {
        return false;
    }

    rpw2_timer_route_finish();
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RPW2_TIMER_ROUTE_ARMED), 0);
    }
    true
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_timer_route_resolver_ready(timeout_ms: u64) -> bool {
    core::hint::black_box(timeout_ms);
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_timer_route_arm(timeout_ms: u64) {
    let delay_ticks = core::cmp::min(
        timeout_ms.saturating_mul(RPW2_TIMER_TICKS_PER_MS),
        u32::MAX as u64,
    );
    let delay_ticks = core::cmp::max(delay_ticks as u32, 1);
    let alarm = unsafe { read_volatile(TIMER_TIMERAWL) }.wrapping_add(delay_ticks);
    unsafe {
        write_volatile(TIMER_DBGPAUSE, 0);
        write_volatile(core::ptr::addr_of_mut!(RPW2_TIMER_ALARM0_READY), 0);
        write_volatile(TIMER_INTR, TIMER_ALARM0_BIT);
        write_volatile(NVIC_ICPR, NVIC_TIMER_IRQ0_BIT);
        write_volatile(TIMER_INTE, read_volatile(TIMER_INTE) | TIMER_ALARM0_BIT);
        write_volatile(NVIC_ISER, NVIC_TIMER_IRQ0_BIT);
        write_volatile(TIMER_ALARM0, alarm);
        asm!("cpsie i", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_timer_route_ready() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(RPW2_TIMER_ALARM0_READY)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_timer_route_finish() {
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
        write_volatile(core::ptr::addr_of_mut!(RPW2_TIMER_ALARM0_READY), 1);
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
    rp2350_sio::core_id()
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
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LINE_DIAG), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_TX_WORD), 0);
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_RX_WORD), 0);
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CYW43_TRANSFER_TRACE_COUNT),
        0,
    );
    let mut cyw43_trace_index = 0usize;
    while cyw43_trace_index < 48 {
        unsafe {
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CYW43_TX_TRACE[cyw43_trace_index]),
                0,
            );
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CYW43_RX_TRACE[cyw43_trace_index]),
                0,
            );
        }
        cyw43_trace_index += 1;
    }
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

#[allow(dead_code)]
fn record_cyw43_line_diag(diag: u32) {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LINE_DIAG), diag);
}

#[allow(dead_code)]
fn record_cyw43_last_tx_word(word: u32) {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_TX_WORD), word);
}

#[allow(dead_code)]
fn record_cyw43_last_rx_word(word: u32) {
    write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_RX_WORD), word);
}

#[allow(dead_code)]
fn record_cyw43_transfer_trace(tx_word: u32, rx_word: u32) {
    let index = read_marker(core::ptr::addr_of!(HIBANA_CYW43_TRANSFER_TRACE_COUNT)) as usize;
    if index < 48 {
        unsafe {
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CYW43_TX_TRACE[index]),
                tx_word,
            );
            write_marker(
                core::ptr::addr_of_mut!(HIBANA_CYW43_RX_TRACE[index]),
                rx_word,
            );
        }
    }
    write_marker(
        core::ptr::addr_of_mut!(HIBANA_CYW43_TRANSFER_TRACE_COUNT),
        (index as u32).saturating_add(1),
    );
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
fn rpw2_gpio_bank_init() {
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
fn rpw2_gpio_bank_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_gpio_init_output(pin: u8) {
    rpw2_gpio_bank_init();
    unsafe {
        write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
        write_volatile(GPIO_OE_SET, 1u32 << pin);
        write_volatile(GPIO_OUT_CLR, 1u32 << pin);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_gpio_init_output(pin: u8) {
    rpw2_gpio_bank_init();
    core::hint::black_box(pin);
}

fn init_rpw2_safe_state_outputs() {
    let mut index = 0usize;
    while index < RPW2_SAFE_STATE_LED_PINS.len() {
        rpw2_gpio_init_output(RPW2_SAFE_STATE_LED_PINS[index]);
        index += 1usize;
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_gpio_write(pin: u8, high: bool) {
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
pub fn rpw2_gpio_write(pin: u8, high: bool) {
    core::hint::black_box((pin, high));
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_gpio_init_input(pin: u8) {
    rpw2_gpio_bank_init();
    unsafe {
        write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
        write_volatile(GPIO_OE_CLR, 1u32 << pin);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn rpw2_gpio_init_input(pin: u8) {
    core::hint::black_box(pin);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_gpio_set_output_enabled(pin: u8, enabled: bool) {
    let bit = 1u32 << pin;
    unsafe {
        if enabled {
            write_volatile(GPIO_OE_SET, bit);
        } else {
            write_volatile(GPIO_OE_CLR, bit);
        }
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[allow(dead_code)]
fn rpw2_gpio_set_output_enabled(pin: u8, enabled: bool) {
    core::hint::black_box((pin, enabled));
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_gpio_read(pin: u8) -> bool {
    unsafe { read_volatile(GPIO_IN) & (1u32 << pin) != 0 }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[allow(dead_code)]
fn rpw2_gpio_read(pin: u8) -> bool {
    core::hint::black_box(pin);
    false
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rpw2SensorSample {
    pub dht20_ok: bool,
    pub temp_c_x100: i32,
    pub humidity_x100: u32,
    pub light_raw: u16,
}

pub const RPW2_UNO_Q_SENSOR_UDP_SRC_PORT: u16 = 43210;
pub const RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rpw2UnoQWifiTarget {
    pub local_mac: hibana_wifi::proto::ethernet::MacAddr,
    pub uno_q_mac: hibana_wifi::proto::ethernet::MacAddr,
    pub local_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
    pub uno_q_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
    pub src_port: u16,
}

impl Rpw2UnoQWifiTarget {
    pub const fn new(
        local_mac: hibana_wifi::proto::ethernet::MacAddr,
        uno_q_mac: hibana_wifi::proto::ethernet::MacAddr,
        local_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
        uno_q_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
        src_port: u16,
    ) -> Self {
        Self {
            local_mac,
            uno_q_mac,
            local_ip,
            uno_q_ip,
            src_port,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rpw2WifiFrameError {
    Datagram(hibana_wifi::proto::udp::UdpDatagramError),
    Frame(hibana_wifi::proto::udp::UdpTxFrameError),
}

impl From<hibana_wifi::proto::udp::UdpDatagramError> for Rpw2WifiFrameError {
    fn from(error: hibana_wifi::proto::udp::UdpDatagramError) -> Self {
        Self::Datagram(error)
    }
}

impl From<hibana_wifi::proto::udp::UdpTxFrameError> for Rpw2WifiFrameError {
    fn from(error: hibana_wifi::proto::udp::UdpTxFrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<hibana_wifi::proto::ethernet::EthernetError> for Rpw2WifiFrameError {
    fn from(error: hibana_wifi::proto::ethernet::EthernetError) -> Self {
        Self::Frame(error.into())
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_BASE: usize = 0x4007_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_DR: *mut u32 = (UART0_BASE + 0x00) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_FR: *const u32 = (UART0_BASE + 0x18) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_IBRD: *mut u32 = (UART0_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_FBRD: *mut u32 = (UART0_BASE + 0x28) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_LCR_H: *mut u32 = (UART0_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_CR: *mut u32 = (UART0_BASE + 0x30) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_ICR: *mut u32 = (UART0_BASE + 0x44) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_DMACR: *mut u32 = (UART0_BASE + 0x48) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTFR_TXFF: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTLCR_H_WLEN_8: u32 = 0x60;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTLCR_H_FEN: u32 = 1 << 4;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTCR_UARTEN: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTCR_TXE: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTCR_RXE: u32 = 1 << 9;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_BASE: usize = 0x4008_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_CR0: *mut u32 = (SPI0_BASE + 0x00) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_CR1: *mut u32 = (SPI0_BASE + 0x04) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_DR: *mut u32 = (SPI0_BASE + 0x08) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_SR: *const u32 = (SPI0_BASE + 0x0c) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_CPSR: *mut u32 = (SPI0_BASE + 0x10) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_IMSC: *mut u32 = (SPI0_BASE + 0x14) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_ICR: *mut u32 = (SPI0_BASE + 0x20) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI0_DMACR: *mut u32 = (SPI0_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI_CR0_DSS_8BIT: u32 = 0x07;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI_CR1_SSE: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI_SR_TNF: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI_SR_RNE: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SPI_TIMEOUT_SPINS: u32 = 500_000;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C0_BASE: usize = 0x4009_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C1_BASE: usize = 0x4009_8000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CON: usize = 0x00;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_TAR: usize = 0x04;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_DATA_CMD: usize = 0x10;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_SS_SCL_HCNT: usize = 0x14;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_SS_SCL_LCNT: usize = 0x18;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_RAW_INTR_STAT: usize = 0x34;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CLR_INTR: usize = 0x40;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CLR_TX_ABRT: usize = 0x54;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_ENABLE: usize = 0x6c;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_STATUS: usize = 0x70;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_RXFLR: usize = 0x78;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CON_MASTER_MODE: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CON_SPEED_STANDARD: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CON_RESTART_EN: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_CON_SLAVE_DISABLE: u32 = 1 << 6;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_DATA_CMD_READ: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_DATA_CMD_STOP: u32 = 1 << 9;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_STATUS_TFNF: u32 = 1 << 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_STATUS_TFE: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_STATUS_MST_ACTIVITY: u32 = 1 << 5;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const I2C_INTR_TX_ABRT: u32 = 1 << 6;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_BASE: usize = 0x400a_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_CS: *mut u32 = ADC_BASE as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_RESULT: *const u32 = (ADC_BASE + 0x04) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_CS_EN: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_CS_START_ONCE: u32 = 1 << 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_CS_READY: u32 = 1 << 8;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADC_CS_AINSEL_ADC0: u32 = 0 << 12;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const LCD_I2C_BASE: usize = I2C0_BASE;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const DHT20_I2C_BASE: usize = I2C1_BASE;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const LCD_ADDR: u8 = 0x3e;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const DHT20_ADDR: u8 = 0x38;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_BOARD_INIT_DONE: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_DHT20_INIT_DONE: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_ADC_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_LCD_BUS: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RPW2_DHT20_BUS: u32 = 1;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_I2C_DETECT_MASK: u32 = 0;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_LCD_INIT_OK: u32 = 0;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_LAST_LIGHT_RAW: u32 = 0;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_LAST_DHT20_OK: u32 = 0;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_LAST_TEMP_C_X100: i32 = 0;
#[used]
#[unsafe(no_mangle)]
pub static mut RPW2_LAST_HUMIDITY_X100: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn reset_deassert(mask: u32) -> bool {
    unsafe {
        write_volatile(RESETS_RESET_CLR, mask);
    }
    let mut spin = 0u32;
    while unsafe { read_volatile(RESETS_RESET_DONE) } & mask != mask {
        if spin > 500_000 {
            return false;
        }
        spin += 1;
        core::hint::spin_loop();
    }
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn mmio(base: usize, offset: usize) -> *mut u32 {
    (base + offset) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn mmio_read(base: usize, offset: usize) -> u32 {
    unsafe { read_volatile(mmio(base, offset)) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn mmio_write(base: usize, offset: usize, value: u32) {
    unsafe {
        write_volatile(mmio(base, offset), value);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_uart0_init() {
    rpw2_gpio_bank_init();
    reset_deassert(RESETS_UART0);
    unsafe {
        write_volatile(gpio_pad(0), GPIO_PAD_DEFAULT);
        write_volatile(gpio_pad(1), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(0), GPIO_FUNC_UART);
        write_volatile(gpio_ctrl(1), GPIO_FUNC_UART);
        write_volatile(UART0_CR, 0);
        write_volatile(UART0_ICR, 0x07ff);
        write_volatile(UART0_DMACR, 0);
        write_volatile(UART0_IBRD, 67);
        write_volatile(UART0_FBRD, 52);
        write_volatile(UART0_LCR_H, UARTLCR_H_WLEN_8 | UARTLCR_H_FEN);
        write_volatile(UART0_CR, UARTCR_UARTEN | UARTCR_TXE | UARTCR_RXE);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_uart0_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_uart0_write_byte(byte: u8) {
    while unsafe { read_volatile(UART0_FR) } & UARTFR_TXFF != 0 {
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(UART0_DR, u32::from(byte));
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_uart0_write_byte(byte: u8) {
    core::hint::black_box(byte);
}

pub fn rpw2_uart0_write_bytes(bytes: &[u8]) {
    let mut index = 0usize;
    while index < bytes.len() {
        rpw2_uart0_write_byte(bytes[index]);
        index += 1;
    }
}

pub fn rpw2_uart0_write_str(text: &str) {
    rpw2_uart0_write_bytes(text.as_bytes());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rpw2Cyw43SpiError {
    Timeout,
    Unavailable,
}

pub type Rpw2Cyw43DriverError = hibana_wifi::cyw43::driver::Cyw43DriverError<Rpw2Cyw43SpiError>;
pub type Rpw2Cyw43GspiError = hibana_wifi::cyw43::gspi::Cyw43GspiError<Rpw2Cyw43SpiError>;
pub type Rpw2Cyw43GspiDriver = hibana_wifi::cyw43::gspi::Cyw43GspiDriver<Rpw2Cyw43GspiBitbang>;

fn rpw2_wifi_frame_error_to_gspi_error(error: Rpw2WifiFrameError) -> Rpw2Cyw43GspiError {
    match error {
        Rpw2WifiFrameError::Datagram(
            hibana_wifi::proto::udp::UdpDatagramError::PayloadTooLarge,
        ) => hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge,
        Rpw2WifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Ethernet(
            hibana_wifi::proto::ethernet::EthernetError::BufferTooSmall,
        )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall,
        Rpw2WifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Ethernet(
            hibana_wifi::proto::ethernet::EthernetError::PayloadTooLarge,
        )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge,
        Rpw2WifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Cyw43(
            hibana_wifi::proto::cyw43::Cyw43Error::BufferTooSmall,
        )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall,
        Rpw2WifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Cyw43(_)) => {
            hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rpw2Cyw43Spi;

impl Rpw2Cyw43Spi {
    pub const fn new() -> Self {
        Self
    }
}

impl hibana_wifi::cyw43::driver::Cyw43Bus for Rpw2Cyw43Spi {
    type Error = Rpw2Cyw43SpiError;

    fn transfer(&mut self, byte: u8) -> Result<u8, Self::Error> {
        rpw2_cyw43_spi_transfer(byte)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rpw2Cyw43GspiBitbang;

impl Rpw2Cyw43GspiBitbang {
    pub const fn new() -> Self {
        Self
    }
}

impl hibana_wifi::cyw43::gspi::Cyw43GspiBus for Rpw2Cyw43GspiBitbang {
    type Error = Rpw2Cyw43SpiError;

    fn init(&mut self) -> Result<(), Self::Error> {
        rpw2_cyw43_gspi_init();
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Self::Error> {
        rpw2_cyw43_gspi_reset();
        Ok(())
    }

    fn delay_ms(&mut self, ms: u32) {
        rpw2_poll_delay(u64::from(ms));
    }

    fn transfer(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(), Self::Error> {
        rpw2_cyw43_gspi_transfer(tx, rx)
    }
}

const CYW43_PIN_WL_REG_ON: u8 = 23;
const CYW43_PIN_WL_DATA: u8 = 24;
const CYW43_PIN_WL_CS: u8 = 25;
const CYW43_PIN_WL_CLOCK: u8 = 29;
#[allow(dead_code)]
const CYW43_SPI_BIT_DELAY_SPINS: u8 = 64;

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn cyw43_spi_bit_delay() {
    let mut spin = 0;
    while spin < CYW43_SPI_BIT_DELAY_SPINS {
        core::hint::spin_loop();
        spin += 1;
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[allow(dead_code)]
fn cyw43_spi_bit_delay() {}

pub fn rpw2_cyw43_gspi_init() {
    rpw2_gpio_init_output(CYW43_PIN_WL_REG_ON);
    rpw2_gpio_init_output(CYW43_PIN_WL_DATA);
    rpw2_gpio_init_output(CYW43_PIN_WL_CS);
    rpw2_gpio_init_output(CYW43_PIN_WL_CLOCK);
    rpw2_gpio_write(CYW43_PIN_WL_REG_ON, false);
    rpw2_gpio_write(CYW43_PIN_WL_DATA, false);
    rpw2_gpio_write(CYW43_PIN_WL_CLOCK, false);
    rpw2_gpio_write(CYW43_PIN_WL_CS, true);
}

pub fn rpw2_cyw43_gspi_reset() {
    rpw2_gpio_write(CYW43_PIN_WL_REG_ON, false);
    rpw2_poll_delay(20);
    rpw2_gpio_write(CYW43_PIN_WL_REG_ON, true);
    rpw2_poll_delay(1_000);
    rpw2_gpio_init_input(CYW43_PIN_WL_DATA);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_cyw43_gspi_line_diag() -> u32 {
    rpw2_cyw43_gspi_init();
    let mut mask = 0u32;
    rpw2_gpio_write(CYW43_PIN_WL_DATA, false);
    cyw43_spi_bit_delay();
    if rpw2_gpio_read(CYW43_PIN_WL_DATA) {
        mask |= 1 << 0;
    }
    rpw2_gpio_write(CYW43_PIN_WL_DATA, true);
    cyw43_spi_bit_delay();
    if rpw2_gpio_read(CYW43_PIN_WL_DATA) {
        mask |= 1 << 1;
    }
    rpw2_gpio_write(CYW43_PIN_WL_DATA, false);
    cyw43_spi_bit_delay();
    if rpw2_gpio_read(CYW43_PIN_WL_DATA) {
        mask |= 1 << 2;
    }
    rpw2_gpio_set_output_enabled(CYW43_PIN_WL_DATA, false);
    cyw43_spi_bit_delay();
    if rpw2_gpio_read(CYW43_PIN_WL_DATA) {
        mask |= 1 << 3;
    }
    rpw2_gpio_set_output_enabled(CYW43_PIN_WL_DATA, true);
    rpw2_gpio_write(CYW43_PIN_WL_DATA, false);
    record_cyw43_line_diag(0x5749_3000 | mask);
    mask
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_cyw43_gspi_line_diag() -> u32 {
    0
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_instr_mem(index: usize) -> *mut u32 {
    (PIO_INSTR_MEM0 + index * 4) as *mut u32
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_pinctrl(set_base: u8) -> u32 {
    u32::from(CYW43_PIN_WL_DATA)
        | (u32::from(set_base) << 5)
        | (u32::from(CYW43_PIN_WL_CLOCK) << 10)
        | (u32::from(CYW43_PIN_WL_DATA) << 15)
        | (1 << 20)
        | (1 << 26)
        | (1 << 29)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_exec(instr: u16) {
    unsafe {
        write_volatile(PIO_SM0_INSTR, u32::from(instr));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_put(value: u32) -> Result<(), Rpw2Cyw43SpiError> {
    let mut spin = 0u32;
    while unsafe { read_volatile(PIO_FSTAT) } & PIO_FSTAT_TXFULL_SM0 != 0 {
        if spin > SPI_TIMEOUT_SPINS {
            return Err(Rpw2Cyw43SpiError::Timeout);
        }
        spin += 1;
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(PIO_TXF0, value);
    }
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_get() -> Result<u32, Rpw2Cyw43SpiError> {
    let mut spin = 0u32;
    while unsafe { read_volatile(PIO_FSTAT) } & PIO_FSTAT_RXEMPTY_SM0 != 0 {
        if spin > SPI_TIMEOUT_SPINS {
            return Err(Rpw2Cyw43SpiError::Timeout);
        }
        spin += 1;
        core::hint::spin_loop();
    }
    Ok(unsafe { read_volatile(PIO_RXF0) })
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_clear_fifos(shiftctrl: u32) {
    unsafe {
        write_volatile(PIO_SM0_SHIFTCTRL, shiftctrl | (1 << 31));
        write_volatile(PIO_SM0_SHIFTCTRL, shiftctrl);
        write_volatile(PIO_FDEBUG, PIO_FDEBUG_SM0_FLAGS);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_wait_tx_stalled() -> Result<(), Rpw2Cyw43SpiError> {
    unsafe {
        write_volatile(PIO_FDEBUG, 1 << 24);
    }
    let mut spin = 0u32;
    while unsafe { read_volatile(PIO_FDEBUG) } & (1 << 24) == 0 {
        if spin > SPI_TIMEOUT_SPINS {
            return Err(Rpw2Cyw43SpiError::Timeout);
        }
        spin += 1;
        core::hint::spin_loop();
    }
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn pio_sm0_set_pindir(pin: u8, output: bool) {
    unsafe {
        write_volatile(PIO_SM0_PINCTRL, pio_sm0_pinctrl(pin));
    }
    pio_sm0_exec(0xe080 | (output as u16));
    unsafe {
        write_volatile(PIO_SM0_PINCTRL, pio_sm0_pinctrl(CYW43_PIN_WL_DATA));
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_cyw43_gspi_pio_init() {
    reset_deassert(RESETS_PIO0);
    unsafe {
        write_volatile(PIO_CTRL, 0);
        write_volatile(pio_instr_mem(0), 0x6001);
        write_volatile(pio_instr_mem(1), 0x1040);
        write_volatile(pio_instr_mem(2), 0xe080);
        write_volatile(pio_instr_mem(3), 0x5001);
        write_volatile(pio_instr_mem(4), 0x0083);
        write_volatile(pio_instr_mem(5), 0x0000);
        write_volatile(gpio_pad(CYW43_PIN_WL_DATA), GPIO_PAD_DEFAULT);
        write_volatile(gpio_pad(CYW43_PIN_WL_CLOCK), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(CYW43_PIN_WL_DATA), GPIO_FUNC_PIO0);
        write_volatile(gpio_ctrl(CYW43_PIN_WL_CLOCK), GPIO_FUNC_PIO0);
        write_volatile(
            PIO_INPUT_SYNC_BYPASS,
            read_volatile(PIO_INPUT_SYNC_BYPASS) | (1u32 << CYW43_PIN_WL_DATA),
        );
        write_volatile(PIO_SM0_CLKDIV, 32 << 16);
        write_volatile(PIO_SM0_EXECCTRL, 4 << 12);
        write_volatile(PIO_SM0_SHIFTCTRL, (1 << 16) | (1 << 17));
        write_volatile(PIO_SM0_PINCTRL, pio_sm0_pinctrl(CYW43_PIN_WL_DATA));
        write_volatile(PIO_SM0_ADDR, 0);
        write_volatile(PIO_CTRL, PIO_SM0_RESTART | PIO_SM0_CLKDIV_RESTART);
    }
    pio_sm0_set_pindir(CYW43_PIN_WL_CLOCK, true);
    pio_sm0_set_pindir(CYW43_PIN_WL_DATA, true);
    pio_sm0_exec(0xa003);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn bytes_to_pio_word(bytes: &[u8], index: usize) -> u32 {
    let start = index * 4;
    u32::from_be_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
    ])
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_cyw43_gspi_transfer(tx: &[u8], rx: &mut [u8]) -> Result<(), Rpw2Cyw43SpiError> {
    let tx_word = if tx.len() >= 4 {
        u32::from_be_bytes([tx[0], tx[1], tx[2], tx[3]])
    } else {
        0
    };
    if tx.len() >= 4 {
        record_cyw43_last_tx_word(tx_word);
    }

    if tx.is_empty() || tx.len() & 3 != 0 || rx.len() & 3 != 0 {
        return Err(Rpw2Cyw43SpiError::Unavailable);
    }

    rpw2_cyw43_gspi_pio_init();
    let shiftctrl = (1 << 16) | (1 << 17);
    let rx_bits = rx.len() * 8;
    let tx_bits = tx.len() * 8;
    let wrap_top = if rx.is_empty() { 1 } else { 4 };
    unsafe {
        write_volatile(PIO_CTRL, 0);
        write_volatile(PIO_SM0_EXECCTRL, wrap_top << 12);
        write_volatile(PIO_SM0_SHIFTCTRL, shiftctrl);
        write_volatile(PIO_SM0_PINCTRL, pio_sm0_pinctrl(CYW43_PIN_WL_DATA));
        write_volatile(PIO_SM0_ADDR, 0);
    }
    pio_sm0_clear_fifos(shiftctrl);
    pio_sm0_set_pindir(CYW43_PIN_WL_DATA, true);
    unsafe {
        write_volatile(PIO_CTRL, PIO_SM0_RESTART | PIO_SM0_CLKDIV_RESTART);
    }
    pio_sm0_put((tx_bits - 1) as u32)?;
    pio_sm0_exec(0x6020);
    pio_sm0_put(if rx_bits == 0 {
        0
    } else {
        (rx_bits - 1) as u32
    })?;
    pio_sm0_exec(0x6040);
    pio_sm0_exec(0x0000);

    rpw2_gpio_write(CYW43_PIN_WL_CS, false);
    cyw43_spi_bit_delay();

    unsafe {
        write_volatile(PIO_CTRL, PIO_SM0_ENABLE);
    }
    let mut tx_word_index = 0usize;
    while tx_word_index < tx.len() / 4 {
        pio_sm0_put(bytes_to_pio_word(tx, tx_word_index))?;
        tx_word_index += 1;
    }

    if !rx.is_empty() {
        let mut rx_word_index = 0usize;
        while rx_word_index < rx.len() / 4 {
            let word = pio_sm0_get()?;
            let bytes = word.to_be_bytes();
            let start = rx_word_index * 4;
            rx[start] = bytes[0];
            rx[start + 1] = bytes[1];
            rx[start + 2] = bytes[2];
            rx[start + 3] = bytes[3];
            rx_word_index += 1;
        }
        if rx.len() >= 4 {
            let rx_word = u32::from_be_bytes([rx[0], rx[1], rx[2], rx[3]]);
            record_cyw43_last_rx_word(rx_word);
            record_cyw43_transfer_trace(tx_word, rx_word);
        }
    } else {
        pio_sm0_wait_tx_stalled()?;
    }

    unsafe {
        write_volatile(PIO_CTRL, 0);
    }
    pio_sm0_set_pindir(CYW43_PIN_WL_DATA, false);
    pio_sm0_exec(0xa003);
    rpw2_gpio_write(CYW43_PIN_WL_CS, true);
    cyw43_spi_bit_delay();
    Ok(())
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_cyw43_gspi_transfer(tx: &[u8], rx: &mut [u8]) -> Result<(), Rpw2Cyw43SpiError> {
    core::hint::black_box((tx, rx));
    Err(Rpw2Cyw43SpiError::Unavailable)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_cyw43_spi_init() {
    rpw2_gpio_bank_init();
    reset_deassert(RESETS_SPI0);
    unsafe {
        write_volatile(SPI0_CR1, 0);
        write_volatile(SPI0_IMSC, 0);
        write_volatile(SPI0_DMACR, 0);
        write_volatile(SPI0_ICR, 0x03);
        write_volatile(SPI0_CPSR, 2);
        write_volatile(SPI0_CR0, SPI_CR0_DSS_8BIT);
        write_volatile(SPI0_CR1, SPI_CR1_SSE);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_cyw43_spi_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_cyw43_spi_transfer(byte: u8) -> Result<u8, Rpw2Cyw43SpiError> {
    let mut spin = 0u32;
    while unsafe { read_volatile(SPI0_SR) } & SPI_SR_TNF == 0 {
        if spin > SPI_TIMEOUT_SPINS {
            return Err(Rpw2Cyw43SpiError::Timeout);
        }
        spin += 1;
        core::hint::spin_loop();
    }
    unsafe {
        write_volatile(SPI0_DR, u32::from(byte));
    }

    spin = 0;
    while unsafe { read_volatile(SPI0_SR) } & SPI_SR_RNE == 0 {
        if spin > SPI_TIMEOUT_SPINS {
            return Err(Rpw2Cyw43SpiError::Timeout);
        }
        spin += 1;
        core::hint::spin_loop();
    }
    Ok((unsafe { read_volatile(SPI0_DR) } & 0xff) as u8)
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_cyw43_spi_transfer(byte: u8) -> Result<u8, Rpw2Cyw43SpiError> {
    core::hint::black_box(byte);
    Err(Rpw2Cyw43SpiError::Unavailable)
}

pub fn rpw2_cyw43_boot_qemu_model(
    firmware: &[u8],
    clm: &[u8],
    nvram: &[u8],
    local_node: u8,
    peer_node: u8,
) -> Result<(), Rpw2Cyw43DriverError> {
    use hibana_wifi::{
        cyw43::driver::{Cyw43Driver, FirmwareImage, StationConfig},
        proto::firmware::fnv1a32,
    };

    rpw2_cyw43_spi_init();
    let mut driver = Cyw43Driver::new(Rpw2Cyw43Spi::new());
    driver.bring_up_station(StationConfig {
        local_node,
        peer_node,
        firmware: FirmwareImage::pico_w43439(firmware),
        clm: FirmwareImage::pico_w43439_clm(clm),
        nvram: FirmwareImage::new(nvram, nvram.len() as u32, fnv1a32(nvram)),
    })
}

pub fn rpw2_cyw43_send_frame_qemu_model(
    dst_node: u8,
    frame: &[u8],
) -> Result<(), Rpw2Cyw43DriverError> {
    use hibana_wifi::cyw43::driver::Cyw43Driver;

    let mut driver = Cyw43Driver::new(Rpw2Cyw43Spi::new());
    driver.transmit_frame(dst_node, frame)
}

pub fn rpw2_cyw43_probe_real_gspi() -> Result<u32, Rpw2Cyw43GspiError> {
    use hibana_wifi::cyw43::gspi::Cyw43GspiDriver;

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    driver.read_status()
}

pub fn rpw2_cyw43_probe_real_backplane_clock() -> Result<u32, Rpw2Cyw43GspiError> {
    use hibana_wifi::cyw43::gspi::Cyw43GspiDriver;

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    Ok(u32::from(driver.bring_up_backplane_clock()?))
}

pub fn rpw2_cyw43_probe_real_backplane_regs() -> Result<(u8, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let chipcommon_sr_control1 = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    Ok((clock, chipcommon_sr_control1))
}

pub fn rpw2_cyw43_probe_real_download_prep() -> Result<(u8, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let chipcommon_sr_control1 = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    Ok((clock, chipcommon_sr_control1))
}

pub fn rpw2_cyw43_probe_real_sram_roundtrip() -> Result<(u8, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    driver.write_backplane_u32(0, 0x4849_4241)?;
    let value = driver.read_backplane_u32(0)?;
    Ok((clock, value))
}

pub fn rpw2_cyw43_probe_real_firmware_upload(
    firmware: &[u8],
) -> Result<(u8, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let first_word = driver.read_backplane_u32(0)?;
    Ok((clock, first_word))
}

pub fn rpw2_cyw43_probe_real_sram_bytes_roundtrip() -> Result<(u8, u32, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let bytes = [
        0x00, 0x00, 0x00, 0x00, 0x65, 0x14, 0x00, 0x00, 0x91, 0x13, 0x00, 0x00, 0x91, 0x13, 0x00,
        0x00,
    ];
    let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
    driver.write_backplane_bytes(0, &bytes, &mut scratch)?;
    let first_word = driver.read_backplane_u32(0)?;
    let second_word = driver.read_backplane_u32(4)?;
    Ok((clock, first_word, second_word))
}

pub fn rpw2_cyw43_probe_real_firmware_prefix_upload(
    firmware: &[u8],
) -> Result<(u8, u32, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let len = core::cmp::min(firmware.len(), 512);
    let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
    driver.write_backplane_bytes(0, &firmware[..len], &mut scratch)?;
    let first_word = driver.read_backplane_u32(0)?;
    let word_100 = driver.read_backplane_u32(0x100)?;
    Ok((clock, first_word, word_100))
}

pub fn rpw2_cyw43_probe_real_firmware_boot(
    firmware: &[u8],
    nvram: &[u8],
) -> Result<(u8, u8), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    Ok((clock, ht_clock))
}

pub fn rpw2_cyw43_probe_real_ioctl_up(
    firmware: &[u8],
    nvram: &[u8],
) -> Result<(u8, u8), Rpw2Cyw43GspiError> {
    use hibana_wifi::{
        cyw43::gspi::Cyw43GspiDriver,
        proto::cyw43::{backplane, ioctl},
    };

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; 512];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    driver.ioctl_set(ioctl::WLC_UP, &[], &mut scratch)?;
    Ok((clock, ht_clock))
}

pub fn rpw2_cyw43_probe_real_clm_ioctl_up(
    firmware: &[u8],
    nvram: &[u8],
    clm: &[u8],
) -> Result<(u8, u8), Rpw2Cyw43GspiError> {
    use hibana_wifi::{
        cyw43::gspi::Cyw43GspiDriver,
        proto::cyw43::{backplane, ioctl},
    };

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; 1536];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    driver.load_clm(clm, &mut scratch)?;
    driver.ioctl_set(ioctl::WLC_UP, &[], &mut scratch)?;
    Ok((clock, ht_clock))
}

pub fn rpw2_cyw43_probe_real_wifi_join(
    firmware: &[u8],
    nvram: &[u8],
    clm: &[u8],
    ssid: &[u8],
    key: &[u8],
) -> Result<(u8, u8), Rpw2Cyw43GspiError> {
    use hibana_wifi::{
        cyw43::gspi::Cyw43GspiDriver,
        proto::cyw43::{backplane, ioctl},
    };

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; 1536];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    driver.load_clm(clm, &mut scratch)?;
    driver.ioctl_set(ioctl::WLC_UP, &[], &mut scratch)?;
    driver.join_wpa2(ssid, key, &mut scratch)?;
    Ok((clock, ht_clock))
}

pub fn rpw2_cyw43_real_wifi_join_driver(
    firmware: &[u8],
    nvram: &[u8],
    clm: &[u8],
    ssid: &[u8],
    key: &[u8],
    local_mac: [u8; 6],
) -> Result<(Rpw2Cyw43GspiDriver, u8, u8, [u8; 6]), Rpw2Cyw43GspiError> {
    use hibana_wifi::proto::cyw43::{backplane, ioctl};

    let mut driver = Rpw2Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; 1536];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    driver.load_clm(clm, &mut scratch)?;
    driver.set_station_mac(local_mac, &mut scratch)?;
    driver.ioctl_set(ioctl::WLC_UP, &[], &mut scratch)?;
    driver.join_wpa2(ssid, key, &mut scratch)?;
    let bssid = driver.wait_for_bssid(&mut scratch)?;
    Ok((driver, clock, ht_clock, bssid))
}

pub fn rpw2_cyw43_probe_real_wifi_join_send_uno_q(
    firmware: &[u8],
    nvram: &[u8],
    clm: &[u8],
    ssid: &[u8],
    key: &[u8],
    target: Rpw2UnoQWifiTarget,
) -> Result<(u8, u8, usize, [u8; 6]), Rpw2Cyw43GspiError> {
    use hibana_wifi::{
        cyw43::gspi::Cyw43GspiDriver,
        proto::cyw43::{backplane, ioctl},
    };

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; 1536];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
    driver.load_clm(clm, &mut scratch)?;
    driver.set_station_mac(target.local_mac.0, &mut scratch)?;
    driver.ioctl_set(ioctl::WLC_UP, &[], &mut scratch)?;
    driver.join_wpa2(ssid, key, &mut scratch)?;
    let bssid = driver.wait_for_bssid(&mut scratch)?;

    let mut ethernet_frame = [0u8; 256];
    let frame_len = rpw2_build_uno_q_sensor_ethernet_frame(
        rpw2_read_sensor_sample(),
        target,
        &mut ethernet_frame,
    )
    .map_err(rpw2_wifi_frame_error_to_gspi_error)?;

    let mut sent = 0u8;
    while sent < 6 {
        if sent != 0 {
            rpw2_poll_delay(1_000);
        }
        driver.send_ethernet_frame(&ethernet_frame[..frame_len], &mut scratch)?;
        sent = sent.wrapping_add(1);
    }
    Ok((clock, ht_clock, frame_len, bssid))
}

pub fn rpw2_cyw43_probe_real_firmware_upload_samples(
    firmware: &[u8],
) -> Result<(u8, u8, u32), Rpw2Cyw43GspiError> {
    use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

    let mut driver = Cyw43GspiDriver::new(Rpw2Cyw43GspiBitbang::new());
    driver.bring_up_bus()?;
    let clock = driver.bring_up_backplane_clock()?;
    let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
    driver.prepare_firmware_download()?;
    let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
    driver.write_backplane_bytes(0, firmware, &mut scratch)?;
    let offsets = [
        0usize,
        0x100usize,
        0x8000usize,
        0x10000usize,
        0x20000usize,
        0x30000usize,
        (firmware.len().saturating_sub(4)) & !3usize,
    ];
    let mut index = 0usize;
    while index < offsets.len() {
        let offset = offsets[index];
        let got = driver.read_backplane_u32(offset as u32)?;
        let expected = u32::from_be_bytes([
            firmware[offset],
            firmware[offset + 1],
            firmware[offset + 2],
            firmware[offset + 3],
        ]);
        if got != expected {
            return Ok((clock, index as u8, got));
        }
        index += 1;
    }
    Ok((clock, 0xff, 0))
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_i2c_init(base: usize, reset_mask: u32, sda_pin: u8, scl_pin: u8) {
    rpw2_gpio_bank_init();
    reset_deassert(reset_mask);
    unsafe {
        write_volatile(gpio_pad(sda_pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_pad(scl_pin), GPIO_PAD_DEFAULT);
        write_volatile(gpio_ctrl(sda_pin), GPIO_FUNC_I2C);
        write_volatile(gpio_ctrl(scl_pin), GPIO_FUNC_I2C);
    }
    mmio_write(base, I2C_ENABLE, 0);
    mmio_write(
        base,
        I2C_CON,
        I2C_CON_MASTER_MODE | I2C_CON_SPEED_STANDARD | I2C_CON_RESTART_EN | I2C_CON_SLAVE_DISABLE,
    );
    mmio_write(base, I2C_SS_SCL_HCNT, 625);
    mmio_write(base, I2C_SS_SCL_LCNT, 625);
    i2c_clear_intr(base);
    mmio_write(base, I2C_ENABLE, 1);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_i2c0_init() {
    rpw2_i2c_init(I2C0_BASE, RESETS_I2C0, 8, 9);
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_i2c0_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_i2c1_init() {
    rpw2_i2c_init(I2C1_BASE, RESETS_I2C1, 6, 7);
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_i2c1_init() {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_clear_intr(base: usize) {
    let _ = mmio_read(base, I2C_CLR_INTR);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_aborted(base: usize) -> bool {
    if mmio_read(base, I2C_RAW_INTR_STAT) & I2C_INTR_TX_ABRT == 0 {
        return false;
    }
    let _ = mmio_read(base, I2C_CLR_TX_ABRT);
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_wait_for(base: usize, mask: u32, set: bool) -> bool {
    let mut spin = 0u32;
    while spin < 500_000 {
        let present = mmio_read(base, I2C_STATUS) & mask != 0;
        if present == set {
            return true;
        }
        if i2c_aborted(base) {
            return false;
        }
        spin += 1;
        core::hint::spin_loop();
    }
    false
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_wait_idle(base: usize) -> bool {
    let mut spin = 0u32;
    while spin < 500_000 {
        let status = mmio_read(base, I2C_STATUS);
        if status & (I2C_STATUS_TFE | I2C_STATUS_MST_ACTIVITY) == I2C_STATUS_TFE {
            return true;
        }
        spin += 1;
        core::hint::spin_loop();
    }
    false
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_set_target(base: usize, addr: u8) -> bool {
    if !i2c_wait_idle(base) {
        return false;
    }
    i2c_clear_intr(base);
    mmio_write(base, I2C_ENABLE, 0);
    mmio_write(base, I2C_TAR, u32::from(addr));
    i2c_clear_intr(base);
    mmio_write(base, I2C_ENABLE, 1);
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_write(base: usize, addr: u8, bytes: &[u8]) -> bool {
    if bytes.is_empty() || !i2c_set_target(base, addr) {
        return false;
    }
    let mut index = 0usize;
    while index < bytes.len() {
        if !i2c_wait_for(base, I2C_STATUS_TFNF, true) {
            return false;
        }
        let stop = if index + 1 == bytes.len() {
            I2C_DATA_CMD_STOP
        } else {
            0
        };
        mmio_write(base, I2C_DATA_CMD, u32::from(bytes[index]) | stop);
        index += 1;
    }
    i2c_wait_idle(base) && !i2c_aborted(base)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_probe_write(base: usize, addr: u8, bytes: &[u8]) -> bool {
    i2c_write(base, addr, bytes)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn i2c_read(base: usize, addr: u8, out: &mut [u8]) -> bool {
    if out.is_empty() || !i2c_set_target(base, addr) {
        return false;
    }
    let mut issued = 0usize;
    let mut received = 0usize;
    let mut spin = 0u32;
    while received < out.len() && spin < 1_000_000 {
        while issued < out.len() && mmio_read(base, I2C_STATUS) & I2C_STATUS_TFNF != 0 {
            let stop = if issued + 1 == out.len() {
                I2C_DATA_CMD_STOP
            } else {
                0
            };
            mmio_write(base, I2C_DATA_CMD, I2C_DATA_CMD_READ | stop);
            issued += 1;
        }
        while received < out.len() && mmio_read(base, I2C_RXFLR) != 0 {
            out[received] = (mmio_read(base, I2C_DATA_CMD) & 0xff) as u8;
            received += 1;
        }
        if i2c_aborted(base) {
            return false;
        }
        spin += 1;
        core::hint::spin_loop();
    }
    received == out.len() && i2c_wait_idle(base) && !i2c_aborted(base)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_adc0_init() {
    rpw2_gpio_bank_init();
    if !reset_deassert(RESETS_ADC) {
        return;
    }
    unsafe {
        write_volatile(gpio_pad(26), GPIO_PAD_ANALOG);
        write_volatile(gpio_ctrl(26), GPIO_FUNC_NULL);
        write_volatile(ADC_CS, ADC_CS_EN | ADC_CS_AINSEL_ADC0);
        write_volatile(core::ptr::addr_of_mut!(RPW2_ADC_READY), 1);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn rpw2_adc0_read() -> u16 {
    if unsafe { read_volatile(core::ptr::addr_of!(RPW2_ADC_READY)) } == 0 {
        return 0;
    }
    unsafe {
        write_volatile(ADC_CS, ADC_CS_EN | ADC_CS_AINSEL_ADC0 | ADC_CS_START_ONCE);
        let mut spin = 0u32;
        while read_volatile(ADC_CS) & ADC_CS_READY == 0 && spin < 500_000 {
            spin += 1;
            core::hint::spin_loop();
        }
        (read_volatile(ADC_RESULT) & 0x0fff) as u16
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn rpw2_adc0_read() -> u16 {
    0
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn lcd_command(command: u8) -> bool {
    i2c_write(lcd_i2c_base(), LCD_ADDR, &[0x80, command])
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn lcd_data(bytes: &[u8]) -> bool {
    let len = core::cmp::min(bytes.len(), 16);
    let mut index = 0usize;
    let mut ok = true;
    while index < len {
        ok &= i2c_write(lcd_i2c_base(), LCD_ADDR, &[0x40, bytes[index]]);
        index += 1;
    }
    ok
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn lcd_i2c_base() -> usize {
    if unsafe { read_volatile(core::ptr::addr_of!(RPW2_LCD_BUS)) } == 1 {
        I2C1_BASE
    } else {
        LCD_I2C_BASE
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_lcd_init() -> bool {
    let probe = i2c_write(lcd_i2c_base(), LCD_ADDR, &[0x00]);
    rpw2_poll_delay(50);
    let mut ok = true;
    ok &= lcd_command(0x28);
    rpw2_poll_delay(5);
    ok &= lcd_command(0x28);
    rpw2_poll_delay(1);
    ok &= lcd_command(0x28);
    ok &= lcd_command(0x28);
    ok &= lcd_command(0x08);
    ok &= lcd_command(0x01);
    rpw2_poll_delay(2);
    ok &= lcd_command(0x06);
    ok &= lcd_command(0x0c);
    ok &= lcd_command(0x01);
    rpw2_poll_delay(2);
    probe && ok
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_lcd_init() -> bool {
    true
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn copy_lcd_line(dst: &mut [u8; 16], text: &[u8]) {
    let mut index = 0usize;
    while index < dst.len() {
        dst[index] = b' ';
        index += 1;
    }
    let len = core::cmp::min(text.len(), dst.len());
    dst[..len].copy_from_slice(&text[..len]);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_lcd_write_lines(line1: &[u8], line2: &[u8]) -> bool {
    let mut first = [b' '; 16];
    let mut second = [b' '; 16];
    copy_lcd_line(&mut first, line1);
    copy_lcd_line(&mut second, line2);
    let mut ok = true;
    ok &= lcd_command(0x80);
    ok &= lcd_data(&first);
    ok &= lcd_command(0xc0);
    ok &= lcd_data(&second);
    ok
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_lcd_write_lines(line1: &[u8], line2: &[u8]) -> bool {
    core::hint::black_box((line1, line2));
    true
}

pub fn rpw2_lcd_write_payload(bytes: &[u8]) -> bool {
    let mut split = bytes.len();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            split = index;
            break;
        }
        index += 1;
    }
    let mut second_start = split;
    while second_start < bytes.len()
        && (bytes[second_start] == b'\n' || bytes[second_start] == b'\r')
    {
        second_start += 1;
    }
    let mut second_end = bytes.len();
    index = second_start;
    while index < bytes.len() {
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            second_end = index;
            break;
        }
        index += 1;
    }
    rpw2_lcd_write_lines(&bytes[..split], &bytes[second_start..second_end])
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn dht20_crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0xffu8;
    let mut index = 0usize;
    while index < bytes.len() {
        crc ^= bytes[index];
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            };
            bit += 1;
        }
        index += 1;
    }
    crc
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn dht20_init_once() {
    unsafe {
        if read_volatile(core::ptr::addr_of!(RPW2_DHT20_INIT_DONE)) != 0 {
            return;
        }
    }
    let mut status = [0u8; 1];
    let base = dht20_i2c_base();
    if i2c_read(base, DHT20_ADDR, &mut status) && status[0] & 0x18 != 0x18 {
        let _ = i2c_write(base, DHT20_ADDR, &[0xbe, 0x08, 0x00]);
        rpw2_poll_delay(10);
    }
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RPW2_DHT20_INIT_DONE), 1);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn dht20_read() -> Option<(i32, u32)> {
    dht20_init_once();
    let base = dht20_i2c_base();
    if !i2c_write(base, DHT20_ADDR, &[0xac, 0x33, 0x00]) {
        return None;
    }
    rpw2_poll_delay(80);
    let mut bytes = [0u8; 7];
    if !i2c_read(base, DHT20_ADDR, &mut bytes) {
        return None;
    }
    if bytes[0] & 0x80 != 0 || dht20_crc8(&bytes[..6]) != bytes[6] {
        return None;
    }
    let raw_h = ((bytes[1] as u32) << 12) | ((bytes[2] as u32) << 4) | ((bytes[3] as u32) >> 4);
    let raw_t = (((bytes[3] as u32) & 0x0f) << 16) | ((bytes[4] as u32) << 8) | bytes[5] as u32;
    let humidity_x100 = ((raw_h as u64) * 10_000 / 1_048_576) as u32;
    let temp_c_x100 = ((raw_t as u64) * 20_000 / 1_048_576) as i32 - 5_000;
    Some((temp_c_x100, humidity_x100))
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn dht20_i2c_base() -> usize {
    if unsafe { read_volatile(core::ptr::addr_of!(RPW2_DHT20_BUS)) } == 0 {
        I2C0_BASE
    } else {
        DHT20_I2C_BASE
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn dht20_read() -> Option<(i32, u32)> {
    None
}

pub fn rpw2_read_sensor_sample() -> Rpw2SensorSample {
    let light_raw = rpw2_adc0_read();
    let sample = match dht20_read() {
        Some((temp_c_x100, humidity_x100)) => {
            #[cfg(all(target_arch = "arm", target_os = "none"))]
            unsafe {
                write_volatile(core::ptr::addr_of_mut!(RPW2_LAST_DHT20_OK), 1);
                write_volatile(core::ptr::addr_of_mut!(RPW2_LAST_TEMP_C_X100), temp_c_x100);
                write_volatile(
                    core::ptr::addr_of_mut!(RPW2_LAST_HUMIDITY_X100),
                    humidity_x100,
                );
            }
            Rpw2SensorSample {
                dht20_ok: true,
                temp_c_x100,
                humidity_x100,
                light_raw,
            }
        }
        None => {
            #[cfg(all(target_arch = "arm", target_os = "none"))]
            unsafe {
                write_volatile(core::ptr::addr_of_mut!(RPW2_LAST_DHT20_OK), 0);
            }
            Rpw2SensorSample {
                dht20_ok: false,
                temp_c_x100: 0,
                humidity_x100: 0,
                light_raw,
            }
        }
    };
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(RPW2_LAST_LIGHT_RAW),
            u32::from(light_raw),
        );
    }
    sample
}

fn push_byte(out: &mut [u8], len: &mut usize, byte: u8) {
    if *len < out.len() {
        out[*len] = byte;
        *len += 1;
    }
}

fn push_bytes(out: &mut [u8], len: &mut usize, bytes: &[u8]) {
    let mut index = 0usize;
    while index < bytes.len() {
        push_byte(out, len, bytes[index]);
        index += 1;
    }
}

fn push_u32(out: &mut [u8], len: &mut usize, mut value: u32) {
    let mut digits = [0u8; 10];
    let mut count = 0usize;
    loop {
        digits[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        push_byte(out, len, digits[count]);
    }
}

fn push_fixed_x100(out: &mut [u8], len: &mut usize, value: i32) {
    let magnitude = if value < 0 {
        push_byte(out, len, b'-');
        value.wrapping_neg() as u32
    } else {
        value as u32
    };
    push_u32(out, len, magnitude / 100);
    push_byte(out, len, b'.');
    let frac = magnitude % 100;
    push_byte(out, len, b'0' + (frac / 10) as u8);
    push_byte(out, len, b'0' + (frac % 10) as u8);
}

pub fn rpw2_format_sensor_sample(sample: Rpw2SensorSample, out: &mut [u8]) -> usize {
    let mut len = 0usize;
    if sample.dht20_ok {
        push_bytes(out, &mut len, b"T:");
        push_fixed_x100(out, &mut len, sample.temp_c_x100);
        push_bytes(out, &mut len, b"C H:");
        push_u32(out, &mut len, sample.humidity_x100 / 100);
        push_byte(out, &mut len, b'%');
    } else {
        push_bytes(out, &mut len, b"T:--.--C H:--%");
    }
    push_byte(out, &mut len, b'\n');
    push_bytes(out, &mut len, b"L:");
    push_u32(out, &mut len, u32::from(sample.light_raw));
    push_byte(out, &mut len, b'\n');
    len
}

pub fn rpw2_read_sensor_text(out: &mut [u8]) -> usize {
    rpw2_format_sensor_sample(rpw2_read_sensor_sample(), out)
}

pub fn rpw2_build_uno_q_sensor_ethernet_frame(
    sample: Rpw2SensorSample,
    target: Rpw2UnoQWifiTarget,
    out: &mut [u8],
) -> Result<usize, Rpw2WifiFrameError> {
    let mut payload = [0u8; RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES];
    let payload_len = rpw2_format_sensor_sample(sample, &mut payload);
    rpw2_build_uno_q_payload_ethernet_frame(&payload[..payload_len], target, out)
}

pub fn rpw2_build_uno_q_payload_ethernet_frame(
    payload: &[u8],
    target: Rpw2UnoQWifiTarget,
    out: &mut [u8],
) -> Result<usize, Rpw2WifiFrameError> {
    type SensorDatagram = hibana_wifi::proto::udp::UdpDatagram<RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES>;

    let datagram = SensorDatagram::new(
        target.uno_q_ip,
        target.src_port,
        hibana_wifi::proto::udp::UNO_Q_SENSOR_UDP_PORT,
        payload,
    )?;
    Ok(hibana_wifi::proto::udp::build_udp_tx_ethernet_frame(
        out,
        target.local_mac,
        target.uno_q_mac,
        target.local_ip,
        &datagram,
    )?)
}

pub fn rpw2_build_uno_q_sensor_cyw43_frame(
    sample: Rpw2SensorSample,
    target: Rpw2UnoQWifiTarget,
    sdpcm_sequence: u8,
    out: &mut [u8],
) -> Result<usize, Rpw2WifiFrameError> {
    let mut payload = [0u8; RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES];
    let payload_len = rpw2_format_sensor_sample(sample, &mut payload);
    rpw2_build_uno_q_payload_cyw43_frame(&payload[..payload_len], target, sdpcm_sequence, out)
}

pub fn rpw2_build_uno_q_payload_cyw43_frame(
    payload: &[u8],
    target: Rpw2UnoQWifiTarget,
    sdpcm_sequence: u8,
    out: &mut [u8],
) -> Result<usize, Rpw2WifiFrameError> {
    type SensorDatagram = hibana_wifi::proto::udp::UdpDatagram<RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES>;

    let datagram = SensorDatagram::new(
        target.uno_q_ip,
        target.src_port,
        hibana_wifi::proto::udp::UNO_Q_SENSOR_UDP_PORT,
        payload,
    )?;
    Ok(hibana_wifi::proto::udp::build_udp_tx_cyw43_data_frame(
        out,
        sdpcm_sequence,
        target.local_mac,
        target.uno_q_mac,
        target.local_ip,
        &datagram,
    )?)
}

pub fn rpw2_read_uno_q_sensor_cyw43_frame(
    target: Rpw2UnoQWifiTarget,
    sdpcm_sequence: u8,
    out: &mut [u8],
) -> Result<usize, Rpw2WifiFrameError> {
    rpw2_build_uno_q_sensor_cyw43_frame(rpw2_read_sensor_sample(), target, sdpcm_sequence, out)
}

pub fn rpw2_cyw43_send_uno_q_payload_frame(
    driver: &mut Rpw2Cyw43GspiDriver,
    target: Rpw2UnoQWifiTarget,
    payload: &[u8],
    ethernet_frame: &mut [u8],
    scratch: &mut [u8],
) -> Result<usize, Rpw2Cyw43GspiError> {
    let len = rpw2_build_uno_q_payload_ethernet_frame(payload, target, ethernet_frame)
        .map_err(rpw2_wifi_frame_error_to_gspi_error)?;
    driver.send_ethernet_frame(&ethernet_frame[..len], scratch)?;
    Ok(len)
}

pub fn rpw2_cyw43_send_uno_q_datagram_frame<const N: usize>(
    driver: &mut Rpw2Cyw43GspiDriver,
    target: Rpw2UnoQWifiTarget,
    datagram: &hibana_wifi::proto::udp::UdpDatagram<N>,
    ethernet_frame: &mut [u8],
    scratch: &mut [u8],
) -> Result<usize, Rpw2Cyw43GspiError> {
    let len = hibana_wifi::proto::udp::build_udp_tx_ethernet_frame(
        ethernet_frame,
        target.local_mac,
        target.uno_q_mac,
        target.local_ip,
        datagram,
    )
    .map_err(|error| {
        rpw2_wifi_frame_error_to_gspi_error(Rpw2WifiFrameError::Frame(error.into()))
    })?;
    driver.send_ethernet_frame(&ethernet_frame[..len], scratch)?;
    Ok(len)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rpw2UnoQWifiSendError {
    Frame(Rpw2WifiFrameError),
    Driver(Rpw2Cyw43DriverError),
}

impl From<Rpw2WifiFrameError> for Rpw2UnoQWifiSendError {
    fn from(error: Rpw2WifiFrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<Rpw2Cyw43DriverError> for Rpw2UnoQWifiSendError {
    fn from(error: Rpw2Cyw43DriverError) -> Self {
        Self::Driver(error)
    }
}

pub fn rpw2_send_uno_q_sensor_cyw43_frame(
    target: Rpw2UnoQWifiTarget,
    sdpcm_sequence: u8,
    dst_node: u8,
    scratch: &mut [u8],
) -> Result<usize, Rpw2UnoQWifiSendError> {
    let len = rpw2_read_uno_q_sensor_cyw43_frame(target, sdpcm_sequence, scratch)?;
    rpw2_cyw43_send_frame_qemu_model(dst_node, &scratch[..len])?;
    Ok(len)
}

#[cfg(test)]
mod uno_q_wifi_tests {
    use hibana_wifi::proto::{
        cyw43::{BDC_HEADER_LEN, SDPCM_HEADER_LEN, SdpcmChannel, SdpcmHeader},
        ethernet::{ETH_HEADER_LEN, IPV4_HEADER_LEN, Ipv4Addr, MacAddr},
        udp::UNO_Q_SENSOR_UDP_PORT,
    };

    use super::{
        RPW2_UNO_Q_SENSOR_UDP_SRC_PORT, Rpw2SensorSample, Rpw2UnoQWifiTarget,
        rpw2_build_uno_q_sensor_cyw43_frame,
    };

    #[test]
    fn sensor_sample_materializes_as_uno_q_cyw43_data_frame() {
        let target = Rpw2UnoQWifiTarget::new(
            MacAddr([0x02, 0x12, 0x34, 0x56, 0x78, 0x9a]),
            MacAddr([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
            Ipv4Addr([172, 20, 10, 5]),
            Ipv4Addr([172, 20, 10, 2]),
            RPW2_UNO_Q_SENSOR_UDP_SRC_PORT,
        );
        let sample = Rpw2SensorSample {
            dht20_ok: true,
            temp_c_x100: 2260,
            humidity_x100: 6000,
            light_raw: 2500,
        };
        let mut frame = [0u8; 192];
        let len = rpw2_build_uno_q_sensor_cyw43_frame(sample, target, 9, &mut frame).unwrap();

        let header = SdpcmHeader::decode(&frame[..SDPCM_HEADER_LEN]).unwrap();
        assert_eq!(header.total_len as usize, len);
        assert_eq!(header.sequence, 9);
        assert_eq!(header.channel, SdpcmChannel::Data);
        assert_eq!(
            &frame[SDPCM_HEADER_LEN..SDPCM_HEADER_LEN + BDC_HEADER_LEN],
            &[0; 4]
        );

        let ethernet_start = SDPCM_HEADER_LEN + BDC_HEADER_LEN;
        let udp_payload_start = ethernet_start + ETH_HEADER_LEN + IPV4_HEADER_LEN + 8;
        assert_eq!(
            &frame[ethernet_start..ethernet_start + 6],
            &target.uno_q_mac.0
        );
        assert_eq!(
            u16::from_be_bytes([frame[ethernet_start + 34], frame[ethernet_start + 35]]),
            RPW2_UNO_Q_SENSOR_UDP_SRC_PORT
        );
        assert_eq!(
            u16::from_be_bytes([frame[ethernet_start + 36], frame[ethernet_start + 37]]),
            UNO_Q_SENSOR_UDP_PORT
        );
        assert_eq!(&frame[udp_payload_start..len], b"T:22.60C H:60%\nL:2500\n");
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn rpw2_board_init() {
    unsafe {
        if read_volatile(core::ptr::addr_of!(RPW2_BOARD_INIT_DONE)) != 0 {
            return;
        }
    }
    rpw2_uart0_init();
    rpw2_uart0_write_str("\r\nrpw2 sensor panel boot\r\n");
    rpw2_i2c0_init();
    rpw2_i2c1_init();
    rpw2_adc0_init();
    let lcd_i2c0 = i2c_probe_write(I2C0_BASE, LCD_ADDR, &[0x00]);
    let lcd_i2c1 = i2c_probe_write(I2C1_BASE, LCD_ADDR, &[0x00]);
    let mut dht_status = [0u8; 1];
    let dht_i2c0 = i2c_read(I2C0_BASE, DHT20_ADDR, &mut dht_status);
    let dht_i2c1 = i2c_read(I2C1_BASE, DHT20_ADDR, &mut dht_status);
    let detect_mask = (lcd_i2c0 as u32)
        | ((lcd_i2c1 as u32) << 1)
        | ((dht_i2c0 as u32) << 2)
        | ((dht_i2c1 as u32) << 3);
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RPW2_I2C_DETECT_MASK), detect_mask);
        write_volatile(
            core::ptr::addr_of_mut!(RPW2_LCD_BUS),
            if lcd_i2c0 {
                0
            } else if lcd_i2c1 {
                1
            } else {
                0
            },
        );
        write_volatile(
            core::ptr::addr_of_mut!(RPW2_DHT20_BUS),
            if dht_i2c1 {
                1
            } else if dht_i2c0 {
                0
            } else {
                1
            },
        );
    }
    let lcd_ok = rpw2_lcd_init();
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RPW2_LCD_INIT_OK), lcd_ok as u32);
    }
    let _ = rpw2_lcd_write_lines(b"RPW2 sensor", b"booting");
    if lcd_ok {
        rpw2_uart0_write_str("lcd ok\r\n");
    } else {
        rpw2_uart0_write_str("lcd init failed\r\n");
    }
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(RPW2_BOARD_INIT_DONE), 1);
    }
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn rpw2_board_init() {}

fn write_rpw2_safe_state_leds() {
    let mut index = 0usize;
    while index < RPW2_SAFE_STATE_LED_PINS.len() {
        rpw2_gpio_write(RPW2_SAFE_STATE_LED_PINS[index], false);
        index += 1usize;
    }
}

pub fn mark_safe_state() {
    init_rpw2_safe_state_outputs();
    write_rpw2_safe_state_leds();
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
    C: Rpw2CapsuleFacts + 'static,
{
    type Artifact = C::DriverArtifact;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = Rpw2SioTransport<C>
    where
        C: 'a;

    const IMAGE_ID: appkit::ImageId = C::DRIVER_IMAGE_ID;
    const SITE_ID: appkit::SiteId = appkit::SiteId(2350);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b101);
    const CARRIER: appkit::CarrierKind = rp2350_sio::SIO;
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
        Rpw2SioTransport::<C>::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        rpw2_driver_attach_storage()
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        C::driver_facts()
    }
}

impl<C> appkit::LogicalImage<C> for EngineImage
where
    C: Rpw2CapsuleFacts + 'static,
{
    type Artifact = C::EngineArtifact;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a>
        = Rpw2SioTransport<C>
    where
        C: 'a;

    const IMAGE_ID: appkit::ImageId = C::ENGINE_IMAGE_ID;
    const SITE_ID: appkit::SiteId = appkit::SiteId(2350);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(1);
    const CARRIER: appkit::CarrierKind = rp2350_sio::SIO;
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
        Rpw2SioTransport::<C>::new()
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn attach_storage() -> appkit::EmbeddedAttachStorageRef<'static> {
        rpw2_engine_attach_storage()
    }
}

#[cfg(feature = "wasm-engine-core")]
impl<C> appkit::WasiGuestImage<C> for EngineImage
where
    C: Rpw2CapsuleFacts + 'static,
{
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        rpw2_engine_wasi_guest_lease::<ROLE>()
    }
}

static ARTIFACTS: Rpw2Artifacts = Rpw2Artifacts;

pub fn run<C>() -> !
where
    C: Rpw2CapsuleFacts + 'static,
    C::DriverArtifact: appkit::ArtifactGuestStorage<C, DriverImage>,
    C::EngineArtifact: appkit::ArtifactGuestStorage<C, EngineImage>,
    Rpw2Artifacts:
        appkit::ArtifactForImage<C, DriverImage> + appkit::ArtifactForImage<C, EngineImage>,
{
    mark_stage(STAGE_RUNTIME_BEGIN);
    if rp2350_sio::core_id() == 0 {
        let mut report =
            appkit::run::<DriverImage, C>(
                <Rpw2Artifacts as appkit::ArtifactBundle<C>>::for_image::<DriverImage>(&ARTIFACTS),
            );
        check_report(&report, 0);
        <DriverImage as appkit::LogicalImage<C>>::safe_state(report.image_mut());
    } else {
        let mut report =
            appkit::run::<EngineImage, C>(
                <Rpw2Artifacts as appkit::ArtifactBundle<C>>::for_image::<EngineImage>(&ARTIFACTS),
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
fn init_rpw2_clock_tick() {
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

        write_volatile(CLOCKS_CLK_ADC_DIV, CLOCKS_CLK_ADC_DIV_3);
        write_volatile(
            CLOCKS_CLK_ADC_CTRL,
            CLOCKS_CLK_ADC_ENABLE | (CLOCKS_CLK_ADC_AUXSRC_PLL_SYS << 5),
        );

        write_volatile(TICKS_TIMER0_CYCLES, RPW2_TIMER_TICK_CYCLES & 0x01ff);
        write_volatile(TICKS_TIMER0_CTRL, TICKS_TIMER0_CTRL_ENABLE);
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
    unsafe { rpw2_selected_run() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    init_ram();
    init_rpw2_clock_tick();
    mark_stage(STAGE_CORE0_START);
    ensure_core1_launched();
    mark_stage(STAGE_PROGRAM_READY);
    unsafe { rpw2_selected_run() }
}
