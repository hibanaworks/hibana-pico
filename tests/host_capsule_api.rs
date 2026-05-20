use hibana::{
    g,
    integration::{
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
        EngineReq, EngineRet, EnvironDone, EnvironGet, EnvironSizes, EnvironSizesGet, FdWrite,
        FdWriteDone, LABEL_WASI_ENVIRON_GET, LABEL_WASI_ENVIRON_GET_RET,
        LABEL_WASI_ENVIRON_SIZES_GET, LABEL_WASI_ENVIRON_SIZES_GET_RET, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET,
        LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, PathOpen,
        PathOpened, RouteControl, WasiImportLoopBreak, WasiImportLoopContinue,
    },
    site,
};
use std::{
    cell::{Cell, UnsafeCell},
    collections::VecDeque,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

const WASM_FD_WRITE: &[u8] = b"\0asm\x01\0\0\0\
    \x01\x04\x01\x60\x00\x00\
    \x02\x23\x01\x16wasi_snapshot_preview1\x08fd_write\x00\x00";
const TEST_LOCAL_QUEUE_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(1001);
const TEST_TCP: appkit::CarrierKind = appkit::CarrierKind::new(1002);
const TEST_UART: appkit::CarrierKind = appkit::CarrierKind::new(1004);
const TEST_CARRIER_ROLES: usize = appkit::HIBANA_TYPED_ROLE_DOMAIN_SIZE as usize;
const TEST_CARRIER_QUEUE_DEPTH: usize = 16;
const TEST_CARRIER_FRAME_BYTES: usize = 256;

#[cfg(feature = "wasm-engine-core")]
const SECTION_TYPE: u8 = 1;
#[cfg(feature = "wasm-engine-core")]
const SECTION_IMPORT: u8 = 2;
#[cfg(feature = "wasm-engine-core")]
const SECTION_FUNCTION: u8 = 3;
#[cfg(feature = "wasm-engine-core")]
const SECTION_MEMORY: u8 = 5;
#[cfg(feature = "wasm-engine-core")]
const SECTION_EXPORT: u8 = 7;
#[cfg(feature = "wasm-engine-core")]
const SECTION_CODE: u8 = 10;
#[cfg(feature = "wasm-engine-core")]
const SECTION_DATA: u8 = 11;
#[cfg(feature = "wasm-engine-core")]
const EXTERNAL_KIND_FUNC: u8 = 0;
#[cfg(feature = "wasm-engine-core")]
const VALTYPE_I32: u8 = 0x7f;
#[cfg(feature = "wasm-engine-core")]
const VALTYPE_I64: u8 = 0x7e;
#[cfg(feature = "wasm-engine-core")]
const OPCODE_I32_CONST: u8 = 0x41;
#[cfg(feature = "wasm-engine-core")]
const OPCODE_I64_CONST: u8 = 0x42;
#[cfg(feature = "wasm-engine-core")]
const OPCODE_CALL: u8 = 0x10;
#[cfg(feature = "wasm-engine-core")]
const OPCODE_DROP: u8 = 0x1a;
#[cfg(feature = "wasm-engine-core")]
const OPCODE_END: u8 = 0x0b;

#[cfg(feature = "wasm-engine-core")]
fn push_leb_u32(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
fn push_leb_i64(out: &mut Vec<u8>, value: i64) {
    let mut value = value as u64;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == !0 && byte & 0x40 != 0);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
fn push_leb_i32(out: &mut Vec<u8>, value: i32) {
    let mut value = value;
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

#[cfg(feature = "wasm-engine-core")]
fn push_i32_const(out: &mut Vec<u8>, value: i32) {
    out.push(OPCODE_I32_CONST);
    push_leb_i32(out, value);
}

#[cfg(feature = "wasm-engine-core")]
fn push_i64_const(out: &mut Vec<u8>, value: i64) {
    out.push(OPCODE_I64_CONST);
    push_leb_i64(out, value);
}

#[cfg(feature = "wasm-engine-core")]
fn push_name(out: &mut Vec<u8>, name: &[u8]) {
    push_leb_u32(out, name.len() as u32);
    out.extend_from_slice(name);
}

#[cfg(feature = "wasm-engine-core")]
fn push_section(module: &mut Vec<u8>, section: u8, bytes: &[u8]) {
    module.push(section);
    push_leb_u32(module, bytes.len() as u32);
    module.extend_from_slice(bytes);
}

#[cfg(feature = "wasm-engine-core")]
fn fd_write_guest_module() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    let mut types = Vec::new();
    push_leb_u32(&mut types, 2);
    types.push(0x60);
    push_leb_u32(&mut types, 4);
    types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 0);
    push_leb_u32(&mut types, 0);
    push_section(&mut module, SECTION_TYPE, &types);

    let mut imports = Vec::new();
    push_leb_u32(&mut imports, 1);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"fd_write");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 0);
    push_section(&mut module, SECTION_IMPORT, &imports);

    let mut functions = Vec::new();
    push_leb_u32(&mut functions, 1);
    push_leb_u32(&mut functions, 1);
    push_section(&mut module, SECTION_FUNCTION, &functions);

    push_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);

    let mut exports = Vec::new();
    push_leb_u32(&mut exports, 1);
    push_name(&mut exports, b"_start");
    exports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut exports, 1);
    push_section(&mut module, SECTION_EXPORT, &exports);

    let mut body = Vec::new();
    push_leb_u32(&mut body, 0);
    push_i32_const(&mut body, 1);
    push_i32_const(&mut body, 0);
    push_i32_const(&mut body, 1);
    push_i32_const(&mut body, 8);
    body.push(OPCODE_CALL);
    push_leb_u32(&mut body, 0);
    body.push(OPCODE_DROP);
    body.push(OPCODE_END);
    let mut code = Vec::new();
    push_leb_u32(&mut code, 1);
    push_leb_u32(&mut code, body.len() as u32);
    code.extend_from_slice(&body);
    push_section(&mut module, SECTION_CODE, &code);

    let mut segment = [0u8; 21];
    segment[0..4].copy_from_slice(&16u32.to_le_bytes());
    segment[4..8].copy_from_slice(&5u32.to_le_bytes());
    segment[16..21].copy_from_slice(b"hello");
    let mut data = Vec::new();
    push_leb_u32(&mut data, 1);
    push_leb_u32(&mut data, 0);
    data.push(OPCODE_I32_CONST);
    data.push(0);
    data.push(OPCODE_END);
    push_leb_u32(&mut data, segment.len() as u32);
    data.extend_from_slice(&segment);
    push_section(&mut module, SECTION_DATA, &data);

    module
}

