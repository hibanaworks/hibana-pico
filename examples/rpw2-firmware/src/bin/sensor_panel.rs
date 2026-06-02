#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use hibana::{g, integration::runtime::LabelUniverse};
use hibana_pico::{
    appkit,
    choreography::protocol::{
        EngineReq, EngineRet, FdRead, FdReadDone, FdWrite, FdWriteDone, LABEL_WASI_FD_READ,
        LABEL_WASI_FD_READ_RET, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PATH_OPEN,
        LABEL_WASI_PATH_OPEN_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASI_PROC_EXIT, PathOpen, PathOpened, PollReady, WasiImportLoopBreak,
        WasiImportLoopContinue,
    },
};
use hibana_wifi::proto::{
    ethernet::Ipv4Addr,
    protocol::{labels as wifi_labels, roles as wifi_roles},
    udp::{UNO_Q_SENSOR_UDP_PORT, UdpDatagram},
};
use rpw2_firmware::{DriverImage, EngineImage, Rpw2Artifacts, Rpw2CapsuleFacts, Rpw2Placement};

const DEVICE_PREOPEN_FD: u8 = 9;
const SENSOR_SAMPLE_FD: u8 = 3;
const DISPLAY_FD: u8 = 4;
const UNO_Q_UDP_FD: u8 = 5;
const FD_READ_RIGHT: u64 = 1 << 1;
const FD_WRITE_RIGHT: u64 = 1 << 6;
const EXPECTED_POLL_TIMEOUT_MS: u64 = 1_000;
const READY_CYCLES: u32 = 1;
#[cfg(feature = "embed-cyw43-artifacts")]
const RPW2_WIFI_LOCAL_MAC: [u8; 6] = [0x02, 0x12, 0x34, 0x56, 0x78, 0x9a];
#[cfg(feature = "embed-cyw43-artifacts")]
const UNO_Q_WIFI_MAC: [u8; 6] = [0x14, 0xb5, 0xcd, 0x0f, 0x41, 0x7d];
#[cfg(feature = "embed-cyw43-artifacts")]
const RPW2_WIFI_LOCAL_IP: [u8; 4] = [172, 20, 10, 5];
const UNO_Q_WIFI_IP: [u8; 4] = [172, 20, 10, 8];
const UNO_Q_SENSOR_UDP_PATH: &[u8] = b"device/rpw2/udp/172.20.10.8/8787";
#[cfg(feature = "embed-cyw43-artifacts")]
const RPW2_WIFI_SSID: &[u8] = b"iPad (2)";
#[cfg(feature = "embed-cyw43-artifacts")]
const RPW2_WIFI_KEY: &[u8] = b"hayato8810";
#[allow(dead_code)]
const WIFI_TRACE_BOOT_ATTEMPT: u32 = 0x5749_1000;
#[cfg(feature = "embed-cyw43-artifacts")]
const WIFI_TRACE_WIFI_SEND_OK: u32 = 0x5749_4200;
#[cfg(not(feature = "embed-cyw43-artifacts"))]
#[allow(dead_code)]
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
const WASI_ERRNO_IO: u16 = 5;

type SensorWifiDatagram = UdpDatagram<{ rpw2_firmware::RPW2_UNO_Q_SENSOR_UDP_PAYLOAD_BYTES }>;
type WifiTxUdpMsg = g::Msg<{ wifi_labels::TX_UDP }, SensorWifiDatagram>;
type WifiTxResultMsg = g::Msg<{ wifi_labels::TX_RESULT }, u32>;

#[derive(Clone, Copy, Debug, Default)]
pub struct SensorPanelUniverse;

impl LabelUniverse for SensorPanelUniverse {
    const MAX_LABEL: u8 = wifi_labels::MAX;
}

