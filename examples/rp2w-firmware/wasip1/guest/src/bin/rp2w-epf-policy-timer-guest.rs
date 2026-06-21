use hibana_wasip1_guest::choreofs;

const DEVICE_PREOPEN_FD: u32 = 9;
const SENSOR_SAMPLE_PATH: &str = "device/rp2w/sample";
const DISPLAY_PATH: &str = "device/rp2w/display";

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasip1_guest::Result<()> {
    let mut buffer = [0u8; 30];
    loop {
        let sample = choreofs::open_read(DEVICE_PREOPEN_FD, SENSOR_SAMPLE_PATH)?;
        let display = choreofs::open_write(DEVICE_PREOPEN_FD, DISPLAY_PATH)?;
        let len = sample.read_once(&mut buffer)?;
        display.write_once_exact(&buffer[..len])?;
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
