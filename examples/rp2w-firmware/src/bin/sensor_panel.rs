#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use hibana::{
    g,
    runtime::{
        program::Projectable,
        wire::{CodecError, Payload, WireEncode, WirePayload},
    },
};
use hibana_pico::appkit;
use hibana_wasip1_runtime::protocol::{
    EngineReq, EngineRet, FdRead, FdReadDone, FdWrite, FdWriteDone, LABEL_WASI_FD_READ,
    LABEL_WASI_FD_READ_RET, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
    LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
    LABEL_WASI_PROC_EXIT, PathOpen, PathOpened, PollReady,
};
#[cfg(feature = "embed-cyw43-artifacts")]
use hibana_wifi::proto::udp::parse_udp_ipv4_packet;
use hibana_wifi::proto::{
    ethernet::Ipv4Addr,
    protocol::{labels as wifi_labels, roles as wifi_roles},
    udp::UNO_Q_SENSOR_UDP_PORT,
};
use rp2w_firmware::{Rp2wCapsuleFacts, Rp2wPlacement};
use uno_q_heterogeneous::protocol::{
    PICO2W_SENSOR_SAMPLE_BYTES, Pico2wSensorSample, decode_pico2w_sensor_udp_ack,
};

const DEVICE_PREOPEN_FD: u8 = 9;
const SENSOR_SAMPLE_FD: u8 = 3;
const DISPLAY_FD: u8 = 4;
const UNO_Q_UDP_FD: u8 = 5;
const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const EXPECTED_POLL_TIMEOUT_MS: u64 = 1_000;
const READY_CYCLES: u32 = 1;
#[cfg(feature = "embed-cyw43-artifacts")]
const RP2W_WIFI_LOCAL_MAC: [u8; 6] = parse_mac_const(env!(
    "RP2W_SENSOR_PANEL_LOCAL_MAC",
    "set RP2W_SENSOR_PANEL_LOCAL_MAC, for example 02:12:34:56:78:9a"
));
#[cfg(feature = "embed-cyw43-artifacts")]
const UNO_Q_WIFI_MAC: [u8; 6] = parse_mac_const(env!(
    "RP2W_SENSOR_PANEL_UNO_Q_MAC",
    "set RP2W_SENSOR_PANEL_UNO_Q_MAC to the Uno Q wlan0 MAC"
));
#[cfg(feature = "embed-cyw43-artifacts")]
const RP2W_WIFI_LOCAL_IP: [u8; 4] = parse_ipv4_const(env!(
    "RP2W_SENSOR_PANEL_LOCAL_IP",
    "set RP2W_SENSOR_PANEL_LOCAL_IP to a Pico 2 W source IP on the hotspot subnet"
));
#[cfg(feature = "embed-cyw43-artifacts")]
const UNO_Q_WIFI_IP: [u8; 4] = parse_ipv4_const(env!(
    "RP2W_SENSOR_PANEL_UNO_Q_IP",
    "set RP2W_SENSOR_PANEL_UNO_Q_IP to the Uno Q wlan0 IPv4 address"
));
#[cfg(not(feature = "embed-cyw43-artifacts"))]
const UNO_Q_WIFI_IP: [u8; 4] = [127, 0, 0, 1];
const UNO_Q_SENSOR_UDP_PATH: &[u8] = b"device/rp2w/udp/uno-q";
#[cfg(feature = "embed-cyw43-artifacts")]
const RP2W_WIFI_SSID: &[u8] = env!(
    "RP2W_SENSOR_PANEL_WIFI_SSID",
    "set RP2W_SENSOR_PANEL_WIFI_SSID to the hotspot SSID"
)
.as_bytes();
#[cfg(feature = "embed-cyw43-artifacts")]
const RP2W_WIFI_KEY: &[u8] = env!(
    "RP2W_SENSOR_PANEL_WIFI_KEY",
    "set RP2W_SENSOR_PANEL_WIFI_KEY to the hotspot passphrase"
)
.as_bytes();

#[cfg(any(feature = "embed-cyw43-artifacts", test))]
const fn parse_ipv4_const(input: &str) -> [u8; 4] {
    let bytes = input.as_bytes();
    let mut out = [0u8; 4];
    let mut out_i = 0usize;
    let mut i = 0usize;
    let mut value = 0u16;
    let mut digits = 0u8;

    while i <= bytes.len() {
        if i == bytes.len() || bytes[i] == b'.' {
            if digits == 0 || out_i >= out.len() {
                panic!("invalid IPv4 literal");
            }
            out[out_i] = value as u8;
            out_i += 1;
            value = 0;
            digits = 0;
            i += 1;
            continue;
        }
        let byte = bytes[i];
        if byte < b'0' || byte > b'9' {
            panic!("invalid IPv4 literal");
        }
        value = value * 10 + (byte - b'0') as u16;
        if value > 255 || digits == 3 {
            panic!("invalid IPv4 literal");
        }
        digits += 1;
        i += 1;
    }

    if out_i != out.len() {
        panic!("invalid IPv4 literal");
    }
    out
}

#[cfg(any(feature = "embed-cyw43-artifacts", test))]
const fn parse_mac_const(input: &str) -> [u8; 6] {
    let bytes = input.as_bytes();
    let mut out = [0u8; 6];
    let mut out_i = 0usize;
    let mut i = 0usize;

    while out_i < out.len() {
        if i + 1 >= bytes.len() {
            panic!("invalid MAC literal");
        }
        out[out_i] = (parse_hex_nibble(bytes[i]) << 4) | parse_hex_nibble(bytes[i + 1]);
        i += 2;
        out_i += 1;
        if out_i < out.len() {
            if i >= bytes.len() || bytes[i] != b':' {
                panic!("invalid MAC literal");
            }
            i += 1;
        }
    }

    if i != bytes.len() {
        panic!("invalid MAC literal");
    }
    out
}

#[cfg(any(feature = "embed-cyw43-artifacts", test))]
const fn parse_hex_nibble(byte: u8) -> u8 {
    if byte >= b'0' && byte <= b'9' {
        byte - b'0'
    } else if byte >= b'a' && byte <= b'f' {
        byte - b'a' + 10
    } else if byte >= b'A' && byte <= b'F' {
        byte - b'A' + 10
    } else {
        panic!("invalid MAC literal");
    }
}
const WIFI_TRACE_BOOT_ATTEMPT: u32 = 0x5749_1000;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_WIFI_SEND_OK: u32 = 0x5749_4200;
#[cfg(not(feature = "embed-cyw43-artifacts"))]
const WIFI_TRACE_BOOT_ERR: u32 = 0x5749_10ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_BUS_TIMEOUT: u32 = 0x5749_1101;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_BUS_UNAVAILABLE: u32 = 0x5749_1102;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_BUFFER_TOO_SMALL: u32 = 0x5749_1201;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_TRANSFER_TOO_LARGE: u32 = 0x5749_1202;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_UNALIGNED_TRANSFER: u32 = 0x5749_1203;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_GSPI_TEST_MISMATCH: u32 = 0x5749_13ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_BACKPLANE_CLOCK_TIMEOUT: u32 = 0x5749_30ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_HT_CLOCK_TIMEOUT: u32 = 0x5749_38ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_F2_READY_TIMEOUT: u32 = 0x5749_39ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_CORE_NOT_IN_RESET: u32 = 0x5749_33ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_CORE_NOT_UP: u32 = 0x5749_3aff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_F2_PACKET_TIMEOUT: u32 = 0x5749_3cff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_IOCTL_TIMEOUT: u32 = 0x5749_3dff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_IOCTL_STATUS: u32 = 0x5749_3eff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_IOCTL_RESPONSE_ERR: u32 = 0x5749_3fff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_TX_OK: u32 = 0x5749_2000;
const WIFI_TRACE_TX_ERR: u32 = 0x5749_20ff;
const WIFI_TRACE_TX_REQ: u32 = 0x5749_2100;
const WIFI_TRACE_TX_ROLE_READY: u32 = 0x5749_2200;
const WIFI_TRACE_RX_REQ: u32 = 0x5749_2300;
const WIFI_TRACE_RX_READY: u32 = 0x5749_2400;
const WIFI_TRACE_RX_PENDING: u32 = 0x5749_24ff;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_ARP_REPLY: u32 = 0x5749_2500;
const WASI_ERRNO_IO: u16 = 5;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_RX_EMPTY_POLL_BUDGET: usize = 16;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_RX_FRAME_BUDGET: usize = 4;

const SENSOR_WIFI_UDP_PAYLOAD_BYTES: usize = rp2w_firmware::RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES;
const SENSOR_WIFI_DATAGRAM_HEADER_BYTES: usize = 10;
const SENSOR_WIFI_PACKET_HEADER_BYTES: usize = 14;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SensorWifiPayloadError {
    PayloadTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SensorWifiDatagram {
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload_len: u16,
    payload: [u8; SENSOR_WIFI_UDP_PAYLOAD_BYTES],
}

impl SensorWifiDatagram {
    const fn empty(dst_ip: Ipv4Addr, src_port: u16, dst_port: u16) -> Self {
        Self {
            dst_ip,
            src_port,
            dst_port,
            payload_len: 0,
            payload: [0; SENSOR_WIFI_UDP_PAYLOAD_BYTES],
        }
    }

    fn new(
        dst_ip: Ipv4Addr,
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
    ) -> Result<Self, SensorWifiPayloadError> {
        if payload.len() > SENSOR_WIFI_UDP_PAYLOAD_BYTES || payload.len() > u16::MAX as usize {
            return Err(SensorWifiPayloadError::PayloadTooLarge);
        }
        let mut out = Self::empty(dst_ip, src_port, dst_port);
        out.payload[..payload.len()].copy_from_slice(payload);
        out.payload_len = payload.len() as u16;
        Ok(out)
    }

    const fn payload_len(&self) -> usize {
        self.payload_len as usize
    }

    fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len()]
    }
}

impl WireEncode for SensorWifiDatagram {
    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = SENSOR_WIFI_DATAGRAM_HEADER_BYTES + self.payload_len();
        if out.len() < len {
            return Err(CodecError::Truncated);
        }
        out[0..4].copy_from_slice(&self.dst_ip.0);
        out[4..6].copy_from_slice(&self.src_port.to_be_bytes());
        out[6..8].copy_from_slice(&self.dst_port.to_be_bytes());
        out[8..10].copy_from_slice(&self.payload_len.to_be_bytes());
        out[10..len].copy_from_slice(self.payload());
        Ok(len)
    }
}

impl WirePayload for SensorWifiDatagram {
    type Decoded<'a> = Self;

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < SENSOR_WIFI_DATAGRAM_HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        let payload_len = u16::from_be_bytes([bytes[8], bytes[9]]) as usize;
        if payload_len > SENSOR_WIFI_UDP_PAYLOAD_BYTES {
            return Err(CodecError::Malformed);
        }
        if bytes.len() != SENSOR_WIFI_DATAGRAM_HEADER_BYTES + payload_len {
            return Err(CodecError::Malformed);
        }
        Ok(())
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        let bytes = input.as_bytes();
        let payload_len = u16::from_be_bytes([bytes[8], bytes[9]]) as usize;
        let mut payload = [0u8; SENSOR_WIFI_UDP_PAYLOAD_BYTES];
        payload[..payload_len].copy_from_slice(&bytes[10..10 + payload_len]);
        Self {
            dst_ip: Ipv4Addr([bytes[0], bytes[1], bytes[2], bytes[3]]),
            src_port: u16::from_be_bytes([bytes[4], bytes[5]]),
            dst_port: u16::from_be_bytes([bytes[6], bytes[7]]),
            payload_len: payload_len as u16,
            payload,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SensorWifiPacket {
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload_len: u16,
    payload: [u8; SENSOR_WIFI_UDP_PAYLOAD_BYTES],
}

impl SensorWifiPacket {
    #[cfg(feature = "embed-cyw43-artifacts")]
    fn from_proto(
        packet: hibana_wifi::proto::udp::UdpPacket<{ SENSOR_WIFI_UDP_PAYLOAD_BYTES }>,
    ) -> Self {
        let mut out = Self {
            src_ip: packet.src_ip(),
            dst_ip: packet.dst_ip(),
            src_port: packet.src_port(),
            dst_port: packet.dst_port(),
            payload_len: packet.payload_len() as u16,
            payload: [0; SENSOR_WIFI_UDP_PAYLOAD_BYTES],
        };
        out.payload[..packet.payload_len()].copy_from_slice(packet.payload());
        out
    }

    const fn payload_len(&self) -> usize {
        self.payload_len as usize
    }

    fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len()]
    }
}

impl WireEncode for SensorWifiPacket {
    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = SENSOR_WIFI_PACKET_HEADER_BYTES + self.payload_len();
        if out.len() < len {
            return Err(CodecError::Truncated);
        }
        out[0..4].copy_from_slice(&self.src_ip.0);
        out[4..8].copy_from_slice(&self.dst_ip.0);
        out[8..10].copy_from_slice(&self.src_port.to_be_bytes());
        out[10..12].copy_from_slice(&self.dst_port.to_be_bytes());
        out[12..14].copy_from_slice(&self.payload_len.to_be_bytes());
        out[14..len].copy_from_slice(self.payload());
        Ok(len)
    }
}

