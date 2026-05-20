use hibana::integration::wire::{CodecError, Payload, WireEncode, WirePayload};

pub const ROLE_M33_LED_KERNEL: u8 = 0;
pub const ROLE_WASI_LLM_CELL: u8 = 1;
pub const ROLE_PSEUDO_LLM: u8 = 2;

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

fn validate_display_face(face: u8) -> Result<(), ProtocolError> {
    match face {
        FACE_NEUTRAL | FACE_HAPPY | FACE_SAD | FACE_ANGRY | FACE_SURPRISED | FACE_THINKING
        | FACE_SPEAKING | FACE_MOUTH_CLOSED | FACE_MOUTH_SMALL | FACE_MOUTH_WIDE
        | FACE_MOUTH_ROUND => Ok(()),
        _ => Err(ProtocolError::InvalidEmotion),
    }
}
