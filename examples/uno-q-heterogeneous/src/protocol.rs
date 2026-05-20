use hibana::integration::wire::{CodecError, Payload, WireEncode, WirePayload};

pub const ROLE_M33_LED_KERNEL: u8 = 0;
pub const ROLE_WASI_LLM_CELL: u8 = 1;
pub const ROLE_LINUX_KERNEL: u8 = 2;
pub const ROLE_LLM_SIDECAR: u8 = 3;
pub const ROLE_CHALLENGER_KERNEL: u8 = 4;
pub const ROLE_IOS_PROMPT_INGRESS: u8 = 5;

pub const LABEL_IOS_PROMPT_REQUEST: u8 = 151;
pub const LABEL_IOS_PROMPT_FACT: u8 = 152;
pub const LABEL_LLM_PROMPT_TO_LINUX: u8 = 153;
pub const LABEL_LLM_REQUEST_TO_SIDECAR: u8 = 154;
pub const LABEL_LLM_PROPOSAL_TO_LINUX: u8 = 155;
pub const LABEL_FACE_CANDIDATE_TO_M33: u8 = 156;
pub const LABEL_CHALLENGER_PACKET: u8 = 157;
pub const LABEL_CHALLENGER_RECEIPT: u8 = 158;
pub const LABEL_CHALLENGER_READ: u8 = 159;
pub const LABEL_CHALLENGER_READ_RET: u8 = 160;
pub const LABEL_FACE_ACK_COMMIT: u8 = 161;
pub const LABEL_FINAL_COMMIT: u8 = 162;

pub const FACE_NEUTRAL: u8 = 0;
pub const FACE_HAPPY: u8 = 1;
pub const FACE_SAD: u8 = 2;
pub const FACE_ANGRY: u8 = 3;
pub const FACE_SURPRISED: u8 = 4;
pub const FACE_THINKING: u8 = 5;
pub const FACE_SPEAKING: u8 = 6;

pub const MAX_TEXT_BYTES: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    TextTooLong,
    InvalidEmotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmallText {
    bytes: [u8; MAX_TEXT_BYTES],
    len: u8,
}

impl SmallText {
    pub const EMPTY: Self = Self {
        bytes: [0; MAX_TEXT_BYTES],
        len: 0,
    };

    pub fn new(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::TextTooLong);
        }
        let mut out = [0u8; MAX_TEXT_BYTES];
        let mut index = 0usize;
        while index < bytes.len() {
            out[index] = bytes[index];
            index += 1;
        }
        Ok(Self {
            bytes: out,
            len: bytes.len() as u8,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.is_empty() {
            return Err(CodecError::Truncated);
        }
        let len = bytes[0] as usize;
        if len > MAX_TEXT_BYTES {
            return Err(CodecError::Invalid("small text exceeds capacity"));
        }
        if bytes.len() != 1 + len {
            return Err(CodecError::Invalid("small text length mismatch"));
        }
        Self::new(&bytes[1..]).map_err(|_| CodecError::Invalid("invalid small text"))
    }
}