#[cfg(feature = "embed-wasip1-artifacts")]
const WASM_SENSOR_PANEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasip1-apps/wasm32-wasip1/release/rpw2-sensor-panel-guest.wasm"
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
    b"device/rpw2/sample",
    appkit::ObjectId(1),
    appkit::FdSpec::new(SENSOR_SAMPLE_FD as u32, FD_READ_RIGHT, 1),
);
const DISPLAY: appkit::ChoreoFsObject = appkit::ChoreoFsObject::new(
    b"device/rpw2/display",
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

pub struct SensorPanel;
pub struct SensorPanelLocal;

#[derive(Debug)]
pub enum SensorPanelError {
    Endpoint(hibana::EndpointError),
    Wire(hibana::integration::wire::CodecError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for SensorPanelError {
    fn from(error: hibana::EndpointError) -> Self {
        Self::Endpoint(error)
    }
}

impl From<hibana::integration::wire::CodecError> for SensorPanelError {
    fn from(error: hibana::integration::wire::CodecError) -> Self {
        Self::Wire(error)
    }
}

impl appkit::Capsule for SensorPanel {
    type Universe = SensorPanelUniverse;
    type Placement = Rpw2Placement;
    type Local = SensorPanelLocal;
    type Report = core::convert::Infallible;

    fn choreography() -> impl hibana::integration::program::Projectable {
        let path_open = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_PATH_OPEN, EngineReq>, 1>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>, 1>(),
            )
        };
        let fd_read = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_FD_READ, EngineReq>, 1>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 1>(),
            )
        };
        let fd_write = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
            )
        };
        let wifi_udp_tx = || {
            g::seq(
                g::send::<
                    { wifi_roles::CHOREOGRAPHIC_KERNEL },
                    { wifi_roles::CYW43_DRIVER },
                    WifiTxUdpMsg,
                    0,
                >(),
                g::send::<
                    { wifi_roles::CYW43_DRIVER },
                    { wifi_roles::CHOREOGRAPHIC_KERNEL },
                    WifiTxResultMsg,
                    0,
                >(),
            )
        };
        let udp_fd_write = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                g::seq(
                    wifi_udp_tx(),
                    g::send::<0, 1, g::Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
                ),
            )
        };
        let poll = || {
            g::seq(
                g::send::<1, 0, g::Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>, 1>(),
                g::send::<0, 1, g::Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>, 1>(),
            )
        };
        let sample_cycle = || {
            g::seq(
                fd_read(),
                g::seq(fd_write(), g::seq(udp_fd_write(), poll())),
            )
        };
        let admitted_cycle = || {
            g::route(
                g::seq(g::send::<0, 0, WasiImportLoopContinue, 1>(), sample_cycle()),
                g::seq(
                    g::send::<0, 0, WasiImportLoopBreak, 1>(),
                    g::send::<1, 0, g::Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
                ),
            )
        };
        g::seq(
            path_open(),
            g::seq(path_open(), g::seq(path_open(), admitted_cycle())),
        )
    }
}

impl Rpw2CapsuleFacts for SensorPanel {
    type DriverArtifact = appkit::NoWasi;
    type EngineArtifact = appkit::WasiImage<'static>;

    const DRIVER_IMAGE_ID: appkit::ImageId = appkit::ImageId(30);
    const ENGINE_IMAGE_ID: appkit::ImageId = appkit::ImageId(31);

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
                rpw2_firmware::reset_choreofs_markers();
                rpw2_firmware::record_choreofs_engine_status(
                    rpw2_firmware::CHOREOFS_DRIVER_STARTED,
                );
                rpw2_firmware::rpw2_board_init();
                rpw2_firmware::record_choreofs_engine_status(rpw2_firmware::CHOREOFS_GPIO_READY);

                driver_path_open(&mut ctx, SENSOR_SAMPLE_FD, SENSOR_SAMPLE.object()).await?;
                driver_path_open(&mut ctx, DISPLAY_FD, DISPLAY.object()).await?;
                driver_path_open(&mut ctx, UNO_Q_UDP_FD, UNO_Q_UDP.object()).await?;

