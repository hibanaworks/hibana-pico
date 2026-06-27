use std::{
    fs::OpenOptions,
    io::{self, Write},
    thread,
    time::Duration,
};

const COLOR_STEP: Duration = Duration::from_millis(40);
const YELLOW_BLINK_STEP: Duration = Duration::from_millis(20);
const TRAFFIC_STATE_PATH: &str = "/device/traffic/state";
const TRAFFIC_GREEN: &[u8] = b"G";
const TRAFFIC_YELLOW: &[u8] = b"Y";
const TRAFFIC_DARK: &[u8] = b"0";
const TRAFFIC_RED: &[u8] = b"R";

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> io::Result<()> {
    let mut traffic = open_traffic_state()?;

    set_state(&mut traffic, TRAFFIC_GREEN, COLOR_STEP)?;
    set_state(&mut traffic, TRAFFIC_YELLOW, COLOR_STEP)?;

    set_state(&mut traffic, TRAFFIC_DARK, YELLOW_BLINK_STEP)?;
    set_state(&mut traffic, TRAFFIC_YELLOW, YELLOW_BLINK_STEP)?;
    set_state(&mut traffic, TRAFFIC_DARK, YELLOW_BLINK_STEP)?;
    set_state(&mut traffic, TRAFFIC_YELLOW, YELLOW_BLINK_STEP)?;

    set_state(&mut traffic, TRAFFIC_RED, COLOR_STEP)
}

fn open_traffic_state() -> io::Result<std::fs::File> {
    OpenOptions::new().write(true).open(TRAFFIC_STATE_PATH)
}

fn set_state(traffic: &mut std::fs::File, state: &[u8], delay: Duration) -> io::Result<()> {
    let written = traffic.write(state)?;
    if written != state.len() {
        return Err(io::Error::from(io::ErrorKind::WriteZero));
    }
    wait(delay);
    Ok(())
}

fn wait(delay: Duration) {
    thread::sleep(delay);
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