#[cfg(feature = "wasm-engine-core")]
fn fd_write_with_unused_std_wasi_imports_module() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    let mut types = Vec::new();
    push_leb_u32(&mut types, 5);
    types.push(0x60);
    push_leb_u32(&mut types, 4);
    types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 2);
    types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    push_leb_u32(&mut types, 0);
    types.push(0x60);
    push_leb_u32(&mut types, 0);
    push_leb_u32(&mut types, 0);
    types.push(0x60);
    push_leb_u32(&mut types, 0);
    push_leb_u32(&mut types, 0);
    push_section(&mut module, SECTION_TYPE, &types);

    let mut imports = Vec::new();
    push_leb_u32(&mut imports, 4);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"fd_write");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 0);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"environ_sizes_get");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 1);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"environ_get");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 1);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"proc_exit");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 2);
    push_section(&mut module, SECTION_IMPORT, &imports);

    let mut functions = Vec::new();
    push_leb_u32(&mut functions, 1);
    push_leb_u32(&mut functions, 4);
    push_section(&mut module, SECTION_FUNCTION, &functions);

    push_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);

    let mut exports = Vec::new();
    push_leb_u32(&mut exports, 1);
    push_name(&mut exports, b"_start");
    exports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut exports, 4);
    push_section(&mut module, SECTION_EXPORT, &exports);

    let mut body = Vec::new();
    push_leb_u32(&mut body, 0);
    push_i32_const(&mut body, 1);
    push_i32_const(&mut body, 0);
    push_i32_const(&mut body, 1);
    push_i32_const(&mut body, 8);
    body.push(OPCODE_CALL);
    push_leb_u32(&mut body, 0);
    body.push(OPCODE_DROP);
    body.push(OPCODE_END);
    let mut code = Vec::new();
    push_leb_u32(&mut code, 1);
    push_leb_u32(&mut code, body.len() as u32);
    code.extend_from_slice(&body);
    push_section(&mut module, SECTION_CODE, &code);

    let mut segment = [0u8; 21];
    segment[0..4].copy_from_slice(&16u32.to_le_bytes());
    segment[4..8].copy_from_slice(&5u32.to_le_bytes());
    segment[16..21].copy_from_slice(b"hello");
    let mut data = Vec::new();
    push_leb_u32(&mut data, 1);
    push_leb_u32(&mut data, 0);
    data.push(OPCODE_I32_CONST);
    data.push(0);
    data.push(OPCODE_END);
    push_leb_u32(&mut data, segment.len() as u32);
    data.extend_from_slice(&segment);
    push_section(&mut module, SECTION_DATA, &data);

    module
}

#[cfg(feature = "wasm-engine-core")]
fn fd_write_with_non_wasi_import_module() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    let mut types = Vec::new();
    push_leb_u32(&mut types, 2);
    types.push(0x60);
    push_leb_u32(&mut types, 4);
    types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 0);
    push_leb_u32(&mut types, 0);
    push_section(&mut module, SECTION_TYPE, &types);

    let mut imports = Vec::new();
    push_leb_u32(&mut imports, 2);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"fd_write");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 0);
    push_name(&mut imports, b"env");
    push_name(&mut imports, b"host_side_effect");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 1);
    push_section(&mut module, SECTION_IMPORT, &imports);

    module
}

#[cfg(feature = "wasm-engine-core")]
fn path_open_fd_write_guest_module() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    let mut types = Vec::new();
    push_leb_u32(&mut types, 3);
    types.push(0x60);
    push_leb_u32(&mut types, 9);
    types.extend_from_slice(&[
        VALTYPE_I32,
        VALTYPE_I32,
        VALTYPE_I32,
        VALTYPE_I32,
        VALTYPE_I32,
        VALTYPE_I64,
        VALTYPE_I64,
        VALTYPE_I32,
        VALTYPE_I32,
    ]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 4);
    types.extend_from_slice(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32]);
    push_leb_u32(&mut types, 1);
    types.push(VALTYPE_I32);
    types.push(0x60);
    push_leb_u32(&mut types, 0);
    push_leb_u32(&mut types, 0);
    push_section(&mut module, SECTION_TYPE, &types);

    let mut imports = Vec::new();
    push_leb_u32(&mut imports, 2);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"path_open");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 0);
    push_name(&mut imports, b"wasi_snapshot_preview1");
    push_name(&mut imports, b"fd_write");
    imports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut imports, 1);
    push_section(&mut module, SECTION_IMPORT, &imports);

    let mut functions = Vec::new();
    push_leb_u32(&mut functions, 1);
    push_leb_u32(&mut functions, 2);
    push_section(&mut module, SECTION_FUNCTION, &functions);

    push_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);

    let mut exports = Vec::new();
    push_leb_u32(&mut exports, 1);
    push_name(&mut exports, b"_start");
    exports.push(EXTERNAL_KIND_FUNC);
    push_leb_u32(&mut exports, 2);
    push_section(&mut module, SECTION_EXPORT, &exports);

    let mut body = Vec::new();
    push_leb_u32(&mut body, 0);
    push_i32_const(&mut body, 3);
    push_i32_const(&mut body, 0);
    push_i32_const(&mut body, 32);
    push_i32_const(&mut body, 16);
    push_i32_const(&mut body, 0);
    push_i64_const(&mut body, 0x2);
    push_i64_const(&mut body, 0);
    push_i32_const(&mut body, 0);
    push_i32_const(&mut body, 64);
    body.push(OPCODE_CALL);
    push_leb_u32(&mut body, 0);
    body.push(OPCODE_DROP);
    push_i32_const(&mut body, 4);
    push_i32_const(&mut body, 0);
    push_i32_const(&mut body, 1);
    push_i32_const(&mut body, 80);
    body.push(OPCODE_CALL);
    push_leb_u32(&mut body, 1);
    body.push(OPCODE_DROP);
    body.push(OPCODE_END);
    let mut code = Vec::new();
    push_leb_u32(&mut code, 1);
    push_leb_u32(&mut code, body.len() as u32);
    code.extend_from_slice(&body);
    push_section(&mut module, SECTION_CODE, &code);

    let mut segment = [0u8; 48];
    segment[0..4].copy_from_slice(&16u32.to_le_bytes());
    segment[4..8].copy_from_slice(&8u32.to_le_bytes());
    segment[16..24].copy_from_slice(b"green=on");
    segment[32..48].copy_from_slice(b"device/led/green");
    let mut data = Vec::new();
    push_leb_u32(&mut data, 1);
    push_leb_u32(&mut data, 0);
    data.push(OPCODE_I32_CONST);
    data.push(0);
    data.push(OPCODE_END);
    push_leb_u32(&mut data, segment.len() as u32);
    data.extend_from_slice(&segment);
    push_section(&mut module, SECTION_DATA, &data);

    module
}

