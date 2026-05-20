use crate::{
    choreography::protocol::{BudgetExpired, BudgetRun, WASIP1_STREAM_CHUNK_CAPACITY},
    kernel::features::{WASIP1_PREVIEW1_MODULE, Wasip1HandlerSet, Wasip1ImportName, Wasip1Syscall},
};

const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

const SECTION_TYPE: u8 = 1;
const SECTION_IMPORT: u8 = 2;
const SECTION_FUNCTION: u8 = 3;
const SECTION_TABLE: u8 = 4;
const SECTION_ELEMENT: u8 = 9;
const SECTION_EXPORT: u8 = 7;
const SECTION_CODE: u8 = 10;
const SECTION_DATA: u8 = 11;
const SECTION_CUSTOM: u8 = 0;

const EXTERNAL_KIND_FUNC: u8 = 0;
const FUNC_TYPE_FORM: u8 = 0x60;
const VALTYPE_I32: u8 = 0x7f;
const VALTYPE_I64: u8 = 0x7e;
const VALTYPE_F32: u8 = 0x7d;
const VALTYPE_F64: u8 = 0x7c;
const VALTYPE_FUNCREF: u8 = 0x70;

const OPCODE_UNREACHABLE: u8 = 0x00;
const OPCODE_NOP: u8 = 0x01;
const OPCODE_BLOCK: u8 = 0x02;
const OPCODE_LOOP: u8 = 0x03;
const OPCODE_IF: u8 = 0x04;
const OPCODE_ELSE: u8 = 0x05;
const OPCODE_BR: u8 = 0x0c;
const OPCODE_BR_IF: u8 = 0x0d;
const OPCODE_BR_TABLE: u8 = 0x0e;
const OPCODE_RETURN: u8 = 0x0f;
const OPCODE_CALL: u8 = 0x10;
const OPCODE_CALL_INDIRECT: u8 = 0x11;
const OPCODE_SELECT: u8 = 0x1b;
const OPCODE_DROP: u8 = 0x1a;
const OPCODE_LOCAL_GET: u8 = 0x20;
const OPCODE_LOCAL_SET: u8 = 0x21;
const OPCODE_LOCAL_TEE: u8 = 0x22;
const OPCODE_GLOBAL_GET: u8 = 0x23;
const OPCODE_GLOBAL_SET: u8 = 0x24;
const OPCODE_TABLE_GET: u8 = 0x25;
const OPCODE_TABLE_SET: u8 = 0x26;
const OPCODE_I32_LOAD: u8 = 0x28;
const OPCODE_I64_LOAD: u8 = 0x29;
const OPCODE_F32_LOAD: u8 = 0x2a;
const OPCODE_F64_LOAD: u8 = 0x2b;
const OPCODE_I32_LOAD8_S: u8 = 0x2c;
const OPCODE_I32_LOAD8_U: u8 = 0x2d;
const OPCODE_I32_LOAD16_S: u8 = 0x2e;
const OPCODE_I32_LOAD16_U: u8 = 0x2f;
const OPCODE_I64_LOAD8_S: u8 = 0x30;
const OPCODE_I64_LOAD8_U: u8 = 0x31;
const OPCODE_I64_LOAD16_S: u8 = 0x32;
const OPCODE_I64_LOAD16_U: u8 = 0x33;
const OPCODE_I64_LOAD32_S: u8 = 0x34;
const OPCODE_I64_LOAD32_U: u8 = 0x35;
const OPCODE_I32_STORE: u8 = 0x36;
const OPCODE_I64_STORE: u8 = 0x37;
const OPCODE_F32_STORE: u8 = 0x38;
const OPCODE_F64_STORE: u8 = 0x39;
const OPCODE_I32_STORE8: u8 = 0x3a;
const OPCODE_I32_STORE16: u8 = 0x3b;
const OPCODE_I64_STORE8: u8 = 0x3c;
const OPCODE_I64_STORE16: u8 = 0x3d;
const OPCODE_I64_STORE32: u8 = 0x3e;
const OPCODE_MEMORY_SIZE: u8 = 0x3f;
const OPCODE_MEMORY_GROW: u8 = 0x40;
const OPCODE_I32_CONST: u8 = 0x41;
const OPCODE_I64_CONST: u8 = 0x42;
const OPCODE_F32_CONST: u8 = 0x43;
const OPCODE_F64_CONST: u8 = 0x44;
const OPCODE_I32_EQZ: u8 = 0x45;
const OPCODE_I32_EQ: u8 = 0x46;
const OPCODE_I32_NE: u8 = 0x47;
const OPCODE_I32_LT_S: u8 = 0x48;
const OPCODE_I32_LT_U: u8 = 0x49;
const OPCODE_I32_GT_S: u8 = 0x4a;
const OPCODE_I32_GT_U: u8 = 0x4b;
const OPCODE_I32_LE_S: u8 = 0x4c;
const OPCODE_I32_LE_U: u8 = 0x4d;
const OPCODE_I32_GE_S: u8 = 0x4e;
const OPCODE_I32_GE_U: u8 = 0x4f;
const OPCODE_I64_EQZ: u8 = 0x50;
const OPCODE_I64_EQ: u8 = 0x51;
const OPCODE_I64_NE: u8 = 0x52;
const OPCODE_I64_LT_S: u8 = 0x53;
const OPCODE_I64_LT_U: u8 = 0x54;
const OPCODE_I64_GT_S: u8 = 0x55;
const OPCODE_I64_GT_U: u8 = 0x56;
const OPCODE_I64_LE_S: u8 = 0x57;
const OPCODE_I64_LE_U: u8 = 0x58;
const OPCODE_I64_GE_S: u8 = 0x59;
const OPCODE_I64_GE_U: u8 = 0x5a;
const OPCODE_F32_EQ: u8 = 0x5b;
const OPCODE_F32_NE: u8 = 0x5c;
const OPCODE_F32_LT: u8 = 0x5d;
const OPCODE_F32_GT: u8 = 0x5e;
const OPCODE_F32_LE: u8 = 0x5f;
const OPCODE_F32_GE: u8 = 0x60;
const OPCODE_F64_EQ: u8 = 0x61;
const OPCODE_F64_NE: u8 = 0x62;
const OPCODE_F64_LT: u8 = 0x63;
const OPCODE_F64_GT: u8 = 0x64;
const OPCODE_F64_LE: u8 = 0x65;
const OPCODE_F64_GE: u8 = 0x66;
const OPCODE_I32_CLZ: u8 = 0x67;
const OPCODE_I32_CTZ: u8 = 0x68;
const OPCODE_I32_POPCNT: u8 = 0x69;
const OPCODE_I32_ADD: u8 = 0x6a;
const OPCODE_I32_SUB: u8 = 0x6b;
const OPCODE_I32_MUL: u8 = 0x6c;
const OPCODE_I32_DIV_S: u8 = 0x6d;
const OPCODE_I32_DIV_U: u8 = 0x6e;
const OPCODE_I32_REM_S: u8 = 0x6f;
const OPCODE_I32_REM_U: u8 = 0x70;
const OPCODE_I32_AND: u8 = 0x71;
const OPCODE_I32_OR: u8 = 0x72;
const OPCODE_I32_XOR: u8 = 0x73;
const OPCODE_I32_SHL: u8 = 0x74;
const OPCODE_I32_SHR_S: u8 = 0x75;
const OPCODE_I32_SHR_U: u8 = 0x76;
const OPCODE_I32_ROTL: u8 = 0x77;
const OPCODE_I32_ROTR: u8 = 0x78;
const OPCODE_I64_CLZ: u8 = 0x79;
const OPCODE_I64_CTZ: u8 = 0x7a;
const OPCODE_I64_POPCNT: u8 = 0x7b;
const OPCODE_I64_ADD: u8 = 0x7c;
const OPCODE_I64_SUB: u8 = 0x7d;
const OPCODE_I64_MUL: u8 = 0x7e;
const OPCODE_I64_DIV_S: u8 = 0x7f;
const OPCODE_I64_DIV_U: u8 = 0x80;
const OPCODE_I64_REM_S: u8 = 0x81;
const OPCODE_I64_REM_U: u8 = 0x82;
const OPCODE_I64_AND: u8 = 0x83;
const OPCODE_I64_OR: u8 = 0x84;
const OPCODE_I64_XOR: u8 = 0x85;
const OPCODE_I64_SHL: u8 = 0x86;
const OPCODE_I64_SHR_S: u8 = 0x87;
const OPCODE_I64_SHR_U: u8 = 0x88;
const OPCODE_I64_ROTL: u8 = 0x89;
const OPCODE_I64_ROTR: u8 = 0x8a;
const OPCODE_F32_ABS: u8 = 0x8b;
const OPCODE_F32_NEG: u8 = 0x8c;
const OPCODE_F32_CEIL: u8 = 0x8d;
const OPCODE_F32_FLOOR: u8 = 0x8e;
const OPCODE_F32_TRUNC: u8 = 0x8f;
const OPCODE_F32_NEAREST: u8 = 0x90;
const OPCODE_F32_SQRT: u8 = 0x91;
const OPCODE_F32_ADD: u8 = 0x92;
const OPCODE_F32_SUB: u8 = 0x93;
const OPCODE_F32_MUL: u8 = 0x94;
const OPCODE_F32_DIV: u8 = 0x95;
const OPCODE_F32_MIN: u8 = 0x96;
const OPCODE_F32_MAX: u8 = 0x97;
const OPCODE_F32_COPYSIGN: u8 = 0x98;
const OPCODE_F64_ABS: u8 = 0x99;
const OPCODE_F64_NEG: u8 = 0x9a;
const OPCODE_F64_CEIL: u8 = 0x9b;
const OPCODE_F64_FLOOR: u8 = 0x9c;
const OPCODE_F64_TRUNC: u8 = 0x9d;
const OPCODE_F64_NEAREST: u8 = 0x9e;
const OPCODE_F64_SQRT: u8 = 0x9f;
const OPCODE_F64_ADD: u8 = 0xa0;
const OPCODE_F64_SUB: u8 = 0xa1;
const OPCODE_F64_MUL: u8 = 0xa2;
const OPCODE_F64_DIV: u8 = 0xa3;
const OPCODE_F64_MIN: u8 = 0xa4;
const OPCODE_F64_MAX: u8 = 0xa5;
const OPCODE_F64_COPYSIGN: u8 = 0xa6;
const OPCODE_I32_WRAP_I64: u8 = 0xa7;
const OPCODE_I32_TRUNC_F32_S: u8 = 0xa8;
const OPCODE_I32_TRUNC_F32_U: u8 = 0xa9;
const OPCODE_I32_TRUNC_F64_S: u8 = 0xaa;
const OPCODE_I32_TRUNC_F64_U: u8 = 0xab;
const OPCODE_I64_EXTEND_I32_S: u8 = 0xac;
const OPCODE_I64_EXTEND_I32_U: u8 = 0xad;
const OPCODE_I64_TRUNC_F32_S: u8 = 0xae;
const OPCODE_I64_TRUNC_F32_U: u8 = 0xaf;
const OPCODE_I64_TRUNC_F64_S: u8 = 0xb0;
const OPCODE_I64_TRUNC_F64_U: u8 = 0xb1;
const OPCODE_F32_CONVERT_I32_S: u8 = 0xb2;
const OPCODE_F32_CONVERT_I32_U: u8 = 0xb3;
const OPCODE_F32_CONVERT_I64_S: u8 = 0xb4;
const OPCODE_F32_CONVERT_I64_U: u8 = 0xb5;
const OPCODE_F32_DEMOTE_F64: u8 = 0xb6;
const OPCODE_F64_CONVERT_I32_S: u8 = 0xb7;
const OPCODE_F64_CONVERT_I32_U: u8 = 0xb8;
const OPCODE_F64_CONVERT_I64_S: u8 = 0xb9;
const OPCODE_F64_CONVERT_I64_U: u8 = 0xba;
const OPCODE_F64_PROMOTE_F32: u8 = 0xbb;
const OPCODE_I32_REINTERPRET_F32: u8 = 0xbc;
const OPCODE_I64_REINTERPRET_F64: u8 = 0xbd;
const OPCODE_F32_REINTERPRET_I32: u8 = 0xbe;
const OPCODE_F64_REINTERPRET_I64: u8 = 0xbf;
const OPCODE_I32_EXTEND8_S: u8 = 0xc0;
const OPCODE_I32_EXTEND16_S: u8 = 0xc1;
const OPCODE_I64_EXTEND8_S: u8 = 0xc2;
const OPCODE_I64_EXTEND16_S: u8 = 0xc3;
const OPCODE_I64_EXTEND32_S: u8 = 0xc4;
const OPCODE_REF_NULL: u8 = 0xd0;
const OPCODE_REF_IS_NULL: u8 = 0xd1;
const OPCODE_REF_FUNC: u8 = 0xd2;
const OPCODE_MISC: u8 = 0xfc;
const OPCODE_END: u8 = 0x0b;

const SECTION_MEMORY: u8 = 5;
const SECTION_GLOBAL: u8 = 6;
const WASM_MAX_DATA_SEGMENTS: usize = 8;
#[cfg(test)]
const TEST_RESUME_FUEL: u32 = 1024;
const WASM_BLOCKTYPE_EMPTY: u8 = 0x40;
const CORE_WASM_MAX_TYPES: usize = 16;
const CORE_WASM_MAX_IMPORTS: usize = 16;
const CORE_WASM_MAX_FUNCTIONS: usize = 112;
const CORE_WASM_MAX_GLOBALS: usize = 16;
const CORE_WASM_MAX_PARAMS: usize = if cfg!(any(
    feature = "wasip1-sys-path-open",
    feature = "wasm-engine-core"
)) {
    12
} else {
    8
};
const CORE_WASM_MAX_RESULTS: usize = 1;
const CORE_WASM_VALUE_STACK_CAPACITY: usize = 64;
const CORE_WASM_LOCAL_CAPACITY: usize = 32;
const CORE_WASM_CALL_STACK_CAPACITY: usize = 8;
const CORE_WASM_CONTROL_STACK_CAPACITY: usize = 16;
const CORE_WASM_CONTROL_TARGET_CAPACITY: usize = 56;
const CORE_WASM_BR_TABLE_CAPACITY: usize = 8;
const CORE_WASIP1_PATH_CAPACITY: usize = 64;
const CORE_WASM_TABLE_CAPACITY: usize = 40;
const CORE_WASM_MAX_ELEMENT_SEGMENTS: usize = 8;
const CORE_WASM_PAGE_SIZE: usize = 64 * 1024;
const CORE_WASM_MAX_MEMORY_PAGES: u32 = 1;
const CORE_WASM_MEMORY_SIZE: usize = CORE_WASM_PAGE_SIZE * CORE_WASM_MAX_MEMORY_PAGES as usize;
const WASIP1_EVENTTYPE_CLOCK: u8 = 0;
const WASIP1_SUBSCRIPTION_USERDATA_OFFSET: u32 = 0;
const WASIP1_SUBSCRIPTION_EVENTTYPE_OFFSET: u32 = 8;
const WASIP1_SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET: u32 = 24;
const WASIP1_EVENT_ERROR_OFFSET: u32 = 8;
const WASIP1_EVENT_TYPE_OFFSET: u32 = 10;
const WASIP1_EVENT_SIZE: usize = 32;
#[cfg(test)]
pub const WASIP1_FILETYPE_REGULAR_FILE: u8 = 4;
pub const WASIP1_FDSTAT_SIZE: usize = 24;
pub const WASIP1_FDSTAT_FILETYPE_OFFSET: u32 = 0;
pub const WASIP1_FDSTAT_FLAGS_OFFSET: u32 = 2;
pub const WASIP1_FDSTAT_RIGHTS_BASE_OFFSET: u32 = 8;
pub const WASIP1_FDSTAT_RIGHTS_INHERITING_OFFSET: u32 = 16;

