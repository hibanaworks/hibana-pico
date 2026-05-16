static mut TICK: u32 = 0;

fn main() {
    loop {
        unsafe {
            let next = core::ptr::read_volatile(&raw const TICK).wrapping_add(1);
            core::ptr::write_volatile(&raw mut TICK, next);
        }
        core::hint::spin_loop();
    }
}