#[cfg(feature = "wasm-engine-core")]
fn leak_wasm(mut module: Vec<u8>) -> &'static [u8] {
    module.shrink_to_fit();
    Box::leak(module.into_boxed_slice())
}

#[derive(Clone, Copy, Debug)]
struct TestLocalFrame {
    occupied: bool,
    lane: u8,
    frame_label: FrameLabel,
    len: usize,
    bytes: [u8; TEST_CARRIER_FRAME_BYTES],
}

impl TestLocalFrame {
    const EMPTY: Self = Self {
        occupied: false,
        lane: 0,
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
        lane: u8,
        frame_label: FrameLabel,
        payload: Payload<'_>,
    ) -> Result<(), TransportError> {
        let bytes = payload.as_bytes();
        if bytes.len() > TEST_CARRIER_FRAME_BYTES || self.len == TEST_CARRIER_QUEUE_DEPTH {
            return Err(TransportError::Failed);
        }
        let idx = (self.head + self.len) % TEST_CARRIER_QUEUE_DEPTH;
        self.frames[idx].occupied = true;
        self.frames[idx].lane = lane;
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

    fn pop_front(&mut self, lane: u8) -> Option<TestLocalFrame> {
        if self.len == 0 {
            return None;
        }
        let mut matched = None;
        for offset in 0..self.len {
            let idx = (self.head + offset) % TEST_CARRIER_QUEUE_DEPTH;
            if self.frames[idx].occupied && self.frames[idx].lane == lane {
                matched = Some(idx);
                break;
            }
        }
        let idx = matched?;
        let frame = self.frames[idx];
        let tail = (self.head + self.len - 1) % TEST_CARRIER_QUEUE_DEPTH;
        let mut cursor = idx;
        while cursor != tail {
            let next = (cursor + 1) % TEST_CARRIER_QUEUE_DEPTH;
            self.frames[cursor] = self.frames[next];
            cursor = next;
        }
        self.frames[tail] = TestLocalFrame::EMPTY;
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
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
    queues: UnsafeCell<TestLocalQueues>,
}

impl TestLocalQueueCarrier {
    fn new() -> Self {
        Self {
            queues: UnsafeCell::new(TestLocalQueues::EMPTY),
        }
    }

    fn queues(&self) -> &mut TestLocalQueues {
        // These host tests poll one role at a time on one thread. The queue is
        // a local carrier medium, not protocol state or cross-thread sharing.
        unsafe { &mut *self.queues.get() }
    }
}

#[derive(Clone, Copy, Debug)]
struct TestLocalQueueTx {
    local_role: u8,
    session_id: u32,
    lane: u8,
}

#[derive(Clone, Copy, Debug)]
struct TestLocalQueueRx {
    local_role: u8,
    session_id: u32,
    lane: u8,
    frame: Option<TestLocalFrame>,
}

impl hibana::integration::Transport for TestLocalQueueCarrier {
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

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (
            TestLocalQueueTx {
                local_role,
                session_id,
                lane,
            },
            TestLocalQueueRx {
                local_role,
                session_id,
                lane,
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
        if outgoing.lane() != tx.lane {
            return Poll::Ready(Err(TransportError::Failed));
        }
        let result = self.queues().by_role[peer].push_back(
            outgoing.lane(),
            outgoing.frame_label(),
            outgoing.payload(),
        );
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
        let Some(frame) = self.queues().by_role[local_role].pop_front(rx.lane) else {
            return Poll::Pending;
        };
        if frame.lane != rx.lane {
            return Poll::Ready(Err(TransportError::Failed));
        }
        rx.frame = Some(frame);
        cx.waker().wake_by_ref();
        Poll::Ready(Ok(rx.frame.as_ref().expect("frame stored").payload()))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        if let Some(frame) = rx.frame.take() {
            let local_role = rx.local_role as usize;
            if local_role < TEST_CARRIER_ROLES {
                self.queues().by_role[local_role].push_front(frame);
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
        scope: hibana::integration::cap::advanced::ScopeId,
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

thread_local! {
    static COUNTING_ENGINE_POLLS: Cell<usize> = const { Cell::new(0) };
    static COUNTING_DRIVER_POLLS: Cell<usize> = const { Cell::new(0) };
    static COUNTING_BOUNDARY_POLLS: Cell<usize> = const { Cell::new(0) };
    static CHOREOFS_RUNTIME_COMPLETIONS: Cell<usize> = const { Cell::new(0) };
}

fn increment_cell(cell: &'static std::thread::LocalKey<Cell<usize>>) {
    cell.with(|count| count.set(count.get() + 1));
}

fn read_cell(cell: &'static std::thread::LocalKey<Cell<usize>>) -> usize {
    cell.with(Cell::get)
}
const CHOREOFS_RUNTIME_OBJECT: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/led/green",
    appkit::ObjectId(7),
    appkit::FdSpec::new(4, 0x2, 11),
);
static CHOREOFS_RUNTIME_FACTS: appkit::ChoreoFsObjectSet<1> =
    appkit::ChoreoFsObjectSet::new([CHOREOFS_RUNTIME_OBJECT]);

struct WrappedRunExit<R, I> {
    #[cfg(feature = "wasm-engine-core")]
    report: appkit::RunReport<R, I>,
    #[cfg(not(feature = "wasm-engine-core"))]
    report_type: core::marker::PhantomData<(R, I)>,
}

impl<R, I> appkit::FromRunReport<R, I> for WrappedRunExit<R, I> {
    fn from_run_report(report: appkit::RunReport<R, I>) -> Self {
        #[cfg(feature = "wasm-engine-core")]
        {
            Self { report }
        }
        #[cfg(not(feature = "wasm-engine-core"))]
        {
            core::hint::black_box(report);
            Self {
                report_type: core::marker::PhantomData,
            }
        }
    }
}

impl appkit::Capsule for RichCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = RichPlacement;
    type Local = RichLocal;
    type Report = usize;

    fn choreography() -> impl Projectable<Self::Universe> {
        let fd_write = g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
        );
        let direct = g::route(
            g::seq(
                g::send::<g::Role<0>, g::Role<0>, WasiImportLoopContinue, 0>().policy::<8>(),
                fd_write,
            ),
            g::send::<g::Role<0>, g::Role<0>, WasiImportLoopBreak, 0>().policy::<8>(),
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            let mut ctx = ctx;
            if ROLE == 1 {
                let branch = ctx
                    .endpoint()
                    .offer()
                    .await
                    .expect("rich driver offers fd_write branch");
                assert_eq!(branch.label(), LABEL_WASI_FD_WRITE);
                let request = branch
                    .decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                    .await
                    .expect("rich driver decodes fd_write through endpoint");
                let EngineReq::FdWrite(write) = request else {
                    panic!("rich driver expected fd_write request");
                };
                let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
                ctx.endpoint()
                    .flow::<g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
                    .expect("rich driver opens fd_write reply flow")
                    .send(&reply)
                    .await
                    .expect("rich driver replies to fd_write");
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, RichCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl appkit::Capsule for IncompleteCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = IncompletePlacement;
    type Local = IncompleteLocal;
    type Report = usize;

    fn choreography() -> impl Projectable<Self::Universe> {
        g::route(
            g::seq(
                g::send::<g::Role<0>, g::Role<0>, WasiImportLoopContinue, 0>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
            ),
            g::send::<g::Role<0>, g::Role<0>, WasiImportLoopBreak, 0>(),
        )
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, IncompleteCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, CustomLabelCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        increment_cell(&COUNTING_ENGINE_POLLS);
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        increment_cell(&COUNTING_DRIVER_POLLS);
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        increment_cell(&COUNTING_BOUNDARY_POLLS);
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, CountingCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl appkit::Capsule for ChoreoFsRuntimeCapsule {
    type Universe = DefaultLabelUniverse;
    type Placement = ChoreoFsRuntimePlacement;
    type Local = ChoreoFsRuntimeLocal;
    type Report = ();

    fn choreography() -> impl Projectable<Self::Universe> {
        let path_open = g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 0>(),
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 0>(),
        );
        let fd_write = g::route(
            g::seq(
                g::send::<g::Role<0>, g::Role<0>, WasiImportLoopContinue, 0>(),
                g::seq(
                    g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
                    g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(
                    ),
                ),
            ),
            g::send::<g::Role<0>, g::Role<0>, WasiImportLoopBreak, 0>(),
        );
        g::seq(path_open, fd_write)
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
    type Error = core::convert::Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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

            let write_branch = ctx
                .endpoint()
                .offer()
                .await
                .expect("driver offers fd_write branch");
            assert_eq!(write_branch.label(), LABEL_WASI_FD_WRITE);
            let write_request = write_branch
                .decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
                .await
                .expect("driver decodes fd_write through endpoint");
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
            increment_cell(&CHOREOFS_RUNTIME_COMPLETIONS);
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, ChoreoFsRuntimeCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
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

struct MemoryFrame {
    lane: u8,
    bytes: Vec<u8>,
}

struct MemoryTransport {
    queues: [UnsafeCell<VecDeque<MemoryFrame>>; MEMORY_ROLE_COUNT],
}

struct MemoryTx {
    role: u8,
    lane: u8,
}

struct MemoryRx {
    role: u8,
    lane: u8,
    current: Option<Vec<u8>>,
    delivered: bool,
}

impl MemoryTransport {
    fn new() -> Self {
        Self {
            queues: std::array::from_fn(|role| {
                assert!(role < MEMORY_ROLE_COUNT);
                UnsafeCell::new(VecDeque::new())
            }),
        }
    }

    fn queue(&self, role: usize) -> &mut VecDeque<MemoryFrame> {
        assert!(role < MEMORY_ROLE_COUNT);
        // The memory transport is a single-threaded test carrier. Its queue is
        // only mutated during one poll operation and never models protocol authority.
        unsafe { &mut *self.queues[role].get() }
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

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        assert!((local_role as usize) < MEMORY_ROLE_COUNT);
        core::hint::black_box(session_id);
        (
            MemoryTx {
                role: local_role,
                lane,
            },
            MemoryRx {
                role: local_role,
                lane,
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
        if outgoing.lane() != tx.lane {
            return Poll::Ready(Err(TransportError::Failed));
        }
        self.queue(peer).push_back(MemoryFrame {
            lane: outgoing.lane(),
            bytes: outgoing.payload().as_bytes().to_vec(),
        });
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
            let queue = self.queue(rx.role as usize);
            let mut selected = None;
            for idx in 0..queue.len() {
                if queue[idx].lane == rx.lane {
                    selected = Some(idx);
                    break;
                }
            }
            rx.current = selected.and_then(|idx| queue.remove(idx).map(|frame| frame.bytes));
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
        if let Some(bytes) = rx.current.take() {
            self.queue(rx.role as usize).push_front(MemoryFrame {
                lane: rx.lane,
                bytes,
            });
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
    let yellow = g::seq(
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
    g::seq(path_open, g::seq(green, g::seq(yellow, red)))
}

fn single_exchange_program() -> impl Projectable<DefaultLabelUniverse> {
    g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 0>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 0>(),
    )
}

fn choreofs_std_wasi_prefix_program() -> impl Projectable<DefaultLabelUniverse> {
    type WithEnv = g::Msg<147, GenericCapToken<RouteControl<147, 0>>, RouteControl<147, 0>>;
    type Direct = g::Msg<148, GenericCapToken<RouteControl<148, 1>>, RouteControl<148, 1>>;

    let environ_sizes_get = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_ENVIRON_SIZES_GET, EngineReq>, 1>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_ENVIRON_SIZES_GET_RET, EngineRet>, 1>(),
    );
    let environ_get = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_ENVIRON_GET, EngineReq>, 1>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_ENVIRON_GET_RET, EngineRet>, 1>(),
    );
    let path_open = || {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(),
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 1>(),
        )
    };
    g::route(
        g::seq(
            g::send::<g::Role<1>, g::Role<1>, WithEnv, 1>(),
            g::seq(environ_sizes_get, g::seq(environ_get, path_open())),
        ),
        g::seq(g::send::<g::Role<1>, g::Role<1>, Direct, 1>(), path_open()),
    )
}

#[test]
fn std_wasi_env_prefix_reaches_choreofs_path_open() {
    let program = choreofs_std_wasi_prefix_program();
    assert!(choreofs_traffic_attach_succeeds::<0>(&program, 262_144));
    assert!(choreofs_traffic_attach_succeeds::<1>(&program, 262_144));
    let role0: RoleProgram<0> = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
    let role1: RoleProgram<1> = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); 128];
    let mut slab = [0u8; 262_144];
    let clock = CounterClock::new();
    let kit = hibana::integration::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        2,
    >::new(&clock);
    let rv = kit
        .add_rendezvous_from_config(
            Config::from_resources(&mut tap_buf, &mut slab, CounterClock::new()),
            MemoryTransport::new(),
        )
        .expect("register rendezvous");
    let sid = SessionId::new(0x5745);
    let mut driver = kit
        .enter::<0, _>(rv, sid, &role0, NoBinding)
        .expect("enter driver role");
    let mut engine = kit
        .enter::<1, _>(rv, sid, &role1, NoBinding)
        .expect("enter engine role");

    type WithEnv = g::Msg<147, GenericCapToken<RouteControl<147, 0>>, RouteControl<147, 0>>;
    block_on(
        engine
            .flow::<WithEnv>()
            .expect("engine selects env prefix route")
            .send(()),
    )
    .expect("send env prefix route selection");

    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_ENVIRON_SIZES_GET, EngineReq>>()
            .expect("engine environ_sizes_get flow")
            .send(&EngineReq::EnvironSizesGet(EnvironSizesGet::new())),
    )
    .expect("send environ_sizes_get");
    let branch = block_on(driver.offer()).expect("driver offers optional env/path_open branch");
    assert_eq!(branch.label(), LABEL_WASI_ENVIRON_SIZES_GET);
    assert_eq!(
        block_on(branch.decode::<g::Msg<LABEL_WASI_ENVIRON_SIZES_GET, EngineReq>>())
            .expect("driver decodes environ_sizes_get branch"),
        EngineReq::EnvironSizesGet(EnvironSizesGet::new())
    );
    block_on(
        driver
            .flow::<g::Msg<LABEL_WASI_ENVIRON_SIZES_GET_RET, EngineRet>>()
            .expect("driver environ_sizes_get reply flow")
            .send(&EngineRet::EnvironSizes(EnvironSizes::new(1, 8))),
    )
    .expect("send environ_sizes_get reply");
    assert_eq!(
        block_on(engine.recv::<g::Msg<LABEL_WASI_ENVIRON_SIZES_GET_RET, EngineRet>>())
            .expect("engine receives environ_sizes_get reply"),
        EngineRet::EnvironSizes(EnvironSizes::new(1, 8))
    );

    let environ_get = EngineReq::EnvironGet(EnvironGet::new(8).expect("environ_get"));
    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_ENVIRON_GET, EngineReq>>()
            .expect("engine environ_get flow")
            .send(&environ_get),
    )
    .expect("send environ_get");
    assert_eq!(
        block_on(driver.recv::<g::Msg<LABEL_WASI_ENVIRON_GET, EngineReq>>())
            .expect("driver receives environ_get"),
        environ_get
    );
    let environ_done =
        EngineRet::EnvironDone(EnvironDone::new_with_lease(0, b"HIBANA").expect("env done"));
    block_on(
        driver
            .flow::<g::Msg<LABEL_WASI_ENVIRON_GET_RET, EngineRet>>()
            .expect("driver environ_get reply flow")
            .send(&environ_done),
    )
    .expect("send environ_get reply");
    assert_eq!(
        block_on(engine.recv::<g::Msg<LABEL_WASI_ENVIRON_GET_RET, EngineRet>>())
            .expect("engine receives environ_get reply"),
        environ_done
    );

    let path_open = EngineReq::PathOpen(
        PathOpen::new(3, 0, 0x2, b"device/led/green").expect("path_open request"),
    );
    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
            .expect("engine path_open flow after env prefix")
            .send(&path_open),
    )
    .expect("send path_open after env prefix");
    assert_eq!(
        block_on(driver.recv::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>())
            .expect("driver receives path_open after env prefix"),
        path_open
    );
}

#[test]
fn std_wasi_optional_env_prefix_allows_direct_choreofs_path_open() {
    let program = choreofs_std_wasi_prefix_program();
    let role0: RoleProgram<0> = Projectable::<DefaultLabelUniverse>::project::<0>(&program);
    let role1: RoleProgram<1> = Projectable::<DefaultLabelUniverse>::project::<1>(&program);
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); 128];
    let mut slab = [0u8; 262_144];
    let clock = CounterClock::new();
    let kit = hibana::integration::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        2,
    >::new(&clock);
    let rv = kit
        .add_rendezvous_from_config(
            Config::from_resources(&mut tap_buf, &mut slab, CounterClock::new()),
            MemoryTransport::new(),
        )
        .expect("register rendezvous");
    let sid = SessionId::new(0x5746);
    let mut driver = kit
        .enter::<0, _>(rv, sid, &role0, NoBinding)
        .expect("enter driver role");
    let mut engine = kit
        .enter::<1, _>(rv, sid, &role1, NoBinding)
        .expect("enter engine role");

    type Direct = g::Msg<148, GenericCapToken<RouteControl<148, 1>>, RouteControl<148, 1>>;
    block_on(
        engine
            .flow::<Direct>()
            .expect("engine selects direct path_open route")
            .send(()),
    )
    .expect("send direct path_open route selection");

    let path_open = EngineReq::PathOpen(
        PathOpen::new(3, 0, 0x2, b"device/led/green").expect("path_open request"),
    );
    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
            .expect("engine path_open flow without env prefix")
            .send(&path_open),
    )
    .expect("send path_open without env prefix");
    let branch = block_on(driver.offer()).expect("driver offers optional env/path_open branch");
    assert_eq!(branch.label(), LABEL_WASI_PATH_OPEN);
    assert_eq!(
        block_on(branch.decode::<g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>())
            .expect("driver decodes direct path_open branch"),
        path_open
    );
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

fn choreofs_baker_wasi_traffic_program() -> impl Projectable<DefaultLabelUniverse> {
    type OpenWithEnv = g::Msg<147, GenericCapToken<RouteControl<147, 0>>, RouteControl<147, 0>>;
    type OpenDirect = g::Msg<148, GenericCapToken<RouteControl<148, 1>>, RouteControl<148, 1>>;
    type Continue =
        g::Msg<{ TEST_LOOP_CONTINUE_LOGICAL }, GenericCapToken<LoopContinueKind>, LoopContinueKind>;
    type Break = g::Msg<{ TEST_LOOP_BREAK_LOGICAL }, GenericCapToken<LoopBreakKind>, LoopBreakKind>;

    let environ_sizes_get = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_ENVIRON_SIZES_GET, EngineReq>, 1>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_ENVIRON_SIZES_GET_RET, EngineRet>, 1>(),
    );
    let environ_get = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_ENVIRON_GET, EngineReq>, 1>(),
        g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_ENVIRON_GET_RET, EngineRet>, 1>(),
    );
    let path_open = || {
        g::seq(
            g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(),
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 1>(),
        )
    };
    let open_leds = || g::seq(path_open(), g::seq(path_open(), path_open()));
    let open_leds_with_optional_env = g::route(
        g::seq(
            g::send::<g::Role<1>, g::Role<1>, OpenWithEnv, 1>(),
            g::seq(environ_sizes_get, g::seq(environ_get, open_leds())),
        ),
        g::seq(
            g::send::<g::Role<1>, g::Role<1>, OpenDirect, 1>(),
            open_leds(),
        ),
    );
    let cycle = g::seq(
        g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
        g::seq(
            g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
            g::seq(
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 1>(),
                g::send::<g::Role<0>, g::Role<1>, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 1>(
                ),
            ),
        ),
    );
    g::seq(
        open_leds_with_optional_env,
        g::route(
            g::seq(g::send::<g::Role<1>, g::Role<1>, Continue, 1>(), cycle),
            g::seq(
                g::send::<g::Role<1>, g::Role<1>, Break, 1>(),
                g::send::<g::Role<1>, g::Role<0>, g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
            ),
        ),
    )
}

fn choreofs_traffic_attach_succeeds<const ROLE: u8>(
    program: &impl Projectable<DefaultLabelUniverse>,
    slab_bytes: usize,
) -> bool {
    let role = program.project::<ROLE>();
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); 128];
    let mut slab = vec![0u8; slab_bytes];
    let clock = CounterClock::new();
    let transport = MemoryTransport::new();
    let kit = hibana::integration::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        1,
    >::new(&clock);
    let Ok(rendezvous) = kit.add_rendezvous_from_config(
        Config::from_resources(&mut tap_buf, slab.as_mut_slice(), CounterClock::new()),
        transport,
    ) else {
        return false;
    };
    kit.enter::<ROLE, _>(rendezvous, SessionId::new(2040), &role, NoBinding)
        .is_ok()
}