impl WirePayload for SensorWifiPacket {
    type Decoded<'a> = Self;

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < SENSOR_WIFI_PACKET_HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        let payload_len = u16::from_be_bytes([bytes[12], bytes[13]]) as usize;
        if payload_len > SENSOR_WIFI_UDP_PAYLOAD_BYTES {
            return Err(CodecError::Malformed);
        }
        if bytes.len() != SENSOR_WIFI_PACKET_HEADER_BYTES + payload_len {
            return Err(CodecError::Malformed);
        }
        Ok(())
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        let bytes = input.as_bytes();
        let payload_len = u16::from_be_bytes([bytes[12], bytes[13]]) as usize;
        let mut payload = [0u8; SENSOR_WIFI_UDP_PAYLOAD_BYTES];
        payload[..payload_len].copy_from_slice(&bytes[14..14 + payload_len]);
        Self {
            src_ip: Ipv4Addr([bytes[0], bytes[1], bytes[2], bytes[3]]),
            dst_ip: Ipv4Addr([bytes[4], bytes[5], bytes[6], bytes[7]]),
            src_port: u16::from_be_bytes([bytes[8], bytes[9]]),
            dst_port: u16::from_be_bytes([bytes[10], bytes[11]]),
            payload_len: payload_len as u16,
            payload,
        }
    }
}

type WifiTxUdpMsg = g::Msg<{ wifi_labels::TX_UDP }, SensorWifiDatagram>;
type WifiTxDoneMsg = g::Msg<{ wifi_labels::TX_DONE }, u32>;
type WifiTxErrMsg = g::Msg<{ wifi_labels::TX_ERR }, u32>;
type WifiRxPollMsg = g::Msg<{ wifi_labels::RX_UDP_POLL }, u16>;
type WifiRxPacketMsg = g::Msg<{ wifi_labels::RX_UDP }, SensorWifiPacket>;
type WifiRxPendingMsg = g::Msg<{ wifi_labels::RX_PENDING }, u32>;
type WifiPowerOnMsg = g::Msg<{ wifi_labels::POWER_ON }, u32>;
type WifiPowerReadyMsg = g::Msg<{ wifi_labels::POWER_READY }, u32>;
type WifiGspiProbeMsg = g::Msg<{ wifi_labels::GSPI_PROBE }, u32>;
type WifiGspiReadyMsg = g::Msg<{ wifi_labels::GSPI_READY }, u32>;
type WifiBackplaneInitMsg = g::Msg<{ wifi_labels::BACKPLANE_INIT }, u32>;
type WifiBackplaneReadyMsg = g::Msg<{ wifi_labels::BACKPLANE_READY }, u32>;
type WifiFirmwareLoadMsg = g::Msg<{ wifi_labels::FIRMWARE_LOAD }, u32>;
type WifiFirmwareLoadedMsg = g::Msg<{ wifi_labels::FIRMWARE_LOADED }, u32>;
type WifiNvramLoadMsg = g::Msg<{ wifi_labels::NVRAM_LOAD }, u32>;
type WifiNvramLoadedMsg = g::Msg<{ wifi_labels::NVRAM_LOADED }, u32>;
type WifiFirmwareStartMsg = g::Msg<{ wifi_labels::FIRMWARE_START }, u32>;
type WifiFirmwareStartedMsg = g::Msg<{ wifi_labels::FIRMWARE_STARTED }, u32>;
type WifiClmLoadMsg = g::Msg<{ wifi_labels::CLM_LOAD }, u32>;
type WifiClmDoneMsg = g::Msg<{ wifi_labels::CLM_DONE }, u32>;
type WifiCdcUpMsg = g::Msg<{ wifi_labels::CDC_UP }, u32>;
type WifiCdcUpDoneMsg = g::Msg<{ wifi_labels::CDC_UP_DONE }, u32>;
type WifiJoinMsg = g::Msg<{ wifi_labels::JOIN }, u32>;
type WifiJoinDoneMsg = g::Msg<{ wifi_labels::JOIN_DONE }, u32>;
type WifiLinkPollMsg = g::Msg<{ wifi_labels::LINK_POLL }, u32>;
type WifiLinkUpMsg = g::Msg<{ wifi_labels::LINK_UP }, u32>;

static mut SENSOR_UDP_SEQ: u16 = 0;

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_SENSOR_PANEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/rp2w-sensor-panel-guest.wasm"
));
#[cfg(not(feature = "embed-wasip1-artifacts"))]
const WASM_SENSOR_PANEL: &[u8] = &[];

#[cfg(feature = "embed-cyw43-artifacts")]
const CYW43_FIRMWARE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../firmware/cyw43/w43439A0_7_95_49_00_firmware.bin"
));

#[cfg(feature = "embed-cyw43-artifacts")]
const CYW43_CLM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../firmware/cyw43/w43439A0_7_95_49_00_clm.bin"
));

#[cfg(feature = "embed-cyw43-artifacts")]
const CYW43_NVRAM: &[u8] = concat!(
    "NVRAMRev=$Rev$\0",
    "manfid=0x2d0\0",
    "prodid=0x0727\0",
    "vendid=0x14e4\0",
    "devid=0x43e2\0",
    "boardtype=0x0887\0",
    "boardrev=0x1100\0",
    "boardnum=22\0",
    "macaddr=00:A0:50:b5:59:5e\0",
    "sromrev=11\0",
    "boardflags=0x00404001\0",
    "boardflags3=0x04000000\0",
    "xtalfreq=37400\0",
    "nocrc=1\0",
    "ag0=255\0",
    "aa2g=1\0",
    "ccode=ALL\0",
    "pa0itssit=0x20\0",
    "extpagain2g=0\0",
    "pa2ga0=-168,7161,-820\0",
    "AvVmid_c0=0x0,0xc8\0",
    "cckpwroffset0=5\0",
    "maxp2ga0=84\0",
    "txpwrbckof=6\0",
    "cckbw202gpo=0\0",
    "legofdmbw202gpo=0x66111111\0",
    "mcsbw202gpo=0x77711111\0",
    "propbw202gpo=0xdd\0",
    "ofdmdigfilttype=18\0",
    "ofdmdigfilttypebe=18\0",
    "papdmode=1\0",
    "papdvalidtest=1\0",
    "pacalidx2g=45\0",
    "papdepsoffset=-30\0",
    "papdendidx=58\0",
    "ltecxmux=0\0",
    "ltecxpadnum=0x0102\0",
    "ltecxfnsel=0x44\0",
    "ltecxgcigpio=0x01\0",
    "il0macaddr=00:90:4c:c5:12:38\0",
    "wl0id=0x431b\0",
    "deadman_to=0xffffffff\0",
    "muxenab=0x100\0",
    "spurconfig=0x3\0",
    "glitch_based_crsmin=1\0",
    "btc_mode=1\0",
    "\0\0\0",
)
.as_bytes();

const SENSOR_SAMPLE: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/rp2w/sample",
    appkit::ObjectId(1),
    appkit::FdSpec::new(SENSOR_SAMPLE_FD as u32, FD_READ_RIGHT, 1),
);
const DISPLAY: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/rp2w/display",
    appkit::ObjectId(2),
    appkit::FdSpec::new(DISPLAY_FD as u32, FD_WRITE_RIGHT, 1),
);
const UNO_Q_UDP: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    UNO_Q_SENSOR_UDP_PATH,
    appkit::ObjectId(3),
    appkit::FdSpec::new(UNO_Q_UDP_FD as u32, FD_WRITE_RIGHT, 1),
);

static OBJECT_FACTS: appkit::ChoreoFsObjectSet<3> =
    appkit::ChoreoFsObjectSet::new([SENSOR_SAMPLE, DISPLAY, UNO_Q_UDP]);

struct SensorPanel;
struct SensorPanelLocal;

#[derive(Debug)]
enum SensorPanelError {
    Endpoint(hibana::EndpointError),
    Wire(hibana::runtime::wire::CodecError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for SensorPanelError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<hibana::runtime::wire::CodecError> for SensorPanelError {
    fn from(error: hibana::runtime::wire::CodecError) -> Self {
        Self::Wire(error)
    }
}

impl appkit::Capsule for SensorPanel {
    type Placement = Rp2wPlacement;
    type Local = SensorPanelLocal;

    fn choreography() -> impl Projectable {
        let path_open = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>(),
            )
        };
        let fd_read = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_FD_READ, EngineReq>>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>>(),
            )
        };
        let fd_write_req = || g::send::<1, 0, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>>();
        let fd_write_ret = || g::send::<0, 1, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>();
        let fd_write = || g::seq(fd_write_req(), fd_write_ret());
        let cyw43_bringup = || {
            let power = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiPowerOnMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiPowerReadyMsg,
                    >(),
                )
            };
            let gspi = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiGspiProbeMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiGspiReadyMsg,
                    >(),
                )
            };
            let backplane = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiBackplaneInitMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiBackplaneReadyMsg,
                    >(),
                )
            };
            let firmware = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiFirmwareLoadMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiFirmwareLoadedMsg,
                    >(),
                )
            };
            let nvram = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiNvramLoadMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiNvramLoadedMsg,
                    >(),
                )
            };
            let start = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiFirmwareStartMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiFirmwareStartedMsg,
                    >(),
                )
            };
            g::seq(
                power(),
                g::seq(
                    gspi(),
                    g::seq(backplane(), g::seq(firmware(), g::seq(nvram(), start()))),
                ),
            )
        };
        let cyw43_join = || {
            let clm = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiClmLoadMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiClmDoneMsg,
                    >(),
                )
            };
            let up = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiCdcUpMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiCdcUpDoneMsg,
                    >(),
                )
            };
            let join = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiJoinMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiJoinDoneMsg,
                    >(),
                )
            };
            let link = || {
                g::seq(
                    g::send::<
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        { wifi_roles::CYW43_DRIVER },
                        WifiLinkPollMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiLinkUpMsg,
                    >(),
                )
            };
            g::seq(clm(), g::seq(up(), g::seq(join(), link())))
        };
        let wifi_udp_tx = || {
            g::seq(
                g::send::<
                    { wifi_roles::CHOREOGRAPHIC_KERNEL },
                    { wifi_roles::CYW43_DRIVER },
                    WifiTxUdpMsg,
                >(),
                g::route(
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiTxDoneMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiTxErrMsg,
                    >(),
                ),
            )
        };
        let wifi_udp_rx = || {
            g::seq(
                g::send::<
                    { wifi_roles::CHOREOGRAPHIC_KERNEL },
                    { wifi_roles::CYW43_DRIVER },
                    WifiRxPollMsg,
                >(),
                g::route(
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiRxPacketMsg,
                    >(),
                    g::send::<
                        { wifi_roles::CYW43_DRIVER },
                        { wifi_roles::CHOREOGRAPHIC_KERNEL },
                        WifiRxPendingMsg,
                    >(),
                ),
            )
        };
        let fd_write_after_wifi_ack = || {
            g::seq(
                fd_write_req(),
                g::seq(wifi_udp_tx(), g::seq(wifi_udp_rx(), fd_write_ret())),
            )
        };
        let poll = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>(),
            )
        };
        let sample_cycle = || {
            g::seq(
                fd_read(),
                g::seq(fd_write(), g::seq(fd_write_after_wifi_ack(), poll())),
            )
        };
        let admitted_cycle = || {
            g::route(
                sample_cycle(),
                g::send::<1, 0, g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>>(),
            )
            .roll()
        };
        g::seq(
            cyw43_bringup(),
            g::seq(
                cyw43_join(),
                g::seq(
                    path_open(),
                    g::seq(path_open(), g::seq(path_open(), admitted_cycle())),
                ),
            ),
        )
    }
}

impl Rp2wCapsuleFacts for SensorPanel {
    const DRIVER_REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(
        (1u16 << wifi_roles::CHOREOGRAPHIC_KERNEL) | (1u16 << wifi_roles::CYW43_DRIVER),
    );

    fn run_engine_image() {
        rp2w_firmware::run_engine_wasi::<Self>(appkit::WasiImage::from_static(WASM_SENSOR_PANEL));
    }

    fn driver_facts() -> appkit::DriverFacts<'static> {
        OBJECT_FACTS.driver_facts()
    }
}

impl appkit::Localside<SensorPanel> for SensorPanelLocal {
    type Error = SensorPanelError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 && !ctx.choreofs().entries().is_empty() {
                rp2w_firmware::reset_choreofs_markers();
                rp2w_firmware::record_choreofs_engine_status(
                    rp2w_firmware::CHOREOFS_DRIVER_STARTED,
                );
                rp2w_firmware::rp2w_board_init();
                rp2w_firmware::record_choreofs_engine_status(rp2w_firmware::CHOREOFS_GPIO_READY);

                driver_cyw43_bootstrap(&mut ctx).await?;
                driver_path_open(&mut ctx, SENSOR_SAMPLE_FD, SENSOR_SAMPLE.object()).await?;
                driver_path_open(&mut ctx, DISPLAY_FD, DISPLAY.object()).await?;
                driver_path_open(&mut ctx, UNO_Q_UDP_FD, UNO_Q_UDP.object()).await?;

                let mut cycles = 0u32;
                loop {
                    if !driver_admit_cycle(&mut ctx).await? {
                        break;
                    }
                    driver_fd_write_display(&mut ctx).await?;
                    driver_fd_write_uno_q(&mut ctx).await?;
                    driver_poll_oneoff(&mut ctx).await?;
                    cycles = cycles.saturating_add(1);
                    if cycles == READY_CYCLES {
                        rp2w_firmware::mark_runtime_ready();
                        rp2w_firmware::mark_success(
                            <SensorPanel as Rp2wCapsuleFacts>::SUCCESS_RESULT,
                        );
                    }
                }
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        mut ctx: appkit::BoundaryCtx<'a, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == wifi_roles::CYW43_DRIVER {
                rp2w_firmware::rp2w_board_init();
                rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ROLE_READY);
                let mut wifi = cyw43_boundary_bootstrap(&mut ctx).await?;
                loop {
                    cyw43_boundary_tx_udp(&mut ctx, &mut wifi).await?;
                    cyw43_boundary_rx_udp(&mut ctx, &mut wifi).await?;
                }
            }
            ctx.pending().await
        }
    }
}

