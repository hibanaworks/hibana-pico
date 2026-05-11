use super::*;

pub type GpioWaitMsg = Msg<LABEL_GPIO_WAIT, GpioWait>;
pub type GpioSubscribeMsg = Msg<LABEL_GPIO_SUBSCRIBE, GpioWait>;
pub type GpioEdgeMsg = Msg<LABEL_GPIO_EDGE, GpioEdge>;
pub type GpioWaitRetMsg = Msg<LABEL_GPIO_WAIT_RET, GpioEdge>;
pub type UartWriteMsg = Msg<LABEL_UART_WRITE, UartWrite>;
pub type UartWriteRetMsg = Msg<LABEL_UART_WRITE_RET, UartWriteDone>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerSleepUntil {
    tick: u64,
}

impl TimerSleepUntil {
    pub const fn new(tick: u64) -> Self {
        Self { tick }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }
}

impl WireEncode for TimerSleepUntil {
    fn encoded_len(&self) -> Option<usize> {
        Some(8)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 8 {
            return Err(CodecError::Truncated);
        }
        out[..8].copy_from_slice(&self.tick.to_be_bytes());
        Ok(8)
    }
}

impl WirePayload for TimerSleepUntil {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 8 {
            return Err(CodecError::Invalid(
                "timer sleep request carries eight bytes",
            ));
        }
        let mut tick = [0u8; 8];
        tick.copy_from_slice(bytes);
        Ok(Self::new(u64::from_be_bytes(tick)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerSleepDone {
    tick: u64,
}

impl TimerSleepDone {
    pub const fn new(tick: u64) -> Self {
        Self { tick }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }
}

impl WireEncode for TimerSleepDone {
    fn encoded_len(&self) -> Option<usize> {
        Some(8)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 8 {
            return Err(CodecError::Truncated);
        }
        out[..8].copy_from_slice(&self.tick.to_be_bytes());
        Ok(8)
    }
}

impl WirePayload for TimerSleepDone {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 8 {
            return Err(CodecError::Invalid("timer sleep reply carries eight bytes"));
        }
        let mut tick = [0u8; 8];
        tick.copy_from_slice(bytes);
        Ok(Self::new(u64::from_be_bytes(tick)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpioSet {
    pin: u8,
    high: bool,
}

impl GpioSet {
    pub const fn new(pin: u8, high: bool) -> Self {
        Self { pin, high }
    }

    pub const fn pin(&self) -> u8 {
        self.pin
    }

    pub const fn high(&self) -> bool {
        self.high
    }

    pub const fn from_wasm_value(value: u32) -> Self {
        Self::new((value & 0xff) as u8, (value & 0x100) != 0)
    }
}

impl WireEncode for GpioSet {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.pin;
        out[1] = self.high as u8;
        Ok(2)
    }
}

impl WirePayload for GpioSet {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("gpio set carries two bytes"));
        }
        match bytes[1] {
            0 => Ok(Self::new(bytes[0], false)),
            1 => Ok(Self::new(bytes[0], true)),
            _ => Err(CodecError::Invalid("gpio level must be 0 or 1")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpioWait {
    fd: u8,
    wait_id: u16,
    pin: u8,
    generation: u16,
}

impl GpioWait {
    pub const fn new(fd: u8, wait_id: u16, pin: u8, generation: u16) -> Self {
        Self {
            fd,
            wait_id,
            pin,
            generation,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn wait_id(&self) -> u16 {
        self.wait_id
    }

    pub const fn pin(&self) -> u8 {
        self.pin
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }
}

impl WireEncode for GpioWait {
    fn encoded_len(&self) -> Option<usize> {
        Some(6)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 6 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.wait_id.to_be_bytes());
        out[3] = self.pin;
        out[4..6].copy_from_slice(&self.generation.to_be_bytes());
        Ok(6)
    }
}

impl WirePayload for GpioWait {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 6 {
            return Err(CodecError::Invalid("gpio wait carries six bytes"));
        }
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            bytes[3],
            u16::from_be_bytes([bytes[4], bytes[5]]),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpioEdge {
    wait_id: u16,
    pin: u8,
    high: bool,
    generation: u16,
}

impl GpioEdge {
    pub const fn new(wait_id: u16, pin: u8, high: bool, generation: u16) -> Self {
        Self {
            wait_id,
            pin,
            high,
            generation,
        }
    }

    pub const fn wait_id(&self) -> u16 {
        self.wait_id
    }

    pub const fn pin(&self) -> u8 {
        self.pin
    }

    pub const fn high(&self) -> bool {
        self.high
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }
}

impl WireEncode for GpioEdge {
    fn encoded_len(&self) -> Option<usize> {
        Some(6)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 6 {
            return Err(CodecError::Truncated);
        }
        out[0..2].copy_from_slice(&self.wait_id.to_be_bytes());
        out[2] = self.pin;
        out[3] = self.high as u8;
        out[4..6].copy_from_slice(&self.generation.to_be_bytes());
        Ok(6)
    }
}

impl WirePayload for GpioEdge {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 6 {
            return Err(CodecError::Invalid("gpio edge carries six bytes"));
        }
        let high = match bytes[3] {
            0 => false,
            1 => true,
            _ => return Err(CodecError::Invalid("gpio edge level must be 0 or 1")),
        };
        Ok(Self::new(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            bytes[2],
            high,
            u16::from_be_bytes([bytes[4], bytes[5]]),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UartWrite {
    len: u8,
    bytes: [u8; UART_WRITE_CHUNK_CAPACITY],
}

impl UartWrite {
    pub fn new(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > UART_WRITE_CHUNK_CAPACITY {
            return Err(CodecError::Invalid("uart write exceeds fixed capacity"));
        }
        let mut out = [0u8; UART_WRITE_CHUNK_CAPACITY];
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            len: bytes.len() as u8,
            bytes: out,
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
}

impl WireEncode for UartWrite {
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

impl WirePayload for UartWrite {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        let Some((&len, payload)) = bytes.split_first() else {
            return Err(CodecError::Truncated);
        };
        if payload.len() != len as usize {
            return Err(CodecError::Invalid("uart write length mismatch"));
        }
        Self::new(payload)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UartWriteDone {
    written: u8,
}

impl UartWriteDone {
    pub const fn new(written: u8) -> Self {
        Self { written }
    }

    pub const fn written(&self) -> u8 {
        self.written
    }
}

impl WireEncode for UartWriteDone {
    fn encoded_len(&self) -> Option<usize> {
        Some(1)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let Some(first) = out.first_mut() else {
            return Err(CodecError::Truncated);
        };
        *first = self.written;
        Ok(1)
    }
}

impl WirePayload for UartWriteDone {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 1 {
            return Err(CodecError::Invalid("uart write reply carries one byte"));
        }
        Ok(Self::new(bytes[0]))
    }
}