fn choreofs_traffic_embedded_attach_succeeds<const ROLE: u8>(
    program: &impl Projectable<DefaultLabelUniverse>,
    slab_bytes: usize,
) -> bool {
    let role = program.project::<ROLE>();
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); 128];
    let mut slab = vec![0u8; slab_bytes];
    type Kit<'a> =
        hibana::integration::SessionKit<'a, MemoryTransport, DefaultLabelUniverse, CounterClock, 1>;

    let base = slab.as_mut_ptr() as usize;
    let kit_start = align_up_for_test(base, core::mem::align_of::<Kit<'_>>());
    let Some(kit_end) = kit_start.checked_add(core::mem::size_of::<Kit<'_>>()) else {
        return false;
    };
    let Some(total_end) = base.checked_add(slab.len()) else {
        return false;
    };
    if kit_end > total_end {
        return false;
    }

    let kit_offset = kit_start - base;
    let rest_offset = kit_end - base;
    let clock = CounterClock::new();
    let kit_storage = unsafe {
        &mut *slab
            .as_mut_ptr()
            .add(kit_offset)
            .cast::<core::mem::MaybeUninit<Kit<'_>>>()
    };
    let rendezvous_slab = &mut slab[rest_offset..];
    let kit = hibana::integration::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        1,
    >::init_in_place(kit_storage, &clock);
    let Ok(rendezvous) = kit.add_rendezvous_from_config(
        Config::from_resources(&mut tap_buf, rendezvous_slab, CounterClock::new()),
        MemoryTransport::new(),
    ) else {
        return false;
    };
    kit.enter::<ROLE, _>(rendezvous, SessionId::new(2040), &role, NoBinding)
        .is_ok()
}

