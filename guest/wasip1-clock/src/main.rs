use std::time::{SystemTime, UNIX_EPOCH};

static mut SINK: u64 = 0;

fn main() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64)
        .unwrap_or(0);
    unsafe {
        core::ptr::write_volatile(&raw mut SINK, nanos);
    }
}
