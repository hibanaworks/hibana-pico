use hibana::integration::wire::{CodecError, Payload, WireEncode, WirePayload};

pub const ROLE_M33_LED_KERNEL: u8 = 0;
pub const ROLE_WASI_LLM_CELL: u8 = 1;
pub const ROLE_LOCAL_LLM: u8 = 2;
pub const ROLE_HUMAN_INPUT: u8 = 3;

pub const LABEL_HUMAN_INPUT_TEXT: u8 = 151;
pub const LABEL_HUMAN_INPUT_ACK: u8 = 152;
pub const LABEL_HUMAN_INPUT_REQ: u8 = 153;
pub const HUMAN_INPUT_TEXT_BYTES: usize = 96;

pub const FACE_NEUTRAL: u8 = 0;
pub const FACE_HAPPY: u8 = 1;
pub const FACE_SAD: u8 = 2;
pub const FACE_ANGRY: u8 = 3;
pub const FACE_SURPRISED: u8 = 4;
pub const FACE_THINKING: u8 = 5;
pub const FACE_SPEAKING: u8 = 6;
pub const FACE_MOUTH_CLOSED: u8 = 16;
pub const FACE_MOUTH_SMALL: u8 = 17;
pub const FACE_MOUTH_WIDE: u8 = 18;
pub const FACE_MOUTH_ROUND: u8 = 19;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidEmotion,
    HumanInputTooLong,
    HumanInputInvalidUtf8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceFrame {
    face: u8,
    ordinal: u8,
}

impl FaceFrame {
    pub fn new(face: u8, ordinal: u8) -> Result<Self, ProtocolError> {
        validate_display_face(face)?;
        Ok(Self { face, ordinal })
    }

    pub const fn face(&self) -> u8 {
        self.face
    }

    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
}

impl WireEncode for FaceFrame {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.face;
        out[1] = self.ordinal;
        Ok(2)
    }
}

impl WirePayload for FaceFrame {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("face frame carries two bytes"));
        }
        Self::new(bytes[0], bytes[1]).map_err(|_| CodecError::Invalid("invalid face frame"))
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.len() < 2 {
            return Err(CodecError::Truncated);
        }
        scratch[0] = FACE_NEUTRAL;
        scratch[1] = 0;
        Ok(Payload::new(&scratch[..2]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanInputText {
    len: u8,
    bytes: [u8; HUMAN_INPUT_TEXT_BYTES],
}

impl HumanInputText {
    pub const fn empty() -> Self {
        Self {
            len: 0,
            bytes: [0; HUMAN_INPUT_TEXT_BYTES],
        }
    }

    pub fn new(text: &str) -> Result<Self, ProtocolError> {
        Self::from_bytes(text.as_bytes())
    }

    pub fn from_bytes(input: &[u8]) -> Result<Self, ProtocolError> {
        if input.len() > HUMAN_INPUT_TEXT_BYTES {
            return Err(ProtocolError::HumanInputTooLong);
        }
        if core::str::from_utf8(input).is_err() {
            return Err(ProtocolError::HumanInputInvalidUtf8);
        }
        let mut bytes = [0; HUMAN_INPUT_TEXT_BYTES];
        bytes[..input.len()].copy_from_slice(input);
        Ok(Self {
            len: input.len() as u8,
            bytes,
        })
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

    pub fn as_str(&self) -> Result<&str, ProtocolError> {
        core::str::from_utf8(self.as_bytes()).map_err(|_| ProtocolError::HumanInputInvalidUtf8)
    }
}

impl WireEncode for HumanInputText {
    fn encoded_len(&self) -> Option<usize> {
        Some(1 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 1 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.len;
        out[1..1 + len].copy_from_slice(self.as_bytes());
        Ok(1 + len)
    }
}

impl WirePayload for HumanInputText {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        let Some((&len, rest)) = bytes.split_first() else {
            return Err(CodecError::Invalid("human input text missing length"));
        };
        let len = usize::from(len);
        if len > HUMAN_INPUT_TEXT_BYTES {
            return Err(CodecError::Invalid("human input text exceeds capacity"));
        }
        if rest.len() != len {
            return Err(CodecError::Invalid("human input text length mismatch"));
        }
        Self::from_bytes(rest).map_err(|_| CodecError::Invalid("invalid human input text"))
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.is_empty() {
            return Err(CodecError::Truncated);
        }
        scratch[0] = 0;
        Ok(Payload::new(&scratch[..1]))
    }
}

fn validate_display_face(face: u8) -> Result<(), ProtocolError> {
    match face {
        FACE_NEUTRAL | FACE_HAPPY | FACE_SAD | FACE_ANGRY | FACE_SURPRISED | FACE_THINKING
        | FACE_SPEAKING | FACE_MOUTH_CLOSED | FACE_MOUTH_SMALL | FACE_MOUTH_WIDE
        | FACE_MOUTH_ROUND => Ok(()),
        _ => Err(ProtocolError::InvalidEmotion),
    }
}