                let mut cycles = 0u32;
                loop {
                    driver_loop_continue(&mut ctx).await?;
                    driver_fd_read(&mut ctx).await?;
                    driver_display_fd_write(&mut ctx).await?;
                    driver_udp_fd_write(&mut ctx).await?;
                    driver_poll_oneoff(&mut ctx).await?;
                    cycles = cycles.saturating_add(1);
                    if cycles == READY_CYCLES {
                        rpw2_firmware::mark_runtime_ready();
                        rpw2_firmware::mark_success(
                            <SensorPanel as Rpw2CapsuleFacts>::SUCCESS_RESULT,
                        );
                    }
                }
            } else if ROLE == wifi_roles::CYW43_DRIVER {
                rpw2_firmware::rpw2_board_init();
                rpw2_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ROLE_READY);
                let mut wifi = UnoQWifiSender::unavailable();
                loop {
                    cyw43_driver_tx_udp(&mut ctx, &mut wifi).await?;
                }
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, SensorPanel, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

struct UnoQWifiSender {
    #[cfg(feature = "embed-cyw43-artifacts")]
    driver: Option<rpw2_firmware::Rpw2Cyw43GspiDriver>,
    #[cfg(feature = "embed-cyw43-artifacts")]
    sequence: u8,
}

impl UnoQWifiSender {
    fn unavailable() -> Self {
        Self {
            #[cfg(feature = "embed-cyw43-artifacts")]
            driver: None,
            #[cfg(feature = "embed-cyw43-artifacts")]
            sequence: 0,
        }
    }

    #[cfg(feature = "embed-cyw43-artifacts")]
    fn ready(driver: rpw2_firmware::Rpw2Cyw43GspiDriver) -> Self {
        Self {
            driver: Some(driver),
            sequence: 0,
        }
    }

    fn is_ready(&self) -> bool {
        #[cfg(feature = "embed-cyw43-artifacts")]
        {
            self.driver.is_some()
        }
        #[cfg(not(feature = "embed-cyw43-artifacts"))]
        {
            false
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
        match rpw2_firmware::rpw2_cyw43_send_uno_q_datagram_frame(
            driver,
            uno_q_wifi_target(),
            datagram,
            &mut ethernet_frame,
            &mut scratch,
        ) {
            Ok(_) => {
                rpw2_firmware::record_choreofs_driver_trace(
                    WIFI_TRACE_TX_OK | u32::from(self.sequence),
                );
                lcd_udp_sent(self.sequence, datagram.payload());
                self.sequence = self.sequence.wrapping_add(1);
                Ok(datagram.payload_len())
            }
            Err(error) => {
                rpw2_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ERR);
                rpw2_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
                lcd_network_status(b"UDP tx failed", b"retry next cycle");
                self.driver = None;
                Err(())
            }
        }
    }

    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    fn send_datagram(&mut self, datagram: &SensorWifiDatagram) -> Result<usize, ()> {
        core::hint::black_box(datagram);
        lcd_network_status(b"UDP disabled", b"build feature");
        Err(())
    }

    #[cfg(feature = "embed-cyw43-artifacts")]
    fn rejoin_preserving_sequence(&mut self) {
        let sequence = self.sequence;
        let mut next = boot_uno_q_wifi();
        next.sequence = sequence;
        *self = next;
    }

    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    fn rejoin_preserving_sequence(&mut self) {
        lcd_network_status(b"WiFi disabled", b"build feature");
    }
}

