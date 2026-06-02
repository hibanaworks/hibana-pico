use hibana_wasip1_guest::choreofs;
use rpw2_wasip1_guest::{
    DEVICE_PREOPEN_FD, DISPLAY_PATH, SENSOR_SAMPLE_PATH, UNO_Q_SENSOR_UDP_PATH, sleep_ms,
};

const SAMPLE_MS: u32 = 1_000;
const SENSOR_READ_BYTES: usize = 30;

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasip1_guest::Result<()> {
    let sample = choreofs::open_read(DEVICE_PREOPEN_FD, SENSOR_SAMPLE_PATH)?;
    let display = choreofs::open_write(DEVICE_PREOPEN_FD, DISPLAY_PATH)?;
    let uno_q = choreofs::open_write(DEVICE_PREOPEN_FD, UNO_Q_SENSOR_UDP_PATH)?;
    let mut buffer = [0u8; SENSOR_READ_BYTES];

    loop {
        let len = sample.read_once(&mut buffer)?;
        display.write_once_exact(&buffer[..len])?;
        let _ = uno_q.write_once_exact(&buffer[..len]);
        sleep_ms(SAMPLE_MS)?;
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
