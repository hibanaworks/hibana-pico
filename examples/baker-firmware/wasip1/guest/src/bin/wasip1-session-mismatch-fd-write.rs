use std::{
    fs::OpenOptions,
    io::{self, Write},
};

const SESSION_MISMATCH_PATH: &str = "/device/session-mismatch";
const SESSION_MISMATCH_PAYLOAD: &[u8] = b"session mismatch\n";

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(SESSION_MISMATCH_PATH)?;
    file.write_all(SESSION_MISMATCH_PAYLOAD)?;
    file.flush()
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
