use hibana_wasip1_guest::choreofs;

const DEVICE_PREOPEN_FD: u32 = 9;
const SESSION_MISMATCH_PATH: &str = "device/session-mismatch";
const SESSION_MISMATCH_PAYLOAD: &[u8] = b"session mismatch\n";

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasip1_guest::Result<()> {
    let file = choreofs::open_write(DEVICE_PREOPEN_FD, SESSION_MISMATCH_PATH)?;
    file.write_once_exact(SESSION_MISMATCH_PAYLOAD)
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