fn align_up_for_test(value: usize, align: usize) -> usize {
    let mask = align.saturating_sub(1);
    (value + mask) & !mask
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

fn minimum_choreofs_traffic_embedded_attach_slab<const ROLE: u8>(
    program: &impl Projectable<DefaultLabelUniverse>,
) -> usize {
    let mut slab = 4 * 1024;
    while slab <= 128 * 1024 {
        if choreofs_traffic_embedded_attach_succeeds::<ROLE>(program, slab) {
            return slab;
        }
        slab += 1024;
    }
    panic!("role {ROLE} did not embedded-attach within 128 KiB");
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
    let baker_program = choreofs_baker_wasi_traffic_program();
    let baker_role0 = minimum_choreofs_traffic_attach_slab::<0>(&baker_program);
    let baker_role1 = minimum_choreofs_traffic_attach_slab::<1>(&baker_program);
    let baker_embedded_role0 = minimum_choreofs_traffic_embedded_attach_slab::<0>(&baker_program);
    let baker_embedded_role1 = minimum_choreofs_traffic_embedded_attach_slab::<1>(&baker_program);

    println!("single exchange role0 attach slab bytes: {single_role0}");
    println!("single exchange role1 attach slab bytes: {single_role1}");
    println!("choreofs traffic role0 attach slab bytes: {role0}");
    println!("choreofs traffic role1 attach slab bytes: {role1}");
    println!("choreofs loop traffic role0 attach slab bytes: {loop_role0}");
    println!("choreofs loop traffic role1 attach slab bytes: {loop_role1}");
    println!("baker wasi choreofs traffic role0 attach slab bytes: {baker_role0}");
    println!("baker wasi choreofs traffic role1 attach slab bytes: {baker_role1}");
    println!(
        "baker wasi choreofs traffic role0 embedded attach slab bytes: {baker_embedded_role0}"
    );
    println!(
        "baker wasi choreofs traffic role1 embedded attach slab bytes: {baker_embedded_role1}"
    );
    assert!(single_role0 <= role0);
    assert!(single_role1 <= role1);
    assert!(baker_role0 <= baker_embedded_role0);
    assert!(baker_role1 <= baker_embedded_role1);
}

fn exercise_fd_write_endpoint_round_trip(program: &impl Projectable<DefaultLabelUniverse>) {
    let role0: RoleProgram<0> = Projectable::<DefaultLabelUniverse>::project::<0>(program);
    let role1: RoleProgram<1> = Projectable::<DefaultLabelUniverse>::project::<1>(program);
    let mut tap_buf = [hibana::integration::tap::TapEvent::zero(); 128];
    let mut slab = [0u8; 262_144];
    let clock = CounterClock::new();
    let kit = hibana::integration::SessionKit::<
        MemoryTransport,
        DefaultLabelUniverse,
        CounterClock,
        2,
    >::new(&clock);
    let rv = kit
        .add_rendezvous_from_config(
            Config::from_resources(&mut tap_buf, &mut slab, CounterClock::new()),
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
    block_on(
        engine
            .flow::<WasiImportLoopContinue>()
            .expect("engine opens loop continue flow")
            .send(()),
    )
    .expect("send loop continue through endpoint");
    let request = EngineReq::FdWrite(FdWrite::new(1, b"hello").expect("fd write request"));
    block_on(
        engine
            .flow::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine fd_write flow")
            .send(&request),
    )
    .expect("send fd_write request through endpoint");
    let branch = block_on(driver.offer()).expect("driver offers fd_write branch");
    assert_eq!(branch.label(), LABEL_WASI_FD_WRITE);
    let observed_request = block_on(branch.decode::<g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .expect("driver decodes fd_write request through endpoint");
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
thread_local! {
    static HOST_CAPSULE_WASI_GUEST_ARENA: UnsafeCell<appkit::WasiGuestArena> =
        const { UnsafeCell::new(appkit::WasiGuestArena::empty()) };
}

#[cfg(feature = "wasm-engine-core")]
fn host_capsule_wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
    core::hint::black_box(ROLE);
    HOST_CAPSULE_WASI_GUEST_ARENA.with(|arena| {
        let arena = unsafe { &mut *arena.get() };
        arena.lease()
    })
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
}

#[cfg(feature = "wasm-engine-core")]
impl appkit::WasiGuestImage<RichCapsule> for site::Local<image::Composite> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        host_capsule_wasi_guest_lease::<ROLE>()
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
}

#[cfg(feature = "wasm-engine-core")]
impl appkit::WasiGuestImage<RichCapsule> for site::Local<image::WrappedExit> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        host_capsule_wasi_guest_lease::<ROLE>()
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
}

#[cfg(feature = "wasm-engine-core")]
impl appkit::WasiGuestImage<IncompleteCapsule> for site::Local<image::Composite> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        host_capsule_wasi_guest_lease::<ROLE>()
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

    fn driver_facts() -> appkit::DriverFacts<'static> {
        CHOREOFS_RUNTIME_FACTS.driver_facts()
    }
}

