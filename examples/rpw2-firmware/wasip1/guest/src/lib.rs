#![no_std]

use hibana_wasip1_guest::{Result, time};

pub const DEVICE_PREOPEN_FD: u32 = 9;
pub const SENSOR_SAMPLE_PATH: &str = "device/rpw2/sample";
pub const DISPLAY_PATH: &str = "device/rpw2/display";
pub const UNO_Q_SENSOR_UDP_PATH: &str = "device/rpw2/udp/172.20.10.8/8787";

pub fn sleep_ms(ms: u32) -> Result<()> {
    time::sleep_ms(ms)
}
