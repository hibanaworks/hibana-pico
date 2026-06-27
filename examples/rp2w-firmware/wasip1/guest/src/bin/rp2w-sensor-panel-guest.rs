use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
    thread,
    time::Duration,
};

const SENSOR_SAMPLE_PATH: &str = "/device/rp2w/sample";
const DISPLAY_PATH: &str = "/device/rp2w/display";
const UNO_Q_SENSOR_UDP_PATH: &str = "/device/rp2w/udp/uno-q";
const SAMPLE_MS: u32 = 1_000;
const SENSOR_READ_BYTES: usize = 9;

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> io::Result<()> {
    let mut sample = OpenOptions::new().read(true).open(SENSOR_SAMPLE_PATH)?;
    let mut display = OpenOptions::new().write(true).open(DISPLAY_PATH)?;
    let mut uno_q = OpenOptions::new().write(true).open(UNO_Q_SENSOR_UDP_PATH)?;
    let mut buffer = [0u8; SENSOR_READ_BYTES];

    loop {
        let len = sample.read(&mut buffer)?;
        display.write_all(&buffer[..len])?;
        display.flush()?;
        uno_q.write_all(&buffer[..len])?;
        uno_q.flush()?;
        thread::sleep(Duration::from_millis(u64::from(SAMPLE_MS)));
    }
}

#[cold]
fn abort() -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        core::hint::spin_loop();
    }
}
