use hibana::runtime::wire::{CodecError, Payload, WireEncode, WirePayload};

pub const ROLE_M33_LED_KERNEL: u8 = 0;
pub const ROLE_WASI_LLM_CELL: u8 = 1;
pub const ROLE_LOCAL_LLM: u8 = 2;
pub const ROLE_HUMAN_INPUT: u8 = 3;
pub const ROLE_PICO2W_SENSOR: u8 = 4;

pub const LABEL_HUMAN_INPUT_TEXT: u8 = 151;
pub const LABEL_HUMAN_INPUT_ACK: u8 = 152;
pub const LABEL_HUMAN_INPUT_REQ: u8 = 153;
pub const LABEL_PICO2W_SENSOR_REQ: u8 = 154;
pub const LABEL_PICO2W_SENSOR_SAMPLE: u8 = 155;
pub const LABEL_PICO2W_SENSOR_ACK: u8 = 156;
pub const HUMAN_INPUT_TEXT_BYTES: usize = 96;
pub const PICO2W_SENSOR_SAMPLE_BYTES: usize = 9;
pub const PICO2W_SENSOR_UDP_ACK_BYTES: usize = 2;
pub const PICO2W_SENSOR_STATUS_FRESH: u8 = 0;
pub const PICO2W_SENSOR_STATUS_PENDING: u8 = 1;
pub const PICO2W_SENSOR_STATUS_STALE: u8 = 2;

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
    InvalidPico2wSensorStatus,
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

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Malformed);
        }
        Self::new(bytes[0], bytes[1])
            .map(|_| ())
            .map_err(|_| CodecError::Malformed)
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        let bytes = input.as_bytes();
        match Self::new(bytes[0], bytes[1]) {
            Ok(value) => value,
            Err(_) => panic!("validated face frame must decode"),
        }
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

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        let bytes = input.as_bytes();
        let Some((&len, rest)) = bytes.split_first() else {
            return Err(CodecError::Malformed);
        };
        let len = usize::from(len);
        if len > HUMAN_INPUT_TEXT_BYTES {
            return Err(CodecError::Malformed);
        }
        if rest.len() != len {
            return Err(CodecError::Malformed);
        }
        Self::from_bytes(rest)
            .map(|_| ())
            .map_err(|_| CodecError::Malformed)
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        let bytes = input.as_bytes();
        let len = usize::from(bytes[0]);
        match Self::from_bytes(&bytes[1..1 + len]) {
            Ok(value) => value,
            Err(_) => panic!("validated human input text must decode"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pico2wSensorSample {
    status: u8,
    temperature_c_x10: i16,
    humidity_pct_x10: u16,
    light_raw: u16,
    seq: u16,
}

impl Pico2wSensorSample {
    pub fn new(
        status: u8,
        temperature_c_x10: i16,
        humidity_pct_x10: u16,
        light_raw: u16,
        seq: u16,
    ) -> Result<Self, ProtocolError> {
        validate_pico2w_sensor_status(status)?;
        Ok(Self {
            status,
            temperature_c_x10,
            humidity_pct_x10,
            light_raw,
            seq,
        })
    }

    pub const fn pending(seq: u16) -> Self {
        Self {
            status: PICO2W_SENSOR_STATUS_PENDING,
            temperature_c_x10: 0,
            humidity_pct_x10: 0,
            light_raw: 0,
            seq,
        }
    }

    pub fn with_status_and_seq(self, status: u8, seq: u16) -> Result<Self, ProtocolError> {
        Self::new(
            status,
            self.temperature_c_x10,
            self.humidity_pct_x10,
            self.light_raw,
            seq,
        )
    }

    pub const fn status(&self) -> u8 {
        self.status
    }

    pub const fn temperature_c_x10(&self) -> i16 {
        self.temperature_c_x10
    }

    pub const fn humidity_pct_x10(&self) -> u16 {
        self.humidity_pct_x10
    }

    pub const fn light_raw(&self) -> u16 {
        self.light_raw
    }

    pub const fn seq(&self) -> u16 {
        self.seq
    }
}

pub const fn pico2w_sensor_udp_ack(seq: u16) -> [u8; PICO2W_SENSOR_UDP_ACK_BYTES] {
    seq.to_le_bytes()
}

pub fn decode_pico2w_sensor_udp_ack(input: &[u8]) -> Option<u16> {
    if input.len() != PICO2W_SENSOR_UDP_ACK_BYTES {
        return None;
    }
    Some(u16::from_le_bytes([input[0], input[1]]))
}

impl WireEncode for Pico2wSensorSample {
    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < PICO2W_SENSOR_SAMPLE_BYTES {
            return Err(CodecError::Truncated);
        }
        out[0] = self.status;
        out[1..3].copy_from_slice(&self.temperature_c_x10.to_le_bytes());
        out[3..5].copy_from_slice(&self.humidity_pct_x10.to_le_bytes());
        out[5..7].copy_from_slice(&self.light_raw.to_le_bytes());
        out[7..9].copy_from_slice(&self.seq.to_le_bytes());
        Ok(PICO2W_SENSOR_SAMPLE_BYTES)
    }
}

impl WirePayload for Pico2wSensorSample {
    type Decoded<'a> = Self;

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != PICO2W_SENSOR_SAMPLE_BYTES {
            return Err(CodecError::Malformed);
        }
        validate_pico2w_sensor_status(bytes[0]).map_err(|_| CodecError::Malformed)
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        let bytes = input.as_bytes();
        match Self::new(
            bytes[0],
            i16::from_le_bytes([bytes[1], bytes[2]]),
            u16::from_le_bytes([bytes[3], bytes[4]]),
            u16::from_le_bytes([bytes[5], bytes[6]]),
            u16::from_le_bytes([bytes[7], bytes[8]]),
        ) {
            Ok(value) => value,
            Err(_) => panic!("validated pico2w sensor sample must decode"),
        }
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

fn validate_pico2w_sensor_status(status: u8) -> Result<(), ProtocolError> {
    match status {
        PICO2W_SENSOR_STATUS_FRESH | PICO2W_SENSOR_STATUS_PENDING | PICO2W_SENSOR_STATUS_STALE => {
            Ok(())
        }
        _ => Err(ProtocolError::InvalidPico2wSensorStatus),
    }
}