#[allow(dead_code)]
fn boot_uno_q_wifi() -> UnoQWifiSender {
    lcd_network_status(b"WiFi joining", b"iPad -> UnoQ");
    rpw2_firmware::record_choreofs_driver_trace(WIFI_TRACE_BOOT_ATTEMPT);
    #[cfg(feature = "embed-cyw43-artifacts")]
    {
        let _firmware_inputs = (CYW43_FIRMWARE.len(), CYW43_CLM.len(), CYW43_NVRAM.len());
        rpw2_firmware::record_choreofs_engine_status(
            0x5749_3000 | rpw2_firmware::rpw2_cyw43_gspi_line_diag(),
        );
        match rpw2_firmware::rpw2_cyw43_real_wifi_join_driver(
            CYW43_FIRMWARE,
            CYW43_NVRAM,
            CYW43_CLM,
            RPW2_WIFI_SSID,
            RPW2_WIFI_KEY,
            RPW2_WIFI_LOCAL_MAC,
        ) {
            Ok((driver, clock, ht_clock, bssid)) => {
                rpw2_firmware::record_choreofs_engine_status(u32::from(ht_clock));
                rpw2_firmware::record_choreofs_engine_error_code(u32::from_be_bytes([
                    bssid[0], bssid[1], bssid[2], bssid[3],
                ]));
                rpw2_firmware::record_choreofs_driver_trace(
                    WIFI_TRACE_WIFI_SEND_OK | u32::from(clock),
                );
                lcd_network_status(b"WiFi joined", b"UnoQ 172.20.10.8");
                UnoQWifiSender::ready(driver)
            }
            Err(error) => {
                rpw2_firmware::record_choreofs_driver_trace(wifi_gspi_error_trace(error));
                lcd_network_status(b"WiFi failed", b"check hotspot");
                UnoQWifiSender::unavailable()
            }
        }
    }
    #[cfg(not(feature = "embed-cyw43-artifacts"))]
    {
        rpw2_firmware::record_choreofs_driver_trace(WIFI_TRACE_BOOT_ERR);
        lcd_network_status(b"WiFi disabled", b"build feature");
        UnoQWifiSender::unavailable()
    }
}