struct UnoQWifiSender {
    #[cfg(feature = "embed-cyw43-artifacts")]
    driver: Option<rp2w_firmware::Rp2wCyw43GspiDriver>,
    #[cfg(feature = "embed-cyw43-artifacts")]
    sequence: u8,
}

impl UnoQWifiSender {
    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    fn unavailable() -> Self {
        Self {
            #[cfg(feature = "embed-cyw43-artifacts")]
            driver: None,
            #[cfg(feature = "embed-cyw43-artifacts")]
            sequence: 0,
        }
    }

    #[cfg(feature = "embed-cyw43-artifacts")]
    fn ready(driver: rp2w_firmware::Rp2wCyw43GspiDriver) -> Self {
        Self {
            driver: Some(driver),
            sequence: 0,
        }
    }

    #[cfg(feature = "embed-cyw43-artifacts")]
    fn send_datagram(&mut self, datagram: &SensorWifiDatagram) -> Result<usize, ()> {
        let Some(driver) = self.driver.as_mut() else {
            core::hint::black_box(datagram);
            return Err(());
        };
        let mut ethernet_frame = [0u8; 256];
        let mut scratch = [0u8; 1536];
        match rp2w_firmware::rp2w_cyw43_send_uno_q_payload_frame(
            driver,
            uno_q_wifi_target(),
            datagram.payload(),
            &mut ethernet_frame,
            &mut scratch,
        ) {
            Ok(_) => {
                rp2w_firmware::record_choreofs_driver_trace(
                    WIFI_TRACE_TX_OK | u32::from(self.sequence),
                );
                lcd_udp_sent(self.sequence, datagram.payload());
                self.sequence = self.sequence.wrapping_add(1);
                Ok(datagram.payload_len())
            }
            Err(error) => {
                rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ERR);
                rp2w_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
                lcd_network_status(b"UDP tx failed", b"retry next cycle");
                self.driver = None;
                Err(())
            }
        }
    }

    #[cfg(feature = "embed-cyw43-artifacts")]
    fn recv_packet(&mut self, local_port: u16) -> Result<Option<SensorWifiPacket>, ()> {
        let Some(driver) = self.driver.as_mut() else {
            core::hint::black_box(local_port);
            return Err(());
        };
        let target = uno_q_wifi_target();
        let mut ethernet_frame = [0u8; 256];
        let mut scratch = [0u8; 1536];
        let mut empty_polls = 0usize;
        let mut frames = 0usize;
        while empty_polls < WIFI_RX_EMPTY_POLL_BUDGET && frames < WIFI_RX_FRAME_BUDGET {
            let Some(len) = driver
                .recv_ethernet_frame(&mut ethernet_frame, &mut scratch)
                .map_err(|error| {
                    rp2w_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
                })?
            else {
                empty_polls += 1;
                continue;
            };
            empty_polls = 0;
            frames += 1;
            let frame = &ethernet_frame[..len];
            if let Some(packet) = parse_udp_ipv4_packet::<
                { rp2w_firmware::RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES },
            >(frame, target.local_mac, target.local_ip, local_port)
            {
                return Ok(Some(SensorWifiPacket::from_proto(packet)));
            }
            service_arp_ipv4_request(driver, frame, target, &mut scratch).map_err(|error| {
                rp2w_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
            })?;
        }
        Ok(None)
    }

    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    fn recv_packet(&mut self, local_port: u16) -> Result<Option<SensorWifiPacket>, ()> {
        core::hint::black_box(local_port);
        Ok(None)
    }

    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    fn send_datagram(&mut self, datagram: &SensorWifiDatagram) -> Result<usize, ()> {
        core::hint::black_box(datagram);
        lcd_network_status(b"UDP disabled", b"build feature");
        Err(())
    }
}

