use std::time::Duration;

static mut SINK: u32 = 0;

fn main() {
    std::thread::sleep(Duration::from_millis(1));
    unsafe {
        core::ptr::write_volatile(&raw mut SINK, 1);
    }
}
