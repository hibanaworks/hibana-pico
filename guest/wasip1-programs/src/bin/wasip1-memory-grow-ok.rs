#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::{memory_grow, memory_size};

#[cfg(target_arch = "wasm32")]
fn grow_default_memory() -> (usize, usize) {
    let before = memory_size::<0>();
    let previous = memory_grow::<0>(1);
    assert_ne!(previous, usize::MAX, "hibana wasip1 memory grow ok failed");
    let after = memory_size::<0>();
    assert_eq!(previous, before);
    assert_eq!(after, before + 1);
    (before, after)
}

#[cfg(not(target_arch = "wasm32"))]
fn grow_default_memory() -> (usize, usize) {
    (0, 0)
}

fn main() {
    let (before, after) = grow_default_memory();
    println!("hibana wasip1 memory grow ok {before}->{after}");
}
