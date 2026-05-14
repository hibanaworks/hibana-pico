use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

static mut SINK: u64 = 0;

fn main() {
    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(0x4849_4241_5241_4e44);
    unsafe {
        core::ptr::write_volatile(&raw mut SINK, hasher.finish());
    }
}
