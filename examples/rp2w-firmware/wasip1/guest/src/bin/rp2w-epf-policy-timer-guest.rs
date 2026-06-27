use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
};

const SENSOR_SAMPLE_PATH: &str = "/device/rp2w/sample";
const DISPLAY_PATH: &str = "/device/rp2w/display";

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> io::Result<()> {
    let mut buffer = [0u8; 30];
    loop {
        let mut sample = OpenOptions::new().read(true).open(SENSOR_SAMPLE_PATH)?;
        let mut display = OpenOptions::new().write(true).open(DISPLAY_PATH)?;
        let len = sample.read(&mut buffer)?;
        display.write_all(&buffer[..len])?;
        display.flush()?;
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