type LinearMemory = [u8; CORE_WASM_MEMORY_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmError {
    Truncated,
    Invalid(&'static str),
    Unsupported(&'static str),
    UnsupportedOpcode(u8),
    StackOverflow,
    StackUnderflow,
    PendingCall,
    PendingRequired,
    PendingMismatch,
    Trap,
    FuelExhausted,
}

impl WasmError {
    pub const fn diagnostic_code(self) -> u32 {
        match self {
            Self::Truncated => 0x5700_0001,
            Self::Invalid(message) => 0x5701_0000 | diagnostic_message_code(message),
            Self::Unsupported(message) => 0x5702_0000 | diagnostic_message_code(message),
            Self::UnsupportedOpcode(opcode) => 0x5703_0000 | opcode as u32,
            Self::StackOverflow => 0x5700_0002,
            Self::StackUnderflow => 0x5700_0003,
            Self::PendingCall => 0x5700_0004,
            Self::PendingRequired => 0x5700_0005,
            Self::PendingMismatch => 0x5700_0006,
            Self::Trap => 0x5700_0007,
            Self::FuelExhausted => 0x5700_0008,
        }
    }
}

const fn diagnostic_message_code(message: &'static str) -> u32 {
    let bytes = message.as_bytes();
    let mut code = 0x811c_u32;
    let mut idx = 0usize;
    while idx < bytes.len() {
        code ^= bytes[idx] as u32;
        code = code.wrapping_mul(0x0101);
        idx += 1;
    }
    code & 0x0000_ffff
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValueKind {
    I32,
    I64,
    F32,
    F64,
    FuncRef,
}

impl ValueKind {
    fn decode(byte: u8) -> Result<Self, WasmError> {
        match byte {
            VALTYPE_I32 => Ok(Self::I32),
            VALTYPE_I64 => Ok(Self::I64),
            VALTYPE_F32 => Ok(Self::F32),
            VALTYPE_F64 => Ok(Self::F64),
            VALTYPE_FUNCREF => Ok(Self::FuncRef),
            _ => Err(WasmError::Unsupported("unsupported core value type")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Value {
    I32(u32),
    I64(u64),
    F32(u32),
    F64(u64),
    FuncRef(u32),
}

impl Value {
    const fn zero(kind: ValueKind) -> Self {
        match kind {
            ValueKind::I32 => Self::I32(0),
            ValueKind::I64 => Self::I64(0),
            ValueKind::F32 => Self::F32(0),
            ValueKind::F64 => Self::F64(0),
            ValueKind::FuncRef => Self::FuncRef(u32::MAX),
        }
    }

    fn kind(self) -> ValueKind {
        match self {
            Self::I32(_) => ValueKind::I32,
            Self::I64(_) => ValueKind::I64,
            Self::F32(_) => ValueKind::F32,
            Self::F64(_) => ValueKind::F64,
            Self::FuncRef(_) => ValueKind::FuncRef,
        }
    }

    fn as_i32(self) -> Result<u32, WasmError> {
        match self {
            Self::I32(value) => Ok(value),
            _ => Err(WasmError::Invalid("expected i32 core value")),
        }
    }

    fn as_i64(self) -> Result<u64, WasmError> {
        match self {
            Self::I64(value) => Ok(value),
            _ => Err(WasmError::Invalid("expected i64 core value")),
        }
    }

    fn as_f32_bits(self) -> Result<u32, WasmError> {
        match self {
            Self::F32(value) => Ok(value),
            _ => Err(WasmError::Invalid("expected f32 core value")),
        }
    }

    fn as_f64_bits(self) -> Result<u64, WasmError> {
        match self {
            Self::F64(value) => Ok(value),
            _ => Err(WasmError::Invalid("expected f64 core value")),
        }
    }

    fn as_f32(self) -> Result<f32, WasmError> {
        Ok(f32::from_bits(self.as_f32_bits()?))
    }

    fn as_f64(self) -> Result<f64, WasmError> {
        Ok(f64::from_bits(self.as_f64_bits()?))
    }

    fn as_funcref(self) -> Result<u32, WasmError> {
        match self {
            Self::FuncRef(value) => Ok(value),
            _ => Err(WasmError::Invalid("expected funcref core value")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FuncType {
    params: [ValueKind; CORE_WASM_MAX_PARAMS],
    param_count: usize,
    results: [ValueKind; CORE_WASM_MAX_RESULTS],
    result_count: usize,
}

impl FuncType {
    const EMPTY: Self = Self {
        params: [ValueKind::I32; CORE_WASM_MAX_PARAMS],
        param_count: 0,
        results: [ValueKind::I32; CORE_WASM_MAX_RESULTS],
        result_count: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Import<'a> {
    pub function_index: u32,
    pub host: HostImport,
    module_bytes: core::marker::PhantomData<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostImport {
    Wasip1(Wasip1ImportName),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryGrowEvent {
    pub previous_pages: u32,
    pub requested_pages: u32,
    pub new_pages: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExecutionEvent {
    Wasip1Call,
    MemoryGrow(MemoryGrowEvent),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VmEvent {
    FdWrite(FdWriteCall),
    FdRead(FdReadCall),
    FdFdstatGet(FdRequestCall),
    FdClose(FdRequestCall),
    ClockResGet(ClockResGetCall),
    ClockTimeGet(ClockTimeGetCall),
    PollOneoff(PollOneoffCall),
    RandomGet(RandomGetCall),
    PathOpen(PathOpenCall),
    FdReaddir(FdReaddirCall),
    ArgsSizesGet(ArgsSizesGetCall),
    ArgsGet(ArgsGetCall),
    EnvironSizesGet(EnvironSizesGetCall),
    EnvironGet(EnvironGetCall),
    ProcExit(u32),
    MemoryGrow(MemoryGrowEvent),
    BudgetExpired(BudgetExpired),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingWasip1Call {
    FdWrite(FdWriteCall),
    FdRead(FdReadCall),
    FdFdstatGet(FdRequestCall),
    FdClose(FdRequestCall),
    FdReaddir(FdReaddirCall),
    ClockResGet(ClockResGetCall),
    ClockTimeGet(ClockTimeGetCall),
    PollOneoff(PollOneoffCall),
    RandomGet(RandomGetCall),
    PathOpen(PathOpenCall),
    ArgsSizesGet(ArgsSizesGetCall),
    ArgsGet(ArgsGetCall),
    EnvironSizesGet(EnvironSizesGetCall),
    EnvironGet(EnvironGetCall),
    ProcExit(u32),
}

impl PendingWasip1Call {
    const fn result_count(self) -> usize {
        match self {
            Self::ProcExit(_) => 0,
            _ => 1,
        }
    }

    const fn into_event(self) -> VmEvent {
        match self {
            Self::FdWrite(call) => VmEvent::FdWrite(call),
            Self::FdRead(call) => VmEvent::FdRead(call),
            Self::FdFdstatGet(call) => VmEvent::FdFdstatGet(call),
            Self::FdClose(call) => VmEvent::FdClose(call),
            Self::FdReaddir(call) => VmEvent::FdReaddir(call),
            Self::ClockResGet(call) => VmEvent::ClockResGet(call),
            Self::ClockTimeGet(call) => VmEvent::ClockTimeGet(call),
            Self::PollOneoff(call) => VmEvent::PollOneoff(call),
            Self::RandomGet(call) => VmEvent::RandomGet(call),
            Self::PathOpen(call) => VmEvent::PathOpen(call),
            Self::ArgsSizesGet(call) => VmEvent::ArgsSizesGet(call),
            Self::ArgsGet(call) => VmEvent::ArgsGet(call),
            Self::EnvironSizesGet(call) => VmEvent::EnvironSizesGet(call),
            Self::EnvironGet(call) => VmEvent::EnvironGet(call),
            Self::ProcExit(code) => VmEvent::ProcExit(code),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PathOpenCall {
    fd: u8,
    path_ptr: u32,
    path_len: u32,
    rights_base: u64,
    opened_fd_ptr: u32,
}

impl PathOpenCall {
    pub(super) const fn fd(self) -> u8 {
        self.fd
    }

    pub(super) const fn rights_base(self) -> u64 {
        self.rights_base
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FdReaddirCall {
    fd: u8,
    buf: u32,
    buf_len: u32,
    cookie: u64,
    bufused: u32,
}

impl FdReaddirCall {
    pub(super) const fn fd(self) -> u8 {
        self.fd
    }

    pub(super) const fn cookie(self) -> u64 {
        self.cookie
    }

    pub(super) const fn max_len(self) -> usize {
        self.buf_len as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathBytes {
    bytes: [u8; CORE_WASIP1_PATH_CAPACITY],
    len: usize,
}

impl PathBytes {
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FdStat {
    filetype: u8,
    flags: u16,
    rights_base: u64,
    rights_inheriting: u64,
}

impl FdStat {
    pub const fn new(filetype: u8, flags: u16, rights_base: u64, rights_inheriting: u64) -> Self {
        Self {
            filetype,
            flags,
            rights_base,
            rights_inheriting,
        }
    }

    pub const fn filetype(self) -> u8 {
        self.filetype
    }

    pub const fn flags(self) -> u16 {
        self.flags
    }

    pub const fn rights_base(self) -> u64 {
        self.rights_base
    }

    pub const fn rights_inheriting(self) -> u64 {
        self.rights_inheriting
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FdReadCall {
    fd: u8,
    iovs: u32,
    iovs_len: u32,
    nread: u32,
}

impl FdReadCall {
    pub(super) const fn fd(self) -> u8 {
        self.fd
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn iovs(self) -> u32 {
        self.iovs
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn iovs_len(self) -> u32 {
        self.iovs_len
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn nread(self) -> u32 {
        self.nread
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FdRequestCall {
    fd: u8,
    out_ptr: u32,
}

impl FdRequestCall {
    pub(super) const fn fd(self) -> u8 {
        self.fd
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn out_ptr(self) -> u32 {
        self.out_ptr
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClockResGetCall {
    clock_id: u32,
    resolution_ptr: u32,
}

impl ClockResGetCall {
    pub(super) const fn clock_id(self) -> u32 {
        self.clock_id
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn resolution_ptr(self) -> u32 {
        self.resolution_ptr
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClockTimeGetCall {
    clock_id: u32,
    precision: u64,
    time_ptr: u32,
}

impl ClockTimeGetCall {
    pub(super) const fn clock_id(self) -> u32 {
        self.clock_id
    }

    pub(super) const fn precision(self) -> u64 {
        self.precision
    }

    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn time_ptr(self) -> u32 {
        self.time_ptr
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RandomGetCall {
    buf: u32,
    buf_len: u32,
}

impl RandomGetCall {
    #[cfg(all(test, feature = "wasm-engine-core"))]
    const fn buf(self) -> u32 {
        self.buf
    }

    pub(super) const fn buf_len(self) -> u32 {
        self.buf_len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArgsSizesGetCall {
    argc_ptr: u32,
    argv_buf_size_ptr: u32,
}

impl ArgsSizesGetCall {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArgsGetCall {
    argv: u32,
    argv_buf: u32,
}

impl ArgsGetCall {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnvironSizesGetCall {
    environ_count_ptr: u32,
    environ_buf_size_ptr: u32,
}

impl EnvironSizesGetCall {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnvironGetCall {
    environ: u32,
    environ_buf: u32,
}

impl EnvironGetCall {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CodeBody<'a> {
    code: &'a [u8],
    local_count: usize,
    local_kinds: [ValueKind; CORE_WASM_LOCAL_CAPACITY],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Instr {
    Nop,
    Simple(u8),
    U32(u8, u32),
    CallIndirect { type_index: u32, table_index: u32 },
    Mem { opcode: u8, offset: u32 },
    I32Const(u32),
    I64Const(u64),
    F32Const(u32),
    F64Const(u64),
    BrTable(BrTableInstr),
    Misc(MiscInstr),
    Block(ControlInstr),
    Loop(ControlInstr),
    If(ControlInstr),
    Else,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ControlInstr {
    result_count: usize,
    result_kind: ValueKind,
    else_pos: u16,
    end_pos: u16,
}

impl ControlInstr {
    const NONE: u16 = u16::MAX;

    const fn new(result_count: usize, result_kind: ValueKind) -> Self {
        Self {
            result_count,
            result_kind,
            else_pos: Self::NONE,
            end_pos: Self::NONE,
        }
    }

    const fn else_pos(self) -> usize {
        if self.else_pos == Self::NONE {
            usize::MAX
        } else {
            self.else_pos as usize
        }
    }

    fn end(self) -> Result<usize, WasmError> {
        if self.end_pos == Self::NONE {
            Err(WasmError::Invalid(
                "decoded control instruction missing end",
            ))
        } else {
            Ok(self.end_pos as usize)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoreControlTarget {
    start_pos: u16,
    else_pos: u16,
    end_pos: u16,
}

impl CoreControlTarget {
    const NONE: u16 = u16::MAX;
    const EMPTY: Self = Self {
        start_pos: 0,
        else_pos: Self::NONE,
        end_pos: 0,
    };

    fn new(start_pos: usize) -> Result<Self, WasmError> {
        Ok(Self {
            start_pos: u16::try_from(start_pos)
                .map_err(|_| WasmError::Unsupported("core wasm body too large"))?,
            else_pos: Self::NONE,
            end_pos: Self::NONE,
        })
    }

    const fn start(self) -> usize {
        self.start_pos as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BrTableInstr {
    labels: [u16; CORE_WASM_BR_TABLE_CAPACITY],
    label_count: u8,
    default: u16,
}

impl BrTableInstr {
    const EMPTY: Self = Self {
        labels: [0; CORE_WASM_BR_TABLE_CAPACITY],
        label_count: 0,
        default: 0,
    };

    const fn selected_depth(self, selected: usize) -> usize {
        if selected < self.label_count as usize {
            self.labels[selected] as usize
        } else {
            self.default as usize
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MiscInstr {
    MemoryInit { data_index: usize },
    DataDrop { data_index: usize },
    MemoryCopy,
    MemoryFill,
    TableInit { elem_index: usize, table_index: u32 },
    ElemDrop { elem_index: usize },
    TableCopy { dst_table: u32, src_table: u32 },
    TableGrow { table_index: u32 },
    TableSize { table_index: u32 },
    TableFill { table_index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DataSegment<'a> {
    active: bool,
    offset: u32,
    bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ElementSegment {
    functions: [u32; CORE_WASM_TABLE_CAPACITY],
    function_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Global {
    kind: ValueKind,
    mutable: bool,
    initial: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Frame<'a> {
    code: &'a [u8],
    pc: usize,
    locals: [Value; CORE_WASM_LOCAL_CAPACITY],
    local_kinds: [ValueKind; CORE_WASM_LOCAL_CAPACITY],
    local_count: usize,
    controls: [ControlFrame; CORE_WASM_CONTROL_STACK_CAPACITY],
    control_len: usize,
}

impl<'a> Frame<'a> {
    fn empty() -> Self {
        Self {
            code: &[],
            pc: 0,
            locals: [Value::I32(0); CORE_WASM_LOCAL_CAPACITY],
            local_kinds: [ValueKind::I32; CORE_WASM_LOCAL_CAPACITY],
            local_count: 0,
            controls: [ControlFrame {
                kind: ControlKind::Block,
                start_pos: 0,
                else_pos: usize::MAX,
                end_pos: 0,
                result_count: 0,
                result_kind: ValueKind::I32,
                stack_height: 0,
            }; CORE_WASM_CONTROL_STACK_CAPACITY],
            control_len: 0,
        }
    }
}

type Frames<'a> = [Frame<'a>; CORE_WASM_CALL_STACK_CAPACITY];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingExecution {
    Wasip1(PendingWasip1Call),
    MemoryGrow(MemoryGrowEvent),
}

#[derive(Clone, Copy)]
pub(super) struct Module<'a> {
    types: [FuncType; CORE_WASM_MAX_TYPES],
    type_count: usize,
    imports: [Option<Import<'a>>; CORE_WASM_MAX_IMPORTS],
    import_type_indices: [u32; CORE_WASM_MAX_IMPORTS],
    import_count: usize,
    function_type_indices: [u32; CORE_WASM_MAX_FUNCTIONS],
    function_count: usize,
    globals: [Option<Global>; CORE_WASM_MAX_GLOBALS],
    global_count: usize,
    code_bodies: [Option<CodeBody<'a>>; CORE_WASM_MAX_FUNCTIONS],
    data_segments: [Option<DataSegment<'a>>; WASM_MAX_DATA_SEGMENTS],
    element_segments: [Option<ElementSegment>; CORE_WASM_MAX_ELEMENT_SEGMENTS],
    table_functions: [u32; CORE_WASM_TABLE_CAPACITY],
    table_function_count: usize,
    table_min: usize,
    start_function_index: u32,
    memory_min_pages: u32,
    memory_max_pages: u32,
}

pub(super) struct Interpreter<'a> {
    module: Module<'a>,
    frames: Frames<'a>,
    frame_len: usize,
    values: [Value; CORE_WASM_VALUE_STACK_CAPACITY],
    value_len: usize,
    globals: [Value; CORE_WASM_MAX_GLOBALS],
    global_kinds: [ValueKind; CORE_WASM_MAX_GLOBALS],
    global_mutable: [bool; CORE_WASM_MAX_GLOBALS],
    global_count: usize,
    memory: LinearMemory,
    memory_pages: u32,
    data_dropped: [bool; WASM_MAX_DATA_SEGMENTS],
    element_dropped: [bool; CORE_WASM_MAX_ELEMENT_SEGMENTS],
    table_functions: [u32; CORE_WASM_TABLE_CAPACITY],
    table_size: usize,
    control_targets: [CoreControlTarget; CORE_WASM_CONTROL_TARGET_CAPACITY],
    control_target_count: usize,
    pending: Option<PendingExecution>,
    done: bool,
}

pub(super) struct Vm<'a> {
    core: Interpreter<'a>,
    handlers: Wasip1HandlerSet,
    done: bool,
}

static EMPTY_MODULE: Module<'static> = Module::empty();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FdWriteCall {
    fd: u8,
    iovs: u32,
    iovs_len: u32,
    nwritten: u32,
}

impl FdWriteCall {
    pub(super) const fn fd(self) -> u8 {
        self.fd
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PollOneoffCall {
    in_ptr: u32,
    out_ptr: u32,
    nsubscriptions: u32,
    nevents: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlKind {
    Block,
    Loop,
    If,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ControlFrame {
    kind: ControlKind,
    start_pos: usize,
    else_pos: usize,
    end_pos: usize,
    result_count: usize,
    result_kind: ValueKind,
    stack_height: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InlinePayload {
    bytes: [u8; WASIP1_STREAM_CHUNK_CAPACITY],
    len: u8,
}

impl InlinePayload {
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len as usize).0
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos == self.bytes.len()
    }

    fn read_u8(&mut self) -> Result<u8, WasmError> {
        let byte = *self.bytes.get(self.pos).ok_or(WasmError::Truncated)?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], WasmError> {
        let end = self.pos.checked_add(len).ok_or(WasmError::Truncated)?;
        let slice = self.bytes.get(self.pos..end).ok_or(WasmError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn read_fixed_u32(&mut self) -> Result<u32, WasmError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_fixed_u64(&mut self) -> Result<u64, WasmError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_name(&mut self) -> Result<&'a [u8], WasmError> {
        let len = self.read_var_u32()? as usize;
        self.read_bytes(len)
    }

    fn read_var_u32(&mut self) -> Result<u32, WasmError> {
        let mut shift = 0u32;
        let mut value = 0u32;
        loop {
            if shift >= 35 {
                return Err(WasmError::Invalid("u32 leb too wide"));
            }
            let byte = self.read_u8()?;
            value |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn read_var_i32(&mut self) -> Result<i32, WasmError> {
        let mut shift = 0u32;
        let mut value = 0i32;
        let mut byte;
        loop {
            if shift >= 35 {
                return Err(WasmError::Invalid("i32 leb too wide"));
            }
            byte = self.read_u8()?;
            value |= ((byte & 0x7f) as i32) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        if shift < 32 && (byte & 0x40) != 0 {
            value |= (!0i32) << shift;
        }
        Ok(value)
    }

    fn read_var_i64(&mut self) -> Result<i64, WasmError> {
        let mut shift = 0u32;
        let mut value = 0i64;
        let mut byte;
        loop {
            if shift >= 70 {
                return Err(WasmError::Invalid("i64 leb too wide"));
            }
            byte = self.read_u8()?;
            value |= ((byte & 0x7f) as i64) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        if shift < 64 && (byte & 0x40) != 0 {
            value |= (!0i64) << shift;
        }
        Ok(value)
    }
}

impl<'a> Module<'a> {
    const fn empty() -> Self {
        Self {
            types: [FuncType::EMPTY; CORE_WASM_MAX_TYPES],
            type_count: 0,
            imports: [None; CORE_WASM_MAX_IMPORTS],
            import_type_indices: [0; CORE_WASM_MAX_IMPORTS],
            import_count: 0,
            function_type_indices: [0; CORE_WASM_MAX_FUNCTIONS],
            function_count: 0,
            globals: [None; CORE_WASM_MAX_GLOBALS],
            global_count: 0,
            code_bodies: [None; CORE_WASM_MAX_FUNCTIONS],
            data_segments: [None; WASM_MAX_DATA_SEGMENTS],
            element_segments: [None; CORE_WASM_MAX_ELEMENT_SEGMENTS],
            table_functions: [u32::MAX; CORE_WASM_TABLE_CAPACITY],
            table_function_count: 0,
            table_min: 0,
            start_function_index: u32::MAX,
            memory_min_pages: 0,
            memory_max_pages: 0,
        }
    }

    unsafe fn parse_in_place(dst: *mut Self, bytes: &'a [u8]) -> Result<(), WasmError> {
        unsafe {
            core::ptr::copy_nonoverlapping(
                core::ptr::addr_of!(EMPTY_MODULE).cast::<Self>(),
                dst,
                1,
            );
            (&mut *dst).parse_from(bytes)?;
        }
        Ok(())
    }

    fn parse_from(&mut self, bytes: &'a [u8]) -> Result<(), WasmError> {
        let mut reader = Reader::new(bytes);
        if reader.read_bytes(4)? != WASM_MAGIC {
            return Err(WasmError::Invalid("invalid wasm magic"));
        }
        if reader.read_bytes(4)? != WASM_VERSION {
            return Err(WasmError::Invalid("unsupported wasm version"));
        }

        let mut saw_export = false;

        while !reader.is_empty() {
            let section_id = reader.read_u8()?;
            let section_len = reader.read_var_u32()? as usize;
            let section_bytes = reader.read_bytes(section_len)?;
            let mut section = Reader::new(section_bytes);
            match section_id {
                SECTION_TYPE => self.parse_core_type_section(&mut section)?,
                SECTION_IMPORT => self.parse_core_import_section(&mut section)?,
                SECTION_FUNCTION => self.parse_core_function_section(&mut section)?,
                SECTION_TABLE => self.parse_core_table_section(&mut section)?,
                SECTION_MEMORY => self.parse_core_memory_section(&mut section)?,
                SECTION_GLOBAL => self.parse_core_global_section(&mut section)?,
                SECTION_EXPORT => {
                    self.parse_core_export_section(&mut section)?;
                    saw_export = true;
                }
                SECTION_ELEMENT => self.parse_core_element_section(&mut section)?,
                SECTION_CODE => self.parse_core_code_section(&mut section)?,
                SECTION_DATA => self.parse_core_data_section(&mut section)?,
                SECTION_CUSTOM => {
                    let custom_section =
                        section.read_bytes(section.bytes.len().saturating_sub(section.pos))?;
                    core::hint::black_box(custom_section);
                }
                _ => return Err(WasmError::Unsupported("unsupported core wasm section")),
            }
            if !section.is_empty() {
                return Err(WasmError::Invalid("section has trailing bytes"));
            }
        }

        if !saw_export || self.start_function_index == u32::MAX {
            return Err(WasmError::Invalid("missing _start or __main_void export"));
        }
        if self.function_count > 0
            && self.code_bodies[..self.function_count]
                .iter()
                .any(Option::is_none)
        {
            return Err(WasmError::Invalid("missing core wasm code body"));
        }
        Ok(())
    }

    fn parse_core_type_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > CORE_WASM_MAX_TYPES {
            return Err(WasmError::Unsupported("too many core wasm function types"));
        }
        self.type_count = count;
        for index in 0..count {
            if section.read_u8()? != FUNC_TYPE_FORM {
                return Err(WasmError::Invalid("type section expects function forms"));
            }
            let param_count = section.read_var_u32()? as usize;
            if param_count > CORE_WASM_MAX_PARAMS {
                return Err(WasmError::Unsupported("too many core wasm params"));
            }
            let mut ty = FuncType::EMPTY;
            ty.param_count = param_count;
            for slot in ty.params.iter_mut().take(param_count) {
                *slot = ValueKind::decode(section.read_u8()?)?;
            }

            let result_count = section.read_var_u32()? as usize;
            if result_count > CORE_WASM_MAX_RESULTS {
                return Err(WasmError::Unsupported("too many core wasm results"));
            }
            ty.result_count = result_count;
            for slot in ty.results.iter_mut().take(result_count) {
                *slot = ValueKind::decode(section.read_u8()?)?;
            }
            self.types[index] = ty;
        }
        Ok(())
    }

    fn parse_core_import_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > CORE_WASM_MAX_IMPORTS {
            return Err(WasmError::Unsupported("too many core wasm imports"));
        }
        self.import_count = count;
        for index in 0..count {
            let module = section.read_name()?;
            let name = section.read_name()?;
            let host = decode_host_import(module, name)?;
            if section.read_u8()? != EXTERNAL_KIND_FUNC {
                return Err(WasmError::Unsupported(
                    "core wasm only supports function imports",
                ));
            }
            let type_index = section.read_var_u32()?;
            self.core_func_type(type_index)?;
            self.imports[index] = Some(Import {
                function_index: index as u32,
                host,
                module_bytes: core::marker::PhantomData,
            });
            self.import_type_indices[index] = type_index;
        }
        Ok(())
    }

    fn parse_core_function_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > CORE_WASM_MAX_FUNCTIONS {
            return Err(WasmError::Unsupported("too many core wasm functions"));
        }
        self.function_count = count;
        for index in 0..count {
            let type_index = section.read_var_u32()?;
            self.core_func_type(type_index)?;
            self.function_type_indices[index] = type_index;
        }
        Ok(())
    }

    fn parse_core_table_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()?;
        if count > 1 {
            return Err(WasmError::Unsupported("too many core wasm tables"));
        }
        for table_index in 0..count {
            core::hint::black_box(table_index);
            if section.read_u8()? != VALTYPE_FUNCREF {
                return Err(WasmError::Unsupported("only funcref tables are supported"));
            }
            let flags = section.read_u8()?;
            if flags & !0x01 != 0 {
                return Err(WasmError::Unsupported("unsupported core table limits"));
            }
            let min = section.read_var_u32()? as usize;
            if min > CORE_WASM_TABLE_CAPACITY {
                return Err(WasmError::Unsupported("core table too large"));
            }
            self.table_min = min;
            if flags & 0x01 != 0 {
                let max = section.read_var_u32()? as usize;
                if max < min || max > CORE_WASM_TABLE_CAPACITY {
                    return Err(WasmError::Unsupported("core table limit too large"));
                }
            }
        }
        Ok(())
    }

    fn parse_core_element_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > CORE_WASM_MAX_ELEMENT_SEGMENTS {
            return Err(WasmError::Unsupported("too many core element segments"));
        }
        for slot in self.element_segments.iter_mut() {
            *slot = None;
        }
        for segment_index in 0..count {
            let kind = section.read_var_u32()?;
            match kind {
                0 => {
                    let offset = parse_core_i32_offset_expr(section)? as usize;
                    let segment = self.parse_core_funcidx_element_payload(section)?;
                    self.install_core_element_segment(offset, segment)?;
                    self.element_segments[segment_index] = Some(segment);
                }
                1 => {
                    if section.read_u8()? != 0 {
                        return Err(WasmError::Unsupported(
                            "only function element kind supported",
                        ));
                    }
                    let segment = self.parse_core_funcidx_element_payload(section)?;
                    self.element_segments[segment_index] = Some(segment);
                }
                2 => {
                    if section.read_var_u32()? != 0 {
                        return Err(WasmError::Invalid("core element table index must be zero"));
                    }
                    let offset = parse_core_i32_offset_expr(section)? as usize;
                    if section.read_u8()? != 0 {
                        return Err(WasmError::Unsupported(
                            "only function element kind supported",
                        ));
                    }
                    let segment = self.parse_core_funcidx_element_payload(section)?;
                    self.install_core_element_segment(offset, segment)?;
                    self.element_segments[segment_index] = Some(segment);
                }
                _ => {
                    return Err(WasmError::Unsupported(
                        "unsupported core element section mode",
                    ));
                }
            }
        }
        Ok(())
    }

    fn parse_core_funcidx_element_payload(
        &self,
        section: &mut Reader<'a>,
    ) -> Result<ElementSegment, WasmError> {
        let func_count = section.read_var_u32()? as usize;
        if func_count > CORE_WASM_TABLE_CAPACITY {
            return Err(WasmError::Unsupported("core element segment too large"));
        }
        let mut functions = [u32::MAX; CORE_WASM_TABLE_CAPACITY];
        for slot in functions.iter_mut().take(func_count) {
            let function_index = section.read_var_u32()?;
            self.core_func_type_index(function_index)?;
            *slot = function_index;
        }
        Ok(ElementSegment {
            functions,
            function_count: func_count,
        })
    }

    fn install_core_element_segment(
        &mut self,
        offset: usize,
        segment: ElementSegment,
    ) -> Result<(), WasmError> {
        let end = offset
            .checked_add(segment.function_count)
            .ok_or(WasmError::Unsupported("core element table too large"))?;
        if end > CORE_WASM_TABLE_CAPACITY {
            return Err(WasmError::Unsupported("core element table too large"));
        }
        for (dst, function_index) in self
            .table_functions
            .iter_mut()
            .skip(offset)
            .take(segment.function_count)
            .zip(segment.functions.iter().copied())
        {
            *dst = function_index;
        }
        self.table_function_count = self.table_function_count.max(end);
        Ok(())
    }

    fn parse_core_memory_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()?;
        if count != 1 {
            return Err(WasmError::Unsupported(
                "core wasm supports at most one memory",
            ));
        }
        let flags = section.read_u8()?;
        if flags & !0x01 != 0 {
            return Err(WasmError::Unsupported("unsupported core memory flags"));
        }
        let min = section.read_var_u32()?;
        let max = if flags & 0x01 != 0 {
            section.read_var_u32()?
        } else {
            CORE_WASM_MAX_MEMORY_PAGES
        };
        if min > max || max > CORE_WASM_MAX_MEMORY_PAGES {
            return Err(WasmError::Unsupported("core wasm memory exceeds profile"));
        }
        self.memory_min_pages = min;
        self.memory_max_pages = max;
        Ok(())
    }

    fn parse_core_global_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > CORE_WASM_MAX_GLOBALS {
            return Err(WasmError::Unsupported("too many core wasm globals"));
        }
        self.global_count = count;
        for index in 0..count {
            let kind = ValueKind::decode(section.read_u8()?)?;
            let mutable = match section.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(WasmError::Invalid("invalid core global mutability")),
            };
            let initial = Self::parse_core_const_expr(section, kind)?;
            self.globals[index] = Some(Global {
                kind,
                mutable,
                initial,
            });
        }
        Ok(())
    }

    fn parse_core_const_expr(
        section: &mut Reader<'a>,
        kind: ValueKind,
    ) -> Result<Value, WasmError> {
        let value = match (kind, section.read_u8()?) {
            (ValueKind::I32, OPCODE_I32_CONST) => Value::I32(section.read_var_i32()? as u32),
            (ValueKind::I64, OPCODE_I64_CONST) => Value::I64(section.read_var_i64()? as u64),
            (ValueKind::F32, OPCODE_F32_CONST) => Value::F32(section.read_fixed_u32()?),
            (ValueKind::F64, OPCODE_F64_CONST) => Value::F64(section.read_fixed_u64()?),
            (ValueKind::FuncRef, OPCODE_REF_NULL) => {
                let heap_type = section.read_u8()?;
                if heap_type != VALTYPE_FUNCREF {
                    return Err(WasmError::Unsupported(
                        "only null funcref globals supported",
                    ));
                }
                Value::FuncRef(u32::MAX)
            }
            _ => return Err(WasmError::Unsupported("unsupported core global init expr")),
        };
        if section.read_u8()? != OPCODE_END {
            return Err(WasmError::Invalid("core global init expr must end"));
        }
        Ok(value)
    }

    fn parse_core_export_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()?;
        for export_index in 0..count {
            core::hint::black_box(export_index);
            let name = section.read_name()?;
            let kind = section.read_u8()?;
            let index = section.read_var_u32()?;
            if name == b"__main_void" {
                if kind != EXTERNAL_KIND_FUNC {
                    return Err(WasmError::Invalid("__main_void must export a function"));
                }
                self.core_func_type_index(index)?;
                self.start_function_index = index;
            } else if name == b"_start" && self.start_function_index == u32::MAX {
                if kind != EXTERNAL_KIND_FUNC {
                    return Err(WasmError::Invalid("_start must export a function"));
                }
                self.core_func_type_index(index)?;
                self.start_function_index = index;
            }
        }
        Ok(())
    }

    fn parse_core_code_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count != self.function_count {
            return Err(WasmError::Invalid("core code/function count mismatch"));
        }
        for local_index in 0..count {
            let body_len = section.read_var_u32()? as usize;
            let body = section.read_bytes(body_len)?;
            let mut body_reader = Reader::new(body);
            let mut local_count = 0usize;
            let mut local_kinds = [ValueKind::I32; CORE_WASM_LOCAL_CAPACITY];

            let function_type = self.core_func_type(self.function_type_indices[local_index])?;
            for index in 0..function_type.param_count {
                local_kinds[index] = function_type.params[index];
            }
            local_count += function_type.param_count;

            let local_decl_count = body_reader.read_var_u32()?;
            for local_decl_index in 0..local_decl_count {
                core::hint::black_box(local_decl_index);
                let count = body_reader.read_var_u32()? as usize;
                let kind = ValueKind::decode(body_reader.read_u8()?)?;
                let end = local_count
                    .checked_add(count)
                    .ok_or(WasmError::Unsupported("too many core wasm locals"))?;
                if end > CORE_WASM_LOCAL_CAPACITY {
                    return Err(WasmError::Unsupported("too many core wasm locals"));
                }
                for slot in local_kinds.iter_mut().take(end).skip(local_count) {
                    *slot = kind;
                }
                local_count = end;
            }
            let code = &body[body_reader.pos..];
            self.code_bodies[local_index] = Some(CodeBody {
                code,
                local_count,
                local_kinds,
            });
        }
        Ok(())
    }

    fn parse_core_data_section(&mut self, section: &mut Reader<'a>) -> Result<(), WasmError> {
        let count = section.read_var_u32()? as usize;
        if count > self.data_segments.len() {
            return Err(WasmError::Unsupported("too many core wasm data segments"));
        }
        for slot in self.data_segments.iter_mut() {
            *slot = None;
        }
        for slot in self.data_segments.iter_mut().take(count) {
            let mode = section.read_var_u32()?;
            let (active, offset) = match mode {
                0 => (true, parse_core_i32_offset_expr(section)?),
                1 => (false, 0),
                2 => {
                    if section.read_var_u32()? != 0 {
                        return Err(WasmError::Invalid("core data memory index must be zero"));
                    }
                    (true, parse_core_i32_offset_expr(section)?)
                }
                _ => return Err(WasmError::Unsupported("unsupported core data segment mode")),
            };
            let bytes_len = section.read_var_u32()? as usize;
            let bytes = section.read_bytes(bytes_len)?;
            *slot = Some(DataSegment {
                active,
                offset: offset as u32,
                bytes,
            });
        }
        Ok(())
    }

    fn core_func_type(&self, type_index: u32) -> Result<FuncType, WasmError> {
        self.types
            .get(type_index as usize)
            .copied()
            .filter(|_| (type_index as usize) < self.type_count)
            .ok_or(WasmError::Invalid("core function type index out of range"))
    }

    fn core_func_type_index(&self, function_index: u32) -> Result<u32, WasmError> {
        if function_index < self.import_count as u32 {
            self.import_type_indices
                .get(function_index as usize)
                .copied()
                .ok_or(WasmError::Invalid("core import index out of range"))
        } else {
            let local_index = function_index
                .checked_sub(self.import_count as u32)
                .ok_or(WasmError::Invalid("core function index underflow"))?
                as usize;
            self.function_type_indices
                .get(local_index)
                .copied()
                .filter(|_| local_index < self.function_count)
                .ok_or(WasmError::Invalid("core function index out of range"))
        }
    }

    fn core_function_body(&self, function_index: u32) -> Result<CodeBody<'a>, WasmError> {
        let local_index = function_index
            .checked_sub(self.import_count as u32)
            .ok_or(WasmError::Invalid("core function body points to import"))?
            as usize;
        self.code_bodies
            .get(local_index)
            .copied()
            .flatten()
            .filter(|_| local_index < self.function_count)
            .ok_or(WasmError::Invalid("core function body out of range"))
    }
}

impl<'a> Interpreter<'a> {
    unsafe fn init_from_parsed_module_in_place(dst: *mut Interpreter<'a>) -> Result<(), WasmError> {
        let module = unsafe { &*core::ptr::addr_of!((*dst).module) };
        if module.memory_min_pages > CORE_WASM_MAX_MEMORY_PAGES {
            return Err(WasmError::Unsupported("core wasm memory too large"));
        }
        let memory_min_pages = module.memory_min_pages;
        let global_count = module.global_count;
        let table_size = module.table_min.max(module.table_function_count);
        let start_function_index = module.start_function_index;
        unsafe {
            core::ptr::addr_of_mut!((*dst).memory_pages).write(memory_min_pages);
            write_empty_frames(core::ptr::addr_of_mut!((*dst).frames));
            core::ptr::addr_of_mut!((*dst).frame_len).write(0);
            core::ptr::addr_of_mut!((*dst).values)
                .write([Value::I32(0); CORE_WASM_VALUE_STACK_CAPACITY]);
            core::ptr::addr_of_mut!((*dst).value_len).write(0);
            core::ptr::addr_of_mut!((*dst).globals).write([Value::I32(0); CORE_WASM_MAX_GLOBALS]);
            core::ptr::addr_of_mut!((*dst).global_kinds)
                .write([ValueKind::I32; CORE_WASM_MAX_GLOBALS]);
            core::ptr::addr_of_mut!((*dst).global_mutable).write([false; CORE_WASM_MAX_GLOBALS]);
            core::ptr::addr_of_mut!((*dst).global_count).write(global_count);
            core::ptr::addr_of_mut!((*dst).memory).write_bytes(0, 1);
            core::ptr::addr_of_mut!((*dst).data_dropped).write([false; WASM_MAX_DATA_SEGMENTS]);
            core::ptr::addr_of_mut!((*dst).element_dropped)
                .write([false; CORE_WASM_MAX_ELEMENT_SEGMENTS]);
            core::ptr::addr_of_mut!((*dst).table_functions)
                .write([u32::MAX; CORE_WASM_TABLE_CAPACITY]);
            core::ptr::addr_of_mut!((*dst).table_size).write(table_size);
            core::ptr::addr_of_mut!((*dst).control_targets)
                .write([CoreControlTarget::EMPTY; CORE_WASM_CONTROL_TARGET_CAPACITY]);
            core::ptr::addr_of_mut!((*dst).control_target_count).write(0);
            core::ptr::addr_of_mut!((*dst).pending).write(None);
            core::ptr::addr_of_mut!((*dst).done).write(false);
        }
        let instance = unsafe { &mut *dst };
        for index in 0..CORE_WASM_TABLE_CAPACITY {
            instance.table_functions[index] = instance.module.table_functions[index];
        }
        for (index, global) in instance
            .module
            .globals
            .iter()
            .copied()
            .flatten()
            .take(global_count)
            .enumerate()
        {
            instance.globals[index] = global.initial;
            instance.global_kinds[index] = global.kind;
            instance.global_mutable[index] = global.mutable;
        }
        instance.init_core_data_segments()?;
        instance.push_frame(start_function_index)?;
        Ok(())
    }
}

unsafe fn write_empty_frames<'a>(dst: *mut Frames<'a>) {
    for index in 0..CORE_WASM_CALL_STACK_CAPACITY {
        unsafe {
            core::ptr::addr_of_mut!((*dst)[index]).write(Frame::empty());
        }
    }
}

fn parse_core_i32_offset_expr(section: &mut Reader<'_>) -> Result<i32, WasmError> {
    if section.read_u8()? != OPCODE_I32_CONST {
        return Err(WasmError::Invalid("core offset must be i32.const"));
    }
    let offset = section.read_var_i32()?;
    if offset < 0 {
        return Err(WasmError::Invalid("core offset is negative"));
    }
    if section.read_u8()? != OPCODE_END {
        return Err(WasmError::Invalid("core offset expression must end"));
    }
    Ok(offset)
}

fn decode_host_import(module: &[u8], name: &[u8]) -> Result<HostImport, WasmError> {
    if module != WASIP1_PREVIEW1_MODULE.as_bytes() {
        return Err(WasmError::Unsupported("unsupported host import module"));
    }
    let name = Wasip1ImportName::from_bytes(name)
        .ok_or(WasmError::Unsupported("unsupported wasi p1 import"))?;
    Ok(HostImport::Wasip1(name))
}

fn decode_core_block_type(byte: u8) -> Result<(usize, ValueKind), WasmError> {
    if byte == WASM_BLOCKTYPE_EMPTY {
        Ok((0, ValueKind::I32))
    } else {
        Ok((1, ValueKind::decode(byte)?))
    }
}

fn decode_core_control_targets(
    code: &[u8],
    targets: &mut [CoreControlTarget; CORE_WASM_CONTROL_TARGET_CAPACITY],
) -> Result<usize, WasmError> {
    let mut reader = Reader::new(code);
    let mut stack = [usize::MAX; CORE_WASM_CONTROL_STACK_CAPACITY];
    let mut depth = 0usize;
    let mut count = 0usize;
    let mut saw_function_end = false;

    while !reader.is_empty() {
        let opcode_pos = reader.pos;
        let opcode = reader.read_u8()?;
        match opcode {
            OPCODE_BLOCK | OPCODE_LOOP | OPCODE_IF => {
                let block_type = reader.read_u8()?;
                core::hint::black_box(block_type);
                let target_index = count;
                let slot = targets
                    .get_mut(target_index)
                    .ok_or(WasmError::Unsupported("too many core wasm control targets"))?;
                *slot = CoreControlTarget::new(reader.pos)?;
                count += 1;
                let stack_slot = stack
                    .get_mut(depth)
                    .ok_or(WasmError::Unsupported("core wasm control stack too deep"))?;
                *stack_slot = target_index;
                depth += 1;
            }
            OPCODE_ELSE => {
                if depth == 0 {
                    return Err(WasmError::Invalid("else without if"));
                }
                let target_index = stack[depth - 1];
                let target = targets
                    .get_mut(target_index)
                    .ok_or(WasmError::Invalid("core control target missing"))?;
                if target.else_pos != ControlInstr::NONE {
                    return Err(WasmError::Invalid("duplicate else"));
                }
                target.else_pos = u16::try_from(opcode_pos)
                    .map_err(|_| WasmError::Unsupported("core wasm body too large"))?;
            }
            OPCODE_END => {
                if depth == 0 {
                    saw_function_end = true;
                    if !reader.is_empty() {
                        return Err(WasmError::Invalid("core function has trailing code"));
                    }
                } else {
                    depth -= 1;
                    let target_index = stack[depth];
                    let target = targets
                        .get_mut(target_index)
                        .ok_or(WasmError::Invalid("core control target missing"))?;
                    target.end_pos = u16::try_from(opcode_pos)
                        .map_err(|_| WasmError::Unsupported("core wasm body too large"))?;
                    stack[depth] = usize::MAX;
                }
            }
            _ => skip_core_immediates(&mut reader, opcode)?,
        }
    }

    if !saw_function_end || depth != 0 {
        return Err(WasmError::Invalid(
            "unterminated core wasm control structure",
        ));
    }
    Ok(count)
}

fn skip_core_immediates(reader: &mut Reader<'_>, opcode: u8) -> Result<(), WasmError> {
    match opcode {
        OPCODE_BR | OPCODE_BR_IF | OPCODE_CALL | OPCODE_LOCAL_GET | OPCODE_LOCAL_SET
        | OPCODE_LOCAL_TEE | OPCODE_GLOBAL_GET | OPCODE_GLOBAL_SET | OPCODE_REF_FUNC
        | OPCODE_TABLE_GET | OPCODE_TABLE_SET | OPCODE_MEMORY_SIZE | OPCODE_MEMORY_GROW => {
            reader.read_var_u32()?;
        }
        OPCODE_BR_TABLE => {
            let count = reader.read_var_u32()? as usize;
            for branch_index in 0..count {
                core::hint::black_box(branch_index);
                reader.read_var_u32()?;
            }
            reader.read_var_u32()?;
        }
        OPCODE_CALL_INDIRECT => {
            reader.read_var_u32()?;
            reader.read_var_u32()?;
        }
        OPCODE_I32_LOAD | OPCODE_I64_LOAD | OPCODE_F32_LOAD | OPCODE_F64_LOAD
        | OPCODE_I32_LOAD8_S | OPCODE_I32_LOAD8_U | OPCODE_I32_LOAD16_S | OPCODE_I32_LOAD16_U
        | OPCODE_I64_LOAD8_S | OPCODE_I64_LOAD8_U | OPCODE_I64_LOAD16_S | OPCODE_I64_LOAD16_U
        | OPCODE_I64_LOAD32_S | OPCODE_I64_LOAD32_U | OPCODE_I32_STORE | OPCODE_I64_STORE
        | OPCODE_F32_STORE | OPCODE_F64_STORE | OPCODE_I32_STORE8 | OPCODE_I32_STORE16
        | OPCODE_I64_STORE8 | OPCODE_I64_STORE16 | OPCODE_I64_STORE32 => {
            reader.read_var_u32()?;
            reader.read_var_u32()?;
        }
        OPCODE_I32_CONST => {
            reader.read_var_i32()?;
        }
        OPCODE_I64_CONST => {
            reader.read_var_i64()?;
        }
        OPCODE_F32_CONST => {
            reader.read_fixed_u32()?;
        }
        OPCODE_F64_CONST => {
            reader.read_fixed_u64()?;
        }
        OPCODE_REF_NULL => {
            reader.read_u8()?;
        }
        OPCODE_MISC => {
            decode_core_misc_instr(reader)?;
        }
        _ => {}
    }
    Ok(())
}

fn decode_core_br_table(reader: &mut Reader<'_>) -> Result<BrTableInstr, WasmError> {
    let count = reader.read_var_u32()? as usize;
    if count > CORE_WASM_BR_TABLE_CAPACITY {
        return Err(WasmError::Unsupported("core br_table too large"));
    }
    let mut table = BrTableInstr::EMPTY;
    table.label_count =
        u8::try_from(count).map_err(|_| WasmError::Unsupported("core br_table too large"))?;
    for slot in table.labels.iter_mut().take(count) {
        *slot = u16::try_from(reader.read_var_u32()?)
            .map_err(|_| WasmError::Unsupported("core br_table depth too large"))?;
    }
    table.default = u16::try_from(reader.read_var_u32()?)
        .map_err(|_| WasmError::Unsupported("core br_table depth too large"))?;
    Ok(table)
}

fn decode_core_misc_instr(reader: &mut Reader<'_>) -> Result<MiscInstr, WasmError> {
    let subopcode = reader.read_var_u32()?;
    match subopcode {
        8 => {
            let data_index = reader.read_var_u32()? as usize;
            if reader.read_var_u32()? != 0 {
                return Err(WasmError::Invalid(
                    "core memory.init memory index must be zero",
                ));
            }
            Ok(MiscInstr::MemoryInit { data_index })
        }
        9 => Ok(MiscInstr::DataDrop {
            data_index: reader.read_var_u32()? as usize,
        }),
        10 => {
            if reader.read_var_u32()? != 0 || reader.read_var_u32()? != 0 {
                return Err(WasmError::Invalid(
                    "core memory.copy memory index must be zero",
                ));
            }
            Ok(MiscInstr::MemoryCopy)
        }
        11 => {
            if reader.read_var_u32()? != 0 {
                return Err(WasmError::Invalid(
                    "core memory.fill memory index must be zero",
                ));
            }
            Ok(MiscInstr::MemoryFill)
        }
        12 => {
            let elem_index = reader.read_var_u32()? as usize;
            let table_index = reader.read_var_u32()?;
            Ok(MiscInstr::TableInit {
                elem_index,
                table_index,
            })
        }
        13 => Ok(MiscInstr::ElemDrop {
            elem_index: reader.read_var_u32()? as usize,
        }),
        14 => Ok(MiscInstr::TableCopy {
            dst_table: reader.read_var_u32()?,
            src_table: reader.read_var_u32()?,
        }),
        15 => Ok(MiscInstr::TableGrow {
            table_index: reader.read_var_u32()?,
        }),
        16 => Ok(MiscInstr::TableSize {
            table_index: reader.read_var_u32()?,
        }),
        17 => Ok(MiscInstr::TableFill {
            table_index: reader.read_var_u32()?,
        }),
        _ => Err(WasmError::Unsupported("unsupported misc opcode")),
    }
}

fn disabled_wasip1_import_message(name: Wasip1ImportName) -> &'static str {
    if matches!(name.supported_syscall(), Some(Wasip1Syscall::ArgsEnv)) {
        return "wasip1 args/env disabled by feature profile";
    }
    match name {
        Wasip1ImportName::FdWrite => "wasip1 fd_write disabled by feature profile",
        Wasip1ImportName::FdRead => "wasip1 fd_read disabled by feature profile",
        Wasip1ImportName::FdReaddir => "wasip1 fd_readdir disabled by feature profile",
        Wasip1ImportName::FdFdstatGet => "wasip1 fd_fdstat_get disabled by feature profile",
        Wasip1ImportName::FdClose => "wasip1 fd_close disabled by feature profile",
        Wasip1ImportName::PollOneoff => "wasip1 poll_oneoff disabled by feature profile",
        Wasip1ImportName::ProcExit => "wasip1 proc_exit disabled by feature profile",
        Wasip1ImportName::ProcRaise => "wasip1 proc_raise unsupported",
        Wasip1ImportName::SchedYield => "wasip1 sched_yield unsupported",
        Wasip1ImportName::ClockResGet => "wasip1 clock_res_get disabled by feature profile",
        Wasip1ImportName::ClockTimeGet => "wasip1 clock_time_get disabled by feature profile",
        Wasip1ImportName::RandomGet => "wasip1 random_get disabled by feature profile",
        Wasip1ImportName::SockAccept
        | Wasip1ImportName::SockRecv
        | Wasip1ImportName::SockSend
        | Wasip1ImportName::SockShutdown => "wasip1 sock_* unsupported",
        Wasip1ImportName::ArgsGet
        | Wasip1ImportName::ArgsSizesGet
        | Wasip1ImportName::EnvironGet
        | Wasip1ImportName::EnvironSizesGet => "wasip1 args/env disabled by feature profile",
        Wasip1ImportName::FdAdvise
        | Wasip1ImportName::FdAllocate
        | Wasip1ImportName::FdDatasync
        | Wasip1ImportName::FdFdstatSetFlags
        | Wasip1ImportName::FdFdstatSetRights
        | Wasip1ImportName::FdFilestatGet
        | Wasip1ImportName::FdFilestatSetSize
        | Wasip1ImportName::FdFilestatSetTimes
        | Wasip1ImportName::FdPread
        | Wasip1ImportName::FdPrestatGet
        | Wasip1ImportName::FdPrestatDirName
        | Wasip1ImportName::FdPwrite
        | Wasip1ImportName::FdRenumber
        | Wasip1ImportName::FdSeek
        | Wasip1ImportName::FdSync
        | Wasip1ImportName::FdTell
        | Wasip1ImportName::PathCreateDirectory
        | Wasip1ImportName::PathFilestatGet
        | Wasip1ImportName::PathFilestatSetTimes
        | Wasip1ImportName::PathLink
        | Wasip1ImportName::PathOpen
        | Wasip1ImportName::PathReadlink
        | Wasip1ImportName::PathRemoveDirectory
        | Wasip1ImportName::PathRename
        | Wasip1ImportName::PathSymlink
        | Wasip1ImportName::PathUnlinkFile => "wasip1 import unsupported by hibana-pico core",
    }
}

impl<'a> Interpreter<'a> {
    #[cfg(test)]
    fn resume(&mut self) -> Result<ExecutionEvent, WasmError> {
        self.run(TEST_RESUME_FUEL)
    }

    #[cfg(test)]
    unsafe fn init_in_place(dst: *mut Self, module: &'a [u8]) -> Result<(), WasmError> {
        unsafe {
            Module::parse_in_place(core::ptr::addr_of_mut!((*dst).module), module)?;
            Self::init_from_parsed_module_in_place(dst)?;
        }
        Ok(())
    }

    fn run(&mut self, mut fuel: u32) -> Result<ExecutionEvent, WasmError> {
        if self.done {
            return Ok(ExecutionEvent::Done);
        }
        if self.pending.is_some() {
            return Err(WasmError::PendingCall);
        }

        loop {
            if self.frame_len == 0 {
                self.done = true;
                return Ok(ExecutionEvent::Done);
            }
            if fuel == 0 {
                return Err(WasmError::FuelExhausted);
            }
            fuel -= 1;

            let instr = self.current_instr()?;
            if let Some(event) = self.exec_instr(instr)? {
                return Ok(event);
            }
        }
    }

    fn pending_wasip1_call(&self) -> Result<PendingWasip1Call, WasmError> {
        match self.pending.as_ref() {
            Some(PendingExecution::Wasip1(call)) => Ok(*call),
            _ => Err(WasmError::Invalid("missing pending wasi import")),
        }
    }

    fn exec_instr(&mut self, instr: Instr) -> Result<Option<ExecutionEvent>, WasmError> {
        match instr {
            Instr::Nop => {}
            Instr::Simple(opcode) => return self.exec_simple(opcode),
            Instr::U32(opcode, value) => return self.exec_u32(opcode, value),
            Instr::CallIndirect {
                type_index,
                table_index,
            } => {
                if table_index != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let table_index = self.pop_core_i32()? as usize;
                let function_index = *self
                    .table_functions
                    .get(table_index)
                    .ok_or(WasmError::Invalid("core call_indirect table out of range"))?;
                if table_index >= self.table_size || function_index == u32::MAX {
                    return Err(WasmError::Invalid("core call_indirect empty slot"));
                }
                if self.module.core_func_type_index(function_index)? != type_index {
                    return Err(WasmError::Invalid("core call_indirect type mismatch"));
                }
                if function_index < self.module.import_count as u32 {
                    return Ok(Some(self.call_core_import(function_index)?));
                }
                self.push_frame(function_index)?;
            }
            Instr::Mem { opcode, offset } => self.exec_mem(opcode, offset)?,
            Instr::I32Const(value) => self.push_core_value(Value::I32(value))?,
            Instr::I64Const(value) => self.push_core_value(Value::I64(value))?,
            Instr::F32Const(value) => self.push_core_value(Value::F32(value))?,
            Instr::F64Const(value) => self.push_core_value(Value::F64(value))?,
            Instr::BrTable(table) => {
                let selected = self.pop_core_i32()? as usize;
                self.core_branch(table.selected_depth(selected))?;
            }
            Instr::Misc(instr) => self.exec_misc(instr)?,
            Instr::Block(target) | Instr::Loop(target) => {
                let frame = self.current_frame()?;
                let start_pos = frame.pc;
                let end_pos = target.end()?;
                let kind = match instr {
                    Instr::Loop(_) => ControlKind::Loop,
                    _ => ControlKind::Block,
                };
                let stack_height = self.value_len;
                self.push_core_control(ControlFrame {
                    kind,
                    start_pos,
                    else_pos: usize::MAX,
                    end_pos,
                    result_count: target.result_count,
                    result_kind: target.result_kind,
                    stack_height,
                })?;
            }
            Instr::If(target) => {
                let condition = self.pop_core_i32()?;
                let frame = self.current_frame()?;
                let start_pos = frame.pc;
                let else_pos = target.else_pos();
                let end_pos = target.end()?;
                let stack_height = self.value_len;
                if condition != 0 {
                    self.push_core_control(ControlFrame {
                        kind: ControlKind::If,
                        start_pos,
                        else_pos,
                        end_pos,
                        result_count: target.result_count,
                        result_kind: target.result_kind,
                        stack_height,
                    })?;
                } else if else_pos != usize::MAX {
                    self.push_core_control(ControlFrame {
                        kind: ControlKind::If,
                        start_pos: else_pos.saturating_add(1),
                        else_pos,
                        end_pos,
                        result_count: target.result_count,
                        result_kind: target.result_kind,
                        stack_height,
                    })?;
                    self.current_frame_mut()?.pc = else_pos.saturating_add(1);
                } else if target.result_count == 0 {
                    self.current_frame_mut()?.pc = end_pos.saturating_add(1);
                } else {
                    return Err(WasmError::Invalid("if result requires else arm"));
                }
            }
            Instr::Else => {
                let control = self.pop_core_control()?;
                if control.kind != ControlKind::If {
                    return Err(WasmError::Invalid("else without if"));
                }
                self.normalize_core_control_result(control)?;
                self.current_frame_mut()?.pc = control.end_pos.saturating_add(1);
            }
            Instr::End => {
                if self.current_frame()?.control_len == 0 {
                    self.pop_frame()?;
                } else {
                    let control = self.pop_core_control()?;
                    self.normalize_core_control_result(control)?;
                }
            }
        }
        Ok(None)
    }

    fn exec_u32(&mut self, opcode: u8, value: u32) -> Result<Option<ExecutionEvent>, WasmError> {
        match opcode {
            OPCODE_BR => self.core_branch(value as usize)?,
            OPCODE_BR_IF => {
                if self.pop_core_i32()? != 0 {
                    self.core_branch(value as usize)?;
                }
            }
            OPCODE_CALL => {
                if value < self.module.import_count as u32 {
                    return Ok(Some(self.call_core_import(value)?));
                }
                self.push_frame(value)?;
            }
            OPCODE_LOCAL_GET => {
                let local = value as usize;
                let value = *self
                    .current_frame()?
                    .locals
                    .get(local)
                    .ok_or(WasmError::Invalid("core local.get out of range"))?;
                if local >= self.current_frame()?.local_count {
                    return Err(WasmError::Invalid("core local.get inactive local"));
                }
                self.push_core_value(value)?;
            }
            OPCODE_LOCAL_SET => {
                let value_to_set = self.pop_core_value()?;
                self.set_core_local(value as usize, value_to_set)?;
            }
            OPCODE_LOCAL_TEE => {
                let value_to_set = *self
                    .values
                    .get(self.value_len.saturating_sub(1))
                    .ok_or(WasmError::StackUnderflow)?;
                self.set_core_local(value as usize, value_to_set)?;
            }
            OPCODE_GLOBAL_GET => {
                let global = value as usize;
                if global >= self.global_count {
                    return Err(WasmError::Invalid("core global.get out of range"));
                }
                let value = *self
                    .globals
                    .get(global)
                    .ok_or(WasmError::Invalid("core global.get out of range"))?;
                self.push_core_value(value)?;
            }
            OPCODE_GLOBAL_SET => {
                let global = value as usize;
                if global >= self.global_count {
                    return Err(WasmError::Invalid("core global.set out of range"));
                }
                if !self.global_mutable[global] {
                    return Err(WasmError::Invalid("core global.set immutable global"));
                }
                let value_to_set = self.pop_core_value()?;
                if value_to_set.kind() != self.global_kinds[global] {
                    return Err(WasmError::Invalid("core global type mismatch"));
                }
                self.globals[global] = value_to_set;
            }
            OPCODE_TABLE_GET => {
                if value != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let index = self.pop_core_i32()? as usize;
                if index >= self.table_size {
                    return Err(WasmError::Invalid("core table.get out of range"));
                }
                self.push_core_value(Value::FuncRef(self.table_functions[index]))?;
            }
            OPCODE_TABLE_SET => {
                if value != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let value_to_set = self.pop_core_value()?.as_funcref()?;
                let index = self.pop_core_i32()? as usize;
                if index >= self.table_size {
                    return Err(WasmError::Invalid("core table.set out of range"));
                }
                if value_to_set != u32::MAX {
                    self.module.core_func_type_index(value_to_set)?;
                }
                self.table_functions[index] = value_to_set;
            }
            OPCODE_REF_FUNC => {
                self.module.core_func_type_index(value)?;
                self.push_core_value(Value::FuncRef(value))?;
            }
            OPCODE_REF_NULL => {
                if value as u8 != VALTYPE_FUNCREF {
                    return Err(WasmError::Unsupported("only null funcref is supported"));
                }
                self.push_core_value(Value::FuncRef(u32::MAX))?;
            }
            OPCODE_MEMORY_SIZE => {
                if value != 0 {
                    return Err(WasmError::Invalid(
                        "core memory instruction index must be zero",
                    ));
                }
                self.push_core_value(Value::I32(self.memory_pages))?;
            }
            OPCODE_MEMORY_GROW => {
                if value != 0 {
                    return Err(WasmError::Invalid(
                        "core memory instruction index must be zero",
                    ));
                }
                let requested_pages = self.pop_core_i32()?;
                let previous_pages = self.memory_pages;
                let new_pages = previous_pages
                    .checked_add(requested_pages)
                    .and_then(|pages| {
                        if pages <= self.module.memory_max_pages
                            && pages <= CORE_WASM_MAX_MEMORY_PAGES
                        {
                            Some(pages)
                        } else {
                            None
                        }
                    });
                if let Some(pages) = new_pages {
                    self.memory_pages = pages;
                    self.push_core_value(Value::I32(previous_pages))?;
                } else {
                    self.push_core_value(Value::I32(u32::MAX))?;
                }
                let event = MemoryGrowEvent {
                    previous_pages,
                    requested_pages,
                    new_pages,
                };
                self.pending = Some(PendingExecution::MemoryGrow(event));
                return Ok(Some(ExecutionEvent::MemoryGrow(event)));
            }
            _ => return Err(WasmError::UnsupportedOpcode(opcode)),
        }
        Ok(None)
    }

    fn exec_mem(&mut self, opcode: u8, offset: u32) -> Result<(), WasmError> {
        match opcode {
            OPCODE_I32_LOAD => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I32(self.core_read_u32(addr)?))?;
            }
            OPCODE_I64_LOAD => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64(self.core_read_u64(addr)?))?;
            }
            OPCODE_F32_LOAD => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::F32(self.core_read_u32(addr)?))?;
            }
            OPCODE_F64_LOAD => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::F64(self.core_read_u64(addr)?))?;
            }
            OPCODE_I32_LOAD8_S => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I32((self.core_read_u8(addr)? as i8 as i32) as u32))?;
            }
            OPCODE_I32_LOAD8_U => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I32(self.core_read_u8(addr)? as u32))?;
            }
            OPCODE_I32_LOAD16_S => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I32((self.core_read_u16(addr)? as i16 as i32) as u32))?;
            }
            OPCODE_I32_LOAD16_U => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I32(self.core_read_u16(addr)? as u32))?;
            }
            OPCODE_I64_LOAD8_S => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64((self.core_read_u8(addr)? as i8 as i64) as u64))?;
            }
            OPCODE_I64_LOAD8_U => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64(self.core_read_u8(addr)? as u64))?;
            }
            OPCODE_I64_LOAD16_S => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64((self.core_read_u16(addr)? as i16 as i64) as u64))?;
            }
            OPCODE_I64_LOAD16_U => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64(self.core_read_u16(addr)? as u64))?;
            }
            OPCODE_I64_LOAD32_S => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64((self.core_read_u32(addr)? as i32 as i64) as u64))?;
            }
            OPCODE_I64_LOAD32_U => {
                let addr = self.core_effective_addr(offset)?;
                self.push_core_value(Value::I64(self.core_read_u32(addr)? as u64))?;
            }
            OPCODE_I32_STORE => {
                let value = self.pop_core_i32()?;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u32(addr, value)?;
            }
            OPCODE_I64_STORE => {
                let value = self.pop_core_i64()?;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u64(addr, value)?;
            }
            OPCODE_F32_STORE => {
                let value = self.pop_core_f32_bits()?;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u32(addr, value)?;
            }
            OPCODE_F64_STORE => {
                let value = self.pop_core_f64_bits()?;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u64(addr, value)?;
            }
            OPCODE_I32_STORE8 => {
                let value = self.pop_core_i32()? as u8;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u8(addr, value)?;
            }
            OPCODE_I32_STORE16 => {
                let value = self.pop_core_i32()? as u16;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u16(addr, value)?;
            }
            OPCODE_I64_STORE8 => {
                let value = self.pop_core_i64()? as u8;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u8(addr, value)?;
            }
            OPCODE_I64_STORE16 => {
                let value = self.pop_core_i64()? as u16;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u16(addr, value)?;
            }
            OPCODE_I64_STORE32 => {
                let value = self.pop_core_i64()? as u32;
                let addr = self.core_effective_addr(offset)?;
                self.core_write_u32(addr, value)?;
            }
            _ => return Err(WasmError::UnsupportedOpcode(opcode)),
        }
        Ok(())
    }

    fn exec_simple(&mut self, opcode: u8) -> Result<Option<ExecutionEvent>, WasmError> {
        match opcode {
            OPCODE_UNREACHABLE => return Err(WasmError::Trap),
            OPCODE_NOP => {}
            OPCODE_RETURN => self.pop_frame()?,
            OPCODE_DROP => {
                let dropped_value = self.pop_core_value()?;
                core::hint::black_box(dropped_value);
            }
            OPCODE_SELECT => {
                let condition = self.pop_core_i32()?;
                let alternate = self.pop_core_value()?;
                let consequent = self.pop_core_value()?;
                self.push_core_value(if condition != 0 {
                    consequent
                } else {
                    alternate
                })?;
            }
            OPCODE_I32_EQZ => {
                let value = self.pop_core_i32()?;
                self.push_core_value(Value::I32((value == 0) as u32))?;
            }
            OPCODE_I32_EQ => self.core_binary_i32(|a, b| (a == b) as u32)?,
            OPCODE_I32_NE => self.core_binary_i32(|a, b| (a != b) as u32)?,
            OPCODE_I32_LT_S => self.core_binary_i32(|a, b| ((a as i32) < (b as i32)) as u32)?,
            OPCODE_I32_LT_U => self.core_binary_i32(|a, b| (a < b) as u32)?,
            OPCODE_I32_GT_S => self.core_binary_i32(|a, b| ((a as i32) > (b as i32)) as u32)?,
            OPCODE_I32_GT_U => self.core_binary_i32(|a, b| (a > b) as u32)?,
            OPCODE_I32_LE_S => self.core_binary_i32(|a, b| ((a as i32) <= (b as i32)) as u32)?,
            OPCODE_I32_LE_U => self.core_binary_i32(|a, b| (a <= b) as u32)?,
            OPCODE_I32_GE_S => self.core_binary_i32(|a, b| ((a as i32) >= (b as i32)) as u32)?,
            OPCODE_I32_GE_U => self.core_binary_i32(|a, b| (a >= b) as u32)?,
            OPCODE_I64_EQZ => {
                let value = self.pop_core_i64()?;
                self.push_core_value(Value::I32((value == 0) as u32))?;
            }
            OPCODE_I64_EQ => self.core_binary_i64_cmp(|a, b| a == b)?,
            OPCODE_I64_NE => self.core_binary_i64_cmp(|a, b| a != b)?,
            OPCODE_I64_LT_S => self.core_binary_i64_cmp(|a, b| (a as i64) < (b as i64))?,
            OPCODE_I64_LT_U => self.core_binary_i64_cmp(|a, b| a < b)?,
            OPCODE_I64_GT_S => self.core_binary_i64_cmp(|a, b| (a as i64) > (b as i64))?,
            OPCODE_I64_GT_U => self.core_binary_i64_cmp(|a, b| a > b)?,
            OPCODE_I64_LE_S => self.core_binary_i64_cmp(|a, b| (a as i64) <= (b as i64))?,
            OPCODE_I64_LE_U => self.core_binary_i64_cmp(|a, b| a <= b)?,
            OPCODE_I64_GE_S => self.core_binary_i64_cmp(|a, b| (a as i64) >= (b as i64))?,
            OPCODE_I64_GE_U => self.core_binary_i64_cmp(|a, b| a >= b)?,
            OPCODE_F32_EQ => self.core_binary_f32_cmp(|a, b| a == b)?,
            OPCODE_F32_NE => self.core_binary_f32_cmp(|a, b| a != b)?,
            OPCODE_F32_LT => self.core_binary_f32_cmp(|a, b| a < b)?,
            OPCODE_F32_GT => self.core_binary_f32_cmp(|a, b| a > b)?,
            OPCODE_F32_LE => self.core_binary_f32_cmp(|a, b| a <= b)?,
            OPCODE_F32_GE => self.core_binary_f32_cmp(|a, b| a >= b)?,
            OPCODE_F64_EQ => self.core_binary_f64_cmp(|a, b| a == b)?,
            OPCODE_F64_NE => self.core_binary_f64_cmp(|a, b| a != b)?,
            OPCODE_F64_LT => self.core_binary_f64_cmp(|a, b| a < b)?,
            OPCODE_F64_GT => self.core_binary_f64_cmp(|a, b| a > b)?,
            OPCODE_F64_LE => self.core_binary_f64_cmp(|a, b| a <= b)?,
            OPCODE_F64_GE => self.core_binary_f64_cmp(|a, b| a >= b)?,
            OPCODE_I32_CLZ => {
                let value = self.pop_core_i32()?;
                self.push_core_value(Value::I32(value.leading_zeros()))?;
            }
            OPCODE_I32_CTZ => {
                let value = self.pop_core_i32()?;
                self.push_core_value(Value::I32(value.trailing_zeros()))?;
            }
            OPCODE_I32_POPCNT => {
                let value = self.pop_core_i32()?;
                self.push_core_value(Value::I32(value.count_ones()))?;
            }
            OPCODE_I32_ADD => self.core_binary_i32(u32::wrapping_add)?,
            OPCODE_I32_SUB => self.core_binary_i32(u32::wrapping_sub)?,
            OPCODE_I32_MUL => self.core_binary_i32(u32::wrapping_mul)?,
            OPCODE_I32_DIV_S => {
                let rhs = self.pop_core_i32()? as i32;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i32()? as i32;
                if lhs == i32::MIN && rhs == -1 {
                    return Err(WasmError::Trap);
                }
                self.push_core_value(Value::I32(lhs.wrapping_div(rhs) as u32))?;
            }
            OPCODE_I32_DIV_U => {
                let rhs = self.pop_core_i32()?;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i32()?;
                self.push_core_value(Value::I32(lhs / rhs))?;
            }
            OPCODE_I32_REM_S => {
                let rhs = self.pop_core_i32()? as i32;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i32()? as i32;
                self.push_core_value(Value::I32(lhs.wrapping_rem(rhs) as u32))?;
            }
            OPCODE_I32_REM_U => {
                let rhs = self.pop_core_i32()?;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i32()?;
                self.push_core_value(Value::I32(lhs % rhs))?;
            }
            OPCODE_I32_AND => self.core_binary_i32(|a, b| a & b)?,
            OPCODE_I32_OR => self.core_binary_i32(|a, b| a | b)?,
            OPCODE_I32_XOR => self.core_binary_i32(|a, b| a ^ b)?,
            OPCODE_I32_SHL => self.core_binary_i32(|a, b| a.wrapping_shl(b & 31))?,
            OPCODE_I32_SHR_S => self.core_binary_i32(|a, b| ((a as i32) >> (b & 31)) as u32)?,
            OPCODE_I32_SHR_U => self.core_binary_i32(|a, b| a.wrapping_shr(b & 31))?,
            OPCODE_I32_ROTL => self.core_binary_i32(|a, b| a.rotate_left(b & 31))?,
            OPCODE_I32_ROTR => self.core_binary_i32(|a, b| a.rotate_right(b & 31))?,
            OPCODE_I64_CLZ => {
                let value = self.pop_core_i64()?;
                self.push_core_value(Value::I64(value.leading_zeros() as u64))?;
            }
            OPCODE_I64_CTZ => {
                let value = self.pop_core_i64()?;
                self.push_core_value(Value::I64(value.trailing_zeros() as u64))?;
            }
            OPCODE_I64_POPCNT => {
                let value = self.pop_core_i64()?;
                self.push_core_value(Value::I64(value.count_ones() as u64))?;
            }
            OPCODE_I64_ADD => self.core_binary_i64(u64::wrapping_add)?,
            OPCODE_I64_SUB => self.core_binary_i64(u64::wrapping_sub)?,
            OPCODE_I64_MUL => self.core_binary_i64(u64::wrapping_mul)?,
            OPCODE_I64_DIV_S => {
                let rhs = self.pop_core_i64()? as i64;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i64()? as i64;
                if lhs == i64::MIN && rhs == -1 {
                    return Err(WasmError::Trap);
                }
                self.push_core_value(Value::I64(lhs.wrapping_div(rhs) as u64))?;
            }
            OPCODE_I64_DIV_U => {
                let rhs = self.pop_core_i64()?;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i64()?;
                self.push_core_value(Value::I64(lhs / rhs))?;
            }
            OPCODE_I64_REM_S => {
                let rhs = self.pop_core_i64()? as i64;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i64()? as i64;
                self.push_core_value(Value::I64(lhs.wrapping_rem(rhs) as u64))?;
            }
            OPCODE_I64_REM_U => {
                let rhs = self.pop_core_i64()?;
                if rhs == 0 {
                    return Err(WasmError::Trap);
                }
                let lhs = self.pop_core_i64()?;
                self.push_core_value(Value::I64(lhs % rhs))?;
            }
            OPCODE_I64_AND => self.core_binary_i64(|a, b| a & b)?,
            OPCODE_I64_OR => self.core_binary_i64(|a, b| a | b)?,
            OPCODE_I64_XOR => self.core_binary_i64(|a, b| a ^ b)?,
            OPCODE_I64_SHL => {
                let rhs = self.pop_core_i64()? as u32;
                let lhs = self.pop_core_i64()?;
                self.push_core_value(Value::I64(lhs.wrapping_shl(rhs & 63)))?;
            }
            OPCODE_I64_SHR_S => {
                let rhs = self.pop_core_i64()? as u32;
                let lhs = self.pop_core_i64()? as i64;
                self.push_core_value(Value::I64((lhs >> (rhs & 63)) as u64))?;
            }
            OPCODE_I64_SHR_U => {
                let rhs = self.pop_core_i64()? as u32;
                let lhs = self.pop_core_i64()?;
                self.push_core_value(Value::I64(lhs.wrapping_shr(rhs & 63)))?;
            }
            OPCODE_I64_ROTL => {
                self.core_binary_i64(|a, b| a.rotate_left((b & 63) as u32))?;
            }
            OPCODE_I64_ROTR => {
                self.core_binary_i64(|a, b| a.rotate_right((b & 63) as u32))?;
            }
            OPCODE_F32_ABS => self.core_unary_f32(wasm_f32_abs)?,
            OPCODE_F32_NEG => self.core_unary_f32(wasm_f32_neg)?,
            OPCODE_F32_CEIL => self.core_unary_f32(wasm_f32_ceil)?,
            OPCODE_F32_FLOOR => self.core_unary_f32(wasm_f32_floor)?,
            OPCODE_F32_TRUNC => self.core_unary_f32(wasm_f32_trunc)?,
            OPCODE_F32_NEAREST => self.core_unary_f32(wasm_f32_nearest)?,
            OPCODE_F32_SQRT => self.core_unary_f32(wasm_f32_sqrt)?,
            OPCODE_F32_ADD => self.core_binary_f32(|a, b| a + b)?,
            OPCODE_F32_SUB => self.core_binary_f32(|a, b| a - b)?,
            OPCODE_F32_MUL => self.core_binary_f32(|a, b| a * b)?,
            OPCODE_F32_DIV => self.core_binary_f32(|a, b| a / b)?,
            OPCODE_F32_MIN => self.core_binary_f32(wasm_f32_min)?,
            OPCODE_F32_MAX => self.core_binary_f32(wasm_f32_max)?,
            OPCODE_F32_COPYSIGN => self.core_binary_f32(wasm_f32_copysign)?,
            OPCODE_F64_ABS => self.core_unary_f64(wasm_f64_abs)?,
            OPCODE_F64_NEG => self.core_unary_f64(wasm_f64_neg)?,
            OPCODE_F64_CEIL => self.core_unary_f64(wasm_f64_ceil)?,
            OPCODE_F64_FLOOR => self.core_unary_f64(wasm_f64_floor)?,
            OPCODE_F64_TRUNC => self.core_unary_f64(wasm_f64_trunc)?,
            OPCODE_F64_NEAREST => self.core_unary_f64(wasm_f64_nearest)?,
            OPCODE_F64_SQRT => self.core_unary_f64(wasm_f64_sqrt)?,
            OPCODE_F64_ADD => self.core_binary_f64(|a, b| a + b)?,
            OPCODE_F64_SUB => self.core_binary_f64(|a, b| a - b)?,
            OPCODE_F64_MUL => self.core_binary_f64(|a, b| a * b)?,
            OPCODE_F64_DIV => self.core_binary_f64(|a, b| a / b)?,
            OPCODE_F64_MIN => self.core_binary_f64(wasm_f64_min)?,
            OPCODE_F64_MAX => self.core_binary_f64(wasm_f64_max)?,
            OPCODE_F64_COPYSIGN => self.core_binary_f64(wasm_f64_copysign)?,
            OPCODE_I32_WRAP_I64 => {
                let value = self.pop_core_i64()? as u32;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I32_TRUNC_F32_S => {
                let value = trunc_f32_to_i32_s(self.pop_core_f32()?)?;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I32_TRUNC_F32_U => {
                let value = trunc_f32_to_i32_u(self.pop_core_f32()?)?;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I32_TRUNC_F64_S => {
                let value = trunc_f64_to_i32_s(self.pop_core_f64()?)?;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I32_TRUNC_F64_U => {
                let value = trunc_f64_to_i32_u(self.pop_core_f64()?)?;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I64_EXTEND_I32_S => {
                let value = self.pop_core_i32()? as i32 as i64 as u64;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_EXTEND_I32_U => {
                let value = self.pop_core_i32()? as u64;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_TRUNC_F32_S => {
                let value = trunc_f32_to_i64_s(self.pop_core_f32()?)?;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_TRUNC_F32_U => {
                let value = trunc_f32_to_i64_u(self.pop_core_f32()?)?;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_TRUNC_F64_S => {
                let value = trunc_f64_to_i64_s(self.pop_core_f64()?)?;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_TRUNC_F64_U => {
                let value = trunc_f64_to_i64_u(self.pop_core_f64()?)?;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_F32_CONVERT_I32_S => {
                let value = self.pop_core_i32()? as i32 as f32;
                self.push_core_value(Value::F32(value.to_bits()))?;
            }
            OPCODE_F32_CONVERT_I32_U => {
                let value = self.pop_core_i32()? as f32;
                self.push_core_value(Value::F32(value.to_bits()))?;
            }
            OPCODE_F32_CONVERT_I64_S => {
                let value = self.pop_core_i64()? as i64 as f32;
                self.push_core_value(Value::F32(value.to_bits()))?;
            }
            OPCODE_F32_CONVERT_I64_U => {
                let value = self.pop_core_i64()? as f32;
                self.push_core_value(Value::F32(value.to_bits()))?;
            }
            OPCODE_F32_DEMOTE_F64 => {
                let value = self.pop_core_f64()? as f32;
                self.push_core_value(Value::F32(value.to_bits()))?;
            }
            OPCODE_F64_CONVERT_I32_S => {
                let value = self.pop_core_i32()? as i32 as f64;
                self.push_core_value(Value::F64(value.to_bits()))?;
            }
            OPCODE_F64_CONVERT_I32_U => {
                let value = self.pop_core_i32()? as f64;
                self.push_core_value(Value::F64(value.to_bits()))?;
            }
            OPCODE_F64_CONVERT_I64_S => {
                let value = self.pop_core_i64()? as i64 as f64;
                self.push_core_value(Value::F64(value.to_bits()))?;
            }
            OPCODE_F64_CONVERT_I64_U => {
                let value = self.pop_core_i64()? as f64;
                self.push_core_value(Value::F64(value.to_bits()))?;
            }
            OPCODE_F64_PROMOTE_F32 => {
                let value = self.pop_core_f32()? as f64;
                self.push_core_value(Value::F64(value.to_bits()))?;
            }
            OPCODE_I32_REINTERPRET_F32 => {
                let value = self.pop_core_f32_bits()?;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I64_REINTERPRET_F64 => {
                let value = self.pop_core_f64_bits()?;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_F32_REINTERPRET_I32 => {
                let value = self.pop_core_i32()?;
                self.push_core_value(Value::F32(value))?;
            }
            OPCODE_F64_REINTERPRET_I64 => {
                let value = self.pop_core_i64()?;
                self.push_core_value(Value::F64(value))?;
            }
            OPCODE_I32_EXTEND8_S => {
                let value = self.pop_core_i32()? as i8 as i32 as u32;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I32_EXTEND16_S => {
                let value = self.pop_core_i32()? as i16 as i32 as u32;
                self.push_core_value(Value::I32(value))?;
            }
            OPCODE_I64_EXTEND8_S => {
                let value = self.pop_core_i64()? as i8 as i64 as u64;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_EXTEND16_S => {
                let value = self.pop_core_i64()? as i16 as i64 as u64;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_I64_EXTEND32_S => {
                let value = self.pop_core_i64()? as i32 as i64 as u64;
                self.push_core_value(Value::I64(value))?;
            }
            OPCODE_REF_IS_NULL => {
                let value = self.pop_core_value()?.as_funcref()?;
                self.push_core_value(Value::I32((value == u32::MAX) as u32))?;
            }
            _ => return Err(WasmError::UnsupportedOpcode(opcode)),
        }
        Ok(None)
    }

    fn exec_misc(&mut self, instr: MiscInstr) -> Result<(), WasmError> {
        match instr {
            MiscInstr::MemoryInit { data_index } => {
                let len = self.pop_core_i32()? as usize;
                let src_addr = self.pop_core_i32()? as usize;
                let dst_addr = self.pop_core_i32()?;
                let dst = self.core_translate_addr(dst_addr)?;
                self.core_memory_init(data_index, dst, src_addr, len)?;
            }
            MiscInstr::DataDrop { data_index } => {
                if data_index >= self.data_dropped.len()
                    || self.module.data_segments[data_index].is_none()
                {
                    return Err(WasmError::Invalid("core data.drop out of range"));
                }
                self.data_dropped[data_index] = true;
            }
            MiscInstr::MemoryCopy => {
                let len = self.pop_core_i32()? as usize;
                let src_addr = self.pop_core_i32()?;
                let dst_addr = self.pop_core_i32()?;
                let src = self.core_translate_addr(src_addr)?;
                let dst = self.core_translate_addr(dst_addr)?;
                self.core_memory_copy(dst, src, len)?;
            }
            MiscInstr::MemoryFill => {
                let len = self.pop_core_i32()? as usize;
                let value = self.pop_core_i32()? as u8;
                let dst_addr = self.pop_core_i32()?;
                let dst = self.core_translate_addr(dst_addr)?;
                self.core_memory_fill(dst, value, len)?;
            }
            MiscInstr::TableInit {
                elem_index,
                table_index,
            } => {
                if table_index != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let len = self.pop_core_i32()? as usize;
                let src = self.pop_core_i32()? as usize;
                let dst = self.pop_core_i32()? as usize;
                self.core_table_init(elem_index, dst, src, len)?;
            }
            MiscInstr::ElemDrop { elem_index } => {
                if elem_index >= self.element_dropped.len()
                    || self.module.element_segments[elem_index].is_none()
                {
                    return Err(WasmError::Invalid("core elem.drop out of range"));
                }
                self.element_dropped[elem_index] = true;
            }
            MiscInstr::TableCopy {
                dst_table,
                src_table,
            } => {
                if dst_table != 0 || src_table != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let len = self.pop_core_i32()? as usize;
                let src = self.pop_core_i32()? as usize;
                let dst = self.pop_core_i32()? as usize;
                self.core_table_copy(dst, src, len)?;
            }
            MiscInstr::TableGrow { table_index } => {
                if table_index != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let delta = self.pop_core_i32()? as usize;
                let init = self.pop_core_value()?.as_funcref()?;
                if init != u32::MAX {
                    self.module.core_func_type_index(init)?;
                }
                let previous = self.table_size;
                let Some(new_size) = self.table_size.checked_add(delta) else {
                    self.push_core_value(Value::I32(u32::MAX))?;
                    return Ok(());
                };
                if new_size > CORE_WASM_TABLE_CAPACITY {
                    self.push_core_value(Value::I32(u32::MAX))?;
                } else {
                    for slot in self
                        .table_functions
                        .iter_mut()
                        .take(new_size)
                        .skip(self.table_size)
                    {
                        *slot = init;
                    }
                    self.table_size = new_size;
                    self.push_core_value(Value::I32(previous as u32))?;
                }
            }
            MiscInstr::TableSize { table_index } => {
                if table_index != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                self.push_core_value(Value::I32(self.table_size as u32))?;
            }
            MiscInstr::TableFill { table_index } => {
                if table_index != 0 {
                    return Err(WasmError::Invalid(
                        "core table instruction index must be zero",
                    ));
                }
                let len = self.pop_core_i32()? as usize;
                let value = self.pop_core_value()?.as_funcref()?;
                let start = self.pop_core_i32()? as usize;
                if value != u32::MAX {
                    self.module.core_func_type_index(value)?;
                }
                self.core_table_fill(start, value, len)?;
            }
        }
        Ok(())
    }

    pub(super) fn finish_host_import(&mut self, results: &[Value]) -> Result<(), WasmError> {
        let pending = self.pending.take().ok_or(WasmError::PendingRequired)?;
        let PendingExecution::Wasip1(call) = pending else {
            self.pending = Some(pending);
            return Err(WasmError::PendingMismatch);
        };
        if results.len() != call.result_count() {
            return Err(WasmError::PendingMismatch);
        }
        for result in results.iter().copied() {
            if result.kind() != ValueKind::I32 {
                return Err(WasmError::Invalid("core import result type mismatch"));
            }
            self.push_core_value(result)?;
        }
        Ok(())
    }

    fn finish_matching_host_import(
        &mut self,
        expected: PendingWasip1Call,
        results: &[Value],
    ) -> Result<(), WasmError> {
        let pending = self.pending.take().ok_or(WasmError::PendingRequired)?;
        let PendingExecution::Wasip1(call) = pending else {
            self.pending = Some(pending);
            return Err(WasmError::PendingMismatch);
        };
        if call != expected {
            self.pending = Some(PendingExecution::Wasip1(call));
            return Err(WasmError::PendingMismatch);
        }
        if results.len() != call.result_count() {
            return Err(WasmError::PendingMismatch);
        }
        for result in results.iter().copied() {
            if result.kind() != ValueKind::I32 {
                return Err(WasmError::Invalid("core import result type mismatch"));
            }
            self.push_core_value(result)?;
        }
        Ok(())
    }

    pub(super) fn finish_memory_grow_event(&mut self) -> Result<MemoryGrowEvent, WasmError> {
        let pending = self.pending.take().ok_or(WasmError::PendingRequired)?;
        let PendingExecution::MemoryGrow(event) = pending else {
            self.pending = Some(pending);
            return Err(WasmError::PendingMismatch);
        };
        Ok(event)
    }

    #[cfg(test)]
    fn memory_pages(&self) -> u32 {
        self.memory_pages
    }

    pub(super) fn read_memory(&self, addr: u32, out: &mut [u8]) -> Result<(), WasmError> {
        let start = self.core_translate_addr(addr)?;
        let end = start.checked_add(out.len()).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self.memory.get(start..end).ok_or(WasmError::Truncated)?;
        out.copy_from_slice(bytes);
        Ok(())
    }

    pub(super) fn write_memory(&mut self, addr: u32, bytes: &[u8]) -> Result<(), WasmError> {
        let start = self.core_translate_addr(addr)?;
        let end = start.checked_add(bytes.len()).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let dst = self
            .memory
            .get_mut(start..end)
            .ok_or(WasmError::Truncated)?;
        dst.copy_from_slice(bytes);
        Ok(())
    }

    pub(super) fn read_memory_u32(&self, addr: u32) -> Result<u32, WasmError> {
        let offset = self.core_translate_addr(addr)?;
        self.core_read_u32(offset)
    }

    pub(super) fn write_memory_u32(&mut self, addr: u32, value: u32) -> Result<(), WasmError> {
        let offset = self.core_translate_addr(addr)?;
        self.core_write_u32(offset, value)
    }

    fn init_core_data_segments(&mut self) -> Result<(), WasmError> {
        let segments = self.module.data_segments;
        for (index, segment) in segments.into_iter().flatten().enumerate() {
            if !segment.active {
                continue;
            }
            let start = self.core_translate_addr(segment.offset)?;
            let end = start
                .checked_add(segment.bytes.len())
                .ok_or(WasmError::Truncated)?;
            if end > self.core_memory_len()? {
                return Err(WasmError::Truncated);
            }
            let dst = self
                .memory
                .get_mut(start..end)
                .ok_or(WasmError::Truncated)?;
            dst.copy_from_slice(segment.bytes);
            self.data_dropped[index] = false;
        }
        Ok(())
    }

    fn push_frame(&mut self, function_index: u32) -> Result<(), WasmError> {
        if function_index < self.module.import_count as u32 {
            return Err(WasmError::Invalid("cannot push import frame"));
        }
        let body = self.module.core_function_body(function_index)?;
        let type_index = self.module.core_func_type_index(function_index)?;
        let ty = self.module.core_func_type(type_index)?;
        let arg_start = self
            .value_len
            .checked_sub(ty.param_count)
            .ok_or(WasmError::StackUnderflow)?;
        for index in 0..ty.param_count {
            let value = self.values[arg_start + index];
            if value.kind() != ty.params[index] {
                return Err(WasmError::Invalid("core call argument type mismatch"));
            }
        }
        self.value_len = arg_start;
        {
            let slot = self
                .frames
                .get_mut(self.frame_len)
                .ok_or(WasmError::StackOverflow)?;
            *slot = Frame::empty();
            slot.code = body.code;
            slot.local_count = body.local_count;
            slot.local_kinds = body.local_kinds;

            for index in 0..ty.param_count {
                slot.locals[index] = self.values[arg_start + index];
            }
            for index in ty.param_count..body.local_count {
                slot.locals[index] = Value::zero(body.local_kinds[index]);
            }
        }
        self.frame_len += 1;
        self.decode_current_frame_control_targets()?;
        Ok(())
    }

    fn pop_frame(&mut self) -> Result<(), WasmError> {
        if self.frame_len == 0 {
            return Err(WasmError::StackUnderflow);
        }
        self.frame_len -= 1;
        if self.frame_len == 0 {
            self.done = true;
            self.control_target_count = 0;
        } else {
            self.decode_current_frame_control_targets()?;
        }
        Ok(())
    }

    fn expect_import_signature(
        ty: FuncType,
        params: &[ValueKind],
        result_count: usize,
        message: &'static str,
    ) -> Result<(), WasmError> {
        if ty.param_count != params.len() || ty.result_count != result_count {
            return Err(WasmError::Invalid(message));
        }
        for (index, expected) in params.iter().copied().enumerate() {
            if ty.params[index] != expected {
                return Err(WasmError::Invalid(message));
            }
        }
        Ok(())
    }

    fn pop_import_fd(&mut self) -> Result<u8, WasmError> {
        let fd = self.pop_core_i32()?;
        if fd > u8::MAX as u32 {
            return Err(WasmError::Invalid("fd does not fit u8"));
        }
        Ok(fd as u8)
    }

    fn begin_wasip1_import(
        &mut self,
        name: Wasip1ImportName,
        ty: FuncType,
    ) -> Result<PendingWasip1Call, WasmError> {
        match name {
            Wasip1ImportName::FdWrite => {
                Self::expect_import_signature(
                    ty,
                    &[
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                    ],
                    1,
                    "fd_write import signature mismatch",
                )?;
                let nwritten = self.pop_core_i32()?;
                let iovs_len = self.pop_core_i32()?;
                let iovs = self.pop_core_i32()?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::FdWrite(FdWriteCall {
                    fd,
                    iovs,
                    iovs_len,
                    nwritten,
                }))
            }
            Wasip1ImportName::FdRead => {
                Self::expect_import_signature(
                    ty,
                    &[
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                    ],
                    1,
                    "fd_read import signature mismatch",
                )?;
                let nread = self.pop_core_i32()?;
                let iovs_len = self.pop_core_i32()?;
                let iovs = self.pop_core_i32()?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::FdRead(FdReadCall {
                    fd,
                    iovs,
                    iovs_len,
                    nread,
                }))
            }
            Wasip1ImportName::FdFdstatGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "fd_fdstat_get import signature mismatch",
                )?;
                let out_ptr = self.pop_core_i32()?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::FdFdstatGet(FdRequestCall {
                    fd,
                    out_ptr,
                }))
            }
            Wasip1ImportName::FdClose => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32],
                    1,
                    "fd_close import signature mismatch",
                )?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::FdClose(FdRequestCall { fd, out_ptr: 0 }))
            }
            Wasip1ImportName::FdReaddir => {
                Self::expect_import_signature(
                    ty,
                    &[
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I64,
                        ValueKind::I32,
                    ],
                    1,
                    "fd_readdir import signature mismatch",
                )?;
                let bufused = self.pop_core_i32()?;
                let cookie = self.pop_core_i64()?;
                let buf_len = self.pop_core_i32()?;
                let buf = self.pop_core_i32()?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::FdReaddir(FdReaddirCall {
                    fd,
                    buf,
                    buf_len,
                    cookie,
                    bufused,
                }))
            }
            Wasip1ImportName::ClockResGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "clock_res_get import signature mismatch",
                )?;
                let resolution_ptr = self.pop_core_i32()?;
                let clock_id = self.pop_core_i32()?;
                Ok(PendingWasip1Call::ClockResGet(ClockResGetCall {
                    clock_id,
                    resolution_ptr,
                }))
            }
            Wasip1ImportName::ClockTimeGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I64, ValueKind::I32],
                    1,
                    "clock_time_get import signature mismatch",
                )?;
                let time_ptr = self.pop_core_i32()?;
                let precision = self.pop_core_i64()?;
                let clock_id = self.pop_core_i32()?;
                Ok(PendingWasip1Call::ClockTimeGet(ClockTimeGetCall {
                    clock_id,
                    precision,
                    time_ptr,
                }))
            }
            Wasip1ImportName::PollOneoff => {
                Self::expect_import_signature(
                    ty,
                    &[
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                    ],
                    1,
                    "poll_oneoff import signature mismatch",
                )?;
                let nevents = self.pop_core_i32()?;
                let nsubscriptions = self.pop_core_i32()?;
                let out_ptr = self.pop_core_i32()?;
                let in_ptr = self.pop_core_i32()?;
                Ok(PendingWasip1Call::PollOneoff(PollOneoffCall {
                    in_ptr,
                    out_ptr,
                    nsubscriptions,
                    nevents,
                }))
            }
            Wasip1ImportName::RandomGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "random_get import signature mismatch",
                )?;
                let buf_len = self.pop_core_i32()?;
                let buf = self.pop_core_i32()?;
                Ok(PendingWasip1Call::RandomGet(RandomGetCall { buf, buf_len }))
            }
            Wasip1ImportName::PathOpen => {
                Self::expect_import_signature(
                    ty,
                    &[
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I32,
                        ValueKind::I64,
                        ValueKind::I64,
                        ValueKind::I32,
                        ValueKind::I32,
                    ],
                    1,
                    "path_open import signature mismatch",
                )?;
                let opened_fd_ptr = self.pop_core_i32()?;
                self.pop_core_i32()?;
                self.pop_core_i64()?;
                let rights_base = self.pop_core_i64()?;
                self.pop_core_i32()?;
                let path_len = self.pop_core_i32()?;
                let path_ptr = self.pop_core_i32()?;
                self.pop_core_i32()?;
                let fd = self.pop_import_fd()?;
                Ok(PendingWasip1Call::PathOpen(PathOpenCall {
                    fd,
                    path_ptr,
                    path_len,
                    rights_base,
                    opened_fd_ptr,
                }))
            }
            Wasip1ImportName::ArgsSizesGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "args_sizes_get import signature mismatch",
                )?;
                let argv_buf_size_ptr = self.pop_core_i32()?;
                let argc_ptr = self.pop_core_i32()?;
                Ok(PendingWasip1Call::ArgsSizesGet(ArgsSizesGetCall {
                    argc_ptr,
                    argv_buf_size_ptr,
                }))
            }
            Wasip1ImportName::ArgsGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "args_get import signature mismatch",
                )?;
                let argv_buf = self.pop_core_i32()?;
                let argv = self.pop_core_i32()?;
                Ok(PendingWasip1Call::ArgsGet(ArgsGetCall { argv, argv_buf }))
            }
            Wasip1ImportName::EnvironSizesGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "environ_sizes_get import signature mismatch",
                )?;
                let environ_buf_size_ptr = self.pop_core_i32()?;
                let environ_count_ptr = self.pop_core_i32()?;
                Ok(PendingWasip1Call::EnvironSizesGet(EnvironSizesGetCall {
                    environ_count_ptr,
                    environ_buf_size_ptr,
                }))
            }
            Wasip1ImportName::EnvironGet => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32, ValueKind::I32],
                    1,
                    "environ_get import signature mismatch",
                )?;
                let environ_buf = self.pop_core_i32()?;
                let environ = self.pop_core_i32()?;
                Ok(PendingWasip1Call::EnvironGet(EnvironGetCall {
                    environ,
                    environ_buf,
                }))
            }
            Wasip1ImportName::ProcExit => {
                Self::expect_import_signature(
                    ty,
                    &[ValueKind::I32],
                    0,
                    "proc_exit import signature mismatch",
                )?;
                let code = self.pop_core_i32()?;
                Ok(PendingWasip1Call::ProcExit(code))
            }
            _ => Err(WasmError::Unsupported(disabled_wasip1_import_message(name))),
        }
    }

    fn call_core_import(&mut self, function_index: u32) -> Result<ExecutionEvent, WasmError> {
        let import = self
            .module
            .imports
            .get(function_index as usize)
            .copied()
            .flatten()
            .ok_or(WasmError::Invalid("missing core import"))?;
        let ty = self
            .module
            .core_func_type(self.module.import_type_indices[function_index as usize])?;
        let HostImport::Wasip1(name) = import.host;
        let call = self.begin_wasip1_import(name, ty)?;
        self.pending = Some(PendingExecution::Wasip1(call));
        Ok(ExecutionEvent::Wasip1Call)
    }

    fn current_frame(&self) -> Result<&Frame<'a>, WasmError> {
        if self.frame_len == 0 {
            return Err(WasmError::StackUnderflow);
        }
        self.frames
            .get(self.frame_len - 1)
            .ok_or(WasmError::StackUnderflow)
    }

    fn current_frame_mut(&mut self) -> Result<&mut Frame<'a>, WasmError> {
        if self.frame_len == 0 {
            return Err(WasmError::StackUnderflow);
        }
        self.frames
            .get_mut(self.frame_len - 1)
            .ok_or(WasmError::StackUnderflow)
    }

    fn current_instr(&mut self) -> Result<Instr, WasmError> {
        let opcode = self.current_read_u8()?;
        match opcode {
            OPCODE_NOP => Ok(Instr::Nop),
            OPCODE_BLOCK | OPCODE_LOOP | OPCODE_IF => {
                let block_type = self.current_read_u8()?;
                let (result_count, result_kind) = decode_core_block_type(block_type)?;
                let target = self.current_control_target(self.current_frame()?.pc)?;
                let mut control = ControlInstr::new(result_count, result_kind);
                if target.end_pos == CoreControlTarget::NONE {
                    return Err(WasmError::Invalid("core control target missing end"));
                }
                control.else_pos = target.else_pos;
                control.end_pos = target.end_pos;
                match opcode {
                    OPCODE_BLOCK => Ok(Instr::Block(control)),
                    OPCODE_LOOP => Ok(Instr::Loop(control)),
                    _ => Ok(Instr::If(control)),
                }
            }
            OPCODE_ELSE => Ok(Instr::Else),
            OPCODE_END => Ok(Instr::End),
            OPCODE_BR | OPCODE_BR_IF | OPCODE_CALL | OPCODE_LOCAL_GET | OPCODE_LOCAL_SET
            | OPCODE_LOCAL_TEE | OPCODE_GLOBAL_GET | OPCODE_GLOBAL_SET | OPCODE_REF_FUNC
            | OPCODE_TABLE_GET | OPCODE_TABLE_SET | OPCODE_MEMORY_SIZE | OPCODE_MEMORY_GROW => {
                Ok(Instr::U32(opcode, self.current_read_var_u32()?))
            }
            OPCODE_BR_TABLE => Ok(Instr::BrTable(self.current_br_table()?)),
            OPCODE_CALL_INDIRECT => {
                let type_index = self.current_read_var_u32()?;
                let table_index = self.current_read_var_u32()?;
                Ok(Instr::CallIndirect {
                    type_index,
                    table_index,
                })
            }
            OPCODE_I32_LOAD | OPCODE_I64_LOAD | OPCODE_F32_LOAD | OPCODE_F64_LOAD
            | OPCODE_I32_LOAD8_S | OPCODE_I32_LOAD8_U | OPCODE_I32_LOAD16_S
            | OPCODE_I32_LOAD16_U | OPCODE_I64_LOAD8_S | OPCODE_I64_LOAD8_U
            | OPCODE_I64_LOAD16_S | OPCODE_I64_LOAD16_U | OPCODE_I64_LOAD32_S
            | OPCODE_I64_LOAD32_U | OPCODE_I32_STORE | OPCODE_I64_STORE | OPCODE_F32_STORE
            | OPCODE_F64_STORE | OPCODE_I32_STORE8 | OPCODE_I32_STORE16 | OPCODE_I64_STORE8
            | OPCODE_I64_STORE16 | OPCODE_I64_STORE32 => {
                let align = self.current_read_var_u32()?;
                core::hint::black_box(align);
                Ok(Instr::Mem {
                    opcode,
                    offset: self.current_read_var_u32()?,
                })
            }
            OPCODE_I32_CONST => Ok(Instr::I32Const(self.current_read_var_i32()? as u32)),
            OPCODE_I64_CONST => Ok(Instr::I64Const(self.current_read_var_i64()? as u64)),
            OPCODE_F32_CONST => Ok(Instr::F32Const(self.current_read_fixed_u32()?)),
            OPCODE_F64_CONST => Ok(Instr::F64Const(self.current_read_fixed_u64()?)),
            OPCODE_REF_NULL => Ok(Instr::U32(opcode, self.current_read_u8()? as u32)),
            OPCODE_MISC => Ok(Instr::Misc(self.current_misc_instr()?)),
            _ => Ok(Instr::Simple(opcode)),
        }
    }

    fn current_control_target(&self, start_pos: usize) -> Result<CoreControlTarget, WasmError> {
        self.control_targets
            .get(..self.control_target_count)
            .ok_or(WasmError::Invalid(
                "core control target range out of bounds",
            ))?
            .iter()
            .copied()
            .find(|target| target.start() == start_pos)
            .ok_or(WasmError::Invalid("core control target missing"))
    }

    fn decode_current_frame_control_targets(&mut self) -> Result<(), WasmError> {
        let code: &'a [u8] = self.current_frame()?.code;
        self.control_target_count = decode_core_control_targets(code, &mut self.control_targets)?;
        Ok(())
    }

    fn current_read_u8(&mut self) -> Result<u8, WasmError> {
        let frame = self.current_frame_mut()?;
        let byte = *frame.code.get(frame.pc).ok_or(WasmError::Truncated)?;
        frame.pc += 1;
        Ok(byte)
    }

    fn current_read_var_u32(&mut self) -> Result<u32, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let value = reader.read_var_u32()?;
        frame.pc = reader.pos;
        Ok(value)
    }

    fn current_read_var_i32(&mut self) -> Result<i32, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let value = reader.read_var_i32()?;
        frame.pc = reader.pos;
        Ok(value)
    }

    fn current_read_var_i64(&mut self) -> Result<i64, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let value = reader.read_var_i64()?;
        frame.pc = reader.pos;
        Ok(value)
    }

    fn current_read_fixed_u32(&mut self) -> Result<u32, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let value = reader.read_fixed_u32()?;
        frame.pc = reader.pos;
        Ok(value)
    }

    fn current_read_fixed_u64(&mut self) -> Result<u64, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let value = reader.read_fixed_u64()?;
        frame.pc = reader.pos;
        Ok(value)
    }

    fn current_br_table(&mut self) -> Result<BrTableInstr, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let table = decode_core_br_table(&mut reader)?;
        frame.pc = reader.pos;
        Ok(table)
    }

    fn current_misc_instr(&mut self) -> Result<MiscInstr, WasmError> {
        let frame = self.current_frame_mut()?;
        let mut reader = Reader {
            bytes: frame.code,
            pos: frame.pc,
        };
        let instr = decode_core_misc_instr(&mut reader)?;
        frame.pc = reader.pos;
        Ok(instr)
    }

    fn push_core_value(&mut self, value: Value) -> Result<(), WasmError> {
        let slot = self
            .values
            .get_mut(self.value_len)
            .ok_or(WasmError::StackOverflow)?;
        *slot = value;
        self.value_len += 1;
        Ok(())
    }

    fn pop_core_value(&mut self) -> Result<Value, WasmError> {
        if self.value_len == 0 {
            return Err(WasmError::StackUnderflow);
        }
        self.value_len -= 1;
        Ok(self.values[self.value_len])
    }

    fn pop_core_i32(&mut self) -> Result<u32, WasmError> {
        self.pop_core_value()?.as_i32()
    }

    fn pop_core_i64(&mut self) -> Result<u64, WasmError> {
        self.pop_core_value()?.as_i64()
    }

    fn pop_core_f32_bits(&mut self) -> Result<u32, WasmError> {
        self.pop_core_value()?.as_f32_bits()
    }

    fn pop_core_f64_bits(&mut self) -> Result<u64, WasmError> {
        self.pop_core_value()?.as_f64_bits()
    }

    fn pop_core_f32(&mut self) -> Result<f32, WasmError> {
        self.pop_core_value()?.as_f32()
    }

    fn pop_core_f64(&mut self) -> Result<f64, WasmError> {
        self.pop_core_value()?.as_f64()
    }

    fn set_core_local(&mut self, local: usize, value: Value) -> Result<(), WasmError> {
        let frame = self.current_frame_mut()?;
        if local >= frame.local_count {
            return Err(WasmError::Invalid("core local.set inactive local"));
        }
        if value.kind() != frame.local_kinds[local] {
            return Err(WasmError::Invalid("core local type mismatch"));
        }
        frame.locals[local] = value;
        Ok(())
    }

    fn push_core_control(&mut self, control: ControlFrame) -> Result<(), WasmError> {
        let frame = self.current_frame_mut()?;
        let slot = frame
            .controls
            .get_mut(frame.control_len)
            .ok_or(WasmError::StackOverflow)?;
        *slot = control;
        frame.control_len += 1;
        Ok(())
    }

    fn pop_core_control(&mut self) -> Result<ControlFrame, WasmError> {
        let frame = self.current_frame_mut()?;
        if frame.control_len == 0 {
            return Err(WasmError::StackUnderflow);
        }
        frame.control_len -= 1;
        Ok(frame.controls[frame.control_len])
    }

    fn normalize_core_control_result(&mut self, control: ControlFrame) -> Result<(), WasmError> {
        if control.result_count == 0 {
            self.value_len = self.value_len.min(control.stack_height);
            return Ok(());
        }
        let result = self.pop_core_value()?;
        if result.kind() != control.result_kind {
            return Err(WasmError::Invalid("core block result type mismatch"));
        }
        self.value_len = control.stack_height;
        self.push_core_value(result)
    }

    fn core_branch(&mut self, depth: usize) -> Result<(), WasmError> {
        let frame = self.current_frame()?;
        let Some(target_index) = frame.control_len.checked_sub(depth.saturating_add(1)) else {
            return Err(WasmError::Invalid("core branch target out of range"));
        };
        let control = frame.controls[target_index];
        if control.result_count != 0 {
            let result = self.pop_core_value()?;
            if result.kind() != control.result_kind {
                return Err(WasmError::Invalid("core branch result type mismatch"));
            }
            self.value_len = control.stack_height;
            self.push_core_value(result)?;
        } else {
            self.value_len = self.value_len.min(control.stack_height);
        }
        let frame = self.current_frame_mut()?;
        match control.kind {
            ControlKind::Loop => {
                frame.control_len = target_index + 1;
                frame.pc = control.start_pos;
            }
            ControlKind::Block | ControlKind::If => {
                frame.control_len = target_index;
                frame.pc = control.end_pos.saturating_add(1);
            }
        }
        Ok(())
    }

    fn core_effective_addr(&mut self, offset: u32) -> Result<usize, WasmError> {
        let base = self.pop_core_i32()?;
        self.core_translate_addr(base.checked_add(offset).ok_or(WasmError::Truncated)?)
    }

    fn core_translate_addr(&self, addr: u32) -> Result<usize, WasmError> {
        let len = self.core_memory_len()?;
        let offset = addr as usize;
        if offset < len {
            Ok(offset)
        } else {
            Err(WasmError::Truncated)
        }
    }

    fn core_memory_len(&self) -> Result<usize, WasmError> {
        (self.memory_pages as usize)
            .checked_mul(CORE_WASM_PAGE_SIZE)
            .ok_or(WasmError::Truncated)
    }

    fn core_read_u8(&self, offset: usize) -> Result<u8, WasmError> {
        if offset >= self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        self.memory.get(offset).copied().ok_or(WasmError::Truncated)
    }

    fn core_read_u16(&self, offset: usize) -> Result<u16, WasmError> {
        let end = offset.checked_add(2).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self.memory.get(offset..end).ok_or(WasmError::Truncated)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn core_read_u32(&self, offset: usize) -> Result<u32, WasmError> {
        let end = offset.checked_add(4).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self.memory.get(offset..end).ok_or(WasmError::Truncated)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn core_read_u64(&self, offset: usize) -> Result<u64, WasmError> {
        let end = offset.checked_add(8).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self.memory.get(offset..end).ok_or(WasmError::Truncated)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn core_write_u8(&mut self, offset: usize, value: u8) -> Result<(), WasmError> {
        if offset >= self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let byte = self.memory.get_mut(offset).ok_or(WasmError::Truncated)?;
        *byte = value;
        Ok(())
    }

    fn core_write_u16(&mut self, offset: usize, value: u16) -> Result<(), WasmError> {
        let end = offset.checked_add(2).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self
            .memory
            .get_mut(offset..end)
            .ok_or(WasmError::Truncated)?;
        bytes.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn core_write_u32(&mut self, offset: usize, value: u32) -> Result<(), WasmError> {
        let end = offset.checked_add(4).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self
            .memory
            .get_mut(offset..end)
            .ok_or(WasmError::Truncated)?;
        bytes.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn core_write_u64(&mut self, offset: usize, value: u64) -> Result<(), WasmError> {
        let end = offset.checked_add(8).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self
            .memory
            .get_mut(offset..end)
            .ok_or(WasmError::Truncated)?;
        bytes.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn core_memory_copy(&mut self, dst: usize, src: usize, len: usize) -> Result<(), WasmError> {
        let src_end = src.checked_add(len).ok_or(WasmError::Truncated)?;
        let dst_end = dst.checked_add(len).ok_or(WasmError::Truncated)?;
        let memory_len = self.core_memory_len()?;
        if src_end > memory_len || dst_end > memory_len {
            return Err(WasmError::Truncated);
        }
        self.memory.copy_within(src..src_end, dst);
        Ok(())
    }

    fn core_memory_fill(&mut self, dst: usize, value: u8, len: usize) -> Result<(), WasmError> {
        let end = dst.checked_add(len).ok_or(WasmError::Truncated)?;
        if end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let bytes = self.memory.get_mut(dst..end).ok_or(WasmError::Truncated)?;
        bytes.fill(value);
        Ok(())
    }

    fn core_memory_init(
        &mut self,
        data_index: usize,
        dst: usize,
        src: usize,
        len: usize,
    ) -> Result<(), WasmError> {
        if data_index >= self.data_dropped.len() || self.data_dropped[data_index] {
            return Err(WasmError::Invalid("core memory.init data dropped"));
        }
        let segment = self
            .module
            .data_segments
            .get(data_index)
            .copied()
            .flatten()
            .ok_or(WasmError::Invalid("core memory.init data out of range"))?;
        let src_end = src.checked_add(len).ok_or(WasmError::Truncated)?;
        let dst_end = dst.checked_add(len).ok_or(WasmError::Truncated)?;
        if src_end > segment.bytes.len() || dst_end > self.core_memory_len()? {
            return Err(WasmError::Truncated);
        }
        let src_bytes = segment
            .bytes
            .get(src..src_end)
            .ok_or(WasmError::Truncated)?;
        let dst_bytes = self
            .memory
            .get_mut(dst..dst_end)
            .ok_or(WasmError::Truncated)?;
        dst_bytes.copy_from_slice(src_bytes);
        Ok(())
    }

    fn core_table_copy(&mut self, dst: usize, src: usize, len: usize) -> Result<(), WasmError> {
        let src_end = src.checked_add(len).ok_or(WasmError::Truncated)?;
        let dst_end = dst.checked_add(len).ok_or(WasmError::Truncated)?;
        if src_end > self.table_size || dst_end > self.table_size {
            return Err(WasmError::Invalid("core table.copy out of range"));
        }
        self.table_functions.copy_within(src..src_end, dst);
        Ok(())
    }

    fn core_table_init(
        &mut self,
        elem_index: usize,
        dst: usize,
        src: usize,
        len: usize,
    ) -> Result<(), WasmError> {
        if elem_index >= self.element_dropped.len() || self.element_dropped[elem_index] {
            return Err(WasmError::Invalid("core table.init element dropped"));
        }
        let segment = self
            .module
            .element_segments
            .get(elem_index)
            .copied()
            .flatten()
            .ok_or(WasmError::Invalid("core table.init element out of range"))?;
        let src_end = src.checked_add(len).ok_or(WasmError::Truncated)?;
        let dst_end = dst.checked_add(len).ok_or(WasmError::Truncated)?;
        if src_end > segment.function_count || dst_end > self.table_size {
            return Err(WasmError::Invalid("core table.init out of range"));
        }
        for (dst_slot, function_index) in self
            .table_functions
            .iter_mut()
            .skip(dst)
            .take(len)
            .zip(segment.functions.iter().skip(src).copied())
        {
            *dst_slot = function_index;
        }
        Ok(())
    }

    fn core_table_fill(&mut self, start: usize, value: u32, len: usize) -> Result<(), WasmError> {
        let end = start.checked_add(len).ok_or(WasmError::Truncated)?;
        if end > self.table_size {
            return Err(WasmError::Invalid("core table.fill out of range"));
        }
        for slot in self.table_functions.iter_mut().take(end).skip(start) {
            *slot = value;
        }
        Ok(())
    }

    fn core_binary_i32(&mut self, op: fn(u32, u32) -> u32) -> Result<(), WasmError> {
        let rhs = self.pop_core_i32()?;
        let lhs = self.pop_core_i32()?;
        self.push_core_value(Value::I32(op(lhs, rhs)))
    }

    fn core_binary_i64(&mut self, op: fn(u64, u64) -> u64) -> Result<(), WasmError> {
        let rhs = self.pop_core_i64()?;
        let lhs = self.pop_core_i64()?;
        self.push_core_value(Value::I64(op(lhs, rhs)))
    }

    fn core_binary_i64_cmp(&mut self, op: fn(u64, u64) -> bool) -> Result<(), WasmError> {
        let rhs = self.pop_core_i64()?;
        let lhs = self.pop_core_i64()?;
        self.push_core_value(Value::I32(op(lhs, rhs) as u32))
    }

    fn core_unary_f32(&mut self, op: fn(f32) -> f32) -> Result<(), WasmError> {
        let value = self.pop_core_f32()?;
        self.push_core_value(Value::F32(op(value).to_bits()))
    }

    fn core_binary_f32(&mut self, op: fn(f32, f32) -> f32) -> Result<(), WasmError> {
        let rhs = self.pop_core_f32()?;
        let lhs = self.pop_core_f32()?;
        self.push_core_value(Value::F32(op(lhs, rhs).to_bits()))
    }

    fn core_binary_f32_cmp(&mut self, op: fn(f32, f32) -> bool) -> Result<(), WasmError> {
        let rhs = self.pop_core_f32()?;
        let lhs = self.pop_core_f32()?;
        self.push_core_value(Value::I32(op(lhs, rhs) as u32))
    }

    fn core_unary_f64(&mut self, op: fn(f64) -> f64) -> Result<(), WasmError> {
        let value = self.pop_core_f64()?;
        self.push_core_value(Value::F64(op(value).to_bits()))
    }

    fn core_binary_f64(&mut self, op: fn(f64, f64) -> f64) -> Result<(), WasmError> {
        let rhs = self.pop_core_f64()?;
        let lhs = self.pop_core_f64()?;
        self.push_core_value(Value::F64(op(lhs, rhs).to_bits()))
    }

    fn core_binary_f64_cmp(&mut self, op: fn(f64, f64) -> bool) -> Result<(), WasmError> {
        let rhs = self.pop_core_f64()?;
        let lhs = self.pop_core_f64()?;
        self.push_core_value(Value::I32(op(lhs, rhs) as u32))
    }
}

fn wasm_f32_min(lhs: f32, rhs: f32) -> f32 {
    if lhs.is_nan() || rhs.is_nan() {
        f32::NAN
    } else {
        lhs.min(rhs)
    }
}

fn wasm_f32_abs(value: f32) -> f32 {
    f32::from_bits(value.to_bits() & 0x7fff_ffff)
}

fn wasm_f32_neg(value: f32) -> f32 {
    f32::from_bits(value.to_bits() ^ 0x8000_0000)
}

fn wasm_f32_trunc(value: f32) -> f32 {
    if !value.is_finite() || wasm_f32_abs(value) >= 9_223_372_036_854_775_808.0 {
        return value;
    }
    (value as i64) as f32
}

fn wasm_f32_floor(value: f32) -> f32 {
    let truncated = wasm_f32_trunc(value);
    if truncated > value {
        truncated - 1.0
    } else {
        truncated
    }
}

fn wasm_f32_ceil(value: f32) -> f32 {
    let truncated = wasm_f32_trunc(value);
    if truncated < value {
        truncated + 1.0
    } else {
        truncated
    }
}

fn wasm_f32_nearest(value: f32) -> f32 {
    if !value.is_finite() {
        return value;
    }
    let floor = wasm_f32_floor(value);
    let ceil = wasm_f32_ceil(value);
    let floor_delta = value - floor;
    let ceil_delta = ceil - value;
    if floor_delta < ceil_delta {
        floor
    } else if ceil_delta < floor_delta {
        ceil
    } else if ((floor as i64) & 1) == 0 {
        floor
    } else {
        ceil
    }
}

fn wasm_f32_sqrt(value: f32) -> f32 {
    if value.is_nan() || value < 0.0 {
        return f32::NAN;
    }
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let mut x = if value >= 1.0 { value } else { 1.0 };
    for sqrt_iteration in 0..8 {
        core::hint::black_box(sqrt_iteration);
        x = 0.5 * (x + value / x);
    }
    x
}

fn wasm_f32_max(lhs: f32, rhs: f32) -> f32 {
    if lhs.is_nan() || rhs.is_nan() {
        f32::NAN
    } else {
        lhs.max(rhs)
    }
}

fn wasm_f32_copysign(lhs: f32, rhs: f32) -> f32 {
    f32::from_bits((lhs.to_bits() & 0x7fff_ffff) | (rhs.to_bits() & 0x8000_0000))
}

fn wasm_f64_min(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.min(rhs)
    }
}

fn wasm_f64_abs(value: f64) -> f64 {
    f64::from_bits(value.to_bits() & 0x7fff_ffff_ffff_ffff)
}

fn wasm_f64_neg(value: f64) -> f64 {
    f64::from_bits(value.to_bits() ^ 0x8000_0000_0000_0000)
}

fn wasm_f64_trunc(value: f64) -> f64 {
    if !value.is_finite() || wasm_f64_abs(value) >= 9_223_372_036_854_775_808.0 {
        return value;
    }
    (value as i64) as f64
}

fn wasm_f64_floor(value: f64) -> f64 {
    let truncated = wasm_f64_trunc(value);
    if truncated > value {
        truncated - 1.0
    } else {
        truncated
    }
}

fn wasm_f64_ceil(value: f64) -> f64 {
    let truncated = wasm_f64_trunc(value);
    if truncated < value {
        truncated + 1.0
    } else {
        truncated
    }
}

fn wasm_f64_nearest(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let floor = wasm_f64_floor(value);
    let ceil = wasm_f64_ceil(value);
    let floor_delta = value - floor;
    let ceil_delta = ceil - value;
    if floor_delta < ceil_delta {
        floor
    } else if ceil_delta < floor_delta {
        ceil
    } else if ((floor as i64) & 1) == 0 {
        floor
    } else {
        ceil
    }
}

fn wasm_f64_sqrt(value: f64) -> f64 {
    if value.is_nan() || value < 0.0 {
        return f64::NAN;
    }
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let mut x = if value >= 1.0 { value } else { 1.0 };
    for sqrt_iteration in 0..12 {
        core::hint::black_box(sqrt_iteration);
        x = 0.5 * (x + value / x);
    }
    x
}

fn wasm_f64_max(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.max(rhs)
    }
}

fn wasm_f64_copysign(lhs: f64, rhs: f64) -> f64 {
    f64::from_bits(
        (lhs.to_bits() & 0x7fff_ffff_ffff_ffff) | (rhs.to_bits() & 0x8000_0000_0000_0000),
    )
}

fn trunc_f32_to_i32_s(value: f32) -> Result<u32, WasmError> {
    if !value.is_finite() || value <= i32::MIN as f32 - 1.0 || value >= i32::MAX as f32 + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f32_trunc(value) as i32 as u32)
}

fn trunc_f32_to_i32_u(value: f32) -> Result<u32, WasmError> {
    if !value.is_finite() || value <= -1.0 || value >= (u32::MAX as f32) + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f32_trunc(value) as u32)
}

fn trunc_f64_to_i32_s(value: f64) -> Result<u32, WasmError> {
    if !value.is_finite() || value <= i32::MIN as f64 - 1.0 || value >= i32::MAX as f64 + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f64_trunc(value) as i32 as u32)
}

fn trunc_f64_to_i32_u(value: f64) -> Result<u32, WasmError> {
    if !value.is_finite() || value <= -1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f64_trunc(value) as u32)
}

fn trunc_f32_to_i64_s(value: f32) -> Result<u64, WasmError> {
    if !value.is_finite() || value <= i64::MIN as f32 - 1.0 || value >= i64::MAX as f32 + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f32_trunc(value) as i64 as u64)
}

fn trunc_f32_to_i64_u(value: f32) -> Result<u64, WasmError> {
    if !value.is_finite() || value <= -1.0 || value >= (u64::MAX as f32) + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f32_trunc(value) as u64)
}

fn trunc_f64_to_i64_s(value: f64) -> Result<u64, WasmError> {
    if !value.is_finite() || value <= i64::MIN as f64 - 1.0 || value >= i64::MAX as f64 + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f64_trunc(value) as i64 as u64)
}

fn trunc_f64_to_i64_u(value: f64) -> Result<u64, WasmError> {
    if !value.is_finite() || value <= -1.0 || value >= (u64::MAX as f64) + 1.0 {
        return Err(WasmError::Trap);
    }
    Ok(wasm_f64_trunc(value) as u64)
}

impl<'a> Vm<'a> {
    pub(super) unsafe fn init_in_place(
        dst: *mut Self,
        module: &'a [u8],
        handlers: Wasip1HandlerSet,
    ) -> Result<(), WasmError> {
        unsafe {
            let core = core::ptr::addr_of_mut!((*dst).core);
            let core_module = core::ptr::addr_of_mut!((*core).module);
            Module::parse_in_place(core_module, module)?;
            Interpreter::init_from_parsed_module_in_place(core)?;
            core::ptr::addr_of_mut!((*dst).handlers).write(handlers);
            core::ptr::addr_of_mut!((*dst).done).write(false);
        }
        Ok(())
    }

    pub(super) fn resume(&mut self, budget: BudgetRun) -> Result<VmEvent, WasmError> {
        if self.done {
            return Ok(VmEvent::Done);
        }
        match self.core.run(budget.fuel()) {
            Ok(ExecutionEvent::Done) => {
                self.done = true;
                Ok(VmEvent::Done)
            }
            Ok(ExecutionEvent::MemoryGrow(event)) => Ok(VmEvent::MemoryGrow(event)),
            Ok(ExecutionEvent::Wasip1Call) => self.translate_wasip1_import(),
            Err(WasmError::FuelExhausted) => Ok(VmEvent::BudgetExpired(BudgetExpired::new(
                budget.run_id(),
                budget.generation(),
            ))),
            Err(error) => Err(error),
        }
    }

    pub(super) fn finish_host_call(&mut self, errno: u32) -> Result<(), WasmError> {
        self.core.finish_host_import(&[Value::I32(errno)])
    }

    pub(super) fn finish_memory_grow_event(&mut self) -> Result<MemoryGrowEvent, WasmError> {
        self.core.finish_memory_grow_event()
    }

    pub(super) fn finish_fd_close(
        &mut self,
        call: FdRequestCall,
        errno: u32,
    ) -> Result<(), WasmError> {
        self.core
            .finish_matching_host_import(PendingWasip1Call::FdClose(call), &[Value::I32(errno)])
    }

    pub(super) fn finish_fd_write(
        &mut self,
        call: FdWriteCall,
        errno: u32,
    ) -> Result<(), WasmError> {
        let written = if errno == 0 {
            self.fd_write_total_len(call)?
        } else {
            0
        };
        self.core.write_memory_u32(call.nwritten, written)?;
        self.finish_host_call(errno)
    }

    pub(super) fn finish_fd_read(
        &mut self,
        call: FdReadCall,
        bytes: &[u8],
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            let (dst, max_len) = self.fd_read_iovec(call)?;
            if bytes.len() > max_len as usize {
                return Err(WasmError::Unsupported("fd_read reply exceeds iovec"));
            }
            self.core.write_memory(dst, bytes)?;
            self.core.write_memory_u32(call.nread, bytes.len() as u32)?;
        } else {
            self.core.write_memory_u32(call.nread, 0)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_fd_fdstat_get(
        &mut self,
        call: FdRequestCall,
        stat: FdStat,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            let mut bytes = [0u8; WASIP1_FDSTAT_SIZE];
            bytes[WASIP1_FDSTAT_FILETYPE_OFFSET as usize] = stat.filetype();
            bytes[WASIP1_FDSTAT_FLAGS_OFFSET as usize..WASIP1_FDSTAT_FLAGS_OFFSET as usize + 2]
                .copy_from_slice(&stat.flags().to_le_bytes());
            bytes[WASIP1_FDSTAT_RIGHTS_BASE_OFFSET as usize
                ..WASIP1_FDSTAT_RIGHTS_BASE_OFFSET as usize + 8]
                .copy_from_slice(&stat.rights_base().to_le_bytes());
            bytes[WASIP1_FDSTAT_RIGHTS_INHERITING_OFFSET as usize
                ..WASIP1_FDSTAT_RIGHTS_INHERITING_OFFSET as usize + 8]
                .copy_from_slice(&stat.rights_inheriting().to_le_bytes());
            self.core.write_memory(call.out_ptr, &bytes)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_clock_time_get(
        &mut self,
        call: ClockTimeGetCall,
        nanos: u64,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core
                .write_memory(call.time_ptr, &nanos.to_le_bytes())?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_clock_res_get(
        &mut self,
        call: ClockResGetCall,
        resolution_nanos: u64,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core
                .write_memory(call.resolution_ptr, &resolution_nanos.to_le_bytes())?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_poll_oneoff(
        &mut self,
        call: PollOneoffCall,
        ready: u32,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core.write_memory_u32(call.nevents, ready)?;
            if ready > 0 && call.out_ptr != 0 {
                let mut event = [0u8; WASIP1_EVENT_SIZE];
                self.core.read_memory(
                    call.in_ptr
                        .saturating_add(WASIP1_SUBSCRIPTION_USERDATA_OFFSET),
                    &mut event[..8],
                )?;
                let event_type = self.read_memory_u8(
                    call.in_ptr
                        .saturating_add(WASIP1_SUBSCRIPTION_EVENTTYPE_OFFSET),
                )?;
                event[WASIP1_EVENT_ERROR_OFFSET as usize..WASIP1_EVENT_ERROR_OFFSET as usize + 2]
                    .copy_from_slice(&(0u16).to_le_bytes());
                event[WASIP1_EVENT_TYPE_OFFSET as usize] = event_type;
                self.core.write_memory(call.out_ptr, &event)?;
            }
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_random_get(
        &mut self,
        call: RandomGetCall,
        bytes: &[u8],
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            if bytes.len() > call.buf_len as usize {
                return Err(WasmError::Unsupported("random_get reply too large"));
            }
            self.core.write_memory(call.buf, bytes)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn path_bytes(&self, call: PathOpenCall) -> Result<PathBytes, WasmError> {
        let ptr = call.path_ptr;
        let len = call.path_len;
        if len as usize > CORE_WASIP1_PATH_CAPACITY {
            return Err(WasmError::Unsupported("path import path too long"));
        }
        let mut bytes = [0u8; CORE_WASIP1_PATH_CAPACITY];
        self.core.read_memory(ptr, &mut bytes[..len as usize])?;
        Ok(PathBytes {
            bytes,
            len: len as usize,
        })
    }

    pub(super) fn finish_path_open(
        &mut self,
        call: PathOpenCall,
        opened_fd: u32,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core.write_memory_u32(call.opened_fd_ptr, opened_fd)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_fd_readdir(
        &mut self,
        call: FdReaddirCall,
        bytes: &[u8],
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            if bytes.len() > call.buf_len as usize {
                return Err(WasmError::Unsupported("fd_readdir reply exceeds buffer"));
            }
            self.core.write_memory(call.buf, bytes)?;
            self.core
                .write_memory_u32(call.bufused, bytes.len() as u32)?;
        } else {
            self.core.write_memory_u32(call.bufused, 0)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_args_sizes_get(
        &mut self,
        call: ArgsSizesGetCall,
        argc: u32,
        argv_buf_size: u32,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core.write_memory_u32(call.argc_ptr, argc)?;
            self.core
                .write_memory_u32(call.argv_buf_size_ptr, argv_buf_size)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_environ_sizes_get(
        &mut self,
        call: EnvironSizesGetCall,
        environ_count: u32,
        environ_buf_size: u32,
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.core
                .write_memory_u32(call.environ_count_ptr, environ_count)?;
            self.core
                .write_memory_u32(call.environ_buf_size_ptr, environ_buf_size)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_args_get(
        &mut self,
        call: ArgsGetCall,
        args: &[&[u8]],
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.write_cstr_vector(call.argv, call.argv_buf, args)?;
        }
        self.finish_host_call(errno)
    }

    pub(super) fn finish_environ_get(
        &mut self,
        call: EnvironGetCall,
        environ: &[(&[u8], &[u8])],
        errno: u32,
    ) -> Result<(), WasmError> {
        if errno == 0 {
            self.write_env_vector(call.environ, call.environ_buf, environ)?;
        }
        self.finish_host_call(errno)
    }

    #[cfg(test)]
    fn read_memory(&self, addr: u32, out: &mut [u8]) -> Result<(), WasmError> {
        self.core.read_memory(addr, out)
    }

    #[cfg(test)]
    fn write_memory(&mut self, addr: u32, bytes: &[u8]) -> Result<(), WasmError> {
        self.core.write_memory(addr, bytes)
    }

    #[cfg(test)]
    fn read_memory_u32(&self, addr: u32) -> Result<u32, WasmError> {
        self.core.read_memory_u32(addr)
    }

    pub(super) fn fd_write_payload(&self, call: FdWriteCall) -> Result<InlinePayload, WasmError> {
        let mut bytes = [0u8; WASIP1_STREAM_CHUNK_CAPACITY];
        let payload_len = self.copy_fd_write_payload(call, &mut bytes)?;
        Ok(InlinePayload {
            bytes,
            len: payload_len as u8,
        })
    }

    pub(super) fn copy_fd_write_payload(
        &self,
        call: FdWriteCall,
        out: &mut [u8],
    ) -> Result<usize, WasmError> {
        let payload_len = self.fd_write_total_len(call)? as usize;
        if payload_len > out.len() {
            return Err(WasmError::Unsupported("fd_write payload buffer too small"));
        }
        if call.iovs_len == 0 {
            self.core.read_memory(call.iovs, &mut out[..payload_len])?;
        } else {
            let mut copied = 0usize;
            for index in 0..call.iovs_len {
                let iov = call
                    .iovs
                    .checked_add(index.saturating_mul(8))
                    .ok_or(WasmError::Truncated)?;
                let ptr = self.core.read_memory_u32(iov)?;
                let len = self
                    .core
                    .read_memory_u32(iov.checked_add(4).ok_or(WasmError::Truncated)?)?
                    as usize;
                let end = copied.checked_add(len).ok_or(WasmError::Truncated)?;
                self.core.read_memory(ptr, &mut out[copied..end])?;
                copied = end;
            }
        }
        Ok(payload_len)
    }

    pub(super) fn fd_write_total_len(&self, call: FdWriteCall) -> Result<u32, WasmError> {
        if call.iovs_len == 0 {
            return Ok(call.nwritten);
        }
        let mut total = 0u32;
        for index in 0..call.iovs_len {
            let iov = call
                .iovs
                .checked_add(index.saturating_mul(8))
                .ok_or(WasmError::Truncated)?;
            let len = self.core.read_memory_u32(iov.saturating_add(4))?;
            total = total.checked_add(len).ok_or(WasmError::Truncated)?;
        }
        Ok(total)
    }

    pub(super) fn poll_oneoff_delay_ticks(&self, call: PollOneoffCall) -> Result<u64, WasmError> {
        if call.nsubscriptions != 1 {
            return Err(WasmError::Unsupported(
                "only one poll_oneoff subscription is supported",
            ));
        }
        let event_type = self.read_memory_u8(
            call.in_ptr
                .saturating_add(WASIP1_SUBSCRIPTION_EVENTTYPE_OFFSET),
        )?;
        if event_type != WASIP1_EVENTTYPE_CLOCK {
            return Err(WasmError::Unsupported(
                "poll_oneoff only supports clock subscriptions",
            ));
        }
        let timeout_nanos = self.read_core_u64(
            call.in_ptr
                .saturating_add(WASIP1_SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET),
        )?;
        if timeout_nanos == 0 {
            return Err(WasmError::Truncated);
        }
        Ok(timeout_nanos / 1_000_000)
    }

    fn read_memory_u8(&self, addr: u32) -> Result<u8, WasmError> {
        let mut byte = [0u8; 1];
        self.core.read_memory(addr, &mut byte)?;
        Ok(byte[0])
    }

    fn translate_wasip1_import(&mut self) -> Result<VmEvent, WasmError> {
        let call = self.core.pending_wasip1_call()?;
        match call {
            PendingWasip1Call::FdWrite(_) if !self.handlers.supports(Wasip1Syscall::FdWrite) => {
                Err(WasmError::Unsupported(
                    "wasip1 fd_write disabled by feature profile",
                ))
            }
            PendingWasip1Call::FdRead(_) if !self.handlers.supports(Wasip1Syscall::FdRead) => Err(
                WasmError::Unsupported("wasip1 fd_read disabled by feature profile"),
            ),
            PendingWasip1Call::FdFdstatGet(_)
                if !self.handlers.supports(Wasip1Syscall::FdFdstatGet) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 fd_fdstat_get disabled by feature profile",
                ))
            }
            PendingWasip1Call::FdClose(_) if !self.handlers.supports(Wasip1Syscall::FdClose) => {
                Err(WasmError::Unsupported(
                    "wasip1 fd_close disabled by feature profile",
                ))
            }
            PendingWasip1Call::FdReaddir(_)
                if !self.handlers.supports(Wasip1Syscall::FdReaddir) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 fd_readdir disabled by feature profile",
                ))
            }
            PendingWasip1Call::ClockResGet(_)
                if !self.handlers.supports(Wasip1Syscall::ClockResGet) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 clock_res_get disabled by feature profile",
                ))
            }
            PendingWasip1Call::ClockTimeGet(_)
                if !self.handlers.supports(Wasip1Syscall::ClockTimeGet) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 clock_time_get disabled by feature profile",
                ))
            }
            PendingWasip1Call::PollOneoff(_)
                if !self.handlers.supports(Wasip1Syscall::PollOneoff) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 poll_oneoff disabled by feature profile",
                ))
            }
            PendingWasip1Call::RandomGet(_)
                if !self.handlers.supports(Wasip1Syscall::RandomGet) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 random_get disabled by feature profile",
                ))
            }
            PendingWasip1Call::PathOpen(_) if !self.handlers.supports(Wasip1Syscall::PathOpen) => {
                Err(WasmError::Unsupported(
                    "wasip1 path_open disabled by feature profile",
                ))
            }
            PendingWasip1Call::ArgsSizesGet(_)
            | PendingWasip1Call::ArgsGet(_)
            | PendingWasip1Call::EnvironSizesGet(_)
            | PendingWasip1Call::EnvironGet(_)
                if !self.handlers.supports(Wasip1Syscall::ArgsEnv) =>
            {
                Err(WasmError::Unsupported(
                    "wasip1 args/env disabled by feature profile",
                ))
            }
            PendingWasip1Call::ProcExit(_) if !self.handlers.supports(Wasip1Syscall::ProcExit) => {
                Err(WasmError::Unsupported(
                    "wasip1 proc_exit disabled by feature profile",
                ))
            }
            PendingWasip1Call::ProcExit(code) => {
                self.core.finish_host_import(&[])?;
                self.done = true;
                Ok(VmEvent::ProcExit(code))
            }
            _ => Ok(call.into_event()),
        }
    }

    fn read_core_u64(&self, addr: u32) -> Result<u64, WasmError> {
        let mut bytes = [0u8; 8];
        self.core.read_memory(addr, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(super) fn fd_read_iovec(&self, call: FdReadCall) -> Result<(u32, u32), WasmError> {
        if call.iovs_len != 1 {
            return Err(WasmError::Unsupported(
                "only one fd_read iovec is supported",
            ));
        }
        Ok((
            self.core.read_memory_u32(call.iovs)?,
            self.core.read_memory_u32(call.iovs.saturating_add(4))?,
        ))
    }

    fn write_cstr_vector(
        &mut self,
        ptrs: u32,
        mut buf: u32,
        items: &[&[u8]],
    ) -> Result<(), WasmError> {
        for (index, item) in items.iter().enumerate() {
            self.core
                .write_memory_u32(ptrs.saturating_add((index as u32).saturating_mul(4)), buf)?;
            self.core.write_memory(buf, item)?;
            buf = buf
                .checked_add(item.len() as u32)
                .ok_or(WasmError::Truncated)?;
            self.core.write_memory(buf, &[0])?;
            buf = buf.checked_add(1).ok_or(WasmError::Truncated)?;
        }
        Ok(())
    }

    fn write_env_vector(
        &mut self,
        ptrs: u32,
        mut buf: u32,
        items: &[(&[u8], &[u8])],
    ) -> Result<(), WasmError> {
        for (index, (key, value)) in items.iter().enumerate() {
            self.core
                .write_memory_u32(ptrs.saturating_add((index as u32).saturating_mul(4)), buf)?;
            self.core.write_memory(buf, key)?;
            buf = buf
                .checked_add(key.len() as u32)
                .ok_or(WasmError::Truncated)?;
            self.core.write_memory(buf, b"=")?;
            buf = buf.checked_add(1).ok_or(WasmError::Truncated)?;
            self.core.write_memory(buf, value)?;
            buf = buf
                .checked_add(value.len() as u32)
                .ok_or(WasmError::Truncated)?;
            self.core.write_memory(buf, &[0])?;
            buf = buf.checked_add(1).ok_or(WasmError::Truncated)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EXTERNAL_KIND_FUNC, ExecutionEvent, Interpreter, OPCODE_CALL, OPCODE_DROP, OPCODE_END,
        OPCODE_I32_CONST, OPCODE_I64_CONST, SECTION_CODE, SECTION_EXPORT, SECTION_FUNCTION,
        SECTION_IMPORT, SECTION_MEMORY, SECTION_TYPE, TEST_RESUME_FUEL, VALTYPE_I32, VALTYPE_I64,
        Vm, VmEvent, WASIP1_FILETYPE_REGULAR_FILE, WasmError,
    };
    #[cfg(feature = "wasm-engine-core")]
    use super::{FdStat, WASIP1_FDSTAT_RIGHTS_BASE_OFFSET};
    use crate::{
        choreography::protocol::BudgetRun,
        kernel::features::{WASIP1_PREVIEW1_MODULE, Wasip1HandlerSet, Wasip1ImportName},
    };
    use core::mem::MaybeUninit;
    use std::boxed::Box;
    use std::ops::{Deref, DerefMut};
    use std::vec::Vec;

    struct TestInterpreter<'a> {
        storage: Box<MaybeUninit<Interpreter<'a>>>,
    }

    impl<'a> TestInterpreter<'a> {
        fn new(module: &'a [u8]) -> Result<Self, WasmError> {
            let mut storage = Box::new(MaybeUninit::<Interpreter<'a>>::uninit());
            unsafe {
                Interpreter::init_in_place(storage.as_mut_ptr(), module)?;
            }
            Ok(Self { storage })
        }
    }

    impl<'a> Deref for TestInterpreter<'a> {
        type Target = Interpreter<'a>;

        fn deref(&self) -> &Self::Target {
            unsafe { self.storage.assume_init_ref() }
        }
    }

    impl<'a> DerefMut for TestInterpreter<'a> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.storage.assume_init_mut() }
        }
    }

    struct TestVm<'a> {
        storage: Box<MaybeUninit<Vm<'a>>>,
    }

    impl<'a> TestVm<'a> {
        fn new(module: &'a [u8], handlers: Wasip1HandlerSet) -> Result<Self, WasmError> {
            let mut storage = Box::new(MaybeUninit::<Vm<'a>>::uninit());
            unsafe {
                Vm::init_in_place(storage.as_mut_ptr(), module, handlers)?;
            }
            Ok(Self { storage })
        }
    }

    impl<'a> Deref for TestVm<'a> {
        type Target = Vm<'a>;

        fn deref(&self) -> &Self::Target {
            unsafe { self.storage.assume_init_ref() }
        }
    }

    impl<'a> DerefMut for TestVm<'a> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.storage.assume_init_mut() }
        }
    }

    #[derive(Clone, Copy)]
    enum TestWasmArg {
        I32(u32),
        I64(u64),
    }

    fn test_budget() -> BudgetRun {
        BudgetRun::new(1, 1, TEST_RESUME_FUEL, 0)
    }

    fn push_test_u32(out: &mut Vec<u8>, mut value: u32) {
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

    fn push_test_i32(out: &mut Vec<u8>, value: u32) {
        let mut value = value as i32;
        loop {
            let byte = (value as u8) & 0x7f;
            value >>= 7;
            let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
            out.push(if done { byte } else { byte | 0x80 });
            if done {
                break;
            }
        }
    }

    fn push_test_i64(out: &mut Vec<u8>, value: u64) {
        let mut value = value as i64;
        loop {
            let byte = (value as u8) & 0x7f;
            value >>= 7;
            let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
            out.push(if done { byte } else { byte | 0x80 });
            if done {
                break;
            }
        }
    }

    fn push_test_name(out: &mut Vec<u8>, name: &[u8]) {
        push_test_u32(out, name.len() as u32);
        out.extend_from_slice(name);
    }

    fn push_test_section(out: &mut Vec<u8>, id: u8, section: &[u8]) {
        out.push(id);
        push_test_u32(out, section.len() as u32);
        out.extend_from_slice(section);
    }

    fn core_wasip1_single_import_module(
        import_name: Wasip1ImportName,
        import_params: &[u8],
        import_results: &[u8],
        args: &[TestWasmArg],
        needs_memory: bool,
    ) -> Vec<u8> {
        let mut module = Vec::new();
        module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

        let mut types = Vec::new();
        push_test_u32(&mut types, 2);
        types.push(0x60);
        push_test_u32(&mut types, import_params.len() as u32);
        types.extend_from_slice(import_params);
        push_test_u32(&mut types, import_results.len() as u32);
        types.extend_from_slice(import_results);
        types.push(0x60);
        push_test_u32(&mut types, 0);
        push_test_u32(&mut types, 0);
        push_test_section(&mut module, SECTION_TYPE, &types);

        let mut imports = Vec::new();
        push_test_u32(&mut imports, 1);
        push_test_name(&mut imports, WASIP1_PREVIEW1_MODULE.as_bytes());
        push_test_name(&mut imports, import_name.name().as_bytes());
        imports.push(EXTERNAL_KIND_FUNC);
        push_test_u32(&mut imports, 0);
        push_test_section(&mut module, SECTION_IMPORT, &imports);

        let mut functions = Vec::new();
        push_test_u32(&mut functions, 1);
        push_test_u32(&mut functions, 1);
        push_test_section(&mut module, SECTION_FUNCTION, &functions);

        if needs_memory {
            push_test_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);
        }

        let mut exports = Vec::new();
        push_test_u32(&mut exports, 1);
        push_test_name(&mut exports, b"_start");
        exports.push(EXTERNAL_KIND_FUNC);
        push_test_u32(&mut exports, 1);
        push_test_section(&mut module, SECTION_EXPORT, &exports);

        let mut body = Vec::new();
        push_test_u32(&mut body, 0);
        for arg in args {
            match *arg {
                TestWasmArg::I32(value) => {
                    body.push(OPCODE_I32_CONST);
                    push_test_i32(&mut body, value);
                }
                TestWasmArg::I64(value) => {
                    body.push(OPCODE_I64_CONST);
                    push_test_i64(&mut body, value);
                }
            }
        }
        body.push(OPCODE_CALL);
        push_test_u32(&mut body, 0);
        if !import_results.is_empty() {
            body.push(OPCODE_DROP);
        }
        body.push(OPCODE_END);

        let mut code = Vec::new();
        push_test_u32(&mut code, 1);
        push_test_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
        push_test_section(&mut module, SECTION_CODE, &code);
        module
    }

    fn core_test_module(
        body_instrs: &[u8],
        memory: bool,
        table_min: Option<u32>,
        data_section: Option<&[u8]>,
        element_section: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut module = Vec::new();
        module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

        let mut types = Vec::new();
        push_test_u32(&mut types, 1);
        types.push(0x60);
        push_test_u32(&mut types, 0);
        push_test_u32(&mut types, 0);
        push_test_section(&mut module, SECTION_TYPE, &types);

        let mut functions = Vec::new();
        push_test_u32(&mut functions, 1);
        push_test_u32(&mut functions, 0);
        push_test_section(&mut module, SECTION_FUNCTION, &functions);

        if let Some(min) = table_min {
            let mut table = Vec::new();
            push_test_u32(&mut table, 1);
            table.push(super::VALTYPE_FUNCREF);
            table.push(0x00);
            push_test_u32(&mut table, min);
            push_test_section(&mut module, super::SECTION_TABLE, &table);
        }

        if memory {
            push_test_section(&mut module, SECTION_MEMORY, &[0x01, 0x00, 0x01]);
        }

        let mut exports = Vec::new();
        push_test_u32(&mut exports, 1);
        push_test_name(&mut exports, b"_start");
        exports.push(EXTERNAL_KIND_FUNC);
        push_test_u32(&mut exports, 0);
        push_test_section(&mut module, SECTION_EXPORT, &exports);

        if let Some(elements) = element_section {
            push_test_section(&mut module, super::SECTION_ELEMENT, elements);
        }

        let mut code = Vec::new();
        let mut body = Vec::new();
        push_test_u32(&mut body, 0);
        body.extend_from_slice(body_instrs);
        body.push(OPCODE_END);
        push_test_u32(&mut code, 1);
        push_test_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
        push_test_section(&mut module, SECTION_CODE, &code);

        if let Some(data) = data_section {
            push_test_section(&mut module, super::SECTION_DATA, data);
        }

        module
    }

    #[test]
    fn wasm_vm_rejects_non_wasip1_import_module() {
        static HIBANA_IMPORT_GUEST: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x60, 0x01, 0x7f,
            0x00, 0x60, 0x00, 0x00, 0x02, 0x25, 0x02, 0x06, b'h', b'i', b'b', b'a', b'n', b'a',
            0x07, b'l', b'o', b'g', b'_', b'u', b'3', b'2', 0x00, 0x00, 0x06, b'h', b'i', b'b',
            b'a', b'n', b'a', 0x09, b'y', b'i', b'e', b'l', b'd', b'_', b'n', b'o', b'w', 0x00,
            0x01, 0x03, 0x02, 0x01, 0x01, 0x07, 0x0a, 0x01, 0x06, b'_', b's', b't', b'a', b'r',
            b't', 0x00, 0x02, 0x0a, 0x0e, 0x01, 0x0c, 0x00, 0x41, 0xc1, 0x84, 0xa5, 0xc2, 0x04,
            0x10, 0x00, 0x10, 0x01, 0x0b,
        ];
        match TestInterpreter::new(HIBANA_IMPORT_GUEST) {
            Err(error) => {
                assert_eq!(
                    error,
                    WasmError::Unsupported("unsupported host import module")
                );
            }
            Ok(_) => panic!("non-WASI import module must be rejected"),
        }
    }

    #[test]
    fn core_wasm_memory_grow_is_generic_engine_event_not_lease_policy() {
        static CORE_MEMORY_GROW_GUEST: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00, 0x05, 0x04, 0x01, 0x01, 0x00, 0x01, 0x07, 0x0a, 0x01, 0x06,
            b'_', b's', b't', b'a', b'r', b't', 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x41,
            0x01, 0x40, 0x00, 0x1a, 0x0b,
        ];
        let mut core = TestInterpreter::new(CORE_MEMORY_GROW_GUEST).expect("instantiate core wasm");
        assert_eq!(core.memory_pages(), 0);

        let ExecutionEvent::MemoryGrow(event) = core.resume().expect("resume to memory.grow event")
        else {
            panic!("expected memory.grow core event");
        };
        assert_eq!(event.previous_pages, 0);
        assert_eq!(event.requested_pages, 1);
        assert_eq!(event.new_pages, Some(1));
        assert_eq!(core.memory_pages(), 1);
        assert_eq!(
            core.finish_memory_grow_event()
                .expect("host observes memory.grow"),
            event
        );
        assert_eq!(core.resume().expect("resume to done"), ExecutionEvent::Done);
    }

    #[test]
    fn core_wasm_engine_runs_local_function_calls_without_syscall_features() {
        static CORE_LOCAL_CALL_GUEST: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x60, 0x00, 0x01,
            0x7f, 0x60, 0x00, 0x00, 0x03, 0x03, 0x02, 0x00, 0x01, 0x07, 0x0a, 0x01, 0x06, b'_',
            b's', b't', b'a', b'r', b't', 0x00, 0x01, 0x0a, 0x0c, 0x02, 0x04, 0x00, 0x41, 0x2a,
            0x0b, 0x05, 0x00, 0x10, 0x00, 0x1a, 0x0b,
        ];
        let mut core =
            TestInterpreter::new(CORE_LOCAL_CALL_GUEST).expect("instantiate local-call core wasm");

        assert_eq!(
            core.resume().expect("local call reaches done"),
            ExecutionEvent::Done
        );
    }

    #[test]
    fn core_wasm_engine_executes_if_else_and_block_results() {
        let mut body = Vec::new();
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 1);
        body.push(super::OPCODE_IF);
        body.push(VALTYPE_I32);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 42);
        body.push(super::OPCODE_ELSE);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 7);
        body.push(OPCODE_END);
        body.push(super::OPCODE_I32_STORE);
        push_test_u32(&mut body, 2);
        push_test_u32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 4);
        body.push(super::OPCODE_BLOCK);
        body.push(VALTYPE_I32);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 9);
        body.push(OPCODE_END);
        body.push(super::OPCODE_I32_STORE);
        push_test_u32(&mut body, 2);
        push_test_u32(&mut body, 0);

        let module = core_test_module(&body, true, None, None, None);
        let mut core = TestInterpreter::new(&module).expect("instantiate if/block core wasm");
        assert_eq!(
            core.resume().expect("if/block reaches done"),
            ExecutionEvent::Done
        );
        assert_eq!(core.read_memory_u32(0).expect("if result"), 42);
        assert_eq!(core.read_memory_u32(4).expect("block result"), 9);
    }

    #[test]
    fn core_wasm_engine_executes_sign_extension_ops() {
        let mut body = Vec::new();
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0x80);
        body.push(super::OPCODE_I32_EXTEND8_S);
        body.push(super::OPCODE_I32_STORE);
        push_test_u32(&mut body, 2);
        push_test_u32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 8);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0x8000);
        body.push(super::OPCODE_I64_EXTEND_I32_U);
        body.push(super::OPCODE_I64_EXTEND16_S);
        body.push(super::OPCODE_I64_STORE);
        push_test_u32(&mut body, 3);
        push_test_u32(&mut body, 0);

        let module = core_test_module(&body, true, None, None, None);
        let mut core = TestInterpreter::new(&module).expect("instantiate sign-extension wasm");
        assert_eq!(
            core.resume().expect("sign-extension reaches done"),
            ExecutionEvent::Done
        );
        assert_eq!(core.read_memory_u32(0).expect("i32.extend8_s"), 0xffff_ff80);
        let mut out = [0u8; 8];
        core.read_memory(8, &mut out).expect("i64.extend16_s");
        assert_eq!(u64::from_le_bytes(out), 0xffff_ffff_ffff_8000);
    }

    #[test]
    fn core_wasm_engine_executes_passive_data_memory_init() {
        let mut data = Vec::new();
        push_test_u32(&mut data, 1);
        push_test_u32(&mut data, 1);
        push_test_u32(&mut data, 6);
        data.extend_from_slice(b"hibana");

        let mut body = Vec::new();
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 16);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 1);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 3);
        body.push(super::OPCODE_MISC);
        push_test_u32(&mut body, 8);
        push_test_u32(&mut body, 0);
        body.push(0x00);
        body.push(super::OPCODE_MISC);
        push_test_u32(&mut body, 9);
        push_test_u32(&mut body, 0);

        let module = core_test_module(&body, true, None, Some(&data), None);
        let mut core = TestInterpreter::new(&module).expect("instantiate passive data wasm");
        assert_eq!(
            core.resume().expect("passive data reaches done"),
            ExecutionEvent::Done
        );
        let mut out = [0u8; 3];
        core.read_memory(16, &mut out).expect("memory.init bytes");
        assert_eq!(&out, b"iba");
    }

    #[test]
    fn core_wasm_engine_executes_float_basics() {
        let mut body = Vec::new();
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(super::OPCODE_F32_CONST);
        body.extend_from_slice(&1.5f32.to_bits().to_le_bytes());
        body.push(super::OPCODE_F32_CONST);
        body.extend_from_slice(&2.25f32.to_bits().to_le_bytes());
        body.push(super::OPCODE_F32_ADD);
        body.push(super::OPCODE_I32_REINTERPRET_F32);
        body.push(super::OPCODE_I32_STORE);
        push_test_u32(&mut body, 2);
        push_test_u32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 8);
        body.push(super::OPCODE_F64_CONST);
        body.extend_from_slice(&4.0f64.to_bits().to_le_bytes());
        body.push(super::OPCODE_F64_SQRT);
        body.push(super::OPCODE_I64_REINTERPRET_F64);
        body.push(super::OPCODE_I64_STORE);
        push_test_u32(&mut body, 3);
        push_test_u32(&mut body, 0);

        let module = core_test_module(&body, true, None, None, None);
        let mut core = TestInterpreter::new(&module).expect("instantiate float wasm");
        assert_eq!(
            core.resume().expect("float reaches done"),
            ExecutionEvent::Done
        );
        assert_eq!(
            f32::from_bits(core.read_memory_u32(0).expect("f32 add")),
            3.75
        );
        let mut out = [0u8; 8];
        core.read_memory(8, &mut out).expect("f64 sqrt");
        assert_eq!(f64::from_bits(u64::from_le_bytes(out)), 2.0);
    }

    #[test]
    fn core_wasm_engine_executes_table_ref_basics() {
        let mut body = Vec::new();
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(super::OPCODE_REF_FUNC);
        push_test_u32(&mut body, 0);
        body.push(super::OPCODE_TABLE_SET);
        push_test_u32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 0);
        body.push(super::OPCODE_TABLE_GET);
        push_test_u32(&mut body, 0);
        body.push(super::OPCODE_REF_IS_NULL);
        body.push(super::OPCODE_I32_STORE);
        push_test_u32(&mut body, 2);
        push_test_u32(&mut body, 0);
        body.push(super::OPCODE_REF_NULL);
        body.push(super::VALTYPE_FUNCREF);
        body.push(OPCODE_I32_CONST);
        push_test_i32(&mut body, 1);
        body.push(super::OPCODE_MISC);
        push_test_u32(&mut body, 15);
        push_test_u32(&mut body, 0);
        body.push(OPCODE_DROP);

        let module = core_test_module(&body, true, Some(1), None, None);
        let mut core = TestInterpreter::new(&module).expect("instantiate table/ref wasm");
        assert_eq!(
            core.resume().expect("table/ref reaches done"),
            ExecutionEvent::Done
        );
        assert_eq!(core.read_memory_u32(0).expect("ref.is_null"), 0);
    }

    #[test]
    fn core_wasip1_trampoline_maps_fd_write_only_when_handler_is_enabled() {
        static CORE_WASIP1_FD_WRITE_GUEST: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0c, 0x02, 0x60, 0x04, 0x7f,
            0x7f, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x00, 0x02, 0x23, 0x01, 0x16, b'w', b'a',
            b's', b'i', b'_', b's', b'n', b'a', b'p', b's', b'h', b'o', b't', b'_', b'p', b'r',
            b'e', b'v', b'i', b'e', b'w', b'1', 0x08, b'f', b'd', b'_', b'w', b'r', b'i', b't',
            b'e', 0x00, 0x00, 0x03, 0x02, 0x01, 0x01, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x0a,
            0x01, 0x06, b'_', b's', b't', b'a', b'r', b't', 0x00, 0x01, 0x0a, 0x16, 0x01, 0x14,
            0x00, 0x41, 0x00, 0x41, 0x31, 0x3a, 0x00, 0x00, 0x41, 0x03, 0x41, 0x00, 0x41, 0x00,
            0x41, 0x01, 0x10, 0x00, 0x1a, 0x0b,
        ];

        let mut disabled_guest = TestVm::new(CORE_WASIP1_FD_WRITE_GUEST, Wasip1HandlerSet::EMPTY)
            .expect("static fd_write import is load evidence, not admission authority");
        assert!(matches!(
            disabled_guest.resume(test_budget()),
            Err(WasmError::Unsupported(
                "wasip1 fd_write disabled by feature profile"
            ))
        ));

        let mut guest = TestVm::new(CORE_WASIP1_FD_WRITE_GUEST, Wasip1HandlerSet::PICO_MIN)
            .expect("instantiate core wasip1 fd_write guest");
        let VmEvent::FdWrite(write) = guest
            .resume(test_budget())
            .expect("fd_write trampoline trap")
        else {
            panic!("expected fd_write trap");
        };
        assert_eq!(write.fd(), 3);
        assert_eq!(
            guest
                .fd_write_payload(write)
                .expect("payload comes from core memory")
                .as_bytes(),
            b"1"
        );
        guest.finish_host_call(0).expect("return errno to core");
        assert_eq!(
            guest.resume(test_budget()).expect("done after fd_write"),
            VmEvent::Done
        );
    }

    #[cfg(any(feature = "wasip1-sys-path-open", feature = "wasm-engine-core"))]
    #[test]
    fn core_wasip1_trampoline_maps_full_feature_syscall_surface() {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                {
                    let fd_read = core_wasip1_single_import_module(
                        Wasip1ImportName::FdRead,
                        &[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[
                            TestWasmArg::I32(0),
                            TestWasmArg::I32(16),
                            TestWasmArg::I32(1),
                            TestWasmArg::I32(40),
                        ],
                        true,
                    );
                    let mut disabled_guest = TestVm::new(&fd_read, Wasip1HandlerSet::PICO_MIN)
                        .expect("static fd_read import is not admission authority");
                    assert!(matches!(
                        disabled_guest.resume(test_budget()),
                        Err(WasmError::Unsupported(
                            "wasip1 fd_read disabled by feature profile"
                        ))
                    ));
                    let mut guest =
                        TestVm::new(&fd_read, Wasip1HandlerSet::FULL).expect("fd_read full");
                    let VmEvent::FdRead(read) = guest.resume(test_budget()).expect("fd_read trap")
                    else {
                        panic!("expected fd_read");
                    };
                    assert_eq!(read.fd(), 0);
                    assert_eq!(read.iovs(), 16);
                    assert_eq!(read.iovs_len(), 1);
                    assert_eq!(read.nread(), 40);
                    guest.finish_host_call(0).expect("complete fd_read errno");
                }

                {
                    let fdstat = core_wasip1_single_import_module(
                        Wasip1ImportName::FdFdstatGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(1), TestWasmArg::I32(80)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&fdstat, Wasip1HandlerSet::FULL).expect("fdstat full");
                    let VmEvent::FdFdstatGet(stat) =
                        guest.resume(test_budget()).expect("fdstat trap")
                    else {
                        panic!("expected fd_fdstat_get");
                    };
                    assert_eq!(stat.fd(), 1);
                    assert_eq!(stat.out_ptr(), 80);
                    guest
                        .finish_fd_fdstat_get(
                            stat,
                            FdStat::new(WASIP1_FILETYPE_REGULAR_FILE, 0, 0b11, 0),
                            0,
                        )
                        .expect("complete fdstat");
                    let mut rights = [0u8; 8];
                    guest
                        .read_memory(80 + WASIP1_FDSTAT_RIGHTS_BASE_OFFSET, &mut rights)
                        .expect("fdstat rights memory");
                    assert_eq!(u64::from_le_bytes(rights), 0b11);
                }

                {
                    let fd_close = core_wasip1_single_import_module(
                        Wasip1ImportName::FdClose,
                        &[VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(7)],
                        false,
                    );
                    let mut guest =
                        TestVm::new(&fd_close, Wasip1HandlerSet::FULL).expect("fd_close full");
                    let VmEvent::FdClose(close) =
                        guest.resume(test_budget()).expect("fd_close trap")
                    else {
                        panic!("expected fd_close");
                    };
                    assert_eq!(close.fd(), 7);
                    guest.finish_host_call(0).expect("complete fd_close");
                }

                {
                    let path_open = core_wasip1_single_import_module(
                        Wasip1ImportName::PathOpen,
                        &[
                            VALTYPE_I32,
                            VALTYPE_I32,
                            VALTYPE_I32,
                            VALTYPE_I32,
                            VALTYPE_I32,
                            VALTYPE_I64,
                            VALTYPE_I64,
                            VALTYPE_I32,
                            VALTYPE_I32,
                        ],
                        &[VALTYPE_I32],
                        &[
                            TestWasmArg::I32(3),
                            TestWasmArg::I32(0),
                            TestWasmArg::I32(160),
                            TestWasmArg::I32(4),
                            TestWasmArg::I32(0),
                            TestWasmArg::I64(1),
                            TestWasmArg::I64(1),
                            TestWasmArg::I32(0),
                            TestWasmArg::I32(196),
                        ],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&path_open, Wasip1HandlerSet::FULL).expect("path_open full");
                    let VmEvent::PathOpen(path) =
                        guest.resume(test_budget()).expect("path_open trap")
                    else {
                        panic!("expected path_open trap");
                    };
                    assert_eq!(path.fd(), 3);
                    assert_eq!(path.rights_base(), 1);
                    guest
                        .finish_path_open(path, 0, 52)
                        .expect("complete path_open as ENOSYS");
                    assert_eq!(
                        guest.resume(test_budget()).expect("done after path_open"),
                        VmEvent::Done
                    );
                }

                for unsupported in [
                    Wasip1ImportName::FdPrestatGet,
                    Wasip1ImportName::FdSeek,
                    Wasip1ImportName::FdFdstatSetRights,
                    Wasip1ImportName::PathLink,
                    Wasip1ImportName::SockSend,
                    Wasip1ImportName::SockRecv,
                    Wasip1ImportName::SockAccept,
                    Wasip1ImportName::SockShutdown,
                    Wasip1ImportName::SchedYield,
                    Wasip1ImportName::ProcRaise,
                ] {
                    let module = core_wasip1_single_import_module(
                        unsupported,
                        &[VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(3)],
                        false,
                    );
                    let mut guest = TestVm::new(&module, Wasip1HandlerSet::FULL)
                        .expect("static unsupported import is not admission authority");
                    assert!(
                        matches!(guest.resume(test_budget()), Err(WasmError::Unsupported(_))),
                        "{} should fault only when the unsupported WASI P1 import is called",
                        unsupported.name()
                    );
                }

                {
                    let clock_res = core_wasip1_single_import_module(
                        Wasip1ImportName::ClockResGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(1), TestWasmArg::I32(88)],
                        true,
                    );
                    let mut disabled_guest = TestVm::new(&clock_res, Wasip1HandlerSet::PICO_MIN)
                        .expect("static clock_res_get import is not admission authority");
                    assert!(matches!(
                        disabled_guest.resume(test_budget()),
                        Err(WasmError::Unsupported(
                            "wasip1 clock_res_get disabled by feature profile"
                        ))
                    ));
                    let mut guest = TestVm::new(&clock_res, Wasip1HandlerSet::FULL)
                        .expect("clock_res_get full");
                    let VmEvent::ClockResGet(clock) =
                        guest.resume(test_budget()).expect("clock_res_get trap")
                    else {
                        panic!("expected clock_res_get");
                    };
                    assert_eq!(clock.clock_id(), 1);
                    assert_eq!(clock.resolution_ptr(), 88);
                    guest
                        .finish_clock_res_get(clock, 1_000_000, 0)
                        .expect("complete clock_res_get");
                    let mut resolution = [0u8; 8];
                    guest
                        .read_memory(88, &mut resolution)
                        .expect("read clock resolution result");
                    assert_eq!(u64::from_le_bytes(resolution), 1_000_000);
                }

                {
                    let clock = core_wasip1_single_import_module(
                        Wasip1ImportName::ClockTimeGet,
                        &[VALTYPE_I32, VALTYPE_I64, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[
                            TestWasmArg::I32(1),
                            TestWasmArg::I64(1_000),
                            TestWasmArg::I32(96),
                        ],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&clock, Wasip1HandlerSet::FULL).expect("clock full");
                    let VmEvent::ClockTimeGet(clock) =
                        guest.resume(test_budget()).expect("clock trap")
                    else {
                        panic!("expected clock_time_get");
                    };
                    assert_eq!(clock.clock_id(), 1);
                    assert_eq!(clock.precision(), 1_000);
                    assert_eq!(clock.time_ptr(), 96);
                    guest
                        .finish_clock_time_get(clock, 123_456_789, 0)
                        .expect("complete clock");
                    let mut nanos = [0u8; 8];
                    guest
                        .read_memory(96, &mut nanos)
                        .expect("read clock result");
                    assert_eq!(u64::from_le_bytes(nanos), 123_456_789);
                }

                {
                    let random = core_wasip1_single_import_module(
                        Wasip1ImportName::RandomGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(112), TestWasmArg::I32(4)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&random, Wasip1HandlerSet::FULL).expect("random full");
                    let VmEvent::RandomGet(random) =
                        guest.resume(test_budget()).expect("random trap")
                    else {
                        panic!("expected random_get");
                    };
                    assert_eq!(random.buf(), 112);
                    assert_eq!(random.buf_len(), 4);
                    guest
                        .finish_random_get(random, b"RAND", 0)
                        .expect("complete random");
                    let mut random_out = [0u8; 4];
                    guest
                        .read_memory(112, &mut random_out)
                        .expect("read random result");
                    assert_eq!(&random_out, b"RAND");
                }
            })
            .expect("spawn wasm test")
            .join()
            .expect("wasm test joins");
    }

    #[cfg(any(feature = "wasip1-sys-path-open", feature = "wasm-engine-core"))]
    #[test]
    fn core_wasip1_path_helpers_write_meaningful_choreofs_results() {
        let path_open = core_wasip1_single_import_module(
            Wasip1ImportName::PathOpen,
            &[
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I64,
                VALTYPE_I64,
                VALTYPE_I32,
                VALTYPE_I32,
            ],
            &[VALTYPE_I32],
            &[
                TestWasmArg::I32(3),
                TestWasmArg::I32(0),
                TestWasmArg::I32(160),
                TestWasmArg::I32(10),
                TestWasmArg::I32(0),
                TestWasmArg::I64(1),
                TestWasmArg::I64(1),
                TestWasmArg::I32(0),
                TestWasmArg::I32(196),
            ],
            true,
        );
        let mut guest = TestVm::new(&path_open, Wasip1HandlerSet::FULL).expect("path_open full");
        guest
            .write_memory(160, b"app/config")
            .expect("write path bytes into guest memory");
        let VmEvent::PathOpen(path) = guest.resume(test_budget()).expect("path_open trap") else {
            panic!("expected path_open trap");
        };
        assert_eq!(path.fd(), 3);
        assert_eq!(
            guest.path_bytes(path).expect("read path bytes").as_bytes(),
            b"app/config"
        );
        guest
            .finish_path_open(path, 44, 0)
            .expect("complete path_open with minted fd");
        assert_eq!(guest.read_memory_u32(196).expect("opened fd"), 44);
        assert_eq!(
            guest.resume(test_budget()).expect("done after path_open"),
            VmEvent::Done
        );

        let fd_readdir = core_wasip1_single_import_module(
            Wasip1ImportName::FdReaddir,
            &[
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I32,
                VALTYPE_I64,
                VALTYPE_I32,
            ],
            &[VALTYPE_I32],
            &[
                TestWasmArg::I32(44),
                TestWasmArg::I32(224),
                TestWasmArg::I32(32),
                TestWasmArg::I64(0),
                TestWasmArg::I32(260),
            ],
            true,
        );
        let mut guest = TestVm::new(&fd_readdir, Wasip1HandlerSet::FULL).expect("readdir full");
        let VmEvent::FdReaddir(path) = guest.resume(test_budget()).expect("fd_readdir trap") else {
            panic!("expected fd_readdir trap");
        };
        assert_eq!(path.fd(), 44);
        assert_eq!(path.cookie(), 0);
        assert_eq!(path.max_len(), 32);
        guest
            .finish_fd_readdir(path, b"config\nstate\n", 0)
            .expect("complete fd_readdir with manifest bytes");
        let mut bytes = [0u8; 13];
        guest
            .read_memory(224, &mut bytes)
            .expect("read fd_readdir bytes");
        assert_eq!(&bytes, b"config\nstate\n");
        assert_eq!(guest.read_memory_u32(260).expect("bufused"), 13);
        assert_eq!(
            guest.resume(test_budget()).expect("done after fd_readdir"),
            VmEvent::Done
        );
    }

    #[test]
    fn core_wasip1_trampoline_maps_args_and_environment_imports() {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                {
                    let args_sizes = core_wasip1_single_import_module(
                        Wasip1ImportName::ArgsSizesGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(16), TestWasmArg::I32(20)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&args_sizes, Wasip1HandlerSet::FULL).expect("args sizes full");
                    let VmEvent::ArgsSizesGet(call) =
                        guest.resume(test_budget()).expect("args sizes trap")
                    else {
                        panic!("expected args_sizes_get");
                    };
                    guest
                        .finish_args_sizes_get(call, 2, 12, 0)
                        .expect("complete args sizes");
                    assert_eq!(guest.read_memory_u32(16).expect("argc"), 2);
                    assert_eq!(guest.read_memory_u32(20).expect("argv size"), 12);
                }

                {
                    let args_get = core_wasip1_single_import_module(
                        Wasip1ImportName::ArgsGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(32), TestWasmArg::I32(64)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&args_get, Wasip1HandlerSet::FULL).expect("args get full");
                    let VmEvent::ArgsGet(call) =
                        guest.resume(test_budget()).expect("args get trap")
                    else {
                        panic!("expected args_get");
                    };
                    guest
                        .finish_args_get(call, &[b"hibana", b"pico"], 0)
                        .expect("complete args get");
                    assert_eq!(guest.read_memory_u32(32).expect("argv0 ptr"), 64);
                    assert_eq!(guest.read_memory_u32(36).expect("argv1 ptr"), 71);
                    let mut arg_bytes = [0u8; 12];
                    guest
                        .read_memory(64, &mut arg_bytes)
                        .expect("read args bytes");
                    assert_eq!(&arg_bytes, b"hibana\0pico\0");
                }

                {
                    let env_sizes = core_wasip1_single_import_module(
                        Wasip1ImportName::EnvironSizesGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(128), TestWasmArg::I32(132)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&env_sizes, Wasip1HandlerSet::FULL).expect("env sizes full");
                    let VmEvent::EnvironSizesGet(call) =
                        guest.resume(test_budget()).expect("env sizes trap")
                    else {
                        panic!("expected environ_sizes_get");
                    };
                    guest
                        .finish_environ_sizes_get(call, 1, 10, 0)
                        .expect("complete env sizes");
                    assert_eq!(guest.read_memory_u32(128).expect("env count"), 1);
                    assert_eq!(guest.read_memory_u32(132).expect("env size"), 10);
                }

                {
                    let env_get = core_wasip1_single_import_module(
                        Wasip1ImportName::EnvironGet,
                        &[VALTYPE_I32, VALTYPE_I32],
                        &[VALTYPE_I32],
                        &[TestWasmArg::I32(140), TestWasmArg::I32(160)],
                        true,
                    );
                    let mut guest =
                        TestVm::new(&env_get, Wasip1HandlerSet::FULL).expect("env get full");
                    let VmEvent::EnvironGet(call) =
                        guest.resume(test_budget()).expect("env get trap")
                    else {
                        panic!("expected environ_get");
                    };
                    guest
                        .finish_environ_get(call, &[(b"MODE", b"test")], 0)
                        .expect("complete env get");
                    assert_eq!(guest.read_memory_u32(140).expect("env ptr"), 160);
                    let mut env_bytes = [0u8; 10];
                    guest
                        .read_memory(160, &mut env_bytes)
                        .expect("read env bytes");
                    assert_eq!(&env_bytes, b"MODE=test\0");
                }
            })
            .expect("spawn args/env wasm test")
            .join()
            .expect("args/env wasm test joins");
    }

    #[test]
    fn core_wasip1_trampoline_maps_proc_exit_as_app_termination() {
        static CORE_WASIP1_PROC_EXIT_GUEST: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x60, 0x01, 0x7f,
            0x00, 0x60, 0x00, 0x00, 0x02, 0x24, 0x01, 0x16, b'w', b'a', b's', b'i', b'_', b's',
            b'n', b'a', b'p', b's', b'h', b'o', b't', b'_', b'p', b'r', b'e', b'v', b'i', b'e',
            b'w', b'1', 0x09, b'p', b'r', b'o', b'c', b'_', b'e', b'x', b'i', b't', 0x00, 0x00,
            0x03, 0x02, 0x01, 0x01, 0x07, 0x0a, 0x01, 0x06, b'_', b's', b't', b'a', b'r', b't',
            0x00, 0x01, 0x0a, 0x08, 0x01, 0x06, 0x00, 0x41, 0x07, 0x10, 0x00, 0x0b,
        ];
        let mut guest = TestVm::new(CORE_WASIP1_PROC_EXIT_GUEST, Wasip1HandlerSet::PICO_MIN)
            .expect("instantiate core wasip1 proc_exit guest");

        assert_eq!(
            guest
                .resume(test_budget())
                .expect("proc_exit trampoline trap"),
            VmEvent::ProcExit(7)
        );
        assert_eq!(
            guest
                .resume(test_budget())
                .expect("proc_exit terminates app"),
            VmEvent::Done
        );
    }
}
