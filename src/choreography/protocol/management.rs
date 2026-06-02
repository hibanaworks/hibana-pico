use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtImageBegin {
    slot: u8,
    total_len: u32,
    generation: u32,
}

impl MgmtImageBegin {
    pub const fn new(slot: u8, total_len: u32, generation: u32) -> Self {
        Self {
            slot,
            total_len,
            generation,
        }
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn total_len(&self) -> u32 {
        self.total_len
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

impl WireEncode for MgmtImageBegin {
    fn encoded_len(&self) -> Option<usize> {
        Some(9)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 9 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.slot;
        out[1..5].copy_from_slice(&self.total_len.to_be_bytes());
        out[5..9].copy_from_slice(&self.generation.to_be_bytes());
        Ok(9)
    }
}

impl WirePayload for MgmtImageBegin {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 9 {
            return Err(CodecError::Invalid("image begin carries nine bytes"));
        }
        let mut total_len = [0u8; 4];
        let mut generation = [0u8; 4];
        total_len.copy_from_slice(&bytes[1..5]);
        generation.copy_from_slice(&bytes[5..9]);
        Ok(Self::new(
            bytes[0],
            u32::from_be_bytes(total_len),
            u32::from_be_bytes(generation),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtImageChunk {
    slot: u8,
    offset: u32,
    len: u8,
    bytes: [u8; MGMT_IMAGE_CHUNK_CAPACITY],
}

impl MgmtImageChunk {
    pub fn new(slot: u8, offset: u32, bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > MGMT_IMAGE_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("image chunk exceeds fixed capacity"));
        }
        let mut out = [0u8; MGMT_IMAGE_CHUNK_CAPACITY];
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            slot,
            offset,
            len: bytes.len() as u8,
            bytes: out,
        })
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }
}

impl WireEncode for MgmtImageChunk {
    fn encoded_len(&self) -> Option<usize> {
        Some(6 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 6 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.slot;
        out[1..5].copy_from_slice(&self.offset.to_be_bytes());
        out[5] = self.len;
        out[6..6 + len].copy_from_slice(self.as_bytes());
        Ok(6 + len)
    }
}

impl WirePayload for MgmtImageChunk {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 6 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[5] as usize;
        if bytes.len() != 6 + len {
            return Err(CodecError::Invalid("image chunk length mismatch"));
        }
        let mut offset = [0u8; 4];
        offset.copy_from_slice(&bytes[1..5]);
        Self::new(bytes[0], u32::from_be_bytes(offset), &bytes[6..])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtImageEnd {
    slot: u8,
    expected_len: u32,
}

impl MgmtImageEnd {
    pub const fn new(slot: u8, expected_len: u32) -> Self {
        Self { slot, expected_len }
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn expected_len(&self) -> u32 {
        self.expected_len
    }
}

impl WireEncode for MgmtImageEnd {
    fn encoded_len(&self) -> Option<usize> {
        Some(5)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 5 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.slot;
        out[1..5].copy_from_slice(&self.expected_len.to_be_bytes());
        Ok(5)
    }
}

impl WirePayload for MgmtImageEnd {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 5 {
            return Err(CodecError::Invalid("image end carries five bytes"));
        }
        let mut expected_len = [0u8; 4];
        expected_len.copy_from_slice(&bytes[1..5]);
        Ok(Self::new(bytes[0], u32::from_be_bytes(expected_len)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtImageActivate {
    slot: u8,
    fence_epoch: u32,
}

impl MgmtImageActivate {
    pub const fn new(slot: u8, fence_epoch: u32) -> Self {
        Self { slot, fence_epoch }
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn fence_epoch(&self) -> u32 {
        self.fence_epoch
    }
}

impl WireEncode for MgmtImageActivate {
    fn encoded_len(&self) -> Option<usize> {
        Some(5)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 5 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.slot;
        out[1..5].copy_from_slice(&self.fence_epoch.to_be_bytes());
        Ok(5)
    }
}

impl WirePayload for MgmtImageActivate {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 5 {
            return Err(CodecError::Invalid("image activate carries five bytes"));
        }
        let mut fence_epoch = [0u8; 4];
        fence_epoch.copy_from_slice(&bytes[1..5]);
        Ok(Self::new(bytes[0], u32::from_be_bytes(fence_epoch)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtImageRollback {
    slot: u8,
}

impl MgmtImageRollback {
    pub const fn new(slot: u8) -> Self {
        Self { slot }
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }
}

impl WireEncode for MgmtImageRollback {
    fn encoded_len(&self) -> Option<usize> {
        Some(1)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let Some(first) = out.first_mut() else {
            return Err(CodecError::Truncated);
        };
        *first = self.slot;
        Ok(1)
    }
}

impl WirePayload for MgmtImageRollback {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("image rollback carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MgmtStatusCode {
    Ok,
    InvalidImage,
    NeedFence,
    BadSlot,
    RollbackEmpty,
    BadFenceEpoch,
    AuthFailed,
    BadSessionGeneration,
    ImageTooLarge,
    OffsetMismatch,
    LengthMismatch,
    BadChunkIndex,
}

impl MgmtStatusCode {
    pub const fn tag(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::InvalidImage => 1,
            Self::NeedFence => 2,
            Self::BadSlot => 3,
            Self::RollbackEmpty => 4,
            Self::BadFenceEpoch => 5,
            Self::AuthFailed => 6,
            Self::BadSessionGeneration => 7,
            Self::ImageTooLarge => 8,
            Self::OffsetMismatch => 9,
            Self::LengthMismatch => 10,
            Self::BadChunkIndex => 11,
        }
    }

    fn decode(tag: u8) -> Result<Self, CodecError> {
        match tag {
            0 => Ok(Self::Ok),
            1 => Ok(Self::InvalidImage),
            2 => Ok(Self::NeedFence),
            3 => Ok(Self::BadSlot),
            4 => Ok(Self::RollbackEmpty),
            5 => Ok(Self::BadFenceEpoch),
            6 => Ok(Self::AuthFailed),
            7 => Ok(Self::BadSessionGeneration),
            8 => Ok(Self::ImageTooLarge),
            9 => Ok(Self::OffsetMismatch),
            10 => Ok(Self::LengthMismatch),
            11 => Ok(Self::BadChunkIndex),
            _ => Err(CodecError::Invalid("unknown management status code")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtStatus {
    slot: u8,
    code: MgmtStatusCode,
}

impl MgmtStatus {
    pub const fn new(slot: u8, code: MgmtStatusCode) -> Self {
        Self { slot, code }
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn code(&self) -> MgmtStatusCode {
        self.code
    }
}

impl WireEncode for MgmtStatus {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.slot;
        out[1] = self.code.tag();
        Ok(2)
    }
}

impl WirePayload for MgmtStatus {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("management status carries two bytes"));
        }
        Ok(Self::new(bytes[0], MgmtStatusCode::decode(bytes[1])?))
    }
}