#[cfg(feature = "wasm-engine-core")]
impl appkit::WasiGuestImage<ChoreoFsRuntimeCapsule> for site::Local<image::ChoreoFsRuntime> {
    fn wasi_guest_lease<'guest, const ROLE: u8>() -> appkit::WasiGuestLease<'guest> {
        host_capsule_wasi_guest_lease::<ROLE>()
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
    assert_eq!(caps.policy_count, 2);
    assert!(caps.policies[..caps.policy_count as usize].contains(&7));
    assert!(caps.policies[..caps.policy_count as usize].contains(&8));
    assert!(caps.control_count >= 2);
    assert!(
        caps.control_ops[..caps.control_count as usize].contains(&ControlOp::RouteDecision.as_u8())
    );
    assert!(
        caps.control_ops[..caps.control_count as usize].contains(&ControlOp::LoopContinue.as_u8())
    );
    assert!(caps.control_tap_ids[..caps.control_count as usize].contains(&0x707));
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
fn wasi_capacity_requires_numeric_request_completion_pair() {
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
    let before_engine = read_cell(&COUNTING_ENGINE_POLLS);
    let before_driver = read_cell(&COUNTING_DRIVER_POLLS);
    let before_boundary = read_cell(&COUNTING_BOUNDARY_POLLS);
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
    assert_eq!(read_cell(&COUNTING_ENGINE_POLLS), before_engine + 1);
    assert_eq!(read_cell(&COUNTING_DRIVER_POLLS), before_driver + 1);
    assert_eq!(read_cell(&COUNTING_BOUNDARY_POLLS), before_boundary + 1);
}

#[test]
#[cfg(all(feature = "wasm-engine-core", feature = "wasip1-sys-path-open"))]
fn choreofs_facts_are_consumed_by_driver_ctx_during_endpoint_progress() {
    let before = read_cell(&CHOREOFS_RUNTIME_COMPLETIONS);
    let wasm = leak_wasm(path_open_fd_write_guest_module());
    let report = appkit::run::<site::Local<image::ChoreoFsRuntime>, ChoreoFsRuntimeCapsule>(
        appkit::WasiImage::from_static(wasm),
    );

    assert_eq!(report.artifact_len(), wasm.len());
    assert_eq!(report.attached_endpoint_count(), 2);
    assert_eq!(
        report.endpoint_carrier().wasi_imports(),
        appkit::WasiImports::FD_WRITE.union(appkit::WasiImports::PATH_OPEN)
    );
    assert_eq!(report.endpoint_carrier().wasi_completion_pair_count(), 2);
    assert_eq!(read_cell(&CHOREOFS_RUNTIME_COMPLETIONS), before + 1);
}

#[test]
#[cfg(feature = "wasm-engine-core")]
fn run_takes_artifact_as_dynamic_input() {
    let wasm = leak_wasm(fd_write_guest_module());
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(wasm),
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
    assert_eq!(report.artifact_len(), wasm.len());
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
    assert_eq!(manifest.policy_count, 2);
    assert!(manifest.policies[..manifest.policy_count as usize].contains(&7));
    assert!(manifest.policies[..manifest.policy_count as usize].contains(&8));
    assert!(manifest.control_count >= 2);
    assert!(
        manifest.control_ops[..manifest.control_count as usize]
            .contains(&ControlOp::RouteDecision.as_u8())
    );
    assert!(
        manifest.control_ops[..manifest.control_count as usize]
            .contains(&ControlOp::LoopContinue.as_u8())
    );
    assert!(manifest.control_tap_ids[..manifest.control_count as usize].contains(&0x707));
    assert_eq!(manifest.wasi_completion_pair_count, 1);
    assert_eq!(report.wasi_completion_pair_count(), 1);
}

#[test]
#[cfg(feature = "wasm-engine-core")]
fn image_manifest_peer_attach_requires_mutual_identity_and_matching_shape() {
    let wasm = leak_wasm(fd_write_guest_module());
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(wasm),
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

    let mut host_metadata_peer = peer;
    host_metadata_peer.capsule_fingerprint = [0x11; 2];
    host_metadata_peer.placement_fingerprint = [0x22; 2];
    host_metadata_peer.label_universe_fingerprint = [0x33; 2];
    assert!(
        this.can_attach_peer(&host_metadata_peer),
        "type-name fingerprints are host metadata, not peer attach authority"
    );

    peer.carrier = TEST_TCP;
    assert!(!this.can_attach_peer(&peer));
    peer.carrier = TEST_LOCAL_QUEUE_CARRIER;
    peer.choreography_session_id = peer.choreography_session_id.wrapping_add(1);
    assert!(!this.can_attach_peer(&peer));
}

#[test]
#[cfg(feature = "wasm-engine-core")]
fn run_returns_logical_image_exit_type() {
    let wasm = leak_wasm(fd_write_guest_module());
    let artifacts = RichArtifacts {
        image: appkit::WasiImage::from_static(wasm),
    };
    let image_artifact = <RichArtifacts<'static> as appkit::ArtifactBundle<RichCapsule>>::for_image::<
        site::Local<image::WrappedExit>,
    >(&artifacts);

    let wrapped = appkit::run::<site::Local<image::WrappedExit>, RichCapsule>(image_artifact);

    assert_eq!(wrapped.report.image_id(), appkit::ImageId(47));
    assert_eq!(wrapped.report.attached_endpoint_count(), 4);
    assert_eq!(wrapped.report.artifact_len(), wasm.len());
}

#[test]
#[should_panic(
    expected = "WASI P1 import request label must have a projected numeric EngineRet completion"
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
fn hibana_integration_surfaces_remain_available_to_capsules() {
    fn route_resolution(ctx: ResolverContext) -> Result<RouteResolution, ResolverError> {
        if ctx
            .attr(hibana::integration::policy::signals::core::LANE)
            .is_some()
        {
            Ok(RouteResolution::Arm(0))
        } else {
            Ok(RouteResolution::Defer)
        }
    }

    fn loop_resolution(ctx: ResolverContext) -> Result<LoopResolution, ResolverError> {
        if ctx.input(1) == 0 {
            Ok(LoopResolution::Continue)
        } else {
            Ok(LoopResolution::Defer)
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
#[cfg(feature = "wasm-engine-core")]
fn driver_facts_are_separate_from_progress_authority() {
    const LED_DEVICE: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
        b"device/led/green",
        appkit::ObjectId(7),
        appkit::FdSpec::new(3, 0x2, 11),
    );
    static FACTS: appkit::ChoreoFsObjectSet<1> = appkit::ChoreoFsObjectSet::new([LED_DEVICE]);

    let facts = FACTS.driver_facts();
    let wasm = leak_wasm(fd_write_guest_module());
    let report = appkit::run::<site::Local<image::Composite>, RichCapsule>(
        appkit::WasiImage::from_static(wasm),
    );

    assert_eq!(report.artifact_len(), wasm.len());
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
#[cfg(feature = "wasm-engine-core")]
fn wasi_static_import_table_is_not_choreography_authority() {
    let p1 = leak_wasm(fd_write_guest_module());
    let report = appkit::run::<site::Local<image::Composite>, RichCapsule>(
        appkit::WasiImage::from_static(p1),
    );
    assert_eq!(report.wasi_imports(), appkit::WasiImports::FD_WRITE);

    let std_like = leak_wasm(fd_write_with_unused_std_wasi_imports_module());
    let std_like_report = appkit::run::<site::Local<image::Composite>, RichCapsule>(
        appkit::WasiImage::from_static(std_like),
    );
    assert_eq!(
        std_like_report.wasi_imports(),
        appkit::WasiImports::FD_WRITE
    );

    let extra_import = leak_wasm(path_open_fd_write_guest_module());
    let extra_import_run = std::panic::catch_unwind(|| {
        appkit::run::<site::Local<image::Composite>, RichCapsule>(appkit::WasiImage::from_static(
            extra_import,
        ));
    });
    assert!(extra_import_run.is_err());

    let foreign_import = leak_wasm(fd_write_with_non_wasi_import_module());
    let foreign_import_run = std::panic::catch_unwind(|| {
        appkit::run::<site::Local<image::Composite>, RichCapsule>(appkit::WasiImage::from_static(
            foreign_import,
        ));
    });
    assert!(foreign_import_run.is_err());
}

struct CaptureProgramFacts {
    seen_program: bool,
}

impl hibana::integration::program::ProjectionMetadataVisitor for CaptureProgramFacts {
    fn visit_program(&mut self, facts: hibana::integration::program::ProjectionProgramFacts) {
        self.seen_program = true;
        assert!(facts.eff_count >= 4);
        assert!(facts.parallel_enter_count >= 1);
        assert!(facts.route_scope_count >= 1);
    }

    fn visit_atom(&mut self, spec: hibana::integration::program::ProjectionAtomSpec) {
        if spec.is_control {
            match spec.control_op {
                Some(op) if op == ControlOp::RouteDecision.as_u8() => {
                    assert_eq!(spec.control_tap_id, Some(0x707));
                }
                Some(op)
                    if op == ControlOp::LoopContinue.as_u8()
                        || op == ControlOp::LoopBreak.as_u8() => {}
                other => panic!("unexpected control op in projection metadata: {other:?}"),
            }
            assert!(spec.control_scope.is_some());
            assert!(spec.control_path.is_some());
            assert!(spec.control_shot.is_some());
        }
    }
}