fn lcd_network_status(line1: &[u8], line2: &[u8]) {
    let _ = rpw2_firmware::rpw2_lcd_write_lines(line1, line2);
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn lcd_udp_sent(sequence: u8, payload: &[u8]) {
    let first_line = payload_first_line(payload);
    let mut second_line = [b' '; 16];
    let label = b"WiFi TX #";
    second_line[..label.len()].copy_from_slice(label);
    second_line[label.len()] = b'0' + (sequence / 100);
    second_line[label.len() + 1] = b'0' + ((sequence / 10) % 10);
    second_line[label.len() + 2] = b'0' + (sequence % 10);
    let line1: &[u8] = if first_line.is_empty() {
        &b"RPW2 sensor"[..]
    } else {
        first_line
    };
    let _ = rpw2_firmware::rpw2_lcd_write_lines(line1, &second_line);
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn payload_first_line(payload: &[u8]) -> &[u8] {
    let mut end = payload.len();
    let mut index = 0usize;
    while index < payload.len() {
        if payload[index] == b'\n' || payload[index] == b'\r' {
            end = index;
            break;
        }
        index += 1;
    }
    &payload[..end]
}

#[cfg(feature = "embed-cyw43-artifacts")]
fn wifi_gspi_error_trace(error: rpw2_firmware::Rpw2Cyw43GspiError) -> u32 {
    match error {
        hibana_wifi::cyw43::gspi::Cyw43GspiError::Bus(
            rpw2_firmware::Rpw2Cyw43SpiError::Timeout,
        ) => WIFI_TRACE_GSPI_BUS_TIMEOUT,
        hibana_wifi::cyw43::gspi::Cyw43GspiError::Bus(
            rpw2_firmware::Rpw2Cyw43SpiError::Unavailable,
        ) => WIFI_TRACE_GSPI_BUS_UNAVAILABLE,
        hibana_wifi::cyw43::gspi::Cyw43GspiError::TestPatternMismatch(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_GSPI_TEST_MISMATCH ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::BackplaneClockTimeout(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_BACKPLANE_CLOCK_TIMEOUT ^ u32::from(value)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::HtClockTimeout(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_HT_CLOCK_TIMEOUT ^ u32::from(value)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::F2ReadyTimeout(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_F2_READY_TIMEOUT ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::F2PacketTimeout(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_F2_PACKET_TIMEOUT ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::IoctlTimeout(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(u32::from(value));
            WIFI_TRACE_IOCTL_TIMEOUT ^ u32::from(value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::IoctlStatus(value) => {
            rpw2_firmware::record_choreofs_engine_error_code(value);
            WIFI_TRACE_IOCTL_STATUS ^ (value & 0xff)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::InvalidIoctlResponse => {
            WIFI_TRACE_IOCTL_RESPONSE_ERR
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::CoreNotInReset(core_id, resetctrl) => {
            rpw2_firmware::record_choreofs_engine_error_code(
                (u32::from(core_id) << 8) | u32::from(resetctrl),
            );
            WIFI_TRACE_CORE_NOT_IN_RESET ^ u32::from(resetctrl)
        }
        hibana_wifi::cyw43::gspi::Cyw43GspiError::CoreNotUp(core_id, ioctrl, resetctrl) => {
            rpw2_firmware::record_choreofs_engine_error_code(
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
fn uno_q_wifi_target() -> rpw2_firmware::Rpw2UnoQWifiTarget {
    rpw2_firmware::Rpw2UnoQWifiTarget::new(
        hibana_wifi::proto::ethernet::MacAddr(RPW2_WIFI_LOCAL_MAC),
        hibana_wifi::proto::ethernet::MacAddr(UNO_Q_WIFI_MAC),
        hibana_wifi::proto::ethernet::Ipv4Addr(RPW2_WIFI_LOCAL_IP),
        hibana_wifi::proto::ethernet::Ipv4Addr(UNO_Q_WIFI_IP),
        rpw2_firmware::RPW2_UNO_Q_SENSOR_UDP_SRC_PORT,
    )
}

async fn recv_engine_req<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<EngineReq, SensorPanelError> {
    loop {
        match ctx.endpoint().recv::<g::Msg<LABEL, EngineReq>>().await {
            Ok(request) => return Ok(request),
            Err(error) => {
                core::hint::black_box(error);
                rpw2_firmware::rpw2_poll_delay(1);
            }
        }
    }
}

async fn send_engine_ret<const ROLE: u8, const LABEL: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    reply: EngineRet,
) -> Result<(), SensorPanelError> {
    ctx.endpoint()
        .flow::<g::Msg<LABEL, EngineRet>>()?
        .send(&reply)
        .await?;
    Ok(())
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
    rpw2_firmware::record_choreofs_path_open(object);
    send_engine_ret::<ROLE, LABEL_WASI_PATH_OPEN_RET>(
        ctx,
        EngineRet::PathOpened(PathOpened::new(fact.fd() as u8, 0)),
    )
    .await
}

async fn driver_loop_continue<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    ctx.endpoint()
        .flow::<WasiImportLoopContinue>()?
        .send(&())
        .await?;
    Ok(())
}

async fn driver_fd_read<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_FD_READ>(ctx).await? {
        EngineReq::FdRead(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_fd_read(ctx, request).await
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
    let mut buffer = [0u8; 64];
    let len = rpw2_firmware::rpw2_read_sensor_text(&mut buffer);
    let bytes = bounded_prefix(&buffer[..len], request.max_len() as usize);
    let reply = EngineRet::FdReadDone(FdReadDone::new_with_lease(
        request.fd(),
        request.lease_id(),
        bytes,
    )?);
    rpw2_firmware::record_choreofs_path_open(fact.object());
    send_engine_ret::<ROLE, LABEL_WASI_FD_READ_RET>(ctx, reply).await
}

async fn driver_display_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_FD_WRITE>(ctx).await? {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_display_fd_write(ctx, request).await
}

async fn handle_display_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdWrite,
) -> Result<(), SensorPanelError> {
    let fact = validate_fd_write(ctx, request, DISPLAY_FD, DISPLAY.object())?;
    rpw2_firmware::rpw2_uart0_write_bytes(request.as_bytes());
    let _ = rpw2_firmware::rpw2_lcd_write_payload(request.as_bytes());
    rpw2_firmware::record_choreofs_fd_write(fact.object());
    send_engine_ret::<ROLE, LABEL_WASI_FD_WRITE_RET>(
        ctx,
        EngineRet::FdWriteDone(FdWriteDone::new(request.fd(), request.len() as u8)),
    )
    .await
}

async fn driver_udp_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
) -> Result<(), SensorPanelError> {
    let request = match recv_engine_req::<ROLE, LABEL_WASI_FD_WRITE>(ctx).await? {
        EngineReq::FdWrite(request) => request,
        other => {
            core::hint::black_box(other);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    handle_udp_fd_write(ctx, request).await
}

async fn handle_udp_fd_write<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    request: FdWrite,
) -> Result<(), SensorPanelError> {
    let fact = validate_fd_write(ctx, request, UNO_Q_UDP_FD, UNO_Q_UDP.object())?;
    let datagram = match SensorWifiDatagram::new(
        Ipv4Addr(UNO_Q_WIFI_IP),
        rpw2_firmware::RPW2_UNO_Q_SENSOR_UDP_SRC_PORT,
        UNO_Q_SENSOR_UDP_PORT,
        request.as_bytes(),
    ) {
        Ok(datagram) => datagram,
        Err(error) => {
            core::hint::black_box(error);
            return Err(SensorPanelError::RuntimeViolation);
        }
    };
    ctx.endpoint()
        .flow::<WifiTxUdpMsg>()?
        .send(&datagram)
        .await?;

    let result = ctx.endpoint().recv::<WifiTxResultMsg>().await?;
    let reply = if result & 0x8000_0000 == 0 {
        if result as usize != request.len() {
            core::hint::black_box(result);
            return Err(SensorPanelError::RuntimeViolation);
        }
        rpw2_firmware::record_choreofs_fd_write(fact.object());
        FdWriteDone::new(request.fd(), request.len() as u8)
    } else {
        rpw2_firmware::record_choreofs_driver_trace(WIFI_TRACE_TX_ERR | (result & 0xff));
        FdWriteDone::new_with_errno(request.fd(), 0, WASI_ERRNO_IO)
    };
    send_engine_ret::<ROLE, LABEL_WASI_FD_WRITE_RET>(ctx, EngineRet::FdWriteDone(reply)).await
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

async fn cyw43_driver_tx_udp<const ROLE: u8>(
    ctx: &mut appkit::DriverCtx<'_, SensorPanel, ROLE>,
    wifi: &mut UnoQWifiSender,
) -> Result<(), SensorPanelError> {
    let branch = ctx.endpoint().offer().await?;
    if branch.label() != wifi_labels::TX_UDP {
        core::hint::black_box(branch.label());
        return Err(SensorPanelError::RuntimeViolation);
    }
    let datagram = branch.decode::<WifiTxUdpMsg>().await?;
    rpw2_firmware::record_choreofs_driver_trace(
        WIFI_TRACE_TX_REQ | ((datagram.payload_len() as u32) & 0xff),
    );
    if !wifi.is_ready() {
        wifi.rejoin_preserving_sequence();
    }
    match wifi.send_datagram(&datagram) {
        Ok(written) => {
            ctx.endpoint()
                .flow::<WifiTxResultMsg>()?
                .send(&(written as u32))
                .await?;
        }
        Err(()) => {
            ctx.endpoint()
                .flow::<WifiTxResultMsg>()?
                .send(&0x8000_0001)
                .await?;
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
    rpw2_firmware::record_choreofs_poll_timeout(request.timeout_tick());
    if request.timeout_tick() != EXPECTED_POLL_TIMEOUT_MS {
        #[cfg(feature = "wasm-engine-core")]
        rpw2_firmware::record_choreofs_engine_error_code(0x5250_d000);
        core::hint::black_box(request);
        return Err(SensorPanelError::RuntimeViolation);
    }
    rpw2_firmware::rpw2_poll_delay(request.timeout_tick());
    rpw2_firmware::record_choreofs_poll();
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

fn bounded_prefix(bytes: &[u8], max_len: usize) -> &[u8] {
    let len = core::cmp::min(bytes.len(), max_len);
    &bytes[..len]
}

impl appkit::ArtifactForImage<SensorPanel, DriverImage> for Rpw2Artifacts {
    fn artifact_for_image(&self) -> appkit::NoWasi {
        appkit::NoWasi
    }
}

impl appkit::ArtifactForImage<SensorPanel, EngineImage> for Rpw2Artifacts {
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        appkit::WasiImage::from_static(WASM_SENSOR_PANEL)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    rpw2_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn rpw2_selected_run() -> ! {
    rpw2_firmware::run::<SensorPanel>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    rpw2_firmware::run::<SensorPanel>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    rpw2_firmware::run::<SensorPanel>()
}