impl WireEncode for SmallText {
    fn encoded_len(&self) -> Option<usize> {
        Some(1 + self.as_bytes().len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let bytes = self.as_bytes();
        if out.len() < 1 + bytes.len() {
            return Err(CodecError::Truncated);
        }
        out[0] = bytes.len() as u8;
        out[1..1 + bytes.len()].copy_from_slice(bytes);
        Ok(1 + bytes.len())
    }
}

impl WirePayload for SmallText {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        Self::decode(input.as_bytes())
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.is_empty() {
            return Err(CodecError::Truncated);
        }
        scratch[0] = 0;
        Ok(Payload::new(&scratch[..1]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlmProposal {
    emotion: u8,
    text: SmallText,
}

impl LlmProposal {
    pub fn new(emotion: u8, text: &[u8]) -> Result<Self, ProtocolError> {
        validate_face(emotion)?;
        Ok(Self {
            emotion,
            text: SmallText::new(text)?,
        })
    }

    pub const fn emotion(&self) -> u8 {
        self.emotion
    }

    pub const fn text(&self) -> &SmallText {
        &self.text
    }
}

impl WireEncode for LlmProposal {
    fn encoded_len(&self) -> Option<usize> {
        Some(2 + self.text.as_bytes().len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 + self.text.as_bytes().len() {
            return Err(CodecError::Truncated);
        }
        out[0] = self.emotion;
        self.text.encode_into(&mut out[1..]).map(|len| 1 + len)
    }
}

impl WirePayload for LlmProposal {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 2 {
            return Err(CodecError::Truncated);
        }
        validate_face(bytes[0]).map_err(|_| CodecError::Invalid("invalid face proposal"))?;
        let text = SmallText::decode(&bytes[1..])?;
        Ok(Self {
            emotion: bytes[0],
            text,
        })
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
pub struct FaceCandidate {
    face: u8,
    mouth_frames: u8,
}

impl FaceCandidate {
    pub fn new(face: u8, mouth_frames: u8) -> Result<Self, ProtocolError> {
        validate_face(face)?;
        Ok(Self { face, mouth_frames })
    }

    pub const fn face(&self) -> u8 {
        self.face
    }

    pub const fn mouth_frames(&self) -> u8 {
        self.mouth_frames
    }
}

impl WireEncode for FaceCandidate {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.face;
        out[1] = self.mouth_frames;
        Ok(2)
    }
}

impl WirePayload for FaceCandidate {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("face candidate carries two bytes"));
        }
        Self::new(bytes[0], bytes[1]).map_err(|_| CodecError::Invalid("invalid face candidate"))
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
pub struct NetPacket {
    packet_id: u32,
    body: SmallText,
}

impl NetPacket {
    pub fn new(packet_id: u32, body: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            packet_id,
            body: SmallText::new(body)?,
        })
    }

    pub const fn packet_id(&self) -> u32 {
        self.packet_id
    }

    pub const fn body(&self) -> &SmallText {
        &self.body
    }
}

impl WireEncode for NetPacket {
    fn encoded_len(&self) -> Option<usize> {
        Some(5 + self.body.as_bytes().len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 5 + self.body.as_bytes().len() {
            return Err(CodecError::Truncated);
        }
        out[..4].copy_from_slice(&self.packet_id.to_be_bytes());
        self.body.encode_into(&mut out[4..]).map(|len| 4 + len)
    }
}

impl WirePayload for NetPacket {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 5 {
            return Err(CodecError::Truncated);
        }
        let mut packet_id = [0u8; 4];
        packet_id.copy_from_slice(&bytes[..4]);
        let body = SmallText::decode(&bytes[4..])?;
        Ok(Self {
            packet_id: u32::from_be_bytes(packet_id),
            body,
        })
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.len() < 5 {
            return Err(CodecError::Truncated);
        }
        scratch[..5].fill(0);
        Ok(Payload::new(&scratch[..5]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetReceipt {
    packet_id: u32,
    status: u8,
    body: SmallText,
}

impl NetReceipt {
    pub fn new(packet_id: u32, status: u8, body: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            packet_id,
            status,
            body: SmallText::new(body)?,
        })
    }

    pub const fn packet_id(&self) -> u32 {
        self.packet_id
    }

    pub const fn status(&self) -> u8 {
        self.status
    }

    pub const fn body(&self) -> &SmallText {
        &self.body
    }
}

impl WireEncode for NetReceipt {
    fn encoded_len(&self) -> Option<usize> {
        Some(6 + self.body.as_bytes().len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 6 + self.body.as_bytes().len() {
            return Err(CodecError::Truncated);
        }
        out[..4].copy_from_slice(&self.packet_id.to_be_bytes());
        out[4] = self.status;
        self.body.encode_into(&mut out[5..]).map(|len| 5 + len)
    }
}

impl WirePayload for NetReceipt {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 6 {
            return Err(CodecError::Truncated);
        }
        let mut packet_id = [0u8; 4];
        packet_id.copy_from_slice(&bytes[..4]);
        let body = SmallText::decode(&bytes[5..])?;
        Ok(Self {
            packet_id: u32::from_be_bytes(packet_id),
            status: bytes[4],
            body,
        })
    }

    fn synthetic_payload<'a>(scratch: &'a mut [u8]) -> Result<Payload<'a>, CodecError> {
        if scratch.len() < 6 {
            return Err(CodecError::Truncated);
        }
        scratch[..6].fill(0);
        Ok(Payload::new(&scratch[..6]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitMarker {
    face: u8,
    challenger_status: u8,
}

impl CommitMarker {
    pub fn new(face: u8, challenger_status: u8) -> Result<Self, ProtocolError> {
        validate_face(face)?;
        Ok(Self {
            face,
            challenger_status,
        })
    }

    pub const fn face(&self) -> u8 {
        self.face
    }

    pub const fn challenger_status(&self) -> u8 {
        self.challenger_status
    }
}

impl WireEncode for CommitMarker {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.face;
        out[1] = self.challenger_status;
        Ok(2)
    }
}

impl WirePayload for CommitMarker {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("commit marker carries two bytes"));
        }
        Self::new(bytes[0], bytes[1]).map_err(|_| CodecError::Invalid("invalid commit marker"))
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

fn validate_face(face: u8) -> Result<(), ProtocolError> {
    match face {
        FACE_NEUTRAL | FACE_HAPPY | FACE_SAD | FACE_ANGRY | FACE_SURPRISED | FACE_THINKING
        | FACE_SPEAKING => Ok(()),
        _ => Err(ProtocolError::InvalidEmotion),
    }
}