fn lcd_network_status(line1: &[u8], line2: &[u8]) {
    let _ = rp2w_firmware::rp2w_lcd_write_lines(line1, line2);
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn lcd_udp_sent(sequence: u8, payload: &[u8]) {
    core::hint::black_box(sequence);
    let _ = rp2w_firmware::rp2w_lcd_write_pico2w_sensor_sample(payload);
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn wifi_gspi_error_trace(error: rp2w_firmware::Rp2wCyw43GspiError) -> u32 {
    match error {
        hibana_wifi::cyw43::gspi::Cyw43GspiError::Bus(
            rp2w_firmware::Rp2wCyw43SpiError::Timeout,
        ) => WIFI_TRACE_GSPI_BUS_TIMEOUT,
        hibana_wifi::cyw43::gspi::Cyw43GspiError::Bus(
            rp2w_firmware::Rp2wCyw43SpiError::Unavailable,
        ) => WIFI_TRACE_GSPI_BUS_UNAVAILABLE,
        hibana_wifi::cyw43::gspi::Cyw43GspiError::TestPatternMismatch(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_GSPI_TEST_MISMATCH ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::BackplaneClockTimeout(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_BACKPLANE_CLOCK_TIMEOUT ^ u32::from(value)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::HtClockTimeout(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_HT_CLOCK_TIMEOUT ^ u32::from(value)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::F2ReadyTimeout(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_F2_READY_TIMEOUT ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::F2PacketTimeout(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_F2_PACKET_TIMEOUT ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::IoctlTimeout(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_IOCTL_TIMEOUT ^ u32::from(value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::IoctlStatus(value) => {
            rp2w_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_IOCTL_STATUS ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::InvalidIoctlResponse => {
            WIFI_TRACE_IOCTL_RESPONSE_ERR
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::CoreNotInReset(core_id, resetctrl) => {
            rp2w_firmware::record_choreofs_engine_error_code(
                (u32::from(core_id) << 8) | u32::from(resetctrl),
            );
            WIFI_TRACE_CORE_NOT_IN_RESET ^ u32::from(resetctrl)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::CoreNotUp(core_id, ioctrl, resetctrl) => {
            rp2w_firmware::record_choreofs_engine_error_code(
                (u32::from(core_id) << 16) | (u32::from(ioctrl) << 8) | u32::from(resetctrl),
            );
            WIFI_TRACE_CORE_NOT_UP ^ u32::from(resetctrl)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall => {
            WIFI_TRACE_GSPI_BUFFER_TOO_SMALL
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge => {
            WIFI_TRACE_GSPI_TRANSFER_TOO_LARGE
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::UnalignedTransfer => {
            WIFI_TRACE_GSPI_UNALIGNED_TRANSFER
        }
    }
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn uno_q_wifi_target() -> rp2w_firmware::Rp2wUnoQWifiTarget {
    rp2w_firmware::Rp2wUnoQWifiTarget::new(
        hibana_wifi::proto::ethernet::MacAddr(RP2W_WIFI_LOCAL_MAC),
        hibana_wifi::proto::ethernet::MacAddr(UNO_Q_WIFI_MAC),
        hibana_wifi::proto::ethernet::Ipv4Addr(RP2W_WIFI_LOCAL_IP),
        hibana_wifi::proto::ethernet::Ipv4Addr(UNO_Q_WIFI_IP),
        rp2w_firmware::RP2W_UNO_Q_SENSOR_UDP_SRC_PORT,
    )
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn service_arp_ipv4_request(
    driver: &mut rp2w_firmware::Rp2wCyw43GspiDriver,
    frame: &[u8],
    target: rp2w_firmware::Rp2wUnoQWifiTarget,
    scratch: &mut [u8],
) -> Result<bool, rp2w_firmware::Rp2wCyw43GspiError> {
    let Some((sender_mac, sender_ip)) =
        parse_arp_ipv4_request(frame, target.local_mac, target.local_ip, target.uno_q_ip)
    else {
        return Ok(false);
    };
    let mut reply = [0u8; 64];
    let len = hibana_wifi::proto::ethernet::build_arp_reply(
        &mut reply,
        target.local_mac,
        target.local_ip,
        sender_mac,
        sender_ip,
    )
    .map_err(|_| hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall)?;
    driver.send_ethernet_frame(&reply[..len], scratch)?;
    rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_ARP_REPLY | u32::from(sender_ip.0[3]));
    Ok(true)
}

#[cfg(any(feature = "embed-cyw43-artifacts", test))]
fn parse_arp_ipv4_request(
    frame: &[u8],
    local_mac: hibana_wifi::proto::ethernet::MacAddr,
    local_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
    expected_sender_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
) -> Option<(
    hibana_wifi::proto::ethernet::MacAddr,
    hibana_wifi::proto::ethernet::Ipv4Addr,
)> {
    use hibana_wifi::proto::ethernet::{
        ARP_IPV4_LEN, ETH_HEADER_LEN, ETHERTYPE_ARP, ETHERTYPE_IPV4, MacAddr,
    };

    if frame.len() < ETH_HEADER_LEN + ARP_IPV4_LEN {
        return None;
    }
    let dst_mac = MacAddr([frame[0], frame[1], frame[2], frame[3], frame[4], frame[5]]);
    if dst_mac != local_mac && dst_mac != MacAddr::BROADCAST {
        return None;
    }
    if read_u16_be_local(&frame[12..14]) != ETHERTYPE_ARP {
        return None;
    }
    let arp = &frame[ETH_HEADER_LEN..ETH_HEADER_LEN + ARP_IPV4_LEN];
    if read_u16_be_local(&arp[0..2]) != 1
        || read_u16_be_local(&arp[2..4]) != ETHERTYPE_IPV4
        || arp[4] != 6
        || arp[5] != 4
        || read_u16_be_local(&arp[6..8]) != 1
    {
        return None;
    }
    let sender_mac = MacAddr([arp[8], arp[9], arp[10], arp[11], arp[12], arp[13]]);
    let sender_ip = hibana_wifi::proto::ethernet::Ipv4Addr([arp[14], arp[15], arp[16], arp[17]]);
    let target_ip = hibana_wifi::proto::ethernet::Ipv4Addr([arp[24], arp[25], arp[26], arp[27]]);
    if sender_ip == expected_sender_ip && target_ip == local_ip {
        Some((sender_mac, sender_ip))
    } else {
        None
    }
}

#[cfg(any(feature = "embed-cyw43-artifacts", test))]
fn read_u16_be_local(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

async fn recv_engine_req<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<EngineReq, SensorPanelError> {
    loop {
        match ctx.endpoint().recv::<g::Msg<LABEL, EngineReq>>().await {
            Ok(request) => return Ok(request),
            Err(error) => {
                core::hint::black_box(error);
                rp2w_firmware::rp2w_poll_delay(1);
            }
        }
    }
}

async fn send_engine_ret<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    reply: EngineRet,
) -> Result<(), SensorPanelError> {
    ctx.endpoint()
        .send::<g::Msg<LABEL, EngineRet>>(&reply)
        .await?;
    Ok(())
}

async fn driver_cyw43_bootstrap<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    driver_cyw43_step::<ROLE, WifiPowerOnMsg, WifiPowerReadyMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiGspiProbeMsg, WifiGspiReadyMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiBackplaneInitMsg, WifiBackplaneReadyMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiFirmwareLoadMsg, WifiFirmwareLoadedMsg>(
        ctx,
        cyw43_firmware_len(),
    )
    .await?;
    driver_cyw43_step::<ROLE, WifiNvramLoadMsg, WifiNvramLoadedMsg>(ctx, cyw43_nvram_len()).await?;
    driver_cyw43_step::<ROLE, WifiFirmwareStartMsg, WifiFirmwareStartedMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiClmLoadMsg, WifiClmDoneMsg>(ctx, cyw43_clm_len()).await?;
    driver_cyw43_step::<ROLE, WifiCdcUpMsg, WifiCdcUpDoneMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiJoinMsg, WifiJoinDoneMsg>(ctx, 0).await?;
    driver_cyw43_step::<ROLE, WifiLinkPollMsg, WifiLinkUpMsg>(ctx, 0).await?;
    Ok(())
}

async fn driver_cyw43_step<const ROLE: u8, Req, Ret>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: u32,
) -> Result<u32, SensorPanelError>
where
    Req: hibana::g::Message<Payload = u32>,
    Ret: hibana::g::Message<Payload = u32>,
{
    ctx.endpoint().send::<Req>(&request).await?;
    let reply = ctx.endpoint().recv::<Ret>().await?;
    Ok(reply)
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn cyw43_firmware_len() -> u32 {
    CYW43_FIRMWARE.len() as u32
}

#[cfg(not(feature = "embed-cyw43-artifacts"))]
fn cyw43_firmware_len() -> u32 {
    0
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn cyw43_nvram_len() -> u32 {
    CYW43_NVRAM.len() as u32
}

#[cfg(not(feature = "embed-cyw43-artifacts"))]
fn cyw43_nvram_len() -> u32 {
    0
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn cyw43_clm_len() -> u32 {
    CYW43_CLM.len() as u32
}

#[cfg(not(feature = "embed-cyw43-artifacts"))]
fn cyw43_clm_len() -> u32 {
    0
}

async fn driver_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    expected_fd: u8,
    expected_object: appkit::ObjectId,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_PATH_OPEN>(ctx).await? {
        EngineReq::PathOpen(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_path_open(ctx, request, expected_fd, expected_object).await
}

async fn handle_path_open<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: PathOpen,
    expected_fd: u8,
    expected_object: appkit::ObjectId,
) -> Result<(), SensorPanelError> {
    let expected_rights = if expected_fd == SENSOR_SAMPLE_FD {
        FD_READ_RIGHT
    } else {
        FD_WRITE_RIGHT
    };
    if request.preopen_fd() != DEVICE_PREOPEN_FD || request.rights_base() != expected_rights {
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    let object = match ctx.choreofs().resolve(request.path()) {
        Some(object) => object,
        None => {
            core::hint::black_box(request);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    let fact = match find_ledger_fd(ctx.ledger(), object, request.rights_base()) {
        Some(fact) => fact,
        None => {
            core::hint::black_box((object, request.rights_base()));
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    if fact.fd() != expected_fd as u32 || fact.object() != expected_object {
        core::hint::black_box(fact);
        return Err(SensorPanelError::RuntimeViolation);
    }
    rp2w_firmware::record_choreofs_path_open(object);
    send_engine_ret::<ROLE, LABEL_WASI_PATH_OPEN_RET>(
        ctx,
        EngineRet::PathOpened(PathOpened::new(fact.fd() as u8, 0)),
    )
    .await
}

async fn driver_admit_cycle<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<bool, SensorPanelError> {
    let branch = ctx.endpoint().offer().await?;
    match branch.label() {
        LABEL_WASI_FD_READ => {
            let request = match branch
                .recv::<g::Msg<LABEL_WASI_FD_READ, EngineReq>>()
                .await?
            {
                EngineReq::FdRead(request) => request,
                other => {
                    core::hint::black_box(other);
                    return Err(SensorPanelError::RuntimeViolation);
                }
            };
            handle_fd_read(ctx, request).await?;
            Ok(true)
        }
        LABEL_WASI_PROC_EXIT => {
            let request = branch
                .recv::<g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
                .await?;
            match request {
                EngineReq::ProcExit(status) if status.code() == 0 => Ok(false),
                other => {
                    core::hint::black_box(other);
                    Err(SensorPanelError::RuntimeViolation)
                }
            }
        }
        other => {
            core::hint::black_box(other);
            Err(SensorPanelError::RuntimeViolation)
        }
    }
}

async fn handle_fd_read<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdRead,
) -> Result<(), SensorPanelError> {
    if request.fd() != SENSOR_SAMPLE_FD {
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    let fact = match ctx.ledger().fd(request.fd() as u32) {
        Some(fact) if fact.object() == SENSOR_SAMPLE.object() && fact.rights() == FD_READ_RIGHT => {
            fact
        }
        Some(fact) => {
            core::hint::black_box(fact);
            return Err(SensorPanelError::RuntimeViolation);
        }
        None => {
            core::hint::black_box(request);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    let mut buffer = [0u8; PICO2W_SENSOR_SAMPLE_BYTES];
    let len = rp2w_firmware::rp2w_read_pico2w_sensor_sample(next_sensor_udp_seq(), &mut buffer)
        .map_err(|_| SensorPanelError::RuntimeViolation)?;
    if (request.max_len() as usize) < len {
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    let bytes = &buffer[..len];
    let reply = EngineRet::FdReadDone(FdReadDone::new_with_lease(
        request.fd(),
        request.lease_id(),
        bytes,
    )?);
    rp2w_firmware::record_choreofs_path_open(fact.object());
    send_engine_ret::<ROLE, LABEL_WASI_FD_READ_RET>(ctx, reply).await
}

async fn driver_fd_write_display<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_FD_WRITE>(ctx).await? {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_fd_write_display(ctx, request).await
}

async fn handle_fd_write_display<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdWrite,
) -> Result<(), SensorPanelError> {
    let fact = validate_fd_write(ctx, request, DISPLAY_FD, DISPLAY.object())?;
    let _ = pico2w_sample_from_payload(request.as_bytes())?;
    let _ = rp2w_firmware::rp2w_lcd_write_pico2w_sensor_sample(request.as_bytes());
    rp2w_firmware::record_choreofs_fd_write(fact.object());
    send_engine_ret::<ROLE, LABEL_WASI_FD_WRITE_RET>(
        ctx,
        EngineRet::FdWriteDone(FdWriteDone::new(request.fd(), request.len() as u8)),
    )
    .await
}

async fn driver_fd_write_uno_q<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_FD_WRITE>(ctx).await? {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_fd_write_uno_q(ctx, request).await
}

async fn handle_fd_write_uno_q<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdWrite,
) -> Result<(), SensorPanelError> {
    let fact = validate_fd_write(ctx, request, UNO_Q_UDP_FD, UNO_Q_UDP.object())?;
    let sample = pico2w_sample_from_payload(request.as_bytes())?;
    let payload = request.as_bytes();
    let datagram = match SensorWifiDatagram::new(
        Ipv4Addr(UNO_Q_WIFI_IP),
        rp2w_firmware::RP2W_UNO_Q_SENSOR_UDP_SRC_PORT,
        UNO_Q_SENSOR_UDP_PORT,
        payload,
    ) {
        Ok(datagram) => datagram,
        Err(error) => {
            core::hint::black_box(error);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    ctx.endpoint().send::<WifiTxUdpMsg>(&datagram).await?;

    let branch = ctx.endpoint().offer().await?;
    let tx_ok = match branch.label() {
        wifi_labels::TX_DONE => {
            let result = branch.recv::<WifiTxDoneMsg>().await?;
            if result as usize != payload.len() {
                core::hint::black_box(result);
                return Err(SensorPanelError::RuntimeViolation);
            }
            true
        }
        wifi_labels::TX_ERR => {
            let result = branch.recv::<WifiTxErrMsg>().await?;
            rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ERR | (result & 0xff));
            false
        }
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    let ack_ok = tx_ok && poll_wifi_udp_ack(ctx, sample.seq()).await?;
    let reply = if ack_ok {
        rp2w_firmware::record_choreofs_fd_write(fact.object());
        FdWriteDone::new(request.fd(), request.len() as u8)
    } else {
        FdWriteDone::new_with_errno(request.fd(), 0, WASI_ERRNO_IO)
    };
    send_engine_ret::<ROLE, LABEL_WASI_FD_WRITE_RET>(ctx, EngineRet::FdWriteDone(reply)).await
}

async fn poll_wifi_udp_ack<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    expected_seq: u16,
) -> Result<bool, SensorPanelError> {
    ctx.endpoint()
        .send::<WifiRxPollMsg>(&rp2w_firmware::RP2W_UNO_Q_SENSOR_UDP_SRC_PORT)
        .await?;
    let branch = ctx.endpoint().offer().await?;
    match branch.label() {
        wifi_labels::RX_UDP => {
            let packet = branch.recv::<WifiRxPacketMsg>().await?;
            rp2w_firmware::record_choreofs_driver_trace(
                WIFI_TRACE_RX_READY | ((packet.payload_len() as u32) & 0xff),
            );
            Ok(decode_pico2w_sensor_udp_ack(packet.payload()) == Some(expected_seq))
        }
        wifi_labels::RX_PENDING => {
            let pending = branch.recv::<WifiRxPendingMsg>().await?;
            rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_RX_PENDING | (pending & 0xff));
            Ok(false)
        }
        other => {
            core::hint::black_box(other);
            Err(SensorPanelError::RuntimeViolation)
        }
    }
}

fn validate_fd_write<const ROLE: u8>(
    ctx: &appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdWrite,
    expected_fd: u8,
    expected_object: appkit::ObjectId,
) -> Result<appkit::LedgerFdFact, SensorPanelError> {
    if request.fd() != expected_fd {
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    match ctx.ledger().fd(request.fd() as u32) {
        Some(fact) if fact.object() == expected_object && fact.rights() == FD_WRITE_RIGHT => {
            Ok(fact)
        }
        Some(fact) => {
            core::hint::black_box(fact);
            Err(SensorPanelError::RuntimeViolation)
        }
        None => {
            core::hint::black_box(request);
            Err(SensorPanelError::RuntimeViolation)
        }
    }
}

fn pico2w_sample_from_payload(input: &[u8]) -> Result<Pico2wSensorSample, SensorPanelError> {
    if input.len() != PICO2W_SENSOR_SAMPLE_BYTES {
        return Err(SensorPanelError::RuntimeViolation);
    }
    Pico2wSensorSample::decode_payload(Payload::new(input))
        .map_err(|_| SensorPanelError::RuntimeViolation)
}

fn next_sensor_udp_seq() -> u16 {
    unsafe {
        let seq = SENSOR_UDP_SEQ;
        SENSOR_UDP_SEQ = SENSOR_UDP_SEQ.wrapping_add(1);
        seq
    }
}

#[cfg(test)]
mod sensor_panel_tests {
    use super::{
        PICO2W_SENSOR_SAMPLE_BYTES, parse_ipv4_const, parse_mac_const, pico2w_sample_from_payload,
    };
    use hibana_wifi::proto::ethernet::{Ipv4Addr, MacAddr, build_arp_request};
    use uno_q_heterogeneous::protocol::PICO2W_SENSOR_STATUS_FRESH;

    fn compact(source: &str) -> String {
        source.chars().filter(|ch| !ch.is_whitespace()).collect()
    }

    #[test]
    fn typed_sample_payload_is_the_udp_authority() {
        let sample =
            super::Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 226, 600, 2500, 7).unwrap();
        let mut payload = [0u8; PICO2W_SENSOR_SAMPLE_BYTES];
        hibana::runtime::wire::WireEncode::encode_into(&sample, &mut payload).unwrap();

        assert_eq!(pico2w_sample_from_payload(&payload).unwrap(), sample);
        assert!(pico2w_sample_from_payload(b"T:22.60C H:60%\nL:2500\n").is_err());
    }

    #[test]
    fn wifi_target_literals_are_const_parsed() {
        assert_eq!(parse_ipv4_const("192.168.240.42"), [192, 168, 240, 42]);
        assert_eq!(
            parse_mac_const("14:b5:cd:0f:41:7d"),
            [0x14, 0xb5, 0xcd, 0x0f, 0x41, 0x7d]
        );
    }

    #[test]
    fn arp_request_for_local_static_ip_is_link_layer_control() {
        let local_mac = MacAddr([0x02, 0x12, 0x34, 0x56, 0x78, 0x9a]);
        let uno_q_mac = MacAddr([0x14, 0xb5, 0xcd, 0x0f, 0x41, 0x7d]);
        let local_ip = Ipv4Addr([192, 168, 96, 98]);
        let uno_q_ip = Ipv4Addr([192, 168, 96, 99]);
        let mut frame = [0u8; 64];
        let len = build_arp_request(&mut frame, uno_q_mac, uno_q_ip, local_ip).unwrap();

        assert_eq!(
            super::parse_arp_ipv4_request(&frame[..len], local_mac, local_ip, uno_q_ip),
            Some((uno_q_mac, uno_q_ip))
        );
        assert_eq!(
            super::parse_arp_ipv4_request(
                &frame[..len],
                local_mac,
                local_ip,
                Ipv4Addr([1, 1, 1, 1])
            ),
            None
        );
    }

    #[test]
    fn sensor_panel_choreography_matches_guest_import_loop_order() {
        let panel = compact(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bin/sensor_panel.rs"
        )));
        let guest = compact(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/wasip1/guest/src/bin/rp2w-sensor-panel-guest.rs"
        )));

        assert!(
            panel.contains(
                "g::seq(fd_read(),g::seq(fd_write(),g::seq(fd_write_after_wifi_ack(),poll()))"
            ),
            "sensor-panel choreography must mirror fd_read -> fd_write -> fd_write -> poll"
        );
        assert!(
            panel.contains("if!driver_admit_cycle(&mutctx).await?{break;}driver_fd_write_display(&mutctx).await?;driver_fd_write_uno_q(&mutctx).await?;driver_poll_oneoff(&mutctx).await?;"),
            "local side must admit the rolled read arm before the same two fd_write imports"
        );

        let read = guest.find("letlen=sample.read_once(&mutbuffer)?;").unwrap();
        let display = guest
            .find("display.write_once_exact(&buffer[..len])?;")
            .unwrap();
        let uno_q = guest
            .find("uno_q.write_once_exact(&buffer[..len])?;")
            .unwrap();
        let poll = guest.find("time::sleep_ms(SAMPLE_MS)?;").unwrap();
        assert!(read < display && display < uno_q && uno_q < poll);
    }

    #[test]
    fn uno_q_commit_is_the_second_fd_write_ret_after_wifi_ack() {
        let panel = compact(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bin/sensor_panel.rs"
        )));

        assert!(
            panel.contains(
                "letfd_write_req=||g::send::<1,0,g::Msg<LABEL_WASI_FD_WRITE,EngineReq>>();"
            ),
            "fd_write request must use the normal WASI fd_write label"
        );
        assert!(
            panel.contains(
                "letfd_write_ret=||g::send::<0,1,g::Msg<LABEL_WASI_FD_WRITE_RET,EngineRet>>();"
            ),
            "fd_write return must use the normal WASI fd_write_ret label"
        );
        assert!(
            panel.contains(
                "g::seq(fd_write_req(),g::seq(wifi_udp_tx(),g::seq(wifi_udp_rx(),fd_write_ret()))"
            ),
            "Uno Q write must commit by returning the second normal fd_write only after WiFi ACK choreography"
        );
    }
}

async fn cyw43_boundary_tx_udp<const ROLE: u8>(
    ctx: &mut appkit::BoundaryCtx<'_, SensorPanel, ROLE>,
    wifi: &mut UnoQWifiSender,
) -> Result<(), SensorPanelError> {
    let branch = ctx.endpoint().offer().await?;
    if branch.label() != wifi_labels::TX_UDP {
        core::hint::black_box(branch.label());
        return Err(SensorPanelError::RuntimeViolation);
    }
    let datagram = branch.recv::<WifiTxUdpMsg>().await?;
    rp2w_firmware::record_choreofs_driver_trace(
        WIFI_TRACE_TX_REQ | ((datagram.payload_len() as u32) & 0xff),
    );
    match wifi.send_datagram(&datagram) {
        Ok(written) => {
            ctx.endpoint()
                .send::<WifiTxDoneMsg>(&(written as u32))
                .await?;
        }
        Err(()) => {
            ctx.endpoint().send::<WifiTxErrMsg>(&1).await?;
        }
    }
    Ok(())
}

async fn cyw43_boundary_bootstrap<const ROLE: u8>(
    ctx: &mut appkit::BoundaryCtx<'_, SensorPanel, ROLE>,
) -> Result<UnoQWifiSender, SensorPanelError> {
    lcd_network_status(b"WiFi joining", b"sensor -> UnoQ");
    rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_BOOT_ATTEMPT);

    #[cfg(feature = "embed-cyw43-artifacts")]
    {
        use hibana_wifi::proto::cyw43::{backplane, ioctl};

        let mut driver =
            rp2w_firmware::Rp2wCyw43GspiDriver::new(rp2w_firmware::Rp2wCyw43GspiBitbang::new());
        let mut scratch = [0u8; 1536];

        let _ = ctx.endpoint().recv::<WifiPowerOnMsg>().await?;
        rp2w_firmware::rp2w_cyw43_gspi_init();
        rp2w_firmware::rp2w_cyw43_gspi_reset();
        ctx.endpoint().send::<WifiPowerReadyMsg>(&0).await?;

        let _ = ctx.endpoint().recv::<WifiGspiProbeMsg>().await?;
        driver.bring_up_bus().map_err(cyw43_bootstrap_error)?;
        ctx.endpoint().send::<WifiGspiReadyMsg>(&0).await?;

        let _ = ctx.endpoint().recv::<WifiBackplaneInitMsg>().await?;
        let clock = driver
            .bring_up_backplane_clock()
            .map_err(cyw43_bootstrap_error)?;
        let _ = driver
            .read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)
            .map_err(cyw43_bootstrap_error)?;
        ctx.endpoint()
            .send::<WifiBackplaneReadyMsg>(&u32::from(clock))
            .await?;

        let _ = ctx.endpoint().recv::<WifiFirmwareLoadMsg>().await?;
        driver
            .prepare_firmware_download()
            .map_err(cyw43_bootstrap_error)?;
        driver
            .write_backplane_bytes(0, CYW43_FIRMWARE, &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        ctx.endpoint()
            .send::<WifiFirmwareLoadedMsg>(&cyw43_firmware_len())
            .await?;

        let _ = ctx.endpoint().recv::<WifiNvramLoadMsg>().await?;
        ctx.endpoint()
            .send::<WifiNvramLoadedMsg>(&cyw43_nvram_len())
            .await?;

        let _ = ctx.endpoint().recv::<WifiFirmwareStartMsg>().await?;
        let ht_clock = driver
            .boot_uploaded_firmware(CYW43_NVRAM, &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        rp2w_firmware::record_choreofs_engine_status(u32::from(ht_clock));
        ctx.endpoint()
            .send::<WifiFirmwareStartedMsg>(&u32::from(ht_clock))
            .await?;

        let _ = ctx.endpoint().recv::<WifiClmLoadMsg>().await?;
        driver
            .load_clm(CYW43_CLM, &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        ctx.endpoint()
            .send::<WifiClmDoneMsg>(&cyw43_clm_len())
            .await?;

        let _ = ctx.endpoint().recv::<WifiCdcUpMsg>().await?;
        driver
            .set_station_mac(RP2W_WIFI_LOCAL_MAC, &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        driver
            .ioctl_set(ioctl::WLC_UP, &[], &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        ctx.endpoint().send::<WifiCdcUpDoneMsg>(&0).await?;

        let _ = ctx.endpoint().recv::<WifiJoinMsg>().await?;
        driver
            .join_wpa2(RP2W_WIFI_SSID, RP2W_WIFI_KEY, &mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        ctx.endpoint().send::<WifiJoinDoneMsg>(&0).await?;

        let _ = ctx.endpoint().recv::<WifiLinkPollMsg>().await?;
        let bssid = driver
            .wait_for_bssid(&mut scratch)
            .map_err(cyw43_bootstrap_error)?;
        rp2w_firmware::record_choreofs_engine_error_code(u32::from_be_bytes([
            bssid[0], bssid[1], bssid[2], bssid[3],
        ]));
        rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_WIFI_SEND_OK | u32::from(clock));
        ctx.endpoint()
            .send::<WifiLinkUpMsg>(&u32::from_be_bytes([
                bssid[0], bssid[1], bssid[2], bssid[3],
            ]))
            .await?;
        lcd_network_status(b"WiFi joined", b"UnoQ UDP ready");
        Ok(UnoQWifiSender::ready(driver))
    }

    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    {
        let _ = ctx.endpoint().recv::<WifiPowerOnMsg>().await?;
        ctx.endpoint().send::<WifiPowerReadyMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiGspiProbeMsg>().await?;
        ctx.endpoint().send::<WifiGspiReadyMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiBackplaneInitMsg>().await?;
        ctx.endpoint().send::<WifiBackplaneReadyMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiFirmwareLoadMsg>().await?;
        ctx.endpoint().send::<WifiFirmwareLoadedMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiNvramLoadMsg>().await?;
        ctx.endpoint().send::<WifiNvramLoadedMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiFirmwareStartMsg>().await?;
        ctx.endpoint().send::<WifiFirmwareStartedMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiClmLoadMsg>().await?;
        ctx.endpoint().send::<WifiClmDoneMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiCdcUpMsg>().await?;
        ctx.endpoint().send::<WifiCdcUpDoneMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiJoinMsg>().await?;
        ctx.endpoint().send::<WifiJoinDoneMsg>(&0).await?;
        let _ = ctx.endpoint().recv::<WifiLinkPollMsg>().await?;
        ctx.endpoint().send::<WifiLinkUpMsg>(&0).await?;
        rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_BOOT_ERR);
        lcd_network_status(b"WiFi disabled", b"build feature");
        Ok(UnoQWifiSender::unavailable())
    }
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn cyw43_bootstrap_error(error: rp2w_firmware::Rp2wCyw43GspiError) -> SensorPanelError {
    rp2w_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
    lcd_network_status(b"WiFi failed", b"check hotspot");
    SensorPanelError::RuntimeViolation
}

async fn cyw43_boundary_rx_udp<const ROLE: u8>(
    ctx: &mut appkit::BoundaryCtx<'_, SensorPanel, ROLE>,
    wifi: &mut UnoQWifiSender,
) -> Result<(), SensorPanelError> {
    let branch = ctx.endpoint().offer().await?;
    if branch.label() != wifi_labels::RX_UDP_POLL {
        core::hint::black_box(branch.label());
        return Err(SensorPanelError::RuntimeViolation);
    }
    let local_port = branch.recv::<WifiRxPollMsg>().await?;
    rp2w_firmware::record_choreofs_driver_trace(WIFI_TRACE_RX_REQ | u32::from(local_port & 0xff));
    match wifi.recv_packet(local_port) {
        Ok(Some(packet)) => {
            ctx.endpoint().send::<WifiRxPacketMsg>(&packet).await?;
        }
        Ok(None) | Err(()) => {
            ctx.endpoint().send::<WifiRxPendingMsg>(&0).await?;
        }
    }
    Ok(())
}

async fn driver_poll_oneoff<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_POLL_ONEOFF>(ctx).await? {
        EngineReq::PollOneoff(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    rp2w_firmware::record_choreofs_poll_timeout(request.timeout_tick());
    if request.timeout_tick() != EXPECTED_POLL_TIMEOUT_MS {
        #[cfg(feature = "wasm-engine-core")]
        rp2w_firmware::record_choreofs_engine_error_code(0x5250_d000);
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    rp2w_firmware::rp2w_poll_delay(request.timeout_tick());
    rp2w_firmware::record_choreofs_poll();
    send_engine_ret::<ROLE, LABEL_WASI_POLL_ONEOFF_RET>(
        ctx,
        EngineRet::PollReady(PollReady::new(1)),
    )
    .await
}

fn find_ledger_fd(
    ledger: appkit::LedgerFacts<'_>,
    object: appkit::ObjectId,
    rights: u64,
) -> Option<appkit::LedgerFdFact> {
    let facts = ledger.fds();
    let mut index = 0usize;
    while index < facts.len() {
        let fact = facts[index];
        if fact.object() == object && fact.rights() == rights {
            return Some(fact);
        }
        index += 1usize;
    }
    None
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    rp2w_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn rp2w_selected_run() -> ! {
    rp2w_firmware::run::<SensorPanel>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    rp2w_firmware::run::<SensorPanel>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    rp2w_firmware::run::<SensorPanel>()
}

mod rp2w_firmware {
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    use core::ptr::{read_volatile, write_volatile};

    use hibana_pico::appkit;

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) use ::rp2w_firmware::panic;
    pub(super) use ::rp2w_firmware::{
        Rp2wCapsuleFacts, Rp2wPlacement, mark_runtime_ready, mark_success,
        record_choreofs_driver_trace, record_choreofs_engine_error_code,
        record_choreofs_engine_status, reset_choreofs_markers, run,
    };

    pub(super) const CHOREOFS_DRIVER_STARTED: u32 = 0x5741_0010;
    pub(super) const CHOREOFS_GPIO_READY: u32 = 0x5741_0020;

    pub(super) fn record_choreofs_path_open(object: appkit::ObjectId) {
        ::rp2w_firmware::record_choreofs_path_open(object);
    }

    pub(super) fn record_choreofs_fd_write(object: appkit::ObjectId) {
        ::rp2w_firmware::record_choreofs_fd_write(object);
    }

    pub(super) fn record_choreofs_poll() {
        ::rp2w_firmware::record_choreofs_poll();
    }

    pub(super) fn record_choreofs_poll_timeout(timeout_ticks: u64) {
        ::rp2w_firmware::record_choreofs_poll_timeout(timeout_ticks);
    }

    pub(super) fn rp2w_poll_delay(timeout_ms: u64) {
        ::rp2w_firmware::rp2w_poll_delay(timeout_ms);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_LINE_DIAG: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_LAST_TX_WORD: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_LAST_RX_WORD: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_TRANSFER_TRACE_COUNT: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_TX_TRACE: [u32; 48] = [0; 48];
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    #[used]
    #[unsafe(no_mangle)]
    static mut HIBANA_CYW43_RX_TRACE: [u32; 48] = [0; 48];

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const SIO_BASE: usize = 0xd000_0000;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const IO_BANK0_BASE: usize = 0x4002_8000;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const PADS_BANK0_BASE: usize = 0x4003_8000;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const RESETS_BASE: usize = 0x4002_0000;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const RESETS_RESET_CLR: *mut u32 = (RESETS_BASE + 0x3000) as *mut u32;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const RESETS_RESET_DONE: *const u32 = (RESETS_BASE + 0x08) as *const u32;
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
    const PIO_CTRL: *mut u32 = PIO0_BASE as *mut u32;
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

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn write_marker(slot: *mut u32, value: u32) {
        unsafe { write_volatile(slot, value) }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn read_marker(slot: *const u32) -> u32 {
        unsafe { read_volatile(slot) }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn record_cyw43_line_diag(diag: u32) {
        write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LINE_DIAG), diag);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn record_cyw43_last_tx_word(word: u32) {
        write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_TX_WORD), word);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn record_cyw43_last_rx_word(word: u32) {
        write_marker(core::ptr::addr_of_mut!(HIBANA_CYW43_LAST_RX_WORD), word);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
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

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn gpio_ctrl(pin: u8) -> *mut u32 {
        (IO_BANK0_BASE + 0x04 + pin as usize * 8) as *mut u32
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn gpio_pad(pin: u8) -> *mut u32 {
        (PADS_BANK0_BASE + 0x04 + pin as usize * 4) as *mut u32
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn rp2w_gpio_bank_init() {
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
    fn rp2w_gpio_bank_init() {}

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_gpio_init_output(pin: u8) {
        rp2w_gpio_bank_init();
        unsafe {
            write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
            write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
            write_volatile(GPIO_OE_SET, 1u32 << pin);
            write_volatile(GPIO_OUT_CLR, 1u32 << pin);
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_gpio_init_output(pin: u8) {
        rp2w_gpio_bank_init();
        core::hint::black_box(pin);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_gpio_write(pin: u8, high: bool) {
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
    pub(super) fn rp2w_gpio_write(pin: u8, high: bool) {
        core::hint::black_box((pin, high));
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn rp2w_gpio_init_input(pin: u8) {
        rp2w_gpio_bank_init();
        unsafe {
            write_volatile(gpio_pad(pin), GPIO_PAD_DEFAULT);
            write_volatile(gpio_ctrl(pin), GPIO_FUNC_SIO);
            write_volatile(GPIO_OE_CLR, 1u32 << pin);
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    fn rp2w_gpio_init_input(pin: u8) {
        core::hint::black_box(pin);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn rp2w_gpio_set_output_enabled(pin: u8, enabled: bool) {
        let bit = 1u32 << pin;
        unsafe {
            if enabled {
                write_volatile(GPIO_OE_SET, bit);
            } else {
                write_volatile(GPIO_OE_CLR, bit);
            }
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn rp2w_gpio_read(pin: u8) -> bool {
        unsafe { read_volatile(GPIO_IN) & (1u32 << pin) != 0 }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Rp2wSensorSample {
        pub dht20_ok: bool,
        pub temp_c_x100: i32,
        pub humidity_x100: u32,
        pub light_raw: u16,
    }

    pub(super) const RP2W_UNO_Q_SENSOR_UDP_SRC_PORT: u16 = 43210;
    pub(super) const RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES: usize =
        uno_q_heterogeneous::protocol::PICO2W_SENSOR_SAMPLE_BYTES;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Rp2wUnoQWifiTarget {
        pub local_mac: hibana_wifi::proto::ethernet::MacAddr,
        pub uno_q_mac: hibana_wifi::proto::ethernet::MacAddr,
        pub local_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
        pub uno_q_ip: hibana_wifi::proto::ethernet::Ipv4Addr,
        pub src_port: u16,
    }

    impl Rp2wUnoQWifiTarget {
        pub(super) const fn new(
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
    pub(super) enum Rp2wWifiFrameError {
        Datagram(hibana_wifi::proto::udp::UdpDatagramError),
        Frame(hibana_wifi::proto::udp::UdpTxFrameError),
    }

    impl From<hibana_wifi::proto::udp::UdpDatagramError> for Rp2wWifiFrameError {
        fn from(error: hibana_wifi::proto::udp::UdpDatagramError) -> Self {
            Self::Datagram(error)
        }
    }

    impl From<hibana_wifi::proto::udp::UdpTxFrameError> for Rp2wWifiFrameError {
        fn from(error: hibana_wifi::proto::udp::UdpTxFrameError) -> Self {
            Self::Frame(error)
        }
    }

    impl From<hibana_wifi::proto::ethernet::EthernetError> for Rp2wWifiFrameError {
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
    static mut RP2W_BOARD_INIT_DONE: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut RP2W_DHT20_INIT_DONE: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut RP2W_ADC_READY: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut RP2W_LCD_BUS: u32 = 0;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    static mut RP2W_DHT20_BUS: u32 = 1;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_I2C_DETECT_MASK: u32 = 0;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_LCD_INIT_OK: u32 = 0;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_LAST_LIGHT_RAW: u32 = 0;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_LAST_DHT20_OK: u32 = 0;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_LAST_TEMP_C_X100: i32 = 0;
    #[used]
    #[unsafe(no_mangle)]
    pub static mut RP2W_LAST_HUMIDITY_X100: u32 = 0;

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
    pub(super) fn rp2w_uart0_init() {
        rp2w_gpio_bank_init();
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
    pub(super) fn rp2w_uart0_init() {}

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_uart0_write_byte(byte: u8) {
        while unsafe { read_volatile(UART0_FR) } & UARTFR_TXFF != 0 {
            core::hint::spin_loop();
        }
        unsafe {
            write_volatile(UART0_DR, u32::from(byte));
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_uart0_write_byte(byte: u8) {
        core::hint::black_box(byte);
    }

    pub(super) fn rp2w_uart0_write_bytes(bytes: &[u8]) {
        let mut index = 0usize;
        while index < bytes.len() {
            rp2w_uart0_write_byte(bytes[index]);
            index += 1;
        }
    }

    pub(super) fn rp2w_uart0_write_str(text: &str) {
        rp2w_uart0_write_bytes(text.as_bytes());
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Rp2wCyw43SpiError {
        Timeout,
        Unavailable,
    }

    pub(super) type Rp2wCyw43DriverError =
        hibana_wifi::cyw43::driver::Cyw43DriverError<Rp2wCyw43SpiError>;
    pub(super) type Rp2wCyw43GspiError =
        hibana_wifi::cyw43::gspi::Cyw43GspiError<Rp2wCyw43SpiError>;
    pub(super) type Rp2wCyw43GspiDriver =
        hibana_wifi::cyw43::gspi::Cyw43GspiDriver<Rp2wCyw43GspiBitbang>;

    fn rp2w_wifi_frame_error_to_gspi_error(error: Rp2wWifiFrameError) -> Rp2wCyw43GspiError {
        match error {
            Rp2wWifiFrameError::Datagram(
                hibana_wifi::proto::udp::UdpDatagramError::PayloadTooLarge,
            ) => hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge,
            Rp2wWifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Ethernet(
                hibana_wifi::proto::ethernet::EthernetError::BufferTooSmall,
            )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall,
            Rp2wWifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Ethernet(
                hibana_wifi::proto::ethernet::EthernetError::PayloadTooLarge,
            )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge,
            Rp2wWifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Cyw43(
                hibana_wifi::proto::cyw43::Cyw43Error::BufferTooSmall,
            )) => hibana_wifi::cyw43::gspi::Cyw43GspiError::BufferTooSmall,
            Rp2wWifiFrameError::Frame(hibana_wifi::proto::udp::UdpTxFrameError::Cyw43(_)) => {
                hibana_wifi::cyw43::gspi::Cyw43GspiError::TransferTooLarge
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Rp2wCyw43Spi;

    impl Rp2wCyw43Spi {
        pub(super) const fn new() -> Self {
            Self
        }
    }

    impl hibana_wifi::cyw43::driver::Cyw43Bus for Rp2wCyw43Spi {
        type Error = Rp2wCyw43SpiError;

        fn transfer(&mut self, byte: u8) -> Result<u8, Self::Error> {
            rp2w_cyw43_spi_transfer(byte)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Rp2wCyw43GspiBitbang;

    impl Rp2wCyw43GspiBitbang {
        pub(super) const fn new() -> Self {
            Self
        }
    }

    impl hibana_wifi::cyw43::gspi::Cyw43GspiBus for Rp2wCyw43GspiBitbang {
        type Error = Rp2wCyw43SpiError;

        fn init(&mut self) -> Result<(), Self::Error> {
            rp2w_cyw43_gspi_init();
            Ok(())
        }

        fn reset(&mut self) -> Result<(), Self::Error> {
            rp2w_cyw43_gspi_reset();
            Ok(())
        }

        fn delay_ms(&mut self, ms: u32) {
            rp2w_poll_delay(u64::from(ms));
        }

        fn transfer(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(), Self::Error> {
            rp2w_cyw43_gspi_transfer(tx, rx)
        }
    }

    const CYW43_PIN_WL_REG_ON: u8 = 23;
    const CYW43_PIN_WL_DATA: u8 = 24;
    const CYW43_PIN_WL_CS: u8 = 25;
    const CYW43_PIN_WL_CLOCK: u8 = 29;
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    const CYW43_SPI_BIT_DELAY_SPINS: u8 = 64;

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn cyw43_spi_bit_delay() {
        let mut spin = 0;
        while spin < CYW43_SPI_BIT_DELAY_SPINS {
            core::hint::spin_loop();
            spin += 1;
        }
    }

    pub(super) fn rp2w_cyw43_gspi_init() {
        rp2w_gpio_init_output(CYW43_PIN_WL_REG_ON);
        rp2w_gpio_init_output(CYW43_PIN_WL_DATA);
        rp2w_gpio_init_output(CYW43_PIN_WL_CS);
        rp2w_gpio_init_output(CYW43_PIN_WL_CLOCK);
        rp2w_gpio_write(CYW43_PIN_WL_REG_ON, false);
        rp2w_gpio_write(CYW43_PIN_WL_DATA, false);
        rp2w_gpio_write(CYW43_PIN_WL_CLOCK, false);
        rp2w_gpio_write(CYW43_PIN_WL_CS, true);
    }

    pub(super) fn rp2w_cyw43_gspi_reset() {
        rp2w_gpio_init_output(CYW43_PIN_WL_REG_ON);
        rp2w_gpio_init_output(CYW43_PIN_WL_DATA);
        rp2w_gpio_init_output(CYW43_PIN_WL_CS);
        rp2w_gpio_init_output(CYW43_PIN_WL_CLOCK);
        rp2w_gpio_write(CYW43_PIN_WL_REG_ON, false);
        rp2w_gpio_write(CYW43_PIN_WL_DATA, false);
        rp2w_gpio_write(CYW43_PIN_WL_CLOCK, false);
        rp2w_gpio_write(CYW43_PIN_WL_CS, true);
        rp2w_poll_delay(200);
        rp2w_gpio_write(CYW43_PIN_WL_REG_ON, true);
        rp2w_poll_delay(2_000);
        rp2w_gpio_init_input(CYW43_PIN_WL_DATA);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_cyw43_gspi_line_diag() -> u32 {
        rp2w_cyw43_gspi_init();
        let mut mask = 0u32;
        rp2w_gpio_write(CYW43_PIN_WL_DATA, false);
        cyw43_spi_bit_delay();
        if rp2w_gpio_read(CYW43_PIN_WL_DATA) {
            mask |= 1 << 0;
        }
        rp2w_gpio_write(CYW43_PIN_WL_DATA, true);
        cyw43_spi_bit_delay();
        if rp2w_gpio_read(CYW43_PIN_WL_DATA) {
            mask |= 1 << 1;
        }
        rp2w_gpio_write(CYW43_PIN_WL_DATA, false);
        cyw43_spi_bit_delay();
        if rp2w_gpio_read(CYW43_PIN_WL_DATA) {
            mask |= 1 << 2;
        }
        rp2w_gpio_set_output_enabled(CYW43_PIN_WL_DATA, false);
        cyw43_spi_bit_delay();
        if rp2w_gpio_read(CYW43_PIN_WL_DATA) {
            mask |= 1 << 3;
        }
        rp2w_gpio_set_output_enabled(CYW43_PIN_WL_DATA, true);
        rp2w_gpio_write(CYW43_PIN_WL_DATA, false);
        record_cyw43_line_diag(0x5749_3000 | mask);
        mask
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_cyw43_gspi_line_diag() -> u32 {
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
    fn pio_sm0_put(value: u32) -> Result<(), Rp2wCyw43SpiError> {
        let mut spin = 0u32;
        while unsafe { read_volatile(PIO_FSTAT) } & PIO_FSTAT_TXFULL_SM0 != 0 {
            if spin > SPI_TIMEOUT_SPINS {
                return Err(Rp2wCyw43SpiError::Timeout);
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
    fn pio_sm0_get() -> Result<u32, Rp2wCyw43SpiError> {
        let mut spin = 0u32;
        while unsafe { read_volatile(PIO_FSTAT) } & PIO_FSTAT_RXEMPTY_SM0 != 0 {
            if spin > SPI_TIMEOUT_SPINS {
                return Err(Rp2wCyw43SpiError::Timeout);
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
    fn pio_sm0_wait_tx_stalled() -> Result<(), Rp2wCyw43SpiError> {
        unsafe {
            write_volatile(PIO_FDEBUG, 1 << 24);
        }
        let mut spin = 0u32;
        while unsafe { read_volatile(PIO_FDEBUG) } & (1 << 24) == 0 {
            if spin > SPI_TIMEOUT_SPINS {
                return Err(Rp2wCyw43SpiError::Timeout);
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
    fn rp2w_cyw43_gspi_pio_init() {
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
    pub(super) fn rp2w_cyw43_gspi_transfer(
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(), Rp2wCyw43SpiError> {
        let tx_word = if tx.len() >= 4 {
            u32::from_be_bytes([tx[0], tx[1], tx[2], tx[3]])
        } else {
            0
        };
        if tx.len() >= 4 {
            record_cyw43_last_tx_word(tx_word);
        }

        if tx.is_empty() || tx.len() & 3 != 0 || rx.len() & 3 != 0 {
            return Err(Rp2wCyw43SpiError::Unavailable);
        }

        rp2w_cyw43_gspi_pio_init();
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

        rp2w_gpio_write(CYW43_PIN_WL_CS, false);
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
        rp2w_gpio_write(CYW43_PIN_WL_CS, true);
        cyw43_spi_bit_delay();
        Ok(())
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_cyw43_gspi_transfer(
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(), Rp2wCyw43SpiError> {
        core::hint::black_box((tx, rx));
        Err(Rp2wCyw43SpiError::Unavailable)
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_cyw43_spi_init() {
        rp2w_gpio_bank_init();
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
    pub(super) fn rp2w_cyw43_spi_init() {}

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_cyw43_spi_transfer(byte: u8) -> Result<u8, Rp2wCyw43SpiError> {
        let mut spin = 0u32;
        while unsafe { read_volatile(SPI0_SR) } & SPI_SR_TNF == 0 {
            if spin > SPI_TIMEOUT_SPINS {
                return Err(Rp2wCyw43SpiError::Timeout);
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
                return Err(Rp2wCyw43SpiError::Timeout);
            }
            spin += 1;
            core::hint::spin_loop();
        }
        Ok((unsafe { read_volatile(SPI0_DR) } & 0xff) as u8)
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_cyw43_spi_transfer(byte: u8) -> Result<u8, Rp2wCyw43SpiError> {
        core::hint::black_box(byte);
        Err(Rp2wCyw43SpiError::Unavailable)
    }

    pub(super) fn rp2w_cyw43_boot_qemu_model(
        firmware: &[u8],
        clm: &[u8],
        nvram: &[u8],
        local_node: u8,
        peer_node: u8,
    ) -> Result<(), Rp2wCyw43DriverError> {
        use hibana_wifi::{
            cyw43::driver::{Cyw43Driver, FirmwareImage, StationConfig},
            proto::firmware::fnv1a32,
        };

        rp2w_cyw43_spi_init();
        let mut driver = Cyw43Driver::new(Rp2wCyw43Spi::new());
        driver.bring_up_station(StationConfig {
            local_node,
            peer_node,
            firmware: FirmwareImage::pico_w43439(firmware),
            clm: FirmwareImage::pico_w43439_clm(clm),
            nvram: FirmwareImage::new(nvram, nvram.len() as u32, fnv1a32(nvram)),
        })
    }

    pub(super) fn rp2w_cyw43_send_frame_qemu_model(
        dst_node: u8,
        frame: &[u8],
    ) -> Result<(), Rp2wCyw43DriverError> {
        use hibana_wifi::cyw43::driver::Cyw43Driver;

        let mut driver = Cyw43Driver::new(Rp2wCyw43Spi::new());
        driver.transmit_frame(dst_node, frame)
    }

    pub(super) fn rp2w_cyw43_probe_real_gspi() -> Result<u32, Rp2wCyw43GspiError> {
        use hibana_wifi::cyw43::gspi::Cyw43GspiDriver;

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        driver.read_status()
    }

    pub(super) fn rp2w_cyw43_probe_real_backplane_clock() -> Result<u32, Rp2wCyw43GspiError> {
        use hibana_wifi::cyw43::gspi::Cyw43GspiDriver;

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        Ok(u32::from(driver.bring_up_backplane_clock()?))
    }

    pub(super) fn rp2w_cyw43_probe_real_backplane_regs() -> Result<(u8, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let chipcommon_sr_control1 =
            driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        Ok((clock, chipcommon_sr_control1))
    }

    pub(super) fn rp2w_cyw43_probe_real_download_prep() -> Result<(u8, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let chipcommon_sr_control1 =
            driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        driver.prepare_firmware_download()?;
        Ok((clock, chipcommon_sr_control1))
    }

    pub(super) fn rp2w_cyw43_probe_real_sram_roundtrip() -> Result<(u8, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        driver.prepare_firmware_download()?;
        driver.write_backplane_u32(0, 0x4849_4241)?;
        let value = driver.read_backplane_u32(0)?;
        Ok((clock, value))
    }

    pub(super) fn rp2w_cyw43_probe_real_firmware_upload(
        firmware: &[u8],
    ) -> Result<(u8, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        driver.prepare_firmware_download()?;
        let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
        driver.write_backplane_bytes(0, firmware, &mut scratch)?;
        let first_word = driver.read_backplane_u32(0)?;
        Ok((clock, first_word))
    }

    pub(super) fn rp2w_cyw43_probe_real_sram_bytes_roundtrip()
    -> Result<(u8, u32, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        driver.prepare_firmware_download()?;
        let bytes = [
            0x00, 0x00, 0x00, 0x00, 0x65, 0x14, 0x00, 0x00, 0x91, 0x13, 0x00, 0x00, 0x91, 0x13,
            0x00, 0x00,
        ];
        let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
        driver.write_backplane_bytes(0, &bytes, &mut scratch)?;
        let first_word = driver.read_backplane_u32(0)?;
        let second_word = driver.read_backplane_u32(4)?;
        Ok((clock, first_word, second_word))
    }

    pub(super) fn rp2w_cyw43_probe_real_firmware_prefix_upload(
        firmware: &[u8],
    ) -> Result<(u8, u32, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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

    pub(super) fn rp2w_cyw43_probe_real_firmware_boot(
        firmware: &[u8],
        nvram: &[u8],
    ) -> Result<(u8, u8), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
        driver.bring_up_bus()?;
        let clock = driver.bring_up_backplane_clock()?;
        let _ = driver.read_backplane_u32(backplane::CHIPCOMMON_SR_CONTROL1)?;
        driver.prepare_firmware_download()?;
        let mut scratch = [0u8; hibana_wifi::proto::cyw43::GSPI_MAX_BLOCK_SIZE + 4];
        driver.write_backplane_bytes(0, firmware, &mut scratch)?;
        let ht_clock = driver.boot_uploaded_firmware(nvram, &mut scratch)?;
        Ok((clock, ht_clock))
    }

    pub(super) fn rp2w_cyw43_probe_real_ioctl_up(
        firmware: &[u8],
        nvram: &[u8],
    ) -> Result<(u8, u8), Rp2wCyw43GspiError> {
        use hibana_wifi::{
            cyw43::gspi::Cyw43GspiDriver,
            proto::cyw43::{backplane, ioctl},
        };

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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

    pub(super) fn rp2w_cyw43_probe_real_clm_ioctl_up(
        firmware: &[u8],
        nvram: &[u8],
        clm: &[u8],
    ) -> Result<(u8, u8), Rp2wCyw43GspiError> {
        use hibana_wifi::{
            cyw43::gspi::Cyw43GspiDriver,
            proto::cyw43::{backplane, ioctl},
        };

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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

    pub(super) fn rp2w_cyw43_probe_real_wifi_join(
        firmware: &[u8],
        nvram: &[u8],
        clm: &[u8],
        ssid: &[u8],
        key: &[u8],
    ) -> Result<(u8, u8), Rp2wCyw43GspiError> {
        use hibana_wifi::{
            cyw43::gspi::Cyw43GspiDriver,
            proto::cyw43::{backplane, ioctl},
        };

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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

    pub(super) fn rp2w_cyw43_real_wifi_join_driver(
        firmware: &[u8],
        nvram: &[u8],
        clm: &[u8],
        ssid: &[u8],
        key: &[u8],
        local_mac: [u8; 6],
    ) -> Result<(Rp2wCyw43GspiDriver, u8, u8, [u8; 6]), Rp2wCyw43GspiError> {
        use hibana_wifi::proto::cyw43::{backplane, ioctl};

        let mut driver = Rp2wCyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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

    pub(super) fn rp2w_cyw43_probe_real_wifi_join_send_uno_q(
        firmware: &[u8],
        nvram: &[u8],
        clm: &[u8],
        ssid: &[u8],
        key: &[u8],
        target: Rp2wUnoQWifiTarget,
    ) -> Result<(u8, u8, usize, [u8; 6]), Rp2wCyw43GspiError> {
        use hibana_wifi::{
            cyw43::gspi::Cyw43GspiDriver,
            proto::cyw43::{backplane, ioctl},
        };

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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
        let frame_len = rp2w_build_uno_q_sensor_ethernet_frame(
            rp2w_read_sensor_sample(),
            target,
            &mut ethernet_frame,
        )
        .map_err(rp2w_wifi_frame_error_to_gspi_error)?;

        let mut sent = 0u8;
        while sent < 6 {
            if sent != 0 {
                rp2w_poll_delay(1_000);
            }
            driver.send_ethernet_frame(&ethernet_frame[..frame_len], &mut scratch)?;
            sent = sent.wrapping_add(1);
        }
        Ok((clock, ht_clock, frame_len, bssid))
    }

    pub(super) fn rp2w_cyw43_probe_real_firmware_upload_samples(
        firmware: &[u8],
    ) -> Result<(u8, u8, u32), Rp2wCyw43GspiError> {
        use hibana_wifi::{cyw43::gspi::Cyw43GspiDriver, proto::cyw43::backplane};

        let mut driver = Cyw43GspiDriver::new(Rp2wCyw43GspiBitbang::new());
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
    fn rp2w_i2c_init(base: usize, reset_mask: u32, sda_pin: u8, scl_pin: u8) {
        rp2w_gpio_bank_init();
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
            I2C_CON_MASTER_MODE
                | I2C_CON_SPEED_STANDARD
                | I2C_CON_RESTART_EN
                | I2C_CON_SLAVE_DISABLE,
        );
        mmio_write(base, I2C_SS_SCL_HCNT, 625);
        mmio_write(base, I2C_SS_SCL_LCNT, 625);
        i2c_clear_intr(base);
        mmio_write(base, I2C_ENABLE, 1);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_i2c0_init() {
        rp2w_i2c_init(I2C0_BASE, RESETS_I2C0, 8, 9);
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_i2c0_init() {}

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_i2c1_init() {
        rp2w_i2c_init(I2C1_BASE, RESETS_I2C1, 6, 7);
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_i2c1_init() {}

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
    fn rp2w_adc0_init() {
        rp2w_gpio_bank_init();
        if !reset_deassert(RESETS_ADC) {
            return;
        }
        unsafe {
            write_volatile(gpio_pad(26), GPIO_PAD_ANALOG);
            write_volatile(gpio_ctrl(26), GPIO_FUNC_NULL);
            write_volatile(ADC_CS, ADC_CS_EN | ADC_CS_AINSEL_ADC0);
            write_volatile(core::ptr::addr_of_mut!(RP2W_ADC_READY), 1);
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn rp2w_adc0_read() -> u16 {
        if unsafe { read_volatile(core::ptr::addr_of!(RP2W_ADC_READY)) } == 0 {
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
    fn rp2w_adc0_read() -> u16 {
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
        if unsafe { read_volatile(core::ptr::addr_of!(RP2W_LCD_BUS)) } == 1 {
            I2C1_BASE
        } else {
            LCD_I2C_BASE
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_lcd_init() -> bool {
        let probe = i2c_write(lcd_i2c_base(), LCD_ADDR, &[0x00]);
        rp2w_poll_delay(50);
        let mut ok = true;
        ok &= lcd_command(0x28);
        rp2w_poll_delay(5);
        ok &= lcd_command(0x28);
        rp2w_poll_delay(1);
        ok &= lcd_command(0x28);
        ok &= lcd_command(0x28);
        ok &= lcd_command(0x08);
        ok &= lcd_command(0x01);
        rp2w_poll_delay(2);
        ok &= lcd_command(0x06);
        ok &= lcd_command(0x0c);
        ok &= lcd_command(0x01);
        rp2w_poll_delay(2);
        probe && ok
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_lcd_init() -> bool {
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
    pub(super) fn rp2w_lcd_write_lines(line1: &[u8], line2: &[u8]) -> bool {
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
    pub(super) fn rp2w_lcd_write_lines(line1: &[u8], line2: &[u8]) -> bool {
        core::hint::black_box((line1, line2));
        true
    }

    pub(super) fn rp2w_lcd_write_payload(bytes: &[u8]) -> bool {
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
        rp2w_lcd_write_lines(&bytes[..split], &bytes[second_start..second_end])
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
            if read_volatile(core::ptr::addr_of!(RP2W_DHT20_INIT_DONE)) != 0 {
                return;
            }
        }
        let mut status = [0u8; 1];
        let base = dht20_i2c_base();
        if i2c_read(base, DHT20_ADDR, &mut status) && status[0] & 0x18 != 0x18 {
            let _ = i2c_write(base, DHT20_ADDR, &[0xbe, 0x08, 0x00]);
            rp2w_poll_delay(10);
        }
        unsafe {
            write_volatile(core::ptr::addr_of_mut!(RP2W_DHT20_INIT_DONE), 1);
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    fn dht20_read() -> Option<(i32, u32)> {
        dht20_init_once();
        let base = dht20_i2c_base();
        if !i2c_write(base, DHT20_ADDR, &[0xac, 0x33, 0x00]) {
            return None;
        }
        rp2w_poll_delay(80);
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
        if unsafe { read_volatile(core::ptr::addr_of!(RP2W_DHT20_BUS)) } == 0 {
            I2C0_BASE
        } else {
            DHT20_I2C_BASE
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    fn dht20_read() -> Option<(i32, u32)> {
        None
    }

    pub(super) fn rp2w_read_sensor_sample() -> Rp2wSensorSample {
        let light_raw = rp2w_adc0_read();
        let sample = match dht20_read() {
            Some((temp_c_x100, humidity_x100)) => {
                #[cfg(all(target_arch = "arm", target_os = "none"))]
                unsafe {
                    write_volatile(core::ptr::addr_of_mut!(RP2W_LAST_DHT20_OK), 1);
                    write_volatile(core::ptr::addr_of_mut!(RP2W_LAST_TEMP_C_X100), temp_c_x100);
                    write_volatile(
                        core::ptr::addr_of_mut!(RP2W_LAST_HUMIDITY_X100),
                        humidity_x100,
                    );
                }
                Rp2wSensorSample {
                    dht20_ok: true,
                    temp_c_x100,
                    humidity_x100,
                    light_raw,
                }
            }
            None => {
                #[cfg(all(target_arch = "arm", target_os = "none"))]
                unsafe {
                    write_volatile(core::ptr::addr_of_mut!(RP2W_LAST_DHT20_OK), 0);
                }
                Rp2wSensorSample {
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
                core::ptr::addr_of_mut!(RP2W_LAST_LIGHT_RAW),
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

    fn push_fixed_i16_x10(out: &mut [u8], len: &mut usize, value: i16) {
        let magnitude = if value < 0 {
            push_byte(out, len, b'-');
            i32::from(value).wrapping_neg() as u32
        } else {
            u32::from(value as u16)
        };
        push_u32(out, len, magnitude / 10);
        push_byte(out, len, b'.');
        push_byte(out, len, b'0' + (magnitude % 10) as u8);
    }

    fn push_fixed_u16_x10(out: &mut [u8], len: &mut usize, value: u16) {
        let magnitude = u32::from(value);
        push_u32(out, len, magnitude / 10);
        push_byte(out, len, b'.');
        push_byte(out, len, b'0' + (magnitude % 10) as u8);
    }

    fn clamp_i32_to_i16(value: i32) -> i16 {
        if value < i32::from(i16::MIN) {
            i16::MIN
        } else if value > i32::from(i16::MAX) {
            i16::MAX
        } else {
            value as i16
        }
    }

    fn clamp_u32_to_u16(value: u32) -> u16 {
        if value > u32::from(u16::MAX) {
            u16::MAX
        } else {
            value as u16
        }
    }

    fn rp2w_pico2w_sensor_sample(
        sample: Rp2wSensorSample,
        seq: u16,
    ) -> uno_q_heterogeneous::protocol::Pico2wSensorSample {
        let temperature_c_x10 = if sample.dht20_ok {
            clamp_i32_to_i16(sample.temp_c_x100 / 10)
        } else {
            0
        };
        let humidity_pct_x10 = if sample.dht20_ok {
            clamp_u32_to_u16(sample.humidity_x100 / 10)
        } else {
            0
        };
        match uno_q_heterogeneous::protocol::Pico2wSensorSample::new(
            uno_q_heterogeneous::protocol::PICO2W_SENSOR_STATUS_FRESH,
            temperature_c_x10,
            humidity_pct_x10,
            sample.light_raw,
            seq,
        ) {
            Ok(sample) => sample,
            Err(_) => uno_q_heterogeneous::protocol::Pico2wSensorSample::pending(seq),
        }
    }

    fn rp2w_encode_pico2w_sensor_sample(
        sample: uno_q_heterogeneous::protocol::Pico2wSensorSample,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        hibana::runtime::wire::WireEncode::encode_into(&sample, out).map_err(|_| {
            Rp2wWifiFrameError::Datagram(hibana_wifi::proto::udp::UdpDatagramError::PayloadTooLarge)
        })
    }

    pub(super) fn rp2w_read_pico2w_sensor_sample(seq: u16, out: &mut [u8]) -> Result<usize, ()> {
        let sample = rp2w_pico2w_sensor_sample(rp2w_read_sensor_sample(), seq);
        hibana::runtime::wire::WireEncode::encode_into(&sample, out).map_err(|_| ())
    }

    fn rp2w_format_pico2w_sensor_sample(
        sample: uno_q_heterogeneous::protocol::Pico2wSensorSample,
        out: &mut [u8],
    ) -> usize {
        let mut len = 0usize;
        match sample.status() {
            uno_q_heterogeneous::protocol::PICO2W_SENSOR_STATUS_PENDING => {
                push_bytes(out, &mut len, b"sensor pending\nseq:");
                push_u32(out, &mut len, u32::from(sample.seq()));
            }
            uno_q_heterogeneous::protocol::PICO2W_SENSOR_STATUS_STALE => {
                push_bytes(out, &mut len, b"sensor stale\nL:");
                push_u32(out, &mut len, u32::from(sample.light_raw()));
            }
            _ => {
                push_bytes(out, &mut len, b"T:");
                push_fixed_i16_x10(out, &mut len, sample.temperature_c_x10());
                push_bytes(out, &mut len, b"C H:");
                push_fixed_u16_x10(out, &mut len, sample.humidity_pct_x10());
                push_bytes(out, &mut len, b"%\nL:");
                push_u32(out, &mut len, u32::from(sample.light_raw()));
                push_bytes(out, &mut len, b" #");
                push_u32(out, &mut len, u32::from(sample.seq()));
            }
        }
        len
    }

    pub(super) fn rp2w_lcd_write_pico2w_sensor_sample(payload: &[u8]) -> bool {
        let sample = match <uno_q_heterogeneous::protocol::Pico2wSensorSample as hibana::runtime::wire::WirePayload>::decode_payload(
            hibana::runtime::wire::Payload::new(payload),
        ) {
            Ok(sample) => sample,
            Err(_) => return rp2w_lcd_write_lines(b"bad sample", b"typed payload"),
        };
        let mut text = [0u8; 64];
        let len = rp2w_format_pico2w_sensor_sample(sample, &mut text);
        rp2w_lcd_write_payload(&text[..len])
    }

    pub(super) fn rp2w_build_uno_q_sensor_ethernet_frame(
        sample: Rp2wSensorSample,
        target: Rp2wUnoQWifiTarget,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        let mut payload = [0u8; RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES];
        let payload_len =
            rp2w_encode_pico2w_sensor_sample(rp2w_pico2w_sensor_sample(sample, 0), &mut payload)?;
        rp2w_build_uno_q_payload_ethernet_frame(&payload[..payload_len], target, out)
    }

    pub(super) fn rp2w_build_uno_q_payload_ethernet_frame(
        payload: &[u8],
        target: Rp2wUnoQWifiTarget,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        type SensorDatagram =
            hibana_wifi::proto::udp::UdpDatagram<RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES>;

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

    pub(super) fn rp2w_build_uno_q_sensor_cyw43_frame(
        sample: Rp2wSensorSample,
        target: Rp2wUnoQWifiTarget,
        sdpcm_sequence: u8,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        let mut payload = [0u8; RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES];
        let payload_len = rp2w_encode_pico2w_sensor_sample(
            rp2w_pico2w_sensor_sample(sample, u16::from(sdpcm_sequence)),
            &mut payload,
        )?;
        rp2w_build_uno_q_payload_cyw43_frame(&payload[..payload_len], target, sdpcm_sequence, out)
    }

    pub(super) fn rp2w_build_uno_q_payload_cyw43_frame(
        payload: &[u8],
        target: Rp2wUnoQWifiTarget,
        sdpcm_sequence: u8,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        type SensorDatagram =
            hibana_wifi::proto::udp::UdpDatagram<RP2W_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES>;

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

    pub(super) fn rp2w_read_uno_q_sensor_cyw43_frame(
        target: Rp2wUnoQWifiTarget,
        sdpcm_sequence: u8,
        out: &mut [u8],
    ) -> Result<usize, Rp2wWifiFrameError> {
        rp2w_build_uno_q_sensor_cyw43_frame(rp2w_read_sensor_sample(), target, sdpcm_sequence, out)
    }

    pub(super) fn rp2w_cyw43_send_uno_q_payload_frame(
        driver: &mut Rp2wCyw43GspiDriver,
        target: Rp2wUnoQWifiTarget,
        payload: &[u8],
        ethernet_frame: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, Rp2wCyw43GspiError> {
        let len = rp2w_build_uno_q_payload_ethernet_frame(payload, target, ethernet_frame)
            .map_err(rp2w_wifi_frame_error_to_gspi_error)?;
        driver.send_ethernet_frame(&ethernet_frame[..len], scratch)?;
        Ok(len)
    }

    pub(super) fn rp2w_cyw43_send_uno_q_datagram_frame<const N: usize>(
        driver: &mut Rp2wCyw43GspiDriver,
        target: Rp2wUnoQWifiTarget,
        datagram: &hibana_wifi::proto::udp::UdpDatagram<N>,
        ethernet_frame: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, Rp2wCyw43GspiError> {
        let len = hibana_wifi::proto::udp::build_udp_tx_ethernet_frame(
            ethernet_frame,
            target.local_mac,
            target.uno_q_mac,
            target.local_ip,
            datagram,
        )
        .map_err(|error| {
            rp2w_wifi_frame_error_to_gspi_error(Rp2wWifiFrameError::Frame(error.into()))
        })?;
        driver.send_ethernet_frame(&ethernet_frame[..len], scratch)?;
        Ok(len)
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Rp2wUnoQWifiSendError {
        Frame(Rp2wWifiFrameError),
        Driver(Rp2wCyw43DriverError),
    }

    impl From<Rp2wWifiFrameError> for Rp2wUnoQWifiSendError {
        fn from(error: Rp2wWifiFrameError) -> Self {
            Self::Frame(error)
        }
    }

    impl From<Rp2wCyw43DriverError> for Rp2wUnoQWifiSendError {
        fn from(error: Rp2wCyw43DriverError) -> Self {
            Self::Driver(error)
        }
    }

    pub(super) fn rp2w_send_uno_q_sensor_cyw43_frame(
        target: Rp2wUnoQWifiTarget,
        sdpcm_sequence: u8,
        dst_node: u8,
        scratch: &mut [u8],
    ) -> Result<usize, Rp2wUnoQWifiSendError> {
        let len = rp2w_read_uno_q_sensor_cyw43_frame(target, sdpcm_sequence, scratch)?;
        rp2w_cyw43_send_frame_qemu_model(dst_node, &scratch[..len])?;
        Ok(len)
    }

    #[cfg(test)]
    mod uno_q_wifi_tests {
        use hibana::runtime::wire::{Payload, WirePayload};
        use hibana_wifi::proto::{
            cyw43::{BDC_HEADER_LEN, SDPCM_HEADER_LEN, SdpcmChannel, SdpcmHeader},
            ethernet::{ETH_HEADER_LEN, IPV4_HEADER_LEN, Ipv4Addr, MacAddr},
            udp::UNO_Q_SENSOR_UDP_PORT,
        };
        use uno_q_heterogeneous::protocol::{
            PICO2W_SENSOR_SAMPLE_BYTES, PICO2W_SENSOR_STATUS_FRESH, Pico2wSensorSample,
        };

        use super::{
            RP2W_UNO_Q_SENSOR_UDP_SRC_PORT, Rp2wSensorSample, Rp2wUnoQWifiTarget,
            rp2w_build_uno_q_sensor_cyw43_frame,
        };

        #[test]
        fn sensor_sample_materializes_as_uno_q_cyw43_data_frame() {
            let target = Rp2wUnoQWifiTarget::new(
                MacAddr([0x02, 0x12, 0x34, 0x56, 0x78, 0x9a]),
                MacAddr([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
                Ipv4Addr([172, 20, 10, 5]),
                Ipv4Addr([172, 20, 10, 2]),
                RP2W_UNO_Q_SENSOR_UDP_SRC_PORT,
            );
            let sample = Rp2wSensorSample {
                dht20_ok: true,
                temp_c_x100: 2260,
                humidity_x100: 6000,
                light_raw: 2500,
            };
            let mut frame = [0u8; 192];
            let len = rp2w_build_uno_q_sensor_cyw43_frame(sample, target, 9, &mut frame).unwrap();

            let header = SdpcmHeader::decode(&frame[..SDPCM_HEADER_LEN]).unwrap();
            assert_eq!(header.total_len as usize, len);
            assert_eq!(header.sequence, 9);
            assert_eq!(header.channel, SdpcmChannel::Data);
            assert_eq!(header.header_len, (SDPCM_HEADER_LEN + 2) as u8);
            assert_eq!(
                &frame[SDPCM_HEADER_LEN..usize::from(header.header_len)],
                &[0, 0]
            );
            assert_eq!(
                &frame[usize::from(header.header_len)
                    ..usize::from(header.header_len) + BDC_HEADER_LEN],
                &[0x20, 0, 0, 0]
            );

            let ethernet_start = usize::from(header.header_len) + BDC_HEADER_LEN;
            let udp_payload_start = ethernet_start + ETH_HEADER_LEN + IPV4_HEADER_LEN + 8;
            assert_eq!(
                &frame[ethernet_start..ethernet_start + 6],
                &target.uno_q_mac.0
            );
            assert_eq!(
                u16::from_be_bytes([frame[ethernet_start + 34], frame[ethernet_start + 35]]),
                RP2W_UNO_Q_SENSOR_UDP_SRC_PORT
            );
            assert_eq!(
                u16::from_be_bytes([frame[ethernet_start + 36], frame[ethernet_start + 37]]),
                UNO_Q_SENSOR_UDP_PORT
            );
            assert!(len >= udp_payload_start + PICO2W_SENSOR_SAMPLE_BYTES);
            let payload = &frame[udp_payload_start..udp_payload_start + PICO2W_SENSOR_SAMPLE_BYTES];
            let decoded = Pico2wSensorSample::decode_payload(Payload::new(payload)).unwrap();
            assert_eq!(
                decoded,
                Pico2wSensorSample::new(PICO2W_SENSOR_STATUS_FRESH, 226, 600, 2500, 9).unwrap()
            );
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub(super) fn rp2w_board_init() {
        unsafe {
            if read_volatile(core::ptr::addr_of!(RP2W_BOARD_INIT_DONE)) != 0 {
                return;
            }
        }
        rp2w_uart0_init();
        rp2w_uart0_write_str("\r\nrp2w sensor panel boot\r\n");
        rp2w_i2c0_init();
        rp2w_i2c1_init();
        rp2w_adc0_init();
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
            write_volatile(core::ptr::addr_of_mut!(RP2W_I2C_DETECT_MASK), detect_mask);
            write_volatile(
                core::ptr::addr_of_mut!(RP2W_LCD_BUS),
                if lcd_i2c0 {
                    0
                } else if lcd_i2c1 {
                    1
                } else {
                    0
                },
            );
            write_volatile(
                core::ptr::addr_of_mut!(RP2W_DHT20_BUS),
                if dht_i2c1 {
                    1
                } else if dht_i2c0 {
                    0
                } else {
                    1
                },
            );
        }
        let lcd_ok = rp2w_lcd_init();
        unsafe {
            write_volatile(core::ptr::addr_of_mut!(RP2W_LCD_INIT_OK), lcd_ok as u32);
        }
        let _ = rp2w_lcd_write_lines(b"RP2W sensor", b"booting");
        if lcd_ok {
            rp2w_uart0_write_str("lcd ok\r\n");
        } else {
            rp2w_uart0_write_str("lcd init failed\r\n");
        }
        unsafe {
            write_volatile(core::ptr::addr_of_mut!(RP2W_BOARD_INIT_DONE), 1);
        }
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    pub(super) fn rp2w_board_init() {}
}
